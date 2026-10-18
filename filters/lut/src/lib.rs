//! Film LUTs: a PhotoCraft filter plug-in that gives an image the look of one of its 3D LUTs,
//! film stocks: colour negative, slide and instant films, and Fujifilm-style camera looks.
//!
//! The LUTs are baked in at build time from `luts/film-luts` (see `build.rs`); the lookups
//! and the plug-in plumbing live in `moxocraft_lut_core`.

moxocraft_lut_core::lut_pack!();
