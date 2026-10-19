//! What a snowflake looks like, and drawing a band of them.
//!
//! Real snowflakes at photo distances are irregular clumps of crystals, mostly small with a few
//! big fluffy ones, so an in-focus flake here is an oval blob with up to two smaller blobs stuck
//! to it. Out of focus, the lens's round blur takes over and the engine draws a soft disc with a
//! slightly brighter rim. Falling flakes drift and tumble rather than drop straight like rain:
//! each streak leans its own way, bows a little and flickers as the flake turns in the light.

use core::f32::consts::{PI, TAU};

use moxocraft_particles::{Coverage, Dab, Frame, Lens, Motion, Population, STREAK_COST, Scatter, Spot};
use moxocraft_sdk::{Error, Image};

use crate::settings::Settings;

/// Flakes per unit of area (a square the size of the picture's longer side) at Amount 100, all
/// depths together. Most are far away and small.
const FULL: f32 = 15_000.0;
/// How the flakes spread through depth: the number at a given depth grows with distance to this
/// power. A real snowfall's would be 2 (the space a picture sees grows with the square of the
/// distance); less keeps more of the large, near flakes that make snow read as snow.
const DEPTH_WEIGHT: f32 = 1.0;
/// Flake sizes, as a multiple of Flake Size: from small to large, spread as the cube of an even
/// spread, so most flakes are small and a few are big clumps.
const SMALLEST: f32 = 0.35;
const LARGEST: f32 = 2.0;
/// The average of those sizes (the cube of an even spread averages ¼).
const MEAN_SIZE: f32 = SMALLEST + (LARGEST - SMALLEST) / 4.0;
/// How far a clump's extra blobs reach from its centre, as a multiple of its main blob's radius:
/// set up to 0.85 of it away, and up to 0.65 of it in radius.
const CLUMP_REACH: f32 = 1.5;
/// A clump's blobs show while the flake is at least this sharp (the in-focus radius as a fraction
/// of the drawn one), fading in up to the second value; blurrier flakes are round.
const CLUMP_SHARPNESS: (f32, f32) = (0.45, 0.7);
/// Streak lengths, as a multiple of Motion's: flakes fall at different speeds.
const SHORTEST: f32 = 0.5;
const LONGEST: f32 = 1.5;
/// How widely (in radians, all told) streaks lean away from the fall direction: a little even at
/// Swirl 0, as flakes tumble, and this much more at Swirl 100.
const TUMBLE: f32 = 0.1;
const MAX_DRIFT: f32 = 1.4;
/// How far a streak can bow sideways at Swirl 100, as a fraction of its length.
const MAX_BEND: f32 = 0.22;
/// The farthest flakes are this much fainter than the nearest, as if seen through the snowfall.
const FAR_FADE: f32 = 0.35;

/// Draws one band of snow.
pub(crate) fn render(image: &mut Image<'_>, s: &Settings) -> Result<(), Error> {
    moxocraft_particles::render(image, s.depth, s.tint.colour(), s.opacity, |coverage, frame| draw(coverage, frame, s))
}

fn draw(coverage: &mut Coverage, frame: Frame, s: &Settings) {
    let flakes = Flakes { s, lens: Lens::new(s.subject, s.aperture), long: frame.long_side };
    Scatter::new(s.seed, s.amount, &flakes).for_each(frame.rect, |spot| {
        if s.depth.keeps(spot.depth, s.subject) {
            flake(coverage, spot, s, &flakes.lens, flakes.long);
        }
    });
}

/// Snow's flakes for one set of settings, as the particle engine sees them.
struct Flakes<'a> {
    s: &'a Settings,
    lens: Lens,
    /// The picture's longer side in pixels.
    long: f32,
}

impl Flakes<'_> {
    /// The widest any streak leans from straight down, and the most any bows sideways.
    fn sway(&self) -> (f32, f32) {
        let spread = TUMBLE + MAX_DRIFT * self.s.swirl;
        ((self.s.angle.abs() + 0.5 * spread).min(PI / 2.0 - 0.01), MAX_BEND * self.s.swirl)
    }
}

impl Population for Flakes<'_> {
    fn full(&self) -> f32 {
        FULL
    }

    fn weight(&self, depth: f32) -> f32 {
        self.lens.scale(depth).powf(-DEPTH_WEIGHT)
    }

    fn reach(&self, depth: f32) -> f32 {
        // The largest clump and the farthest point of the longest streak (half its length, or
        // its bow), both shrinking with distance, plus the lens blur, plus the smallest radius a
        // particle is drawn at (under a pixel).
        let (_, bend) = self.sway();
        let biggest = self.s.flake_radius * LARGEST * CLUMP_REACH + self.s.streak * LONGEST * (0.5 + bend);
        biggest * self.lens.scale(depth) + self.lens.blur(depth) + 1.0 / self.long
    }

    fn work(&self, depth: f32) -> f32 {
        let scale = self.lens.scale(depth);
        let sharp = self.s.flake_radius * MEAN_SIZE * scale;
        let r = sharp + self.lens.blur(depth);
        let length = self.s.streak * scale;
        if length <= 0.0 {
            // Clumps add about a quarter while they show.
            let clumps = if sharp >= CLUMP_SHARPNESS.0 * r { 1.25 } else { 1.0 };
            return PI * r * r * clumps;
        }
        // A streak is drawn row by row, each row as wide as the stretch of path within reach.
        let (lean, bend) = self.sway();
        let rows = 2.0 * r + length;
        let width = 2.0 * r + (2.0 * r * (lean.tan() + 4.0 * bend)).min(length * (lean.sin() + 2.0 * bend));
        STREAK_COST * rows * width
    }
}

/// Draws one flake, from its place and its own random numbers.
fn flake(coverage: &mut Coverage, spot: Spot, s: &Settings, lens: &Lens, long: f32) {
    let mut rng = spot.rng;
    let u = rng.unit();
    let size = SMALLEST + (LARGEST - SMALLEST) * u * u * u;
    let brightness = 0.5 + 0.5 * rng.unit();
    let (squash, tilt) = (0.55 + 0.45 * rng.unit(), PI * rng.unit());
    let blobs = match rng.unit() {
        c if c < 0.35 => 0,
        c if c < 0.8 => 1,
        _ => 2,
    };
    let scale = lens.scale(spot.depth);
    let sharp = s.flake_radius * size * scale * long;
    let blur = lens.blur(spot.depth) * long;
    let alpha = brightness * (1.0 - FAR_FADE * spot.depth);
    let (x, y) = (spot.x * long, spot.y * long);

    if s.streak > 0.0 {
        let length = s.streak * scale * long * (SHORTEST + (LONGEST - SHORTEST) * rng.unit());
        let angle = s.angle + (TUMBLE + MAX_DRIFT * s.swirl) * (rng.unit() - 0.5);
        let bend = MAX_BEND * s.swirl * (2.0 * rng.unit() - 1.0);
        let motion = Motion::curved(angle.sin() * length, angle.cos() * length, bend).flickering(0.1 + 0.3 * rng.unit(), 0.4 + 1.0 * rng.unit(), rng.unit());
        coverage.dab(&Dab::new(x, y, sharp, blur, motion, alpha, spot.depth));
        return;
    }

    coverage.dab(&Dab::new(x, y, sharp, blur, Motion::STILL, alpha, spot.depth).squashed(squash, tilt));
    // The clump's other blobs, while the flake is sharp enough for its shape to show.
    let hard = if sharp + blur > 0.0 { sharp / (sharp + blur) } else { 1.0 };
    let (lo, hi) = CLUMP_SHARPNESS;
    let show = ((hard - lo) / (hi - lo)).clamp(0.0, 1.0);
    if show <= 0.0 {
        return;
    }
    for _ in 0..blobs {
        let (angle, away, radius) = (TAU * rng.unit(), sharp * (0.45 + 0.4 * rng.unit()), sharp * (0.35 + 0.3 * rng.unit()));
        let blob = Dab::new(x + angle.cos() * away, y + angle.sin() * away, radius, blur, Motion::STILL, alpha * (0.7 + 0.3 * rng.unit()) * show, spot.depth);
        coverage.dab(&blob);
    }
}
