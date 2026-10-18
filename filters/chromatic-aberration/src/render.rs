//! Filtering one band: copying it out in linear light, the optional segment pass, and the main
//! pass that reads each pixel's taps.
//!
//! Smeared colours are done in two passes so they stay smooth without costing a tap every pixel
//! or two: first every pixel gets a short weighted average along its own displacement direction
//! (a "segment", reaching one tap gap either side and fading out towards its ends), then the main
//! taps read those segments instead of single pixels. Neighbouring segments overlap and add up
//! evenly, so together they cover the whole smear with no holes or steps.

use moxocraft_sdk::{Error, Image, try_vec};

use crate::field::Field;
use crate::settings::Settings;
use crate::taps::{Plan, SEGMENT_SPACING};

/// A block of pixels copied out of the band: `width` pixels per row, its top-left at
/// (`corner`, `corner`) in band coordinates.
struct Plane<'a> {
    data: &'a [f32],
    width: usize,
    corner: usize,
}

/// Filters one band.
pub(crate) fn render(image: &mut Image<'_>, s: &Settings) -> Result<(), Error> {
    let alpha = image.format.alpha;
    let n = image.channels;
    if !image.format.mode.is_rgb() || n != 3 + usize::from(alpha) {
        return Err(Error::UnsupportedMode);
    }
    // Blending between pixels needs at least a 2 × 2 block.
    if s.amount.abs() < 1e-3 || image.width < 2 || image.height < 2 {
        return Ok(());
    }
    let (w, h) = (image.width, image.height);
    let g = image.geometry;
    // PhotoCraft gives us `overlap` pixels of real picture around the band (repeating the edge
    // pixels past the canvas). Every read stays inside that margin, with a few pixels spare for
    // blending between pixels: bands then join without seams, and the pixel loops need no range
    // checks. Without a margin there is nothing to read from, so nothing moves.
    let ov = g.overlap.min(w / 2).min(h / 2);
    let max_shift = (ov as f32 - 4.0).max(0.0) / Plan::worst_reach(s.fringe, s.softness);
    if max_shift <= 0.0 {
        return Ok(());
    }
    let field = Field::new(s, &g, max_shift)?;
    let longest = field.longest();
    // How far main taps reach, and segments beyond them; only that much margin is copied out.
    let tap_margin = ((longest * Plan::worst_tap_reach(s.fringe, s.softness)).ceil() as usize + 2).min(ov);
    let plan = Plan::new(s.fringe, s.softness, longest, &g, tap_margin);
    if plan.reach <= 0.0 {
        return Ok(());
    }
    let src_margin = if plan.segmented { (tap_margin + (longest * plan.segment).ceil() as usize + 2).min(ov) } else { tap_margin };

    // The band in linear light, premultiplied by alpha, so blending between pixels and taps never
    // drags in the colour hidden under transparent pixels.
    let linear = image.format.depth != 32;
    let src_corner = ov - src_margin;
    let (sw, sh) = (w - 2 * src_corner, h - 2 * src_corner);
    let mut src = try_vec(sw * sh * n, 0.0f32)?;
    match (alpha, linear) {
        (false, false) => copy_out::<3, false>(image.data, w, src_corner, &mut src, sw)?,
        (false, true) => copy_out::<3, true>(image.data, w, src_corner, &mut src, sw)?,
        (true, false) => copy_out::<4, false>(image.data, w, src_corner, &mut src, sw)?,
        (true, true) => copy_out::<4, true>(image.data, w, src_corner, &mut src, sw)?,
    }
    let src = Plane { data: &src, width: sw, corner: src_corner };

    let job = Job { w, h, ov, origin: [g.x, g.y], field: &field, plan: &plan, linear };
    let seg_data;
    let read = if plan.segmented {
        let corner = ov - tap_margin;
        let width = w - 2 * corner;
        let mut data = try_vec(width * (h - 2 * corner) * n, 0.0f32)?;
        if alpha {
            job.segments::<4, 8>(&src, &mut data, width, corner);
        } else {
            job.segments::<3, 6>(&src, &mut data, width, corner);
        }
        seg_data = data;
        Plane { data: &seg_data, width, corner }
    } else {
        src
    };
    if alpha {
        job.run::<4, 8, true>(&read, image.data);
    } else {
        job.run::<3, 6, false>(&read, image.data);
    }
    Ok(())
}

/// Copies the block of the band (`band_width` pixels per row) from (`corner`, `corner`), `width`
/// pixels wide, into `out`: in linear light when `LINEAR`, and premultiplied by alpha when there
/// are 4 channels. Written once per layout, so the interpreter checks neither for every pixel.
fn copy_out<const N: usize, const LINEAR: bool>(band: &[f32], band_width: usize, corner: usize, out: &mut [f32], width: usize) -> Result<(), Error> {
    if width == 0 {
        return Ok(());
    }
    let (out, _) = out.as_chunks_mut::<N>();
    for (r, dst) in out.chunks_exact_mut(width).enumerate() {
        let start = ((corner + r) * band_width + corner) * N;
        let line = band.get(start..start + width * N).ok_or(Error::BadInput)?;
        let (line, _) = line.as_chunks::<N>();
        for (d, p) in dst.iter_mut().zip(line) {
            let lin = if LINEAR { [decode(p[0]), decode(p[1]), decode(p[2])] } else { [p[0], p[1], p[2]] };
            if N == 4 {
                let a = p[N - 1];
                d[0] = lin[0] * a;
                d[1] = lin[1] * a;
                d[2] = lin[2] * a;
                d[N - 1] = a;
            } else {
                d[0] = lin[0];
                d[1] = lin[1];
                d[2] = lin[2];
            }
        }
    }
    Ok(())
}

/// Gamma 2 to linear, keeping the sign of out-of-range values.
#[inline(always)]
fn decode(v: f32) -> f32 {
    v * v.abs()
}

/// Linear back to gamma 2, keeping the sign of out-of-range values. The square root of the size
/// with the sign put back gives exactly what a branch per sign would, but the compiled branch ran
/// both square roots for every value.
#[inline(always)]
fn encode(v: f32) -> f32 {
    v.abs().sqrt().copysign(v)
}

/// A random-looking number in 0..1 for a pixel, the same for that pixel in every band.
#[inline(always)]
fn hash01(x: f32, y: f32) -> f32 {
    let mut h = (x as i32 as u32).wrapping_mul(0x8da6_b343) ^ (y as i32 as u32).wrapping_mul(0xd816_3841);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h >> 8) as f32 / 16_777_216.0
}

/// How one pixel samples its segment: `samples` points `step` apart (in units of the segment's
/// half-length), along `[ux, uy]` (the half-length in pixels, pointing along the displacement),
/// slid by a per-pixel amount when `jitter`.
#[derive(Clone, Copy)]
struct Segment {
    samples: usize,
    step: f32,
    jitter: bool,
    ux: f32,
    uy: f32,
}

struct Job<'a> {
    w: usize,
    h: usize,
    ov: usize,
    origin: [i64; 2],
    field: &'a Field,
    plan: &'a Plan,
    linear: bool,
}

impl Job<'_> {
    /// First pass: every pixel the main taps can land on gets a weighted average of the picture
    /// along its own displacement direction, out to one tap gap either side and fading towards
    /// the ends (a "tent"). Neighbouring taps' tents overlap and add up to an even, smooth
    /// coverage, so the smear has no steps.
    fn segments<const N: usize, const M: usize>(&self, src: &Plane<'_>, out: &mut [f32], width: usize, corner: usize) {
        // In Directional mode every pixel has the same displacement, so the same segment.
        let uniform = self.field.uniform().map(|v| self.segment(v));
        let rows = self.h - 2 * corner;
        for r in 0..rows {
            let row = corner + r;
            let y = (self.origin[1] + row as i64) as f32 + 0.5;
            let sy = (row - src.corner) as f32;
            for c in 0..width {
                let col = corner + c;
                let x = (self.origin[0] + col as i64) as f32 + 0.5;
                let sx = (col - src.corner) as f32;
                let o = (r * width + c) * N;
                let Some(dst) = out.get_mut(o..o + N) else { continue };
                let mut value = [0.0f32; N];
                let mut done = false;
                let segment = match uniform {
                    Some(s) => s,
                    None => self.field.at(x, y).and_then(|v| self.segment(v)),
                };
                if let Some(s) = segment {
                    let phase = if s.jitter { hash01(x, y) } else { 0.5 };
                    let mut total = 0.0f32;
                    for i in 0..s.samples {
                        let u = (i as f32 + phase) * s.step - 1.0;
                        let wt = 1.0 - u.abs();
                        if let Some(p) = bilinear::<N, M>(src.data, src.width, sx + u * s.ux, sy + u * s.uy) {
                            for ch in 0..N {
                                value[ch] += wt * p[ch];
                            }
                            total += wt;
                        }
                    }
                    if total > 0.0 {
                        for v in &mut value {
                            *v /= total;
                        }
                        done = true;
                    }
                }
                if !done {
                    let i = ((row - src.corner) * src.width + col - src.corner) * N;
                    let Some(p) = src.data.get(i..i + N) else { continue };
                    value.copy_from_slice(p);
                }
                dst.copy_from_slice(&value);
            }
        }
    }

    /// How a pixel with displacement `[vx, vy]` samples its segment, or `None` when its segment
    /// is too short to need sampling.
    #[inline(always)]
    fn segment(&self, [vx, vy]: [f32; 2]) -> Option<Segment> {
        let len = (vx * vx + vy * vy).sqrt();
        let half = self.plan.segment * len;
        let k = ((2.0 * half / SEGMENT_SPACING).ceil() as usize).clamp(1, self.plan.segment_taps);
        if !(half >= 0.25 && k > 1) {
            return None;
        }
        // Samples further apart than a pixel and a quarter can catch thin lines unevenly. Sliding
        // each pixel's samples by a different amount (`jitter`) turns that into fine grain, which
        // the main taps then average away.
        Some(Segment { samples: k, step: 2.0 / k as f32, jitter: 2.0 * half / k as f32 > 1.25, ux: vx / len * half, uy: vy / len * half })
    }

    /// Second pass: the output. Each pixel reads its taps from `plane`: the segments, or for a
    /// hard split the band itself.
    ///
    /// Written once per layout (`N` channels, last one alpha or not; `M` is always `2 * N`) so the
    /// compiler can unroll the channel loops and drop bounds checks; the interpreter rewards
    /// every instruction saved.
    fn run<const N: usize, const M: usize, const ALPHA: bool>(&self, plane: &Plane<'_>, dst: &mut [f32]) {
        let (w, h, ov) = (self.w, self.h, self.ov);
        let taps = self.plan.taps.as_slice();
        // Under a hundredth of a pixel: nothing visible would change. In Directional mode that's
        // the same answer for every pixel.
        let still = |[vx, vy]: [f32; 2]| (vx * vx + vy * vy).sqrt() * self.plan.reach < 0.01;
        let uniform = self.field.uniform();
        if uniform.is_some_and(still) {
            return;
        }
        for row in ov..h - ov {
            let y = (self.origin[1] + row as i64) as f32 + 0.5;
            let fy = (row - plane.corner) as f32;
            for col in ov..w - ov {
                let x = (self.origin[0] + col as i64) as f32 + 0.5;
                let Some([vx, vy]) = self.field.at(x, y) else { continue };
                if uniform.is_none() && still([vx, vy]) {
                    continue;
                }
                let fx = (col - plane.corner) as f32;
                let mut acc = [0.0f32; 3];
                let mut cover = [0.0f32; 3];
                for &(t, wt) in taps {
                    // Inside the margin by construction (see `render`), so no clamping.
                    let Some(p) = bilinear::<N, M>(plane.data, plane.width, fx + t * vx, fy + t * vy) else { continue };
                    for c in 0..3 {
                        acc[c] += wt[c] * p[c];
                        if ALPHA {
                            cover[c] += wt[c] * p[N - 1];
                        }
                    }
                }
                let i = (row * w + col) * N;
                let Some(out) = dst.get_mut(i..i + N) else { continue };
                let finish = |v: f32| if self.linear { encode(v) } else { v };
                if ALPHA {
                    // Each colour brings its own coverage; the pixel is as opaque as the most
                    // opaque one, and colours that didn't reach it are missing from its tint.
                    let a = cover[0].max(cover[1]).max(cover[2]);
                    if a <= 1e-6 {
                        out[3] = 0.0;
                        continue;
                    }
                    for c in 0..3 {
                        out[c] = finish(acc[c] / a);
                    }
                    out[3] = a.min(1.0);
                } else {
                    for c in 0..3 {
                        out[c] = finish(acc[c]);
                    }
                }
            }
        }
    }
}

/// The picture at fractional position `(x, y)`, blended from the four nearest pixels. The caller
/// keeps `(x, y)` at least a pixel inside the buffer's right and bottom edges; anything else gives
/// `None` rather than crashing.
#[inline(always)]
fn bilinear<const N: usize, const M: usize>(src: &[f32], w: usize, x: f32, y: f32) -> Option<[f32; N]> {
    let (x0, y0) = (x as usize, y as usize);
    let (fx, fy) = (x - x0 as f32, y - y0 as f32);
    let stride = w * N;
    let i0 = (y0 * w + x0) * N;
    // One range check for the whole 2 × 2 block; the compiler can then prove the two rows below
    // fit, so it adds no checks of its own.
    let block = src.get(i0..i0.checked_add(stride + M)?)?;
    let top: &[f32; M] = block.get(..M)?.try_into().ok()?;
    let bottom: &[f32; M] = block.get(stride..stride + M)?.try_into().ok()?;
    let mut out = [0.0f32; N];
    for c in 0..N {
        let t = top[c] + (top[N + c] - top[c]) * fx;
        let b = bottom[c] + (bottom[N + c] - bottom[c]) * fx;
        out[c] = t + (b - t) * fy;
    }
    Some(out)
}
