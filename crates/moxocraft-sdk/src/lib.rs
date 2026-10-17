//! Shared plumbing for MoxoCraft's PhotoCraft filter plug-ins.
//!
//! PhotoCraft runs plug-ins as sandboxed WebAssembly modules (its "plug-in ABI v1", described in
//! PhotoCraft's `docs/plugins.md`). The host cuts the layer into bands, hands each band to the
//! module as interleaved `f32` samples (0–1) plus a JSON string of settings, and writes back
//! whatever the module leaves in the buffer.
//!
//! A filter implements [`Filter`] and calls [`export_filter!`]; that macro writes the four
//! functions PhotoCraft looks for (`pc_abi_version`, `pc_manifest`, `pc_alloc`, `pc_filter`).
//! The [`host`] module copies how PhotoCraft feeds bands to a filter, so filters can be tested and
//! previewed natively without PhotoCraft.

pub mod abi;
pub mod host;
pub mod json;

pub use json::Json;

/// What a filter returns when it can't run. The number is the code PhotoCraft shows in its error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// The buffer or settings PhotoCraft passed don't add up (wrong length, unreadable JSON).
    BadInput = 1,
    /// The filter doesn't work in this colour mode (for example CMYK, or an alpha channel).
    UnsupportedMode = 2,
    /// The filter couldn't get the scratch memory it needs.
    OutOfMemory = 3,
}

/// A colour mode, numbered as in the PSD file format (which is how PhotoCraft sends it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorMode {
    Bitmap,
    Grayscale,
    /// Indexed documents reach plug-ins as plain RGB.
    Indexed,
    Rgb,
    Cmyk,
    Multichannel,
    /// Duotone documents reach plug-ins as grayscale.
    Duotone,
    Lab,
    Unknown(u8),
}

impl ColorMode {
    pub fn from_code(code: u8) -> Self {
        match code {
            0 => Self::Bitmap,
            1 => Self::Grayscale,
            2 => Self::Indexed,
            3 => Self::Rgb,
            4 => Self::Cmyk,
            7 => Self::Multichannel,
            8 => Self::Duotone,
            9 => Self::Lab,
            other => Self::Unknown(other),
        }
    }

    pub fn code(self) -> u8 {
        match self {
            Self::Bitmap => 0,
            Self::Grayscale => 1,
            Self::Indexed => 2,
            Self::Rgb => 3,
            Self::Cmyk => 4,
            Self::Multichannel => 7,
            Self::Duotone => 8,
            Self::Lab => 9,
            Self::Unknown(c) => c,
        }
    }

    /// True for modes whose pixels arrive as red, green and blue.
    pub fn is_rgb(self) -> bool {
        matches!(self, Self::Rgb | Self::Indexed)
    }
}

/// The `format` argument of `pc_filter`: `depth | alpha << 8 | mode << 16`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Format {
    /// The document's bit depth: 8, 16 or 32. Samples are always `f32`; 32-bit documents hold
    /// linear light and may go above 1, 8- and 16-bit ones are gamma-encoded.
    pub depth: u8,
    /// The last channel is straight (not premultiplied) alpha.
    pub alpha: bool,
    pub mode: ColorMode,
}

impl Format {
    pub const RGB8: Format = Format { depth: 8, alpha: false, mode: ColorMode::Rgb };
    pub const RGBA8: Format = Format { depth: 8, alpha: true, mode: ColorMode::Rgb };

    pub fn from_code(code: u32) -> Self {
        Format { depth: (code & 0xff) as u8, alpha: code & (1 << 8) != 0, mode: ColorMode::from_code(((code >> 16) & 0xff) as u8) }
    }

    pub fn code(self) -> u32 {
        u32::from(self.depth) | (u32::from(self.alpha) << 8) | (u32::from(self.mode.code()) << 16)
    }

    /// Samples per pixel, alpha included (multichannel documents are counted as one channel).
    pub fn channels(self) -> usize {
        let colour = match self.mode {
            ColorMode::Rgb | ColorMode::Indexed | ColorMode::Lab => 3,
            ColorMode::Cmyk => 4,
            _ => 1,
        };
        colour + usize::from(self.alpha)
    }
}

/// Where a band sits in the document. PhotoCraft sends it as `_image` in the settings JSON.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Geometry {
    /// Document position of the buffer's top-left sample, margins included.
    pub x: i64,
    pub y: i64,
    /// Extra pixels around the band on every side. They are there to be read; only the inner
    /// rectangle of the result is kept.
    pub overlap: usize,
    pub canvas_width: usize,
    pub canvas_height: usize,
}

/// One band of pixels, filtered in place.
pub struct Image<'a> {
    /// `width × height × channels` interleaved samples, row by row.
    pub data: &'a mut [f32],
    pub width: usize,
    pub height: usize,
    pub channels: usize,
    pub format: Format,
    pub geometry: Geometry,
}

/// The settings the user picked, already checked and clamped by PhotoCraft against the manifest.
pub struct Params {
    root: Json,
}

impl Params {
    pub fn new(root: Json) -> Self {
        Params { root }
    }

    pub fn parse(text: &str) -> Result<Self, Error> {
        Json::parse(text).filter(|j| matches!(j, Json::Object(_))).map(Params::new).ok_or(Error::BadInput)
    }

    /// A number setting, or `default` when it's missing or not a number.
    pub fn number(&self, key: &str, default: f64) -> f64 {
        self.root.get(key).and_then(Json::as_f64).unwrap_or(default)
    }

    /// A choice or text setting.
    pub fn text(&self, key: &str) -> Option<&str> {
        self.root.get(key).and_then(Json::as_str)
    }

    /// A checkbox setting.
    pub fn flag(&self, key: &str, default: bool) -> bool {
        self.root.get(key).and_then(Json::as_bool).unwrap_or(default)
    }

    /// The band's geometry from `_image`, if PhotoCraft sent it.
    pub fn geometry(&self) -> Option<Geometry> {
        let g = self.root.get("_image")?;
        let int = |k: &str| g.get(k).and_then(Json::as_f64).filter(|v| v.is_finite());
        let size = |k: &str| int(k).map(|v| v.clamp(0.0, 1e9) as usize);
        Some(Geometry {
            x: int("x")? as i64,
            y: int("y")? as i64,
            overlap: size("overlap").unwrap_or(0),
            canvas_width: size("canvasWidth")?,
            canvas_height: size("canvasHeight")?,
        })
    }
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
