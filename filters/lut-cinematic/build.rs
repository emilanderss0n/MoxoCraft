//! Bakes the LUTs in `luts/cinematic-luts` into the plug-in (see `moxocraft_lut_core::bake`).

use std::path::Path;

use moxocraft_lut_core::bake::{Pack, bake};

fn main() {
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    bake(&Pack {
        folder: Path::new(&crate_dir).join("../../luts/cinematic-luts"),
        id: "moxocraft.lut-cinematic",
        name: "MoxoCraft - Cinematic LUTs",
        version: env!("CARGO_PKG_VERSION"),
        description: "Gives the image a movie-style grade: cinematic, moody and vintage looks, and black-and-white films. Pick a LUT and set how strongly it applies.",
    });
}
