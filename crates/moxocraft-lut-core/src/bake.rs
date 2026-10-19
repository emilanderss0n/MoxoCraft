//! Bakes a folder of `.cube` files into a LUT plug-in. Called from each LUT plug-in's build script
//! with that plug-in's [`Pack`]; only built for native code.
//!
//! PhotoCraft gives plug-ins no file access, so the LUTs travel inside the `.wasm`, stored as
//! 16-bit values (about 210 KB for a 33-point LUT). The manifest is written here too, so the
//! dialog's LUT list always matches what is baked in. How a plug-in's folder is laid out, and
//! what the build checks, is described in `docs/luts.md`.
//!
//! For LUTs listed in `davinci-intermediate.txt`, made for DaVinci Resolve's DaVinci Wide Gamut /
//! Intermediate working space, the trip into that space and back is baked into the table, so the
//! plug-in still does one lookup per pixel.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use moxocraft_sdk::Json;

use crate::cube::{self, CubeData};
use crate::lut::quantize;

/// The repository's `luts/` folder, found from this crate's own place in the workspace so the LUT
/// plug-ins don't each work it out.
const LUTS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../luts");

/// PhotoCraft refuses modules over 32 MiB; leave room for the code.
const MAX_TOTAL_BYTES: usize = 30 << 20;

/// PhotoCraft refuses a dropdown with more choices than this.
const MAX_CHOICES: usize = 64;

/// What makes one LUT plug-in itself. Everything else about it (its LUTs, their order, the
/// default, the credits) comes from its folder, and its version is the workspace's.
pub struct Pack<'a> {
    /// Its folder under `luts/`, such as `"film-luts"`.
    pub folder: &'a str,
    /// The manifest id (installing a module with the same id replaces the old one).
    pub id: &'a str,
    /// The menu label.
    pub name: &'a str,
    /// The start of the plug-in's description; `CREDITS.txt` follows it.
    pub description: &'a str,
}

struct Cube {
    name: String,
    /// The file, relative to the plug-in's folder.
    file: String,
    /// Listed in `davinci-intermediate.txt`: made for DaVinci Wide Gamut / Intermediate.
    davinci: bool,
    lut: CubeData,
}

/// Bakes `pack`'s folder into `luts.bin`, `luts.rs` and `manifest.json` in `OUT_DIR`.
pub fn bake(pack: &Pack) {
    let dir = &Path::new(LUTS_DIR).join(pack.folder);
    println!("cargo:rerun-if-changed={}", dir.display());

    // Menu order: categories as listed in categories.txt, then LUTs by name within each, with
    // numbers compared as numbers ("50D" before "200T") and a name before its longer variants
    // ("Portra 800" before "Portra 800 (Push 1)").
    let categories_file = dir.join("categories.txt");
    println!("cargo:rerun-if-changed={}", categories_file.display());
    let categories = list_file(&categories_file);
    assert!(!categories.is_empty(), "{} lists no categories", categories_file.display());
    for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("can't read the LUT folder {}: {e}", dir.display())).filter_map(Result::ok) {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            assert!(categories.contains(&name), "the LUT folder {name:?} isn't listed in {}", categories_file.display());
        } else {
            assert!(!is_cube(&path), "{name}: put LUTs in a category folder, not straight into {}", dir.display());
        }
    }
    // (category, file) pairs.
    let mut files: Vec<(String, PathBuf)> = Vec::new();
    for category in &categories {
        let folder = dir.join(category);
        println!("cargo:rerun-if-changed={}", folder.display());
        let mut in_category: Vec<PathBuf> = std::fs::read_dir(&folder)
            .unwrap_or_else(|e| panic!("categories.txt lists {category:?}, but there's no readable folder {}: {e}", folder.display()))
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| is_cube(p))
            .collect();
        assert!(!in_category.is_empty(), "no .cube files in {}", folder.display());
        in_category.sort_by_key(|f| natural_key(&display_name(category, f)));
        files.extend(in_category.into_iter().map(|f| (category.clone(), f)));
    }
    assert!(files.len() <= MAX_CHOICES, "{} holds {} LUTs; PhotoCraft lists at most {MAX_CHOICES} in a dropdown", dir.display(), files.len());

    let davinci_list = dir.join("davinci-intermediate.txt");
    watch_optional(&davinci_list);
    let davinci_input = list_file(&davinci_list);
    // "Category/File name.cube", as davinci-intermediate.txt lists them.
    let rel = |category: &str, f: &Path| format!("{category}/{}", f.file_name().unwrap_or_default().to_string_lossy());
    for name in &davinci_input {
        assert!(files.iter().any(|(c, f)| rel(c, f) == *name), "davinci-intermediate.txt lists {name:?}, but there is no such file in {}", dir.display());
    }

    let mut cubes: Vec<Cube> = Vec::new();
    for (category, f) in &files {
        println!("cargo:rerun-if-changed={}", f.display());
        let lut = read_cube(f).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        let file = rel(category, f);
        let davinci = davinci_input.contains(&file);
        let cube = Cube { name: display_name(category, f), file, davinci, lut: if davinci { cube::bake_davinci(&lut) } else { lut } };
        assert!(!cubes.iter().any(|c| c.name == cube.name), "two LUTs would both be called {:?}", cube.name);
        cubes.push(cube);
    }

    let out = PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    let mut bytes = Vec::new();
    let mut table = String::from("/// The LUTs baked in, in menu order.\npub static LUTS: &[LutInfo] = &[\n");
    for c in &cubes {
        let offset = bytes.len();
        // 16 bits per value; see `lut::quantize`.
        for v in &c.lut.values {
            bytes.extend_from_slice(&quantize(*v).to_le_bytes());
        }
        writeln!(table, "    LutInfo {{ name: {:?}, file: {:?}, size: {}, davinci: {}, offset: {offset}, len: {} }},", c.name, c.file, c.lut.size, c.davinci, bytes.len() - offset).unwrap();
    }
    table.push_str("];\n");
    assert!(bytes.len() <= MAX_TOTAL_BYTES, "{}'s LUTs take {} MB; PhotoCraft's 32 MiB module limit leaves room for about 30", dir.display(), bytes.len() >> 20);
    std::fs::write(out.join("luts.bin"), &bytes).expect("write luts.bin");
    std::fs::write(out.join("luts.rs"), table).expect("write luts.rs");
    // The LUTs' credits travel inside the plug-in, in the description PhotoCraft shows for it.
    let credits_file = dir.join("CREDITS.txt");
    watch_optional(&credits_file);
    let credits = std::fs::read_to_string(&credits_file).unwrap_or_default();
    let credits = credits.lines().filter(|l| !l.trim_start().starts_with('#')).collect::<Vec<_>>().join(" ");
    let credits = credits.split_whitespace().collect::<Vec<_>>().join(" ");
    // The LUT the dialog opens on: named in `default.txt`, or else the first one.
    let default_file = dir.join("default.txt");
    watch_optional(&default_file);
    let default = list_file(&default_file).into_iter().next().unwrap_or_else(|| cubes[0].name.clone());
    assert!(cubes.iter().any(|c| c.name == default), "default.txt names {default:?}, but no LUT is called that");
    // Cargo gives the build script the version of the crate it builds: the workspace's.
    let version = std::env::var("CARGO_PKG_VERSION").expect("cargo sets CARGO_PKG_VERSION");
    std::fs::write(out.join("manifest.json"), manifest(pack, &version, &cubes, &credits, &default)).expect("write manifest.json");
}

/// A menu-safe name, "Category - File name": underscores become spaces, and the characters
/// PhotoCraft doesn't allow in a choice (`|` and `"`) become dashes.
fn display_name(category: &str, path: &Path) -> String {
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let name: String = format!("{category} - {stem}").replace('_', " ").chars().map(|c| if c == '|' || c == '"' || c.is_control() { '-' } else { c }).collect();
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(name.chars().count() <= 64, "{name:?} is too long for PhotoCraft's menu (64 characters at most)");
    name
}

/// Rebuilds when an optional file changes. Only files that exist are named: Cargo treats a missing
/// one as changed on every build, which would re-bake every LUT each time. Creating the file later
/// is still noticed, because it changes the plug-in's folder, which is watched too.
fn watch_optional(path: &Path) {
    if path.exists() {
        println!("cargo:rerun-if-changed={}", path.display());
    }
}

fn is_cube(path: &Path) -> bool {
    path.is_file() && path.extension().is_some_and(|x| x.eq_ignore_ascii_case("cube"))
}

/// The lines of a list file, trimmed, without blank lines and `#` comments; empty if it's missing.
fn list_file(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path).unwrap_or_default().lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).map(str::to_owned).collect()
}

fn read_cube(path: &Path) -> Result<CubeData, String> {
    cube::parse_cube(&std::fs::read_to_string(path).map_err(|e| e.to_string())?)
}

/// A sort key that compares runs of digits by their value.
fn natural_key(name: &str) -> Vec<(u64, String)> {
    let mut key = Vec::new();
    let mut chars = name.chars().peekable();
    while let Some(&c) = chars.peek() {
        let digits = c.is_ascii_digit();
        let mut run = String::new();
        while let Some(&c) = chars.peek() {
            if c.is_ascii_digit() != digits {
                break;
            }
            run.push(c);
            chars.next();
        }
        key.push(if digits { (run.parse().unwrap_or(u64::MAX), String::new()) } else { (0, run.to_lowercase()) });
    }
    key
}

fn manifest(pack: &Pack, version: &str, cubes: &[Cube], credits: &str, default: &str) -> String {
    let options: Vec<String> = cubes.iter().map(|c| json_string(&c.name)).collect();
    let mut description = pack.description.to_owned();
    if !credits.is_empty() {
        description.push(' ');
        description.push_str(credits);
    }
    // PhotoCraft refuses descriptions over 1024 bytes.
    assert!(description.len() <= 1024, "CREDITS.txt makes {}'s description {} bytes; PhotoCraft allows 1024", pack.name, description.len());
    format!(
        r#"{{
  "id": {},
  "name": {},
  "version": {},
  "kind": "filter",
  "author": "MoxoCraft",
  "description": {},
  "params": {{
    "lut": {{"type": "choice", "options": [{}], "default": {}}},
    "intensity": {{"type": "number", "min": 0, "max": 100, "default": 100}}
  }},
  "overlap": 0,
  "area": "content"
}}"#,
        json_string(pack.id),
        json_string(pack.name),
        json_string(version),
        json_string(&description),
        options.join(", "),
        json_string(default)
    )
}

/// `s` as a JSON string literal.
fn json_string(s: &str) -> String {
    Json::String(s.to_owned()).to_text()
}
