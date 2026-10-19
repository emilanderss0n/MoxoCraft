//! Source LUTs: reading `.cube` files, and the build-time maths on them in full precision.
//!
//! Only built for native code (build scripts and tests); nothing here goes into a plug-in.

use crate::color::{DWG_TO_REC709, REC709_TO_DWG, davinci_intermediate, davinci_intermediate_to_linear, mat, srgb_decode, srgb_encode};
use crate::lut::{STORE_MIN, STORE_SPAN};

/// A 3D LUT as read from a `.cube` file.
pub struct CubeData {
    /// Points along each axis.
    pub size: usize,
    /// `size³ × 3` values, red changing fastest, then green, then blue.
    pub values: Vec<f32>,
}

impl CubeData {
    /// The LUT's color for `rgb` (each 0–1, clamped) by tetrahedral interpolation, unclamped.
    /// The same interpolation as [`Lut::lookup`](crate::Lut::lookup), on full-precision values
    /// (see there for why it is written twice).
    fn lookup(&self, rgb: [f32; 3]) -> [f32; 3] {
        let n = self.size;
        let top = (n - 1) as f32;
        let scaled = rgb.map(|v| (v * top).max(0.0).min(top));
        let [r, g, b] = scaled.map(|v| (v as usize).min(n - 2));
        let [fr, fg, fb] = [scaled[0] - r as f32, scaled[1] - g as f32, scaled[2] - b as f32];
        let at = |r: usize, g: usize, b: usize| {
            let i = ((b * n + g) * n + r) * 3;
            [self.values[i], self.values[i + 1], self.values[i + 2]]
        };
        let (c000, c111) = (at(r, g, b), at(r + 1, g + 1, b + 1));
        let (c1, c2, w) = if fr >= fg {
            if fg >= fb {
                (at(r + 1, g, b), at(r + 1, g + 1, b), [fr, fg, fb])
            } else if fr >= fb {
                (at(r + 1, g, b), at(r + 1, g, b + 1), [fr, fb, fg])
            } else {
                (at(r, g, b + 1), at(r + 1, g, b + 1), [fb, fr, fg])
            }
        } else if fb >= fg {
            (at(r, g, b + 1), at(r, g + 1, b + 1), [fb, fg, fr])
        } else if fb >= fr {
            (at(r, g + 1, b), at(r, g + 1, b + 1), [fg, fb, fr])
        } else {
            (at(r, g + 1, b), at(r + 1, g + 1, b), [fg, fr, fb])
        };
        [0, 1, 2].map(|c| c000[c] + w[0] * (c1[c] - c000[c]) + w[1] * (c2[c] - c1[c]) + w[2] * (c111[c] - c2[c]))
    }
}

/// Reads the text of a `.cube` file. Only 3D LUTs with the default 0–1 input range are accepted.
pub fn parse_cube(text: &str) -> Result<CubeData, String> {
    let mut size = 0usize;
    let mut values: Vec<f32> = Vec::new();
    let (mut lo, mut hi) = ([0.0f32; 3], [1.0f32; 3]);
    for (no, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let first = parts.next().unwrap_or_default();
        let floats = |parts: std::str::SplitWhitespace<'_>| -> Result<Vec<f32>, String> {
            parts.map(|p| p.parse::<f32>().map_err(|_| format!("line {}: bad number {p:?}", no + 1))).collect()
        };
        match first {
            "TITLE" => {}
            "LUT_3D_SIZE" => size = parts.next().and_then(|s| s.parse().ok()).ok_or(format!("line {}: bad LUT_3D_SIZE", no + 1))?,
            "LUT_1D_SIZE" => return Err("1D LUTs aren't supported, only 3D ones".into()),
            "DOMAIN_MIN" => lo = floats(parts)?.try_into().map_err(|_| format!("line {}: DOMAIN_MIN needs 3 numbers", no + 1))?,
            "DOMAIN_MAX" => hi = floats(parts)?.try_into().map_err(|_| format!("line {}: DOMAIN_MAX needs 3 numbers", no + 1))?,
            _ if first.starts_with(|c: char| c.is_ascii_digit() || c == '-' || c == '.') => {
                let row = floats(line.split_whitespace())?;
                if row.len() != 3 {
                    return Err(format!("line {}: expected 3 numbers", no + 1));
                }
                values.extend(row);
            }
            // Other keywords (LUT_IN_VIDEO_RANGE and the like) don't change the table.
            _ => {}
        }
    }
    if !(2..=129).contains(&size) {
        return Err(format!("LUT_3D_SIZE {size} is outside 2..=129"));
    }
    if lo != [0.0; 3] || hi != [1.0; 3] {
        return Err("only the default 0–1 input domain is supported".into());
    }
    if values.len() != size * size * size * 3 {
        return Err(format!("expected {} rows for size {size}, found {}", size * size * size, values.len() / 3));
    }
    Ok(CubeData { size, values })
}

/// What a LUT that works inside DaVinci Wide Gamut / Intermediate does to a normal sRGB color:
/// into that space, through the LUT, and back out.
pub fn through_davinci(lut: &CubeData, srgb: [f32; 3]) -> [f32; 3] {
    let wide = mat(&REC709_TO_DWG, srgb.map(srgb_decode)).map(davinci_intermediate);
    let back = mat(&DWG_TO_REC709, lut.lookup(wide).map(davinci_intermediate_to_linear));
    back.map(|v| srgb_encode(v.clamp(0.0, 1.0)))
}

/// [`srgb_encode`] carried on past black (mirrored) and white, for values that are only clamped
/// after interpolation.
fn srgb_encode_unclamped(l: f32) -> f32 {
    if l < 0.0 { -srgb_encode(-l) } else { srgb_encode(l) }
}

/// A new LUT, same size, that takes sRGB color straight to the graded sRGB color of a DaVinci-
/// space LUT. Baking the round trip in once means the plug-in does a single lookup per pixel.
/// Values are left unclamped (within the stored range) so lookups clamp after interpolating.
pub(crate) fn bake_davinci(lut: &CubeData) -> CubeData {
    let n = lut.size;
    let mut values = Vec::with_capacity(n * n * n * 3);
    for b in 0..n {
        for g in 0..n {
            for r in 0..n {
                let srgb = [r, g, b].map(|v| v as f32 / (n - 1) as f32);
                let wide = mat(&REC709_TO_DWG, srgb.map(srgb_decode)).map(davinci_intermediate);
                let back = mat(&DWG_TO_REC709, lut.lookup(wide).map(davinci_intermediate_to_linear));
                values.extend(back.map(|v| srgb_encode_unclamped(v).clamp(STORE_MIN, STORE_MIN + STORE_SPAN)));
            }
        }
    }
    CubeData { size: n, values }
}
