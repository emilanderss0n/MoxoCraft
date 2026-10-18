//! The baked table format, and looking colours up in it.
//!
//! This is the one format both sides agree on: [`bake`](crate::bake) writes it at build time and
//! [`Lut`] reads it inside the plug-in. Each table is `size³ × 3` little-endian `u16` values made
//! with [`quantize`], red changing fastest, then green, then blue (the order of a `.cube` file).

use moxocraft_sdk::Error;

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

/// A baked table found in its plug-in's data and checked against its size, but not yet decoded:
/// everything [`Lut::load`] checks, without the work, so a run that grades nothing can stop here.
pub(crate) struct Stored<'a> {
    bytes: &'a [u8],
    size: usize,
}

impl<'a> Stored<'a> {
    /// The table `info` describes, from its plug-in's `data`.
    pub(crate) fn find(data: &'a [u8], info: &LutInfo) -> Result<Stored<'a>, Error> {
        let bytes = info.offset.checked_add(info.len).and_then(|end| data.get(info.offset..end)).ok_or(Error::BadInput)?;
        Stored::new(bytes, info.size)
    }

    fn new(bytes: &'a [u8], size: usize) -> Result<Stored<'a>, Error> {
        if !(size >= 2 && size.checked_pow(3).is_some_and(|cells| bytes.len() == cells * 6)) {
            return Err(Error::BadInput);
        }
        Ok(Stored { bytes, size })
    }

    /// The table, ready to look colours up in.
    pub(crate) fn decode(self) -> Result<Lut, Error> {
        let Stored { bytes, size } = self;
        let value = |q: &[u8]| dequantize(u16::from_le_bytes([q[0], q[1]]));
        let mut texels = Vec::new();
        texels.try_reserve_exact(bytes.len() / 6).map_err(|_| Error::OutOfMemory)?;
        texels.extend(bytes.chunks_exact(6).map(|t| [value(&t[0..2]), value(&t[2..4]), value(&t[4..6])]));
        Ok(Lut { texels, size, green_step: size, blue_step: size * size })
    }
}

/// A baked-in LUT, ready to look colours up in.
pub struct Lut {
    /// The table's colours, turned from stored `u16`s back into colour values once, when the LUT
    /// is loaded. Every pixel reads four of them, so converting each one there would cost the
    /// interpreter far more than converting the whole table (at most about 430 KB) up front.
    texels: Vec<[f32; 3]>,
    size: usize,
    /// How far apart neighbouring grid points are in `texels` along green and blue (red's are
    /// next to each other), so a lookup finds a cell's corners by adding these.
    green_step: usize,
    blue_step: usize,
}

impl Lut {
    /// The LUT `info` describes, from its plug-in's `data`.
    pub fn new(data: &[u8], info: &LutInfo) -> Option<Lut> {
        Lut::load(data, info).ok()
    }

    /// [`Lut::new`], saying why it failed: [`Error::BadInput`] when `info` doesn't match `data`,
    /// [`Error::OutOfMemory`] when there is no room for the table.
    pub fn load(data: &[u8], info: &LutInfo) -> Result<Lut, Error> {
        Stored::find(data, info)?.decode()
    }

    /// A LUT from a table in the baked format, or `None` if the sizes don't match.
    pub fn from_bytes(data: &[u8], size: usize) -> Option<Lut> {
        Stored::new(data, size).and_then(Stored::decode).ok()
    }

    /// The table entry at index `i` (grid point `r + g × green_step + b × blue_step`).
    #[inline(always)]
    fn texel(&self, i: usize) -> [f32; 3] {
        self.texels.get(i).copied().unwrap_or([0.0; 3])
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
        // Comparisons rather than `clamp` (which panics on a NaN) or `max`/`min` (whose NaN
        // handling costs the interpreter extra work): a NaN fails `> 0.0`, so it lands on 0.
        let scale = |v: f32| {
            let s = v * top;
            let s = if s > 0.0 { s } else { 0.0 };
            if s < top { s } else { top }
        };
        let scaled = [scale(rgb[0]), scale(rgb[1]), scale(rgb[2])];
        let cell = |v: f32| (v as usize).min(self.size - 2);
        let base = [cell(scaled[0]), cell(scaled[1]), cell(scaled[2])];
        let [fr, fg, fb] = [scaled[0] - base[0] as f32, scaled[1] - base[1] as f32, scaled[2] - base[2] as f32];
        // The cell's corners: `i` is its (r, g, b) corner, and stepping one grid point along red,
        // green or blue adds 1, `dg` or `db`.
        let (dg, db) = (self.green_step, self.blue_step);
        let i = base[0] + base[1] * dg + base[2] * db;
        let c000 = self.texel(i);
        let c111 = self.texel(i + 1 + dg + db);
        // Which of the cell's six tetrahedra the colour is in decides the two middle corners and
        // the order the fractions apply in.
        let (c1, c2, w) = if fr >= fg {
            if fg >= fb {
                (self.texel(i + 1), self.texel(i + 1 + dg), [fr, fg, fb])
            } else if fr >= fb {
                (self.texel(i + 1), self.texel(i + 1 + db), [fr, fb, fg])
            } else {
                (self.texel(i + db), self.texel(i + 1 + db), [fb, fr, fg])
            }
        } else if fb >= fg {
            (self.texel(i + db), self.texel(i + dg + db), [fb, fg, fr])
        } else if fb >= fr {
            (self.texel(i + dg), self.texel(i + dg + db), [fg, fb, fr])
        } else {
            (self.texel(i + dg), self.texel(i + 1 + dg), [fg, fr, fb])
        };
        // Tables may run a little past black and white (see `STORE_MIN`); clamp after
        // interpolating. Written per channel rather than with `array::map`, which the compiler
        // doesn't always inline, and a call per pixel is slow in the interpreter.
        let mix = |c: usize| (c000[c] + w[0] * (c1[c] - c000[c]) + w[1] * (c2[c] - c1[c]) + w[2] * (c111[c] - c2[c])).clamp(0.0, 1.0);
        [mix(0), mix(1), mix(2)]
    }
}
