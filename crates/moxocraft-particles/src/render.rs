//! One band from start to finish: check it, set up the coverage buffer, let the plug-in draw its
//! particles, then lay them over the band.

use moxocraft_sdk::{Error, Image};

use crate::depth::{Depth, read_map};
use crate::draw::Coverage;

/// Where the band sits, for placing particles.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    /// The picture's longer side in pixels: the unit particle positions and sizes are measured in.
    pub long_side: f32,
    /// The band as `[left, top, right, bottom]`, in units of the longer side.
    pub rect: [f32; 4],
}

/// Draws a band of a particle plug-in. `draw` dabs every particle into the coverage buffer; then
/// the particles are laid over the band in `colour` (gamma-encoded RGB, 0–1) at `opacity`, exactly
/// as if they were on a layer of their own in Normal mode above it. On an empty layer that gives
/// the particles alone, so a user can keep them separate. With a depth map, `depth` hides
/// particles behind the scene and the band is replaced by the particles.
pub fn render(image: &mut Image<'_>, depth: Depth, colour: [f32; 3], opacity: f32, draw: impl FnOnce(&mut Coverage, Frame)) -> Result<(), Error> {
    let alpha = image.format.alpha;
    let n = image.channels;
    if !image.format.mode.is_rgb() || n != 3 + usize::from(alpha) || image.data.len() != image.width * image.height * n {
        return Err(Error::UnsupportedMode);
    }
    let map = match depth {
        Depth::Map { inverted } => Some(inverted),
        _ => None,
    };
    // Nothing to lay over the band: leave it exactly as it came. A depth map is still replaced,
    // so the result never depends on the settings in a surprising way.
    if opacity <= 0.0 && map.is_none() {
        return Ok(());
    }
    let g = image.geometry;
    let long_side = g.canvas_width.max(g.canvas_height).max(1) as f32;
    let rect = [g.x as f32, g.y as f32, (g.x + image.width as i64) as f32, (g.y + image.height as i64) as f32].map(|v| v / long_side);
    let mut coverage = Coverage::new(image.width, image.height, [g.x, g.y])?;
    if let Some(inverted) = map {
        coverage = coverage.with_scene(read_map(image, inverted)?);
    }
    draw(&mut coverage, Frame { long_side, rect });

    // 32-bit documents hold linear light (roughly gamma 2, as elsewhere in MoxoCraft).
    let colour = if image.format.depth == 32 { colour.map(|c| c * c) } else { colour };
    let opacity = opacity.clamp(0.0, 1.0);
    for (p, covered) in image.data.chunks_exact_mut(n).zip(coverage.covered()) {
        let a = covered * opacity;
        match (map.is_some(), alpha) {
            (true, true) => {
                p[..3].copy_from_slice(&colour);
                p[3] = a;
            }
            // Without transparency, the particles go on black, ready for Screen mode.
            (true, false) => {
                for (v, c) in p.iter_mut().zip(colour) {
                    *v = c * a;
                }
            }
            (false, _) if a <= 0.0 => {}
            (false, true) => {
                let keep = p[3] * (1.0 - a);
                let total = a + keep;
                for (v, c) in p[..3].iter_mut().zip(colour) {
                    *v = (c * a + *v * keep) / total;
                }
                p[3] = total;
            }
            (false, false) => {
                for (v, c) in p.iter_mut().zip(colour) {
                    *v = c * a + *v * (1.0 - a);
                }
            }
        }
    }
    Ok(())
}
