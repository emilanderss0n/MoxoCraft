//! What a floating particle looks like, and drawing a band of them.
//!
//! Dust, pollen and specks of light hanging in the air: tiny soft points when in focus, round discs
//! with a slightly brighter rim when not, as a lens draws them. Glow puts a faint halo around the
//! sharp ones, the way bright points bloom in a real photo. Clustering gathers them into loose
//! clouds, as dust shows only where light catches it, instead of spreading them evenly. The clouds
//! are the same at every depth, as a shaft of light picks out near and far dust alike.

use core::f32::consts::PI;

use moxocraft_particles::{Coverage, Dab, Frame, Lens, Motion, Population, Rng, Scatter, Spot};
use moxocraft_sdk::{Error, Image};

use crate::settings::Settings;

/// Particles per unit of area (a square the size of the picture's longer side) at Amount 100, all
/// depths together, before Clustering thins them out. Most are far away and small.
const FULL: f32 = 5_000.0;
/// How the particles spread through depth: the number at a given depth grows with distance to
/// this power (as for Snow's flakes).
const DEPTH_WEIGHT: f32 = 1.0;
/// Particle sizes at Size Variety 100, as a multiple of Particle Size: from small to large,
/// spread as the cube of an even spread, so most are small and a few are big. Lower Size Variety
/// draws them all closer to Particle Size.
const SMALLEST: f32 = 0.25;
const LARGEST: f32 = 3.0;
/// The faintest particles' opacity, against the brightest one's: each catches the light its own
/// way.
const FAINTEST: f32 = 0.4;
/// How round a sharp particle is at least (its width across against its length).
const ROUNDEST: f32 = 0.75;
/// The farthest particles are this much fainter than the nearest, as if seen through haze.
const FAR_FADE: f32 = 0.35;
/// The glow's radius, as a multiple of its particle's in-focus radius, and its opacity at Glow 100
/// against the particle's.
const GLOW_REACH: f32 = 4.0;
const GLOW_ALPHA: f32 = 0.3;
/// A particle's glow shows while it's at least this sharp (the in-focus radius as a fraction of
/// the drawn one), fading in up to the second value; blurred discs have none.
const GLOW_SHARPNESS: (f32, f32) = (0.3, 0.6);
/// The clouds' size: a cell of the random field they come from, in units of the longer side.
const CLOUD_SIZE: f32 = 0.15;
/// The field's values between which a place goes from outside a cloud to inside one.
const CLOUD_EDGE: (f32, f32) = (0.44, 0.56);
/// How much of the picture the clouds cover, on average (measured: see the tests). Particles are
/// placed `1 / CLOUD_COVER` times as densely as Amount asks and that fraction of them kept: evenly
/// at Clustering 0, and only inside clouds at Clustering 100. So Clustering moves particles about
/// without changing how many there are, or redrawing the ones it keeps.
pub(crate) const CLOUD_COVER: f32 = 0.5;

/// Draws one band of floating particles.
pub(crate) fn render(image: &mut Image<'_>, s: &Settings) -> Result<(), Error> {
    moxocraft_particles::render(image, s.depth, s.color, s.opacity, |coverage, frame| draw(coverage, frame, s))
}

fn draw(coverage: &mut Coverage, frame: Frame, s: &Settings) {
    let particles = Particles { s, lens: Lens::new(s.subject, s.aperture), long: frame.long_side };
    Scatter::new(s.seed, s.amount, &particles).for_each(frame.rect, |spot| {
        if s.depth.keeps(spot.depth, s.subject) {
            particle(coverage, spot, s, &particles.lens, particles.long);
        }
    });
}

/// The particles for one set of settings, as the particle engine sees them.
struct Particles<'a> {
    s: &'a Settings,
    lens: Lens,
    /// The picture's longer side in pixels.
    long: f32,
}

/// The smallest and largest size at this Size Variety (0–1), as a multiple of Particle Size: the
/// cube of an even spread between them gives each particle's.
fn sizes(variety: f32) -> (f32, f32) {
    (1.0 + variety * (SMALLEST - 1.0), 1.0 + variety * (LARGEST - 1.0))
}

impl Population for Particles<'_> {
    fn full(&self) -> f32 {
        FULL / CLOUD_COVER
    }

    fn weight(&self, depth: f32) -> f32 {
        self.lens.scale(depth).powf(-DEPTH_WEIGHT)
    }

    fn reach(&self, depth: f32) -> f32 {
        // The largest particle's glow (or the particle, without one), shrinking with distance,
        // plus the lens blur, plus the smallest radius a particle is drawn at (under a pixel).
        let reach = if self.s.glow > 0.0 { GLOW_REACH } else { 1.0 };
        self.s.radius * sizes(self.s.variety).1 * reach * self.lens.scale(depth) + self.lens.blur(depth) + 1.0 / self.long
    }

    fn work(&self, depth: f32) -> f32 {
        // Area goes with the size squared: for `a + b·u³`, its average is a² + ab/2 + b²/7.
        let (a, hi) = sizes(self.s.variety);
        let b = hi - a;
        let size = (a * a + 0.5 * a * b + b * b / 7.0).sqrt();
        let sharp = self.s.radius * size * self.lens.scale(depth);
        let blur = self.lens.blur(depth);
        let mut area = PI * (sharp + blur).powi(2);
        if self.s.glow > 0.0 && sharp >= GLOW_SHARPNESS.0 * (sharp + blur) {
            area += PI * (GLOW_REACH * sharp + blur).powi(2);
        }
        // Only that fraction of the particles placed is kept and drawn.
        area * CLOUD_COVER
    }
}

/// Draws one particle, from its place and its own random numbers.
fn particle(coverage: &mut Coverage, spot: Spot, s: &Settings, lens: &Lens, long: f32) {
    let mut rng = spot.rng;
    // Whether Clustering keeps it, decided first so the rest of its look doesn't depend on
    // Clustering.
    if rng.unit() >= keep(s, &spot) {
        return;
    }
    let u = rng.unit();
    let (smallest, largest) = sizes(s.variety);
    let size = smallest + (largest - smallest) * u * u * u;
    let brightness = FAINTEST + (1.0 - FAINTEST) * rng.unit();
    let (squash, tilt) = (ROUNDEST + (1.0 - ROUNDEST) * rng.unit(), PI * rng.unit());
    let scale = lens.scale(spot.depth);
    let sharp = s.radius * size * scale * long;
    let blur = lens.blur(spot.depth) * long;
    let alpha = brightness * (1.0 - FAR_FADE * spot.depth);
    let (x, y) = (spot.x * long, spot.y * long);

    // The glow first: the same color, so the order only matters for rounding.
    let hard = if sharp + blur > 0.0 { sharp / (sharp + blur) } else { 1.0 };
    let (lo, hi) = GLOW_SHARPNESS;
    let glow = s.glow * GLOW_ALPHA * ((hard - lo) / (hi - lo)).clamp(0.0, 1.0);
    if glow > 0.0 {
        coverage.dab(&Dab::new(x, y, sharp * GLOW_REACH, blur, Motion::STILL, alpha * glow, spot.depth));
    }
    coverage.dab(&Dab::new(x, y, sharp, blur, Motion::STILL, alpha, spot.depth).squashed(squash, tilt));
}

/// The chance that a particle placed at `spot` is kept: [`CLOUD_COVER`] everywhere at Clustering
/// 0; at Clustering 100, 1 inside clouds and 0 outside them.
fn keep(s: &Settings, spot: &Spot) -> f32 {
    if s.clustering <= 0.0 {
        return CLOUD_COVER;
    }
    let inside = smoothstep(CLOUD_EDGE.0, CLOUD_EDGE.1, field(s.seed, spot.x, spot.y));
    (CLOUD_COVER * (1.0 - s.clustering) + inside * s.clustering).min(1.0)
}

/// A smooth random field, 0–1, worked out from a place alone so every band agrees on it: value
/// noise in two layers, the second finer, so the clouds don't line up with the grid.
pub(crate) fn field(seed: u32, x: f32, y: f32) -> f32 {
    let (x, y) = (x / CLOUD_SIZE, y / CLOUD_SIZE);
    0.65 * noise(seed, 0, x, y) + 0.35 * noise(seed, 1, 2.3 * x + 0.37, 2.3 * y + 0.61)
}

/// Value noise: a random value at each corner of a grid of unit squares, blended smoothly between.
fn noise(seed: u32, layer: u32, x: f32, y: f32) -> f32 {
    let (ix, iy) = (x.floor(), y.floor());
    let (fx, fy) = (smooth(x - ix), smooth(y - iy));
    let (ix, iy) = (ix as i32, iy as i32);
    // 0xC10D marks these apart from the particles' own random numbers.
    let corner = |dx: i32, dy: i32| Rng::from(&[seed, 0xC10D_0000 + layer, (ix + dx) as u32, (iy + dy) as u32]).unit();
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    lerp(lerp(corner(0, 0), corner(1, 0), fx), lerp(corner(0, 1), corner(1, 1), fx), fy)
}

#[inline]
fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

#[inline]
fn smoothstep(lo: f32, hi: f32, v: f32) -> f32 {
    smooth(((v - lo) / (hi - lo)).clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clouds_cover_what_clustering_assumes() {
        // Averaged over many places and layouts, the share of the picture inside a cloud.
        let mut rng = Rng::from(&[1]);
        let n = 200_000;
        let mut sum = 0.0f64;
        for i in 0..n {
            let (x, y) = (rng.unit() * 4.0, rng.unit() * 4.0);
            sum += f64::from(smoothstep(CLOUD_EDGE.0, CLOUD_EDGE.1, field(1 + i % 50, x, y)));
        }
        let cover = (sum / f64::from(n)) as f32;
        assert!((cover - CLOUD_COVER).abs() < 0.02, "clouds cover {cover}, CLOUD_COVER says {CLOUD_COVER}");
    }
}
