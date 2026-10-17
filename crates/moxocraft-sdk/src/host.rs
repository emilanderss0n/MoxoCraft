//! A stand-in for PhotoCraft, for tests and previews.
//!
//! [`apply`] feeds an image to a filter the way PhotoCraft does (see its
//! `crates/plugins/src/apply.rs`): cut into bands, each band padded by the manifest's `overlap`
//! with the edge pixels repeated, settings passed as JSON with `_image` added, only the inner
//! rectangle kept, and broken samples replaced by the originals. Running a filter here and in
//! PhotoCraft gives the same pixels, so tests can check band seams and edge handling natively.

use crate::{Error, Filter, Format, Json, abi};

/// How big each band is. PhotoCraft uses one 256-pixel tile row, a few tiles wide.
#[derive(Clone, Copy, Debug)]
pub struct Bands {
    pub width: usize,
    pub height: usize,
}

impl Bands {
    /// The whole image in one band.
    pub const WHOLE: Bands = Bands { width: usize::MAX, height: usize::MAX };
    /// Roughly what PhotoCraft does for an RGBA layer.
    pub const PHOTOCRAFT: Bands = Bands { width: 1024, height: 256 };
}

/// The `overlap` a manifest asks for (0 when missing).
pub fn manifest_overlap(manifest: &str) -> usize {
    Json::parse(manifest).and_then(|m| m.get("overlap").and_then(Json::as_f64)).map_or(0, |v| v.clamp(0.0, 256.0) as usize)
}

/// Runs filter `F` over `pixels` (`width × height × format.channels()` samples) with the settings
/// object `params` (JSON text), and returns the filtered pixels.
pub fn apply<F: Filter>(pixels: &[f32], width: usize, height: usize, format: Format, params: &str, bands: Bands) -> Result<Vec<f32>, Error> {
    let n = format.channels();
    if width == 0 || height == 0 || pixels.len() != width * height * n {
        return Err(Error::BadInput);
    }
    let Some(Json::Object(fields)) = Json::parse(params) else { return Err(Error::BadInput) };
    let ov = manifest_overlap(F::MANIFEST);
    let (bw, bh) = (bands.width.max(1), bands.height.max(1));
    let mut out = pixels.to_vec();
    for by in (0..height).step_by(bh) {
        for bx in (0..width).step_by(bw) {
            let (w, h) = (bw.min(width - bx), bh.min(height - by));
            // The band plus its margins, repeating the image's edge pixels outside it.
            let (iw, ih) = (w + 2 * ov, h + 2 * ov);
            let (x0, y0) = (bx as i64 - ov as i64, by as i64 - ov as i64);
            let mut data = Vec::with_capacity(iw * ih * n);
            for row in 0..ih as i64 {
                let sy = (y0 + row).clamp(0, height as i64 - 1) as usize;
                for col in 0..iw as i64 {
                    let sx = (x0 + col).clamp(0, width as i64 - 1) as usize;
                    let s = (sy * width + sx) * n;
                    data.extend_from_slice(&pixels[s..s + n]);
                }
            }
            let original = data.clone();
            let mut with_image = fields.clone();
            let num = |v: f64| Json::Number(v);
            with_image.push((
                "_image".into(),
                Json::Object(vec![
                    ("x".into(), num(x0 as f64)),
                    ("y".into(), num(y0 as f64)),
                    ("width".into(), num(iw as f64)),
                    ("height".into(), num(ih as f64)),
                    ("overlap".into(), num(ov as f64)),
                    ("canvasWidth".into(), num(width as f64)),
                    ("canvasHeight".into(), num(height as f64)),
                    ("depth".into(), num(f64::from(format.depth))),
                    ("alpha".into(), Json::Bool(format.alpha)),
                    ("channels".into(), num(n as f64)),
                ]),
            ));
            let text = Json::Object(with_image).to_text();
            match abi::run::<F>(&mut data, iw, ih, n, format.code(), text.as_bytes()) {
                0 => {}
                1 => return Err(Error::BadInput),
                2 => return Err(Error::UnsupportedMode),
                _ => return Err(Error::OutOfMemory),
            }
            // Keep the inner rectangle, cleaned up the way PhotoCraft does it.
            for row in 0..h {
                for col in 0..w {
                    let s = ((row + ov) * iw + col + ov) * n;
                    let d = ((by + row) * width + bx + col) * n;
                    for ch in 0..n {
                        let mut v = data[s + ch];
                        if !v.is_finite() {
                            v = original[s + ch];
                        }
                        if format.depth != 32 || (format.alpha && ch == n - 1) {
                            v = v.clamp(0.0, 1.0);
                        }
                        out[d + ch] = v;
                    }
                }
            }
        }
    }
    Ok(out)
}
