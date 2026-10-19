//! The LUT engine behind MoxoCraft's LUT plug-ins (Film LUTs, Cinematic LUTs, Creative LUTs).
//!
//! A LUT (a lookup table that maps every input color to an output color, the way grading tools
//! store film looks) gives an image its look. PhotoCraft lists at most 64 choices in a dropdown,
//! so the LUTs are spread over several plug-ins, and each plug-in crate is only configuration:
//! its `build.rs` names its folder under `luts/` and its id, name and description in a
//! [`bake::Pack`], and its `lib.rs` is one [`lut_pack!`] call. Everything else lives here.
//!
//! At build time (native code, in the plug-in's build script):
//! - [`cube`] reads `.cube` files and does the full-precision maths on them.
//! - [`bake`] turns a plug-in's folder into the baked tables, their index and the manifest.
//!
//! At run time (inside the `.wasm`):
//! - `lut` holds the baked table format both sides share, and [`Lut::lookup`].
//! - `render` applies the chosen LUT to a band ([`apply`], [`render`]).
//! - [`lut_pack!`] puts a plug-in together from its baked files.
//!
//! [`color`] (sRGB and DaVinci color spaces) is used on both sides. LUTs made for DaVinci
//! Resolve's DaVinci Wide Gamut / Intermediate working space have the trip into that space and back
//! baked in at build time, so at run time they are used like any other LUT.

#[cfg(not(target_arch = "wasm32"))]
pub mod bake;
pub mod color;
#[cfg(not(target_arch = "wasm32"))]
pub mod cube;
mod lut;
mod render;
#[cfg(not(target_arch = "wasm32"))]
#[doc(hidden)]
pub mod testing;

pub use lut::{Lut, LutInfo};
pub use moxocraft_sdk as sdk;
pub use render::{apply, render};

/// Defines a LUT plug-in from the files its build script baked with [`bake::bake`]: `LUTS` (the
/// index), `DATA` (the tables), `MANIFEST` and the `LutFilter` that PhotoCraft runs.
///
/// It has to be a macro: the baked files sit in the plug-in crate's own `OUT_DIR`, and `include!`
/// can only reach them when it is written in that crate.
#[macro_export]
macro_rules! lut_pack {
    () => {
        pub use $crate::LutInfo;

        include!(concat!(env!("OUT_DIR"), "/luts.rs"));

        /// Every baked-in LUT's values, back to back.
        pub static DATA: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/luts.bin"));

        /// The manifest PhotoCraft reads at install time, written by the build script so the LUT
        /// list always matches what is baked in.
        pub const MANIFEST: &str = include_str!(concat!(env!("OUT_DIR"), "/manifest.json"));

        pub struct LutFilter;

        impl $crate::sdk::Filter for LutFilter {
            const MANIFEST: &'static str = MANIFEST;

            fn apply(image: &mut $crate::sdk::Image<'_>, params: &$crate::sdk::Params) -> Result<(), $crate::sdk::Error> {
                $crate::apply(LUTS, DATA, image, params)
            }
        }

        $crate::sdk::export_filter!(LutFilter);
    };
}
