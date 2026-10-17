//! Reading `.cube` files and the colour maths around them.
//!
//! Plain Rust with no dependencies: `build.rs` includes this file directly to bake the LUTs, the
//! plug-in uses the sRGB curves at run time, and the tests use all of it to check the baking.

/// A 3D LUT as read from a `.cube` file.
pub struct CubeData {
    /// Points along each axis.
    pub size: usize,
    /// `size³ × 3` values, red changing fastest, then green, then blue.
    pub values: Vec<f32>,
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

/// The LUT's colour for `rgb` (each 0–1, clamped) by tetrahedral interpolation: the cube cell
/// around the colour is split into six tetrahedra and the colour is blended from the four corners
/// of the one it falls in.
pub fn tetrahedral(lut: &CubeData, rgb: [f32; 3]) -> [f32; 3] {
    let n = lut.size;
    let top = (n - 1) as f32;
    let scaled = rgb.map(|v| (v * top).max(0.0).min(top));
    let [r, g, b] = scaled.map(|v| (v as usize).min(n - 2));
    let [fr, fg, fb] = [scaled[0] - r as f32, scaled[1] - g as f32, scaled[2] - b as f32];
    let at = |r: usize, g: usize, b: usize| {
        let i = ((b * n + g) * n + r) * 3;
        [lut.values[i], lut.values[i + 1], lut.values[i + 2]]
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

/// Stored tables cover this range rather than just 0–1, so a baked table can run a little past
/// black and white and the clamp happens after interpolation, not at the grid points (clamping
/// there leaves creases that interpolation smears into visible errors).
pub const STORE_MIN: f32 = -0.5;
pub const STORE_SPAN: f32 = 2.0;

/// A table value as stored: 16 bits over [`STORE_MIN`]..[`STORE_MIN`] + [`STORE_SPAN`], steps of
/// about 0.00003.
pub fn quantize(v: f32) -> u16 {
    ((v - STORE_MIN) / STORE_SPAN * 65535.0).round().clamp(0.0, 65535.0) as u16
}

pub fn dequantize(q: u16) -> f32 {
    STORE_MIN + f32::from(q) * (STORE_SPAN / 65535.0)
}

pub fn srgb_encode(l: f32) -> f32 {
    if l <= 0.003_130_8 { 12.92 * l } else { 1.055 * l.powf(1.0 / 2.4) - 0.055 }
}

pub fn srgb_decode(v: f32) -> f32 {
    if v <= 0.040_45 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
}

/// Linear Rec.709/sRGB primaries to DaVinci Wide Gamut primaries (both D65), from the two
/// spaces' published RGB-to-XYZ matrices. Each row sums to 1, so white stays white.
pub const REC709_TO_DWG: [[f32; 3]; 3] = [[0.562_857, 0.323_509_1, 0.113_688_7], [0.077_767_1, 0.749_560_1, 0.172_626_6], [0.064_679_5, 0.191_994_2, 0.743_153_8]];

/// The reverse: DaVinci Wide Gamut primaries back to linear Rec.709/sRGB.
pub const DWG_TO_REC709: [[f32; 3]; 3] = [[1.898_313, -0.792_050_1, -0.106_421_9], [-0.168_952_9, 1.489_010_1, -0.320_034_3], [-0.121_568_3, -0.315_751_5, 1.437_559_8]];

pub fn mat(m: &[[f32; 3]; 3], v: [f32; 3]) -> [f32; 3] {
    [0, 1, 2].map(|r| m[r][0] * v[0] + m[r][1] * v[1] + m[r][2] * v[2])
}

/// Linear light to DaVinci Intermediate, Resolve's log encoding (Blackmagic's published curve):
/// 18% grey lands at 0.336, and each stop above the toe adds about 0.073.
pub fn davinci_intermediate(linear: f32) -> f32 {
    if linear <= 0.002_624_09 { linear * 10.444_268 } else { ((linear + 0.0075).log2() + 7.0) * 0.073_292_48 }
}

/// DaVinci Intermediate back to linear light.
pub fn davinci_intermediate_to_linear(v: f32) -> f32 {
    if v <= 0.027_406_68 { v / 10.444_268 } else { (v / 0.073_292_48 - 7.0).exp2() - 0.0075 }
}

/// What a LUT that works inside DaVinci Wide Gamut / Intermediate does to a normal sRGB colour:
/// into that space, through the LUT, and back out.
pub fn through_davinci(lut: &CubeData, srgb: [f32; 3]) -> [f32; 3] {
    let wide = mat(&REC709_TO_DWG, srgb.map(srgb_decode)).map(davinci_intermediate);
    let back = mat(&DWG_TO_REC709, tetrahedral(lut, wide).map(davinci_intermediate_to_linear));
    back.map(|v| srgb_encode(v.clamp(0.0, 1.0)))
}

/// [`srgb_encode`] carried on past black (mirrored) and white, for values that are only clamped
/// after interpolation.
fn srgb_encode_unclamped(l: f32) -> f32 {
    if l < 0.0 { -srgb_encode(-l) } else { srgb_encode(l) }
}

/// A new LUT, same size, that takes sRGB colour straight to the graded sRGB colour of a DaVinci-
/// space LUT. Baking the round trip in once means the plug-in does a single lookup per pixel.
/// Values are left unclamped (within the stored range) so lookups clamp after interpolating.
pub fn bake_davinci(lut: &CubeData) -> CubeData {
    let n = lut.size;
    let mut values = Vec::with_capacity(n * n * n * 3);
    for b in 0..n {
        for g in 0..n {
            for r in 0..n {
                let srgb = [r, g, b].map(|v| v as f32 / (n - 1) as f32);
                let wide = mat(&REC709_TO_DWG, srgb.map(srgb_decode)).map(davinci_intermediate);
                let back = mat(&DWG_TO_REC709, tetrahedral(lut, wide).map(davinci_intermediate_to_linear));
                values.extend(back.map(|v| srgb_encode_unclamped(v).clamp(STORE_MIN, STORE_MIN + STORE_SPAN)));
            }
        }
    }
    CubeData { size: n, values }
}
