//! Where the particles sit.
//!
//! Space in front of the camera is split into [`SLICES`] slices of depth. Each slice is a grid of
//! square cells (measured in units of the picture's longer side, like every position here), sized
//! so a cell holds about [`FULL_PER_CELL`] particles at full Amount. A cell's particles come from a
//! hash of the Variation number, the slice and the cell's coordinates, so any band can work out
//! any cell's particles on its own and every band gets the same answer.
//!
//! A particle's place doesn't depend on Amount: Amount only changes how many of a cell's particles
//! exist, so raising it adds particles without moving the ones already there.
//!
//! The work a band does grows with how much particle area it draws. Measured with
//! `moxocraft-bench`: about 125 interpreter instructions for each pixel a disc goes over (160 with
//! a depth map), twice that for a curved streak, against about 90 per pixel for everything else.
//! With everything at its largest (big, near, blurred, streaked particles) that would run past
//! PhotoCraft's 60-second limit, so the number of particles is capped to keep the drawing within
//! [`BUDGET`]. The cap comes from the settings alone, not the picture's size, so the preview and
//! the result agree.

use crate::hash::Rng;

/// Slices of depth. Each particle gets its own depth within its slice, so the slices don't show
/// as layers; more of them only lets a band skip more of the far, small particles that can't
/// reach it.
pub const SLICES: usize = 32;

/// Particles per cell at full Amount.
const FULL_PER_CELL: f32 = 4.0;

/// The most drawing per pixel, on average, in [`Population::work`]'s units: 9 discs deep. That's
/// about 1500 instructions per pixel with a depth map; Snow's heaviest settings took about 22
/// seconds on a 24-megapixel picture with PhotoCraft on four cores. Beyond it, Amount stops adding
/// particles.
pub const BUDGET: f32 = 9.0;

/// How much more a pixel of streak costs to draw than a pixel of disc (for a curved streak, three
/// straight pieces long).
pub const STREAK_COST: f32 = 2.0;

/// What a plug-in's particles are like, as far as placing them goes. Lengths and areas are in
/// units of the picture's longer side.
pub trait Population {
    /// Particles per unit of area (a square the size of the longer side) at full Amount, all
    /// depths together.
    fn full(&self) -> f32;
    /// How many particles sit at `depth`, compared with other depths.
    fn weight(&self, depth: f32) -> f32;
    /// The furthest a particle at `depth` can reach from its center. For any range of depths it
    /// must be largest at one end of the range. That holds for anything that shrinks with
    /// distance plus [`Lens::blur`](crate::Lens::blur), which falls toward the subject's distance
    /// from both sides.
    fn reach(&self, depth: f32) -> f32;
    /// Roughly how much drawing an average particle at `depth` takes: the area it goes over,
    /// times [`STREAK_COST`] for a streak.
    fn work(&self, depth: f32) -> f32;
}

#[derive(Clone, Copy, Debug, Default)]
struct Slice {
    /// The depths this slice covers.
    near: f32,
    far: f32,
    /// A cell's width and height.
    cell: f32,
    /// Particles per cell at the chosen Amount (a fraction gives some cells one more).
    per_cell: f32,
    /// The furthest any particle of this slice reaches from its center.
    reach: f32,
}

/// One particle's place: its center (in units of the picture's longer side, from the document's
/// top-left corner), its depth (0 at the lens, 1 the farthest) and its own random numbers for
/// whatever else the plug-in decides about it.
#[derive(Clone, Copy, Debug)]
pub struct Spot {
    pub x: f32,
    pub y: f32,
    pub depth: f32,
    pub rng: Rng,
}

/// Every particle's place, for one set of settings.
pub struct Scatter {
    seed: u32,
    slices: [Slice; SLICES],
    /// The drawing per pixel, on average, in [`Population::work`]'s units.
    work: f32,
}

impl Scatter {
    /// Places `amount` (0–1) of `particles`' full number, as far as [`BUDGET`] allows.
    pub fn new(seed: u32, amount: f32, particles: &impl Population) -> Self {
        let step = 1.0 / SLICES as f32;
        let mid = |k: usize| (k as f32 + 0.5) * step;
        let total: f32 = (0..SLICES).map(|k| particles.weight(mid(k)).max(0.0)).sum();
        // Particles per unit of area in each slice at full Amount.
        let density: [f32; SLICES] = core::array::from_fn(|k| {
            let d = if total > 0.0 { particles.full().max(0.0) * particles.weight(mid(k)).max(0.0) / total } else { 0.0 };
            if d.is_finite() { d } else { 0.0 }
        });
        let amount = amount.clamp(0.0, 1.0);
        let wanted: f32 = (0..SLICES).map(|k| density[k] * amount * particles.work(mid(k)).max(0.0)).sum();
        let kept = if wanted > BUDGET { BUDGET / wanted } else { 1.0 };
        let mut slices = [Slice::default(); SLICES];
        for (k, s) in slices.iter_mut().enumerate() {
            if density[k] <= 0.0 {
                continue;
            }
            let (near, far) = (k as f32 * step, (k + 1) as f32 * step);
            let reach = particles.reach(near).max(particles.reach(far)).max(0.0);
            *s = Slice { near, far, cell: (FULL_PER_CELL / density[k]).sqrt(), per_cell: FULL_PER_CELL * amount * kept, reach };
        }
        Scatter { seed, slices, work: wanted.min(BUDGET) }
    }

    /// The drawing per pixel, on average, in [`Population::work`]'s units (at most [`BUDGET`]).
    pub fn work(&self) -> f32 {
        self.work
    }

    /// Calls `f` with every particle that can touch `rect` (`[left, top, right, bottom]`, in
    /// units of the picture's longer side): farthest slice first, then cell by cell, row by row.
    /// Every band meets the particles it shares with another band in the same order, so where
    /// they overlap, both draw exactly the same pixels.
    pub fn for_each(&self, rect: [f32; 4], mut f: impl FnMut(Spot)) {
        for (k, s) in self.slices.iter().enumerate().rev() {
            if s.per_cell <= 0.0 {
                continue;
            }
            let cells = |lo: f32, hi: f32| ((lo - s.reach) / s.cell).floor() as i32..=((hi + s.reach) / s.cell).floor() as i32;
            for cy in cells(rect[1], rect[3]) {
                for cx in cells(rect[0], rect[2]) {
                    let key = [self.seed, k as u32, cx as u32, cy as u32];
                    let count = (s.per_cell + Rng::from(&key).unit()) as u32;
                    for i in 0..count {
                        let mut rng = Rng::from(&[key[0], key[1], key[2], key[3], i + 1]);
                        let x = (cx as f32 + rng.unit()) * s.cell;
                        let y = (cy as f32 + rng.unit()) * s.cell;
                        if x + s.reach < rect[0] || x - s.reach > rect[2] || y + s.reach < rect[1] || y - s.reach > rect[3] {
                            continue;
                        }
                        let depth = s.near + (s.far - s.near) * rng.unit();
                        f(Spot { x, y, depth, rng });
                    }
                }
            }
        }
    }
}
