//! Drawing particles into a band-sized buffer.
//!
//! The buffer keeps, for each pixel, how much of what's underneath still shows through the
//! particles drawn so far ("clear": 1 with no particle, 0 fully covered). Every particle of a
//! plug-in has the same colour, so that's all [`render`](crate::render) needs to lay them over the
//! band afterwards, and particles can be drawn in any order of colour.
//!
//! Every position here is in document pixels, worked out the same way in every band, so a pixel's
//! value never depends on which band it falls in.

use moxocraft_sdk::{Error, try_vec};

use crate::depth::EDGE;

/// Particles never get smaller than this radius in pixels; smaller ones are drawn at this size
/// and fainter, keeping the light they give, so tiny far particles neither flicker nor vanish
/// (they also stay the same on PhotoCraft's reduced live preview).
const MIN_RADIUS: f32 = 0.75;
/// The soft rim of an out-of-focus disc is at least this many pixels wide…
const EDGE_PX: f32 = 1.25;
/// …and at least this fraction of its radius.
const MIN_EDGE: f32 = 0.3;
/// How quickly particles fade as blur spreads them out: 2 would keep their light exactly, which
/// leaves out-of-focus particles almost invisible; real near-lens flakes and dust show more.
const BOKEH_FADE: f32 = 0.75;
/// The same for streaks.
const STREAK_FADE: f32 = 0.6;
/// How much dimmer the middle of a fully out-of-focus disc is than its rim. Real lenses draw
/// out-of-focus specks as discs with a slightly brighter edge, not flat coins. A moving one keeps
/// only a trace of it, as the edges smear along the streak.
const RING: f32 = 0.3;
const MOVING_RING: f32 = 0.1;
/// A streak's soft edge is at least this fraction of its radius: smeared light has no crisp rim.
const MOVING_EDGE: f32 = 0.6;

/// The most points a [`Motion`] path has: three straight pieces, enough for a gentle curve.
pub const PATH_POINTS: usize = 4;

/// How a particle moves while the shutter is open: a path through up to [`PATH_POINTS`] points,
/// and how its brightness flickers along the way, as a tumbling flake catches the light.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Motion {
    /// The path, relative to the particle's centre, in pixels; only the first `count` are used.
    points: [[f32; 2]; PATH_POINTS],
    count: usize,
    /// How far the brightness dips (0–1), in how many waves along the path, starting how far
    /// (0–1) into a wave.
    flicker: f32,
    cycles: f32,
    phase: f32,
}

impl Motion {
    /// Not moving: a disc.
    pub const STILL: Motion = Motion { points: [[0.0; 2]; PATH_POINTS], count: 1, flicker: 0.0, cycles: 0.0, phase: 0.0 };

    /// A straight streak along (`dx`, `dy`) pixels, centred on the particle.
    pub fn straight(dx: f32, dy: f32) -> Self {
        let mut points = [[0.0; 2]; PATH_POINTS];
        points[0] = [-0.5 * dx, -0.5 * dy];
        points[1] = [0.5 * dx, 0.5 * dy];
        Motion { points, count: 2, ..Motion::STILL }
    }

    /// A streak along (`dx`, `dy`) pixels that bows sideways at its middle by `bend` times its
    /// length, as drifting flakes do (positive bows to the left of the way it travels, on
    /// screen). Drawn as three straight pieces through four points of the curve.
    pub fn curved(dx: f32, dy: f32, bend: f32) -> Self {
        // Across the path, as long as the path itself.
        let (nx, ny) = (dy, -dx);
        let points = core::array::from_fn(|k| {
            let u = k as f32 / (PATH_POINTS - 1) as f32;
            let side = 4.0 * u * (1.0 - u) * bend;
            [(u - 0.5) * dx + side * nx, (u - 0.5) * dy + side * ny]
        });
        Motion { points, count: PATH_POINTS, ..Motion::STILL }
    }

    /// The same path with its brightness dipping by up to `depth` (0–1) in `cycles` waves along
    /// it, starting `phase` (0–1) of a wave in.
    pub fn flickering(self, depth: f32, cycles: f32, phase: f32) -> Self {
        Motion { flicker: depth.clamp(0.0, 1.0), cycles: cycles.max(0.0), phase, ..self }
    }

    /// The path's length in pixels.
    fn length(&self) -> f32 {
        self.points[..self.count].windows(2).map(|p| ((p[1][0] - p[0][0]).powi(2) + (p[1][1] - p[0][1]).powi(2)).sqrt()).sum()
    }
}

/// One particle as it's drawn: a soft disc (round or oval), or a soft streak (a round disc swept
/// along a path).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dab {
    /// Its centre, in document pixels.
    x: f32,
    y: f32,
    motion: Motion,
    /// Its radius in pixels.
    radius: f32,
    /// How much of the radius is at full strength before it fades (0 for a rounded dome that fades
    /// from the centre; near 1 for a flat disc with a soft rim).
    core: f32,
    /// How much dimmer the core's middle is than its edge (see [`RING`]).
    ring: f32,
    /// How opaque its strongest part is, 0–1.
    alpha: f32,
    /// Its depth, for hiding it behind the scene in depth-map mode.
    depth: f32,
    /// How sharp it is: the in-focus radius as a fraction of the drawn one.
    hard: f32,
    /// A still dab's outline as an oval: the terms of `a·x² + 2b·x·y + c·y²`, 1, 0, 1 for a circle.
    oval: [f32; 3],
}

impl Dab {
    /// A particle centred on (`x`, `y`), `sharp` pixels in radius when in focus, spread by a blur
    /// disc of radius `blur` and moving along `motion`, at most `alpha` opaque when sharp and
    /// still. Blur and motion spread its light over more pixels, so they also make it fainter.
    pub fn new(x: f32, y: f32, sharp: f32, blur: f32, motion: Motion, alpha: f32, depth: f32) -> Self {
        let (sharp, blur) = (sharp.max(0.0), blur.max(0.0));
        let mut radius = sharp + blur;
        let hard = if radius > 0.0 { sharp / radius } else { 1.0 };
        let mut alpha = alpha * hard.powf(BOKEH_FADE);
        let length = if motion.count > 1 { motion.length() } else { 0.0 };
        // A streak's area against its disc's, as π·r + 2·length against π·r (both times r).
        let area = |r: f32| r * (core::f32::consts::PI * r + 2.0 * length);
        if length > 0.0 {
            alpha *= (core::f32::consts::PI * radius * radius / area(radius)).powf(STREAK_FADE);
        }
        if radius < MIN_RADIUS {
            alpha *= area(radius) / area(MIN_RADIUS);
            radius = MIN_RADIUS;
        }
        let (min_edge, ring) = if length > 0.0 { (MOVING_EDGE, MOVING_RING) } else { (MIN_EDGE, RING) };
        let rim = (EDGE_PX / radius).clamp(min_edge, 1.0);
        let core = (1.0 - hard) * (1.0 - rim);
        let ring = ring * (1.0 - hard) * (1.0 - hard);
        // A small dab's rim takes up more of it, so it gives less light than the same dab drawn
        // larger; make that up, so it looks the same at any size (and on the reduced preview).
        let ideal = (1.0 - hard) * (1.0 - min_edge);
        alpha *= light(ideal, ring) / light(core, ring);
        let motion = if length > 0.0 { motion } else { Motion::STILL };
        Dab { x, y, motion, radius, core, ring, alpha: alpha.clamp(0.0, 1.0), depth, hard, oval: [1.0, 0.0, 1.0] }
    }

    /// Squashes a still dab into an oval `ratio` (0–1) as wide across as along `angle` (radians,
    /// clockwise from pointing right, on screen). Blur rounds it off again, since a lens's blur
    /// disc is round, and a moving dab stays round.
    pub fn squashed(mut self, ratio: f32, angle: f32) -> Self {
        let ratio = 1.0 - (1.0 - ratio.clamp(0.05, 1.0)) * self.hard;
        let (c, s) = (angle.cos(), angle.sin());
        let k = 1.0 / (ratio * ratio);
        // Distance along the long axis squared, plus across it scaled up by 1 / ratio.
        self.oval = [c * c + s * s * k, c * s * (1.0 - k), s * s + c * c * k];
        self
    }

    /// The rectangle it might touch, as `[left, top, right, bottom]` in document pixels.
    fn bounds(&self) -> [f32; 4] {
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for p in &self.motion.points[..self.motion.count] {
            (x0, y0, x1, y1) = (x0.min(p[0]), y0.min(p[1]), x1.max(p[0]), y1.max(p[1]));
        }
        let r = self.radius;
        [self.x + x0 - r, self.y + y0 - r, self.x + x1 + r, self.y + y1 + r]
    }
}

/// The light a disc gives with this `core` and `ring`, as a fraction of a flat disc's. Measured
/// over the radius squared (in which the disc's area grows evenly), the core gives
/// `core² · (1 − ring / 2)` and the smooth fall from it to the rim gives half the rest.
fn light(core: f32, ring: f32) -> f32 {
    let c2 = core * core;
    c2 * (1.0 - 0.5 * ring) + 0.5 * (1.0 - c2)
}

/// How much of the band still shows through the particles drawn into it.
pub struct Coverage {
    clear: Vec<f32>,
    width: usize,
    height: usize,
    /// Document position of the band's top-left pixel.
    origin: [i64; 2],
    /// In depth-map mode, the scene's depth at each pixel.
    scene: Option<Vec<f32>>,
}

impl Coverage {
    /// An empty buffer for a band `width × height` pixels with its top-left pixel at `origin`.
    pub fn new(width: usize, height: usize, origin: [i64; 2]) -> Result<Self, Error> {
        Ok(Coverage { clear: try_vec(width * height, 1.0f32)?, width, height, origin, scene: None })
    }

    /// Hides particles behind `scene` (one depth per pixel, as `depth::read_map` makes them).
    pub(crate) fn with_scene(mut self, scene: Vec<f32>) -> Self {
        self.scene = Some(scene);
        self
    }

    /// How much of each pixel the particles cover, 0–1, row by row.
    pub(crate) fn covered(&self) -> impl Iterator<Item = f32> + '_ {
        self.clear.iter().map(|c| 1.0 - c)
    }

    /// Draws one particle.
    pub fn dab(&mut self, d: &Dab) {
        if d.alpha <= 0.0 || d.radius <= 0.0 {
            return;
        }
        match (d.motion.count > 1, self.scene.is_some()) {
            (false, false) => self.draw::<false, false>(d),
            (false, true) => self.draw::<false, true>(d),
            (true, false) => self.draw::<true, false>(d),
            (true, true) => self.draw::<true, true>(d),
        }
    }

    /// [`dab`](Self::dab), written once per shape and mode so the pixel loop checks neither.
    fn draw<const PATH: bool, const SCENE: bool>(&mut self, d: &Dab) {
        let r = d.radius;
        let [ox, oy] = self.origin;
        // The pixels whose centres fall between `lo` and `hi`, as band columns or rows (and a few
        // more: they're only skipped if the shape doesn't reach them).
        let span = |lo: f32, hi: f32, origin: i64, size: usize| {
            let first = (lo.floor() as i64 - origin).clamp(0, size as i64) as usize;
            let end = (hi.ceil() as i64 - origin).clamp(0, size as i64) as usize;
            (first, end)
        };
        let [bx0, by0, bx1, by1] = d.bounds();
        let (c0, c1) = span(bx0, bx1, ox, self.width);
        let (r0, r1) = span(by0, by1, oy, self.height);
        if c0 >= c1 || r0 >= r1 {
            return;
        }
        let shape = Shape::new(d);
        let m = &d.motion;
        let inv_edge = 1.0 / EDGE;
        let w = self.width;
        for row in r0..r1 {
            // Pixel centres in document coordinates, relative to the dab's centre.
            let py = (oy + row as i64) as f32 + 0.5 - d.y;
            // Only the columns the shape can reach on this row: within `r` of the stretches of
            // the path that come within `r` of the row (or inside the disc).
            let (lo, hi) = if PATH {
                let (mut lo, mut hi) = (f32::MAX, f32::MIN);
                for p in m.points[..m.count].windows(2) {
                    let ([ax, ay], [bx, by]) = (p[0], p[1]);
                    let dy = by - ay;
                    let (s0, s1) = if dy != 0.0 {
                        let (a, b) = ((py - r - ay) / dy, (py + r - ay) / dy);
                        (a.min(b).max(0.0), a.max(b).min(1.0))
                    } else if (py - ay).abs() < r {
                        (0.0, 1.0)
                    } else {
                        (1.0, 0.0)
                    };
                    if s0 <= s1 {
                        let (xa, xb) = (ax + s0 * (bx - ax), ax + s1 * (bx - ax));
                        (lo, hi) = (lo.min(xa.min(xb) - r), hi.max(xa.max(xb) + r));
                    }
                }
                (lo, hi)
            } else {
                let rest = r * r - py * py;
                if rest <= 0.0 {
                    continue;
                }
                let half = rest.sqrt();
                (-half, half)
            };
            if lo > hi {
                continue;
            }
            let (first, end) = span(d.x + lo, d.x + hi, ox, w);
            let (first, end) = (first.max(c0), end.min(c1));
            let line = row * w;
            let px = |i: usize| (ox + (first + i) as i64) as f32 + 0.5 - d.x;
            let row = shape.row(py);
            let Some(clear) = self.clear.get_mut(line + first..line + end) else { continue };
            if SCENE {
                let Some(scene) = self.scene.as_deref().and_then(|s| s.get(line + first..line + end)) else { continue };
                for (i, (c, &behind)) in clear.iter_mut().zip(scene).enumerate() {
                    *c *= 1.0 - shape.cover::<PATH>(px(i), py, row) * ((behind - d.depth) * inv_edge).clamp(0.0, 1.0);
                }
            } else {
                for (i, c) in clear.iter_mut().enumerate() {
                    *c *= 1.0 - shape.cover::<PATH>(px(i), py, row);
                }
            }
        }
    }
}

/// A dab's shape, ready for the pixel loop.
#[derive(Clone, Copy)]
struct Shape {
    /// The path's pieces: where each starts, its direction and length, and one over its length
    /// squared (0 for a piece of no length).
    starts: [[f32; 2]; PATH_POINTS - 1],
    pieces: [[f32; 2]; PATH_POINTS - 1],
    inv_len2: [f32; PATH_POINTS - 1],
    count: usize,
    flicker: f32,
    /// Waves of flicker per piece.
    cycles: f32,
    phase: f32,
    /// The oval's terms (`a`, `2b`, `c`), divided by the radius squared.
    oval: [f32; 3],
    inv_r2: f32,
    core2: f32,
    inv_fade: f32,
    /// The core's brightness in its middle, and how much it rises per unit of distance squared.
    ring_base: f32,
    ring_rise: f32,
    alpha: f32,
}

impl Shape {
    fn new(d: &Dab) -> Self {
        let m = &d.motion;
        let count = m.count.saturating_sub(1);
        let mut shape = Shape {
            starts: [[0.0; 2]; PATH_POINTS - 1],
            pieces: [[0.0; 2]; PATH_POINTS - 1],
            inv_len2: [0.0; PATH_POINTS - 1],
            count,
            flicker: m.flicker,
            cycles: if count > 0 { m.cycles / count as f32 } else { 0.0 },
            phase: m.phase,
            oval: [0.0; 3],
            inv_r2: 1.0 / (d.radius * d.radius),
            core2: d.core * d.core,
            inv_fade: 1.0 / (1.0 - d.core * d.core),
            ring_base: 1.0 - d.ring,
            ring_rise: if d.core > 0.0 { d.ring / (d.core * d.core) } else { 0.0 },
            alpha: d.alpha,
        };
        let [a, b, c] = d.oval;
        shape.oval = [a * shape.inv_r2, 2.0 * b * shape.inv_r2, c * shape.inv_r2];
        for k in 0..count {
            let ([ax, ay], [bx, by]) = (m.points[k], m.points[k + 1]);
            let (vx, vy) = (bx - ax, by - ay);
            let len2 = vx * vx + vy * vy;
            shape.starts[k] = [ax, ay];
            shape.pieces[k] = [vx, vy];
            shape.inv_len2[k] = if len2 > 0.0 { 1.0 / len2 } else { 0.0 };
        }
        shape
    }

    /// The parts of a still dab's oval that stay the same along a row of pixels `py` from its
    /// centre, so the pixel loop doesn't work them out again for every pixel.
    #[inline(always)]
    fn row(&self, py: f32) -> [f32; 2] {
        [self.oval[1] * py, self.oval[2] * py * py]
    }

    /// How opaque the dab is at (`px`, `py`) pixels from its centre (`row` from [`row`](Self::row)):
    /// full strength inside the core (a little less in the middle when out of focus), then a
    /// smooth fall to 0 at the rim, dipping where a moving dab flickers.
    #[inline(always)]
    fn cover<const PATH: bool>(&self, px: f32, py: f32, row: [f32; 2]) -> f32 {
        let (rho2, flicker) = if PATH {
            // From the nearest point of the path, and how far along the path that is.
            let (mut best, mut along) = (f32::MAX, 0.0);
            for k in 0..self.count {
                let ([ax, ay], [vx, vy]) = (self.starts[k], self.pieces[k]);
                let (rx, ry) = (px - ax, py - ay);
                let s = ((rx * vx + ry * vy) * self.inv_len2[k]).clamp(0.0, 1.0);
                let (qx, qy) = (rx - s * vx, ry - s * vy);
                let d2 = qx * qx + qy * qy;
                if d2 < best {
                    best = d2;
                    along = k as f32 + s;
                }
            }
            // A smooth wave: 0 at whole numbers, 1 half way, with no sharp turns.
            let wave = along * self.cycles + self.phase;
            let t = 1.0 - (2.0 * (wave - wave.floor()) - 1.0).abs();
            (best * self.inv_r2, 1.0 - self.flicker * t * t * (3.0 - 2.0 * t))
        } else {
            ((self.oval[0] * px + row[0]) * px + row[1], 1.0)
        };
        if rho2 >= 1.0 {
            return 0.0;
        }
        let s = ((rho2 - self.core2) * self.inv_fade).max(0.0);
        let inner = self.ring_base + self.ring_rise * rho2.min(self.core2);
        self.alpha * flicker * inner * (1.0 - s * s * (3.0 - 2.0 * s))
    }
}
