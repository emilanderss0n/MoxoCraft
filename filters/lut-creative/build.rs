//! Bakes the LUTs in `luts/creative-luts` into the plug-in. This is the plug-in's whole configuration;
//! its LUTs, categories, default and credits live in that folder (see `docs/luts.md`).

use moxocraft_lut_core::bake::{Pack, bake};

fn main() {
    bake(&Pack {
        folder: "creative-luts",
        id: "moxocraft.lut-creative",
        name: "MoxoCraft - Creative LUTs",
        description: "Gives the image a creative grade: colourful, scenic and experimental looks. Pick a LUT and set how strongly it applies.",
    });
}
