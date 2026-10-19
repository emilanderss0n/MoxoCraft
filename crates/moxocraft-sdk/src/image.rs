//! How PhotoCraft describes a band of pixels: its color mode, bit depth and place in the document.

/// A color mode, numbered as in the PSD file format (which is how PhotoCraft sends it).
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
