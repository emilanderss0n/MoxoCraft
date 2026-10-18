//! Bakes every `.cube` file in the repository's `luts` folder into the plug-in.
//!
//! A plug-in can't read files at run time (PhotoCraft gives it no file access), so the LUTs travel
//! inside the `.wasm`. Each one is stored as 16-bit values (red changing fastest, as in the
//! `.cube` file), which keeps 65-point LUTs at about 1.6 MB each with steps far finer than an
//! 8- or 16-bit image can show. The script also writes the manifest, so the dialog's LUT list
//! always matches what is baked in.
//!
//! Add a LUT by dropping a `.cube` file into `luts/` and rebuilding. Its menu name is the file
//! name, with underscores shown as spaces; `luts/default.txt` (optional) names the one the dialog
//! opens on. `luts/CREDITS.txt` (optional; lines starting with `#`
//! are skipped) is added to the plug-in's description, so the LUTs' attribution travels inside the
//! `.wasm`.
//!
//! LUTs made for DaVinci Resolve's DaVinci Wide Gamut / Intermediate working space (rather than
//! normal images) are listed in `luts/davinci-intermediate.txt`. For those, the trip into that
//! space and back is baked in here: the stored table takes a normal sRGB colour straight to the
//! graded one, so the plug-in only ever does one lookup per pixel.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

#[allow(dead_code)]
#[path = "src/cube.rs"]
mod cube;

use cube::CubeData;

/// PhotoCraft refuses modules over 32 MiB; leave room for the code.
const MAX_TOTAL_BYTES: usize = 30 << 20;

struct Cube {
    name: String,
    /// Listed in `davinci-intermediate.txt`: made for DaVinci Wide Gamut / Intermediate.
    davinci: bool,
    lut: CubeData,
}

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
    let dir = manifest_dir.join("../../luts");
    println!("cargo:rerun-if-changed={}", dir.display());

    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("can't read the LUT folder {}: {e}", dir.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("cube")))
        .collect();
    // Menu order: by name, with numbers compared as numbers ("50D" before "200T") and a name
    // before its longer variants ("Portra 800" before "Portra 800 (Push 1)").
    files.sort_by_key(|f| natural_key(&display_name(f)));
    assert!(!files.is_empty(), "no .cube files in {}", dir.display());

    let davinci_list = dir.join("davinci-intermediate.txt");
    println!("cargo:rerun-if-changed={}", davinci_list.display());
    let davinci_input: Vec<String> = std::fs::read_to_string(&davinci_list)
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_owned)
        .collect();
    for name in &davinci_input {
        assert!(files.iter().any(|f| f.file_name().is_some_and(|n| n.to_string_lossy() == *name)), "davinci-intermediate.txt lists {name:?}, but there is no such file in {}", dir.display());
    }

    let mut cubes: Vec<Cube> = Vec::new();
    for f in &files {
        println!("cargo:rerun-if-changed={}", f.display());
        let lut = read_cube(f).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        let davinci = f.file_name().is_some_and(|n| davinci_input.iter().any(|l| *l == n.to_string_lossy()));
        let cube = Cube { name: display_name(f), davinci, lut: if davinci { cube::bake_davinci(&lut) } else { lut } };
        assert!(!cubes.iter().any(|c| c.name == cube.name), "two LUTs would both be called {:?}", cube.name);
        cubes.push(cube);
    }

    let out = PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    let mut bytes = Vec::new();
    let mut table = String::from("/// The LUTs baked in from `luts/`, in file-name order.\npub static LUTS: &[LutInfo] = &[\n");
    for c in &cubes {
        let offset = bytes.len();
        // 16 bits per value (see `cube::quantize`): steps far finer than an 8- or 16-bit image
        // can show.
        for v in &c.lut.values {
            bytes.extend_from_slice(&cube::quantize(*v).to_le_bytes());
        }
        writeln!(table, "    LutInfo {{ name: {:?}, size: {}, davinci: {}, offset: {offset}, len: {} }},", c.name, c.lut.size, c.davinci, bytes.len() - offset).unwrap();
    }
    table.push_str("];\n");
    assert!(bytes.len() <= MAX_TOTAL_BYTES, "the LUTs take {} MB; PhotoCraft's 32 MiB module limit leaves room for about 30", bytes.len() >> 20);
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
    let default = std::fs::read_to_string(&default_file).unwrap_or_default();
    let default = default.lines().map(str::trim).find(|l| !l.is_empty() && !l.starts_with('#')).map(str::to_owned).unwrap_or_else(|| cubes[0].name.clone());
    assert!(cubes.iter().any(|c| c.name == default), "default.txt names {default:?}, but no LUT is called that");
    std::fs::write(out.join("manifest.json"), manifest(&cubes, &credits, &default)).expect("write manifest.json");
}

/// A menu-safe name from the file name: underscores become spaces, and the characters PhotoCraft
/// doesn't allow in a choice (`|` and `"`) become dashes.
fn display_name(path: &Path) -> String {
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let name: String = stem.replace('_', " ").chars().map(|c| if c == '|' || c == '"' || c.is_control() { '-' } else { c }).collect();
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    name.chars().take(64).collect()
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

fn manifest(cubes: &[Cube], credits: &str, default: &str) -> String {
    let options: Vec<String> = cubes.iter().map(|c| json_string(&c.name)).collect();
    let mut description = String::from("Gives the image the colour and contrast of a film stock or grade. Pick a LUT and set how strongly it applies.");
    if !credits.is_empty() {
        description.push(' ');
        description.push_str(credits);
    }
    // PhotoCraft refuses descriptions over 1024 bytes.
    assert!(description.len() <= 1024, "luts/CREDITS.txt makes the plug-in description {} bytes; PhotoCraft allows 1024", description.len());
    format!(
        r#"{{
  "id": "moxocraft.lut",
  "name": "MoxoCraft - LUT",
  "version": "0.1.0",
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
