//! Applying a LUT to a band: picking the LUT the dialog chose and grading every pixel with it.

use moxocraft_sdk::{Error, Image, Params, try_vec};

use crate::color;
use crate::lut::{Lut, LutInfo, Stored};

/// Runs the LUT the dialog picked (`lut`, falling back to the first) at its `intensity`.
pub fn apply(luts: &[LutInfo], data: &[u8], image: &mut Image<'_>, params: &Params) -> Result<(), Error> {
    let name = params.text("lut").unwrap_or_default();
    let info = luts.iter().find(|l| l.name == name).or(luts.first()).ok_or(Error::BadInput)?;
    // The table and the band are checked in that order whatever the intensity, but the table is
    // only decoded when there is grading to do: at intensity 0 the band is left as it is.
    let table = Stored::find(data, info)?;
    let intensity = (params.number("intensity", 100.0) as f32 / 100.0).clamp(0.0, 1.0);
    if leaves_unchanged(image, intensity)? {
        return Ok(());
    }
    render(image, &table.decode()?, intensity)
}

/// Checks that the band is one a LUT can grade, then whether `intensity` leaves it unchanged.
fn leaves_unchanged(image: &Image<'_>, intensity: f32) -> Result<bool, Error> {
    if !image.format.mode.is_rgb() || image.channels != 3 + usize::from(image.format.alpha) {
        return Err(Error::UnsupportedMode);
    }
    Ok(intensity <= 0.0)
}

/// Entries in the tables that convert 32-bit (linear) pixels to sRGB and back.
const CURVE_POINTS: usize = 4096;

/// A transfer curve sampled over 0–1, read with linear interpolation: much cheaper than `powf`
/// per pixel in an interpreter. The table's length is part of its type, so the compiler can see
/// that every read is in range and adds no checks to the pixel loop.
struct Curve(Box<[f32; CURVE_POINTS]>);

impl Curve {
    fn new(f: impl Fn(f32) -> f32) -> Result<Curve, Error> {
        // The conversion can't fail: the length is right.
        let mut table: Box<[f32; CURVE_POINTS]> = try_vec(CURVE_POINTS, 0.0f32)?.into_boxed_slice().try_into().map_err(|_| Error::OutOfMemory)?;
        for (i, v) in table.iter_mut().enumerate() {
            *v = f(i as f32 / (CURVE_POINTS - 1) as f32);
        }
        Ok(Curve(table))
    }

    #[inline(always)]
    fn at(&self, v: f32) -> f32 {
        let pos = v.clamp(0.0, 1.0) * (CURVE_POINTS - 1) as f32;
        let i = (pos as usize).min(CURVE_POINTS - 2);
        let (a, b) = (self.0[i], self.0[i + 1]);
        a + (b - a) * (pos - i as f32)
    }
}

/// Applies `lut` to every pixel of the band, blended with the original by `intensity` (0–1).
///
/// 8- and 16-bit pixels are looked up as they are (LUTs are made for display-encoded values).
/// 32-bit documents hold linear light, so their pixels are converted to sRGB for the lookup and
/// back afterwards. Alpha is never changed.
pub fn render(image: &mut Image<'_>, lut: &Lut, intensity: f32) -> Result<(), Error> {
    if leaves_unchanged(image, intensity)? {
        return Ok(());
    }
    let alpha = image.format.alpha;
    // 32-bit documents are linear light; LUTs expect display-encoded colour.
    let curves = if image.format.depth == 32 { Some((Curve::new(color::srgb_encode)?, Curve::new(color::srgb_decode)?)) } else { None };
    if alpha {
        grade::<4>(image.data, lut, intensity, curves.as_ref());
    } else {
        grade::<3>(image.data, lut, intensity, curves.as_ref());
    }
    Ok(())
}

/// The pixel loop of [`render`], written once per layout (`N` channels, the first three red,
/// green and blue) and once per colour encoding, so the interpreter runs no channel-count or
/// bit-depth checks per pixel.
fn grade<const N: usize>(data: &mut [f32], lut: &Lut, intensity: f32, curves: Option<&(Curve, Curve)>) {
    let (pixels, _) = data.as_chunks_mut::<N>();
    match curves {
        None => {
            for p in pixels {
                let original = [p[0], p[1], p[2]];
                let graded = lut.lookup(original);
                for c in 0..3 {
                    p[c] = original[c] + (graded[c] - original[c]) * intensity;
                }
            }
        }
        Some((enc, dec)) => {
            for p in pixels {
                let original = [p[0], p[1], p[2]];
                let encoded = [enc.at(original[0]), enc.at(original[1]), enc.at(original[2])];
                let graded = lut.lookup(encoded);
                for c in 0..3 {
                    let v = encoded[c] + (graded[c] - encoded[c]) * intensity;
                    // Values above 1 (brighter than white) can't go through a 0–1 LUT; keep the
                    // extra brightness on top of the graded white. (A comparison rather than
                    // `max(0.0)`, whose NaN handling costs the interpreter more; a NaN gives 0
                    // either way.)
                    let over = original[c] - 1.0;
                    p[c] = dec.at(v) + if over > 0.0 { over } else { 0.0 };
                }
            }
        }
    }
}
