//! Shared plumbing for MoxoCraft's PhotoCraft filter plug-ins.
//!
//! PhotoCraft runs plug-ins as sandboxed WebAssembly modules (its "plug-in ABI v1", described in
//! PhotoCraft's `docs/plugins.md`). The host cuts the layer into bands, hands each band to the
//! module as interleaved `f32` samples (0–1) plus a JSON string of settings, and writes back
//! whatever the module leaves in the buffer.
//!
//! A filter implements [`Filter`] and calls [`export_filter!`]; that macro writes the four
//! functions PhotoCraft looks for (`pc_abi_version`, `pc_manifest`, `pc_alloc`, `pc_filter`).
//! Everything a filter needs is re-exported here: [`Image`] and [`Format`] describe the band,
//! [`Params`] reads the settings, [`Error`] is what it returns when it can't run.
//!
//! The other modules sit around that API, not inside it:
//! - [`abi`] turns PhotoCraft's raw pointers and numbers into an [`Image`] and [`Params`]. It is
//!   the only `unsafe` code in MoxoCraft; filters never call it directly.
//! - [`host`] copies how PhotoCraft feeds bands to a filter, so filters can be tested, previewed
//!   and benchmarked without PhotoCraft.
//! - `json` is a small JSON reader and writer ([`Json`]), so plug-ins need no dependency for it.

pub mod abi;
pub mod host;
mod image;
mod json;
mod params;

pub use image::{ColorMode, Format, Geometry, Image};
pub use json::Json;
pub use params::Params;

/// What a filter returns when it can't run. The number is the code PhotoCraft shows in its error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// The buffer or settings PhotoCraft passed don't add up (wrong length, unreadable JSON).
    BadInput = 1,
    /// The filter doesn't work in this color mode (for example CMYK, or an alpha channel).
    UnsupportedMode = 2,
    /// The filter couldn't get the scratch memory it needs.
    OutOfMemory = 3,
}

/// A PhotoCraft filter plug-in.
pub trait Filter {
    /// The manifest JSON PhotoCraft reads at install time: id, menu name, settings, overlap.
    const MANIFEST: &'static str;

    /// Filters one band in place.
    fn apply(image: &mut Image<'_>, params: &Params) -> Result<(), Error>;
}

/// A `Vec` of `len` copies of `value`, or [`Error::OutOfMemory`] instead of aborting when the
/// plug-in's memory cap is reached.
pub fn try_vec<T: Clone>(len: usize, value: T) -> Result<Vec<T>, Error> {
    let mut v = Vec::new();
    v.try_reserve_exact(len).map_err(|_| Error::OutOfMemory)?;
    v.resize(len, value);
    Ok(v)
}

/// Writes the four exports PhotoCraft looks for, wired to the given [`Filter`]. Only does
/// anything when building for `wasm32`.
#[macro_export]
macro_rules! export_filter {
    ($filter:ty) => {
        #[cfg(target_arch = "wasm32")]
        pub mod __photocraft_exports {
            // The filter type is named from where the macro was called.
            #[allow(unused_imports)]
            use super::*;

            #[unsafe(no_mangle)]
            pub extern "C" fn pc_abi_version() -> u32 {
                1
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn pc_manifest() -> u64 {
                $crate::abi::manifest(<$filter as $crate::Filter>::MANIFEST)
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn pc_alloc(size: u32) -> u32 {
                $crate::abi::alloc(size)
            }

            /// # Safety
            /// PhotoCraft guarantees `buf` and `params` point at blocks it got from `pc_alloc`,
            /// holding `len` and `params_len` bytes.
            #[unsafe(no_mangle)]
            pub unsafe extern "C" fn pc_filter(
                buf: *mut f32,
                len: u32,
                width: u32,
                height: u32,
                channels: u32,
                format: u32,
                params: *const u8,
                params_len: u32,
            ) -> i32 {
                // SAFETY: passed straight through; see the function's contract.
                unsafe { $crate::abi::filter::<$filter>(buf, len, width, height, channels, format, params, params_len) }
            }
        }
    };
}
