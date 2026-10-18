//! The LUT engine behind MoxoCraft's LUT plug-ins (Film LUTs, Cinematic LUTs, Creative LUTs).
//!
//! A LUT (a lookup table that maps every input colour to an output colour, the way grading tools
//! store film looks) gives an image its look. Each plug-in is a thin crate: its build script calls
//! [`bake::bake`] on its folder of `.cube` files, and its `lib.rs` is one [`lut_pack!`] call,
//! which includes the baked tables and exports the plug-in. PhotoCraft lists at most 64 choices in
//! a dropdown, so the LUTs are spread over several plug-ins.
//!
//! Each pixel's colour is looked up with tetrahedral interpolation: the table's cube cell around
//! the colour is split into six tetrahedra and the colour is blended from the four corners of the
//! one it falls in. That is what grading software uses: smoother than blending all eight corners,
//! and cheaper, which matters in PhotoCraft's interpreter.
//!
//! 8- and 16-bit pixels are looked up as they are (LUTs are made for display-encoded values).
//! 32-bit documents hold linear light, so their pixels are converted to sRGB for the lookup and
//! back afterwards. Alpha is never changed.
//!
//! LUTs made for DaVinci Resolve's DaVinci Wide Gamut / Intermediate working space have the trip
//! into that space and back baked in at build time, so they are used like any other LUT here.

#[cfg(not(target_arch = "wasm32"))]
pub mod bake;
pub mod cube;
#[cfg(not(target_arch = "wasm32"))]
#[doc(hidden)]
pub mod testing;

pub use moxocraft_sdk as sdk;
use moxocraft_sdk::{Error, Image, Params, try_vec};

/// Where one baked-in LUT lives in its plug-in's data.
#[derive(Clone, Copy, Debug)]
pub struct LutInfo {
    /// The name shown in the dialog, "Category - Name".
    pub name: &'static str,
    /// The `.cube` file it was baked from, relative to the plug-in's LUT folder.
    pub file: &'static str,
    /// Points along each axis.
    pub size: usize,
    /// Made for DaVinci Wide Gamut / Intermediate (the conversion is baked into the table).
    pub davinci: bool,
    /// Byte range in the plug-in's data: `size³ × 3` little-endian values stored with
    /// `cube::quantize`, red changing fastest.
    pub offset: usize,
    pub len: usize,
}

/// Defines a LUT plug-in from the tables its build script baked with [`bake::bake`]: `LUTS`,
/// `DATA`, `MANIFEST` and the `LutFilter` that PhotoCraft runs.
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

/// Runs the LUT the dialog picked (`lut`, falling back to the first) at its `intensity`.
pub fn apply(luts: &[LutInfo], data: &[u8], image: &mut Image<'_>, params: &Params) -> Result<(), Error> {
    let name = params.text("lut").unwrap_or_default();
    let info = luts.iter().find(|l| l.name == name).or(luts.first()).ok_or(Error::BadInput)?;
    let lut = Lut::new(data, info).ok_or(Error::BadInput)?;
    let intensity = (params.number("intensity", 100.0) as f32 / 100.0).clamp(0.0, 1.0);
    render(image, &lut, intensity)
}

/// A baked-in LUT, ready to look colours up in.
pub struct Lut<'a> {
    /// The table as raw little-endian `u16` bytes.
    data: &'a [u8],
    size: usize,
}

impl<'a> Lut<'a> {
    /// The LUT `info` describes, from its plug-in's `data`.
    pub fn new(data: &'a [u8], info: &LutInfo) -> Option<Lut<'a>> {
        let data = data.get(info.offset..info.offset.checked_add(info.len)?)?;
        Lut::from_bytes(data, info.size)
    }

    /// A LUT from `size³ × 3` little-endian values stored with `cube::quantize` (red changing
    /// fastest), or `None` if the sizes don't match.
    pub fn from_bytes(data: &'a [u8], size: usize) -> Option<Lut<'a>> {
        (size >= 2 && data.len() == size.checked_pow(3)? * 6).then_some(Lut { data, size })
    }

    /// The table entry at grid point (`r`, `g`, `b`).
    #[inline(always)]
    fn texel(&self, r: usize, g: usize, b: usize) -> [f32; 3] {
        let i = ((b * self.size + g) * self.size + r) * 6;
        let Some(&[r0, r1, g0, g1, b0, b1]) = self.data.get(i..i + 6) else { return [0.0; 3] };
        [
            cube::dequantize(u16::from_le_bytes([r0, r1])),
            cube::dequantize(u16::from_le_bytes([g0, g1])),
            cube::dequantize(u16::from_le_bytes([b0, b1])),
        ]
    }

    /// The LUT's colour for `rgb` (each 0–1; values outside are clamped), by tetrahedral
    /// interpolation.
    #[inline(always)]
    pub fn lookup(&self, rgb: [f32; 3]) -> [f32; 3] {
        let top = (self.size - 1) as f32;
        // `max`/`min` rather than `clamp`, so a NaN can't panic; it lands on 0.
        let scaled = rgb.map(|v| (v * top).max(0.0).min(top));
        let base = scaled.map(|v| (v as usize).min(self.size - 2));
        let [fr, fg, fb] = [scaled[0] - base[0] as f32, scaled[1] - base[1] as f32, scaled[2] - base[2] as f32];
        let [r, g, b] = base;
        let c000 = self.texel(r, g, b);
        let c111 = self.texel(r + 1, g + 1, b + 1);
        // Which of the cell's six tetrahedra the colour is in decides the two middle corners and
        // the order the fractions apply in.
        let (c1, c2, w) = if fr >= fg {
            if fg >= fb {
                (self.texel(r + 1, g, b), self.texel(r + 1, g + 1, b), [fr, fg, fb])
            } else if fr >= fb {
                (self.texel(r + 1, g, b), self.texel(r + 1, g, b + 1), [fr, fb, fg])
            } else {
                (self.texel(r, g, b + 1), self.texel(r + 1, g, b + 1), [fb, fr, fg])
            }
        } else if fb >= fg {
            (self.texel(r, g, b + 1), self.texel(r, g + 1, b + 1), [fb, fg, fr])
        } else if fb >= fr {
            (self.texel(r, g + 1, b), self.texel(r, g + 1, b + 1), [fg, fb, fr])
        } else {
            (self.texel(r, g + 1, b), self.texel(r + 1, g + 1, b), [fg, fr, fb])
        };
        // Tables may run a little past black and white (see `cube::STORE_MIN`); clamp after
        // interpolating.
        [0, 1, 2].map(|c| (c000[c] + w[0] * (c1[c] - c000[c]) + w[1] * (c2[c] - c1[c]) + w[2] * (c111[c] - c2[c])).clamp(0.0, 1.0))
    }
}

/// Entries in the tables that convert 32-bit (linear) pixels to sRGB and back.
const CURVE_POINTS: usize = 4096;

/// A transfer curve sampled over 0–1, read with linear interpolation: much cheaper than `powf`
/// per pixel in an interpreter.
pub struct Curve(Vec<f32>);

impl Curve {
    pub fn new(f: impl Fn(f32) -> f32) -> Result<Curve, Error> {
        let mut table = try_vec(CURVE_POINTS, 0.0f32)?;
        for (i, v) in table.iter_mut().enumerate() {
            *v = f(i as f32 / (CURVE_POINTS - 1) as f32);
        }
        Ok(Curve(table))
    }

    #[inline(always)]
    fn at(&self, v: f32) -> f32 {
        let pos = v.clamp(0.0, 1.0) * (CURVE_POINTS - 1) as f32;
        let i = (pos as usize).min(CURVE_POINTS - 2);
        let (Some(&a), Some(&b)) = (self.0.get(i), self.0.get(i + 1)) else { return v };
        a + (b - a) * (pos - i as f32)
    }
}

/// Applies `lut` to every pixel of the band, blended with the original by `intensity` (0–1).
pub fn render(image: &mut Image<'_>, lut: &Lut<'_>, intensity: f32) -> Result<(), Error> {
    let alpha = image.format.alpha;
    let n = image.channels;
    if !image.format.mode.is_rgb() || n != 3 + usize::from(alpha) {
        return Err(Error::UnsupportedMode);
    }
    if intensity <= 0.0 {
        return Ok(());
    }
    // 32-bit documents are linear light; LUTs expect display-encoded colour.
    let curves = if image.format.depth == 32 { Some((Curve::new(cube::srgb_encode)?, Curve::new(cube::srgb_decode)?)) } else { None };
    for p in image.data.chunks_exact_mut(n) {
        let original = [p[0], p[1], p[2]];
        let encoded = match &curves {
            Some((enc, _)) => original.map(|v| enc.at(v)),
            None => original,
        };
        let graded = lut.lookup(encoded);
        for c in 0..3 {
            let mut v = encoded[c] + (graded[c] - encoded[c]) * intensity;
            if let Some((_, dec)) = &curves {
                // Values above 1 (brighter than white) can't go through a 0–1 LUT; keep the
                // extra brightness on top of the graded white.
                v = dec.at(v) + (original[c] - 1.0).max(0.0);
            }
            p[c] = v;
        }
    }
    Ok(())
}
