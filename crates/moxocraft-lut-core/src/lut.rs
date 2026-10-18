//! The baked table format, and looking colours up in it.
//!
//! This is the one format both sides agree on: [`bake`](crate::bake) writes it at build time and
//! [`Lut`] reads it inside the plug-in. Each table is `size³ × 3` little-endian `u16` values made
//! with [`quantize`], red changing fastest, then green, then blue (the order of a `.cube` file).

/// Where one baked-in LUT lives in its plug-in's data. The build script writes one per LUT, in
/// menu order, into the plug-in's `LUTS` (see [`lut_pack!`](crate::lut_pack)).
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
    /// Byte range in the plug-in's data.
    pub offset: usize,
    pub len: usize,
}

/// Stored tables cover this range rather than just 0–1, so a baked table can run a little past
/// black and white and the clamp happens after interpolation, not at the grid points (clamping
/// there leaves creases that interpolation smears into visible errors).
pub(crate) const STORE_MIN: f32 = -0.5;
pub(crate) const STORE_SPAN: f32 = 2.0;

/// A table value as stored: 16 bits over [`STORE_MIN`]..[`STORE_MIN`] + [`STORE_SPAN`], steps of
/// about 0.00003, far finer than an 8- or 16-bit image can show. Only the build writes tables.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn quantize(v: f32) -> u16 {
    ((v - STORE_MIN) / STORE_SPAN * 65535.0).round().clamp(0.0, 65535.0) as u16
}

pub(crate) fn dequantize(q: u16) -> f32 {
    STORE_MIN + f32::from(q) * (STORE_SPAN / 65535.0)
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

    /// A LUT from a table in the baked format, or `None` if the sizes don't match.
    pub fn from_bytes(data: &'a [u8], size: usize) -> Option<Lut<'a>> {
        (size >= 2 && data.len() == size.checked_pow(3)? * 6).then_some(Lut { data, size })
    }

    /// The table entry at grid point (`r`, `g`, `b`).
    #[inline(always)]
    fn texel(&self, r: usize, g: usize, b: usize) -> [f32; 3] {
        let i = ((b * self.size + g) * self.size + r) * 6;
        let Some(&[r0, r1, g0, g1, b0, b1]) = self.data.get(i..i + 6) else { return [0.0; 3] };
        [
            dequantize(u16::from_le_bytes([r0, r1])),
            dequantize(u16::from_le_bytes([g0, g1])),
            dequantize(u16::from_le_bytes([b0, b1])),
        ]
    }

    /// The LUT's colour for `rgb` (each 0–1; values outside are clamped), by tetrahedral
    /// interpolation: the table's cube cell around the colour is split into six tetrahedra and the
    /// colour is blended from the four corners of the one it falls in. That is what grading
    /// software uses: smoother than blending all eight corners, and cheaper, which matters in
    /// PhotoCraft's interpreter.
    ///
    /// `cube.rs` has the same interpolation for full-precision tables at build time. This copy is
    /// written out by hand because it runs for every pixel: sharing one generic version costs 0.5%
    /// more interpreter instructions per pixel. Change both together.
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
        // Tables may run a little past black and white (see `STORE_MIN`); clamp after
        // interpolating.
        [0, 1, 2].map(|c| (c000[c] + w[0] * (c1[c] - c000[c]) + w[1] * (c2[c] - c1[c]) + w[2] * (c111[c] - c2[c])).clamp(0.0, 1.0))
    }
}
