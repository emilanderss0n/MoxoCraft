//! Creative LUTs: a PhotoCraft filter plug-in that gives an image the look of one of its 3D LUTs,
//! creative grades: colourful, scenic and experimental looks.
//!
//! The LUTs are baked in at build time from `luts/creative-luts` (see `build.rs`); the lookups
//! and the plug-in plumbing live in `moxocraft_lut_core`.

moxocraft_lut_core::lut_pack!();
