//! Bakes the LUTs in `luts/film-luts` into the plug-in. This is the plug-in's whole configuration;
//! its LUTs, categories, default and credits live in that folder (see `docs/luts.md`).

use moxocraft_lut_core::bake::{Pack, bake};

fn main() {
    bake(&Pack {
        folder: "film-luts",
        // Keeps the id (and file name) of v0.1.0's "MoxoCraft - LUT", which this plug-in grew out of,
        // so installing it replaces the old one instead of leaving both in the menu.
        id: "moxocraft.lut",
        name: "MoxoCraft - Film LUTs",
        description: "Gives the image the colour and contrast of a film stock: colour negative, slide and instant films, and Fujifilm-style camera looks. Pick a LUT and set how strongly it applies.",
    });
}
