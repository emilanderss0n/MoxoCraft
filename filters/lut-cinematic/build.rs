//! Bakes the LUTs in `luts/cinematic-luts` into the plug-in. This is the plug-in's whole configuration;
//! its LUTs, categories, default and credits live in that folder (see `docs/luts.md`).

use moxocraft_lut_core::bake::{Pack, bake};

fn main() {
    bake(&Pack {
        folder: "cinematic-luts",
        id: "moxocraft.lut-cinematic",
        name: "MoxoCraft - Cinematic LUTs",
        description: "Gives the image a movie-style grade: cinematic, moody and vintage looks, and black-and-white films. Pick a LUT and set how strongly it applies.",
    });
}
