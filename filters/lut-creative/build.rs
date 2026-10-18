//! Bakes the LUTs in `luts/creative-luts` into the plug-in (see `moxocraft_lut_core::bake`).

use std::path::Path;

use moxocraft_lut_core::bake::{Pack, bake};

fn main() {
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    bake(&Pack {
        folder: Path::new(&crate_dir).join("../../luts/creative-luts"),
        id: "moxocraft.lut-creative",
        name: "MoxoCraft - Creative LUTs",
        version: env!("CARGO_PKG_VERSION"),
        description: "Gives the image a creative grade: colourful, scenic and experimental looks. Pick a LUT and set how strongly it applies.",
    });
}
