//! Bakes a folder of `.cube` files into a LUT plug-in. Called from each LUT plug-in's build script.
//!
//! A plug-in can't read files at run time (PhotoCraft gives it no file access), so the LUTs travel
//! inside the `.wasm`. Each one is stored as 16-bit values (red changing fastest, as in the
//! `.cube` file), which keeps 33-point LUTs at about 210 KB each with steps far finer than an
//! 8- or 16-bit image can show. The manifest is written here too, so the dialog's LUT list always
//! matches what is baked in.
//!
//! A plug-in's folder (`luts/film-luts`, ...) holds category folders (`Film`, `Negative`, ...),
//! listed in the order of its `categories.txt`. Add a LUT by dropping a `.cube` file into a
//! category folder and rebuilding. Its menu name is "Category - File name", with underscores shown
//! as spaces; `default.txt` (optional) names the one the dialog opens on, and `CREDITS.txt`
//! (optional; lines starting with `#` are skipped) is added to the plug-in's description, so the
//! LUTs' attribution travels inside the `.wasm`. PhotoCraft lists at most 64 choices in a
//! dropdown and loads modules up to 32 MiB; the build stops if a folder goes over either.
//!
//! LUTs made for DaVinci Resolve's DaVinci Wide Gamut / Intermediate working space (rather than
//! normal images) are listed in the folder's `davinci-intermediate.txt`, as
//! `Category/File name.cube`. For those, the trip into that space and back is baked in here: the
//! stored table takes a normal sRGB colour straight to the graded one, so the plug-in only ever
//! does one lookup per pixel.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::cube::{self, CubeData};

/// PhotoCraft refuses modules over 32 MiB; leave room for the code.
const MAX_TOTAL_BYTES: usize = 30 << 20;

/// PhotoCraft refuses a dropdown with more choices than this.
const MAX_CHOICES: usize = 64;

/// One LUT plug-in.
pub struct Pack<'a> {
    /// Its LUT folder.
    pub folder: PathBuf,
    /// The manifest id (installing a module with the same id replaces the old one).
    pub id: &'a str,
    /// The menu label.
    pub name: &'a str,
    /// The plug-in's version (the workspace version).
    pub version: &'a str,
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
    let dir = &pack.folder;
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
    println!("cargo:rerun-if-changed={}", davinci_list.display());
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
        // 16 bits per value (see `cube::quantize`): steps far finer than an 8- or 16-bit image
        // can show.
        for v in &c.lut.values {
            bytes.extend_from_slice(&cube::quantize(*v).to_le_bytes());
        }
        writeln!(table, "    LutInfo {{ name: {:?}, file: {:?}, size: {}, davinci: {}, offset: {offset}, len: {} }},", c.name, c.file, c.lut.size, c.davinci, bytes.len() - offset).unwrap();
    }
    table.push_str("];\n");
    assert!(bytes.len() <= MAX_TOTAL_BYTES, "{}'s LUTs take {} MB; PhotoCraft's 32 MiB module limit leaves room for about 30", dir.display(), bytes.len() >> 20);
    std::fs::write(out.join("luts.bin"), &bytes).expect("write luts.bin");
    std::fs::write(out.join("luts.rs"), table).expect("write luts.rs");
    // The LUTs' credits travel inside the plug-in, in the description PhotoCraft shows for it.
    let credits_file = dir.join("CREDITS.txt");
    println!("cargo:rerun-if-changed={}", credits_file.display());
    let credits = std::fs::read_to_string(&credits_file).unwrap_or_default();
    let credits = credits.lines().filter(|l| !l.trim_start().starts_with('#')).collect::<Vec<_>>().join(" ");
    let credits = credits.split_whitespace().collect::<Vec<_>>().join(" ");
    // The LUT the dialog opens on: named in `default.txt`, or else the first one.
    let default_file = dir.join("default.txt");
    println!("cargo:rerun-if-changed={}", default_file.display());
    let default = list_file(&default_file).into_iter().next().unwrap_or_else(|| cubes[0].name.clone());
    assert!(cubes.iter().any(|c| c.name == default), "default.txt names {default:?}, but no LUT is called that");
    std::fs::write(out.join("manifest.json"), manifest(pack, &cubes, &credits, &default)).expect("write manifest.json");
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

fn manifest(pack: &Pack, cubes: &[Cube], credits: &str, default: &str) -> String {
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
        json_string(pack.version),
        json_string(&description),
        options.join(", "),
        json_string(default)
    )
}

/// `s` as a JSON string literal.
fn json_string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => write!(out, "\\u{:04x}", c as u32).unwrap(),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
