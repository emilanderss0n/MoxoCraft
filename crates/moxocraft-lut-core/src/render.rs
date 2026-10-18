//! Applying a LUT to a band: picking the LUT the dialog chose and grading every pixel with it.

use moxocraft_sdk::{Error, Image, Params, try_vec};

use crate::color;
use crate::lut::{Lut, LutInfo};

/// Runs the LUT the dialog picked (`lut`, falling back to the first) at its `intensity`.
pub fn apply(luts: &[LutInfo], data: &[u8], image: &mut Image<'_>, params: &Params) -> Result<(), Error> {
    let name = params.text("lut").unwrap_or_default();
    let info = luts.iter().find(|l| l.name == name).or(luts.first()).ok_or(Error::BadInput)?;
    let lut = Lut::new(data, info).ok_or(Error::BadInput)?;
    let intensity = (params.number("intensity", 100.0) as f32 / 100.0).clamp(0.0, 1.0);
    render(image, &lut, intensity)
}

/// Entries in the tables that convert 32-bit (linear) pixels to sRGB and back.
const CURVE_POINTS: usize = 4096;

/// A transfer curve sampled over 0–1, read with linear interpolation: much cheaper than `powf`
/// per pixel in an interpreter.
struct Curve(Vec<f32>);

impl Curve {
    fn new(f: impl Fn(f32) -> f32) -> Result<Curve, Error> {
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
///
/// 8- and 16-bit pixels are looked up as they are (LUTs are made for display-encoded values).
/// 32-bit documents hold linear light, so their pixels are converted to sRGB for the lookup and
/// back afterwards. Alpha is never changed.
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
    let curves = if image.format.depth == 32 { Some((Curve::new(color::srgb_encode)?, Curve::new(color::srgb_decode)?)) } else { None };
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
