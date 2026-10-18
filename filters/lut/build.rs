//! Bakes the LUTs in `luts/film-luts` into the plug-in (see `moxocraft_lut_core::bake`).

use std::path::Path;

use moxocraft_lut_core::bake::{Pack, bake};

fn main() {
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    bake(&Pack {
        folder: Path::new(&crate_dir).join("../../luts/film-luts"),
        // Keeps the id (and file name) of v0.1.0's "MoxoCraft - LUT", which this plug-in grew out of,
        // so installing it replaces the old one instead of leaving both in the menu.
        id: "moxocraft.lut",
        name: "MoxoCraft - Film LUTs",
        version: env!("CARGO_PKG_VERSION"),
        description: "Gives the image the colour and contrast of a film stock: colour negative, slide and instant films, and Fujifilm-style camera looks. Pick a LUT and set how strongly it applies.",
    });
}
