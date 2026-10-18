//! Where each pixel reads along its displacement, and how much work that may cost.
//!
//! [`Taps`] lays out the points ("taps") for a fringe and softness; [`Plan`] picks how many taps
//! and segment samples an image gets, so a run stays inside PhotoCraft's time limit.

use moxocraft_sdk::Geometry;

use crate::settings::Fringe;

/// Fewest and most main taps a pixel reads when colours are smeared.
pub(crate) const MIN_TAPS: usize = 5;
pub const MAX_TAPS: usize = 16;

/// Segments are sampled about this many pixels apart: close enough that even a one-pixel line
/// is picked up evenly wherever it falls between samples.
pub(crate) const SEGMENT_SPACING: f32 = 1.0;
/// Most samples in one segment.
const MAX_SEGMENT_TAPS: usize = 32;

/// How much interpreter work a whole image may take, in PhotoCraft's instruction units. At the
/// ~500 million a second one thread manages, this is 30 seconds on 4 threads: well inside the
/// 60-second limit. Big images with long smears get fewer samples to stay inside it.
const WORK_BUDGET: f64 = 60e9;
/// Rough cost of a pixel without taps, and of one tap (measured with `moxocraft-bench`).
const COST_PER_PIXEL: f64 = 600.0;
const COST_PER_TAP: f64 = 150.0;

/// The points each pixel reads along its displacement, and how much each one adds to red, green
/// and blue. Each colour's weights add up to 1, so flat areas come out unchanged.
#[derive(Clone, Debug, PartialEq)]
pub struct Taps {
    /// Position along the displacement, as a fraction of Amount.
    pub offset: Vec<f32>,
    pub weight: Vec<[f32; 3]>,
    /// The distance between neighbouring taps inside a colour's smear, as a fraction of Amount
    /// (0 for a hard split).
    pub gap: f32,
}

impl Taps {
    /// True when each colour comes from a single tap (a crisp split), whatever the tap count.
    pub fn is_hard(fringe: Fringe, softness: f32) -> bool {
        Self::spread(fringe, softness) < 1e-3
    }

    fn spread(fringe: Fringe, softness: f32) -> f32 {
        fringe.centres().1 + 0.75 * softness.clamp(0.0, 1.0)
    }

    /// A layout of about `count` taps.
    pub fn new(fringe: Fringe, softness: f32, count: usize) -> Taps {
        let (centres, _) = fringe.centres();
        let spread = Self::spread(fringe, softness);
        let count = count.clamp(2, MAX_TAPS);
        let mut groups: Vec<f32> = Vec::new();
        for c in centres {
            if !groups.iter().any(|g| (g - c).abs() < 1e-6) {
                groups.push(c);
            }
        }
        let hard = Self::is_hard(fringe, softness);
        let (offset, gap): (Vec<f32>, f32) = if hard {
            // One tap per colour position: a crisp split.
            (groups.clone(), 0.0)
        } else if fringe == Fringe::Spectral {
            // Evenly across the whole rainbow.
            let lo = centres.iter().fold(f32::MAX, |a, &b| a.min(b)) - spread;
            let hi = centres.iter().fold(f32::MIN, |a, &b| a.max(b)) + spread;
            let step = (hi - lo) / count as f32;
            ((0..count).map(|k| lo + (k as f32 + 0.5) * step).collect(), step)
        } else {
            // Spread evenly across each colour's own smear.
            let per = (count / groups.len()).max(1);
            let step = 2.0 * spread / per as f32;
            (groups.iter().flat_map(|&g| (0..per).map(move |k| g - spread + (k as f32 + 0.5) * step)).collect(), step)
        };
        let mut weight: Vec<[f32; 3]> = offset
            .iter()
            .map(|&t| centres.map(|c| if hard { f32::from(u8::from((t - c).abs() < 1e-6)) } else { bump((t - c) / spread) }))
            .collect();
        for c in 0..3 {
            let mut sum: f32 = weight.iter().map(|w| w[c]).sum();
            if sum < 1e-6 {
                // Too few taps to land inside this colour's smear: use the nearest one.
                let nearest = (0..offset.len()).min_by(|&a, &b| (offset[a] - centres[c]).abs().total_cmp(&(offset[b] - centres[c]).abs()));
                if let Some(w) = nearest.and_then(|i| weight.get_mut(i)) {
                    w[c] = 1.0;
                }
                sum = 1.0;
            }
            for w in &mut weight {
                w[c] /= sum;
            }
        }
        // Drop taps no colour uses.
        let (offset, weight) = offset.into_iter().zip(weight).filter(|(_, w)| w.iter().any(|&v| v > 0.0)).unzip();
        Taps { offset, weight, gap }
    }

    /// The furthest any tap reads, as a fraction of Amount.
    pub fn reach(&self) -> f32 {
        self.offset.iter().fold(0.0, |a, &t| a.max(t.abs()))
    }
}

/// A smooth bump: 1 at 0, falling to 0 at ±1 with no hard corners (the "biweight" curve).
fn bump(x: f32) -> f32 {
    if x.abs() >= 1.0 {
        return 0.0;
    }
    let q = 1.0 - x * x;
    q * q
}

/// How this image gets filtered: one tap layout for every pixel, and how the segments are built.
/// Everything here depends only on the settings and the canvas, never on the band, so every
/// band of an image makes the same choices and they join seamlessly.
pub(crate) struct Plan {
    pub taps: Vec<(f32, [f32; 3])>,
    /// True when taps read segments (smeared colours), false for a hard split.
    pub segmented: bool,
    /// Segment half-length as a fraction of Amount: the gap between neighbouring taps.
    pub segment: f32,
    /// Most samples per segment.
    pub segment_taps: usize,
    /// The furthest a tap reads, as a fraction of Amount.
    pub reach: f32,
}

impl Plan {
    /// The furthest any read can go (a tap plus the segment around it), as a fraction of Amount,
    /// whichever tap count the plan ends up with. Needed before the plan, to cap the
    /// displacement.
    pub fn worst_reach(fringe: Fringe, softness: f32) -> f32 {
        (MIN_TAPS..=MAX_TAPS).map(|n| Taps::new(fringe, softness, n)).fold(0.0, |a, t| a.max(t.reach() + t.gap))
    }

    /// The furthest a tap alone can read, whichever tap count the plan ends up with.
    pub fn worst_tap_reach(fringe: Fringe, softness: f32) -> f32 {
        (MIN_TAPS..=MAX_TAPS).map(|n| Taps::new(fringe, softness, n)).fold(0.0, |a, t| a.max(t.reach()))
    }

    /// Picks the tap count and segment samples for displacements up to `longest` pixels, within
    /// the image's work budget.
    pub fn new(fringe: Fringe, softness: f32, longest: f32, g: &Geometry, tap_margin: usize) -> Plan {
        let finish = |t: Taps, segment_taps: usize| {
            let (reach, segment, segmented) = (t.reach(), t.gap, t.gap > 0.0);
            Plan { taps: t.offset.into_iter().zip(t.weight).collect(), segmented, segment, segment_taps, reach }
        };
        if Taps::is_hard(fringe, softness) {
            return finish(Taps::new(fringe, softness, MIN_TAPS), 1);
        }
        // The smear's full length in pixels at its longest.
        let probe = Taps::new(fringe, softness, MAX_TAPS);
        let span = probe.gap * MAX_TAPS as f32 * longest;
        // Segments are also worked out for the band's margin; PhotoCraft's bands are about
        // 1024 × 256.
        let m = tap_margin as f32;
        let margin_cost = (1.0 + 2.0 * m / 256.0) * (1.0 + 2.0 * m / 1024.0);
        // Cost per pixel is about taps + segment samples × margin cost, with segment samples
        // ≈ 2 × span / taps: this tap count makes the total smallest.
        let ideal = ((2.0 * span * margin_cost / SEGMENT_SPACING).sqrt().ceil() as usize).clamp(MIN_TAPS, MAX_TAPS);
        let ideal_samples = ((2.0 * span / ideal as f32 / SEGMENT_SPACING).ceil() as usize).clamp(1, MAX_SEGMENT_TAPS);
        let pixels = (g.canvas_width.max(1) * g.canvas_height.max(1)) as f64;
        let budget = ((WORK_BUDGET / pixels - COST_PER_PIXEL) / COST_PER_TAP) as f32;
        let (count, samples) = if ideal as f32 + ideal_samples as f32 * margin_cost <= budget {
            (ideal, ideal_samples)
        } else {
            // Over budget: split it between taps and segment samples, which keeps the samples
            // as close together as the budget allows.
            let count = ((budget / 2.0) as usize).clamp(MIN_TAPS, MAX_TAPS);
            (count, (((budget - count as f32) / margin_cost) as usize).clamp(1, ideal_samples))
        };
        finish(Taps::new(fringe, softness, count), samples)
    }
}
