//! Colour-space maths: the sRGB transfer curve (used at run time for 32-bit documents) and
//! DaVinci Wide Gamut / Intermediate (used at build time for LUTs made in that space).

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
