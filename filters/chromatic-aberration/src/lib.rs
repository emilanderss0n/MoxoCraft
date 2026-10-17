//! Chromatic Aberration: a PhotoCraft filter plug-in.
//!
//! Real lenses bend red, green and blue light by slightly different amounts, so the three colours
//! land a little apart and bright edges pick up coloured fringes. This filter recreates that.
//!
//! How it works, per output pixel:
//! 1. Work out a displacement: in Lens mode it points away from the centre and grows towards the
//!    corners; in Directional mode it's the same everywhere.
//! 2. Read the picture at several points ("taps") along that displacement. Each tap adds to red,
//!    green and blue with its own weights, so each colour ends up shifted by a different amount.
//!    With Softness at 0 a colour comes from a single tap (a hard split); with more softness, or
//!    in Spectral mode, each colour is spread along the displacement and the fringes become smooth
//!    rainbow gradients.
//! 3. Mix in linear light (8- and 16-bit pixels are roughly gamma 2, so we square them first and
//!    take the square root at the end). Fringes on bright highlights then glow the way real ones
//!    do instead of turning muddy.
//!
//! Smeared colours are done in two passes so they stay smooth without costing a tap every pixel
//! or two: first every pixel gets a short weighted average along its own displacement direction
//! (a "segment", reaching one tap gap either side and fading out towards its ends), then the main
//! taps read those segments instead of single pixels. Neighbouring segments overlap and add up
//! evenly, so together they cover the whole smear with no holes or steps.
//! That matters because PhotoCraft runs plug-ins in an interpreter (roughly 100× slower than
//! native code) and stops a run after 60 seconds.
//!
//! Transparent layers work too: colours are mixed premultiplied by alpha, and fringes may spill
//! out past the layer's edge into transparent pixels.

use moxocraft_sdk::{Error, Filter, Geometry, Image, Params, export_filter, try_vec};

/// The manifest PhotoCraft reads at install time. Its settings become the dialog, in this order.
///
/// `overlap` is how many pixels of surrounding picture PhotoCraft hands us around each band; it's
/// also the furthest any colour can move, so displacements are capped to it.
pub const MANIFEST: &str = r#"{
  "id": "moxocraft.chromatic-aberration",
  "name": "Chromatic Aberration",
  "version": "0.1.0",
  "kind": "filter",
  "author": "MoxoCraft",
  "description": "Splits red, green and blue apart like a real lens. Lens pushes colours outward from a centre point, strongest at the corners; Directional shifts them one way. Amount is the shift in pixels (at the corners in Lens mode); negative values swap the fringe colours. Softness and Spectral blend the colours into smooth rainbow fringes. Very large amounts are capped at what PhotoCraft lets a plug-in read around each pixel: about 65 to 125 px, less with more softness.",
  "params": {
    "mode": {"type": "choice", "options": ["Lens", "Directional"], "default": "Lens"},
    "fringe": {"type": "choice", "options": ["Spectral", "RGB Split", "Red/Cyan", "Green/Magenta", "Blue/Yellow"], "default": "Spectral"},
    "amount": {"type": "number", "min": -128, "max": 128, "default": 12},
    "softness": {"type": "number", "min": 0, "max": 100, "default": 30},
    "clearCenter": {"type": "number", "min": 0, "max": 100, "default": 25},
    "falloff": {"type": "number", "min": 0.25, "max": 4, "default": 1.5},
    "centerX": {"type": "number", "min": 0, "max": 100, "default": 50},
    "centerY": {"type": "number", "min": 0, "max": 100, "default": 50},
    "angle": {"type": "number", "min": -180, "max": 180, "default": 0}
  },
  "overlap": 128,
  "area": "content"
}"#;

/// Fewest and most main taps a pixel reads when colours are smeared.
pub const MIN_TAPS: usize = 5;
pub const MAX_TAPS: usize = 16;

/// Segments are sampled about this many pixels apart: close enough that even a one-pixel line
/// is picked up evenly wherever it falls between samples.
const SEGMENT_SPACING: f32 = 1.0;
/// Most samples in one segment.
const MAX_SEGMENT_TAPS: usize = 32;

/// How much interpreter work a whole image may take, in PhotoCraft's instruction units. At the
/// ~500 million a second one thread manages, this is 30 seconds on 4 threads: well inside the
/// 60-second limit. Big images with long smears get fewer samples to stay inside it.
const WORK_BUDGET: f64 = 60e9;
/// Rough cost of a pixel without taps, and of one tap (measured with `moxocraft-bench`).
const COST_PER_PIXEL: f64 = 600.0;
const COST_PER_TAP: f64 = 150.0;

pub struct ChromaticAberration;

export_filter!(ChromaticAberration);

impl Filter for ChromaticAberration {
    const MANIFEST: &'static str = MANIFEST;

    fn apply(image: &mut Image<'_>, params: &Params) -> Result<(), Error> {
        render(image, &Settings::from_params(params))
    }
}

/// Which way the colours are pushed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Away from a centre point, growing towards the corners, like a real lens.
    Lens,
    /// The same direction and distance everywhere.
    Directional,
}

/// Which colours separate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fringe {
    /// A smooth rainbow: red, yellow, green, cyan, blue.
    Spectral,
    /// Red one way, blue the other, green stays put.
    RgbSplit,
    /// Red moves, green and blue (cyan) stay.
    RedCyan,
    /// Red and blue (magenta) move, green stays.
    GreenMagenta,
    /// Blue moves, red and green (yellow) stay.
    BlueYellow,
}

impl Fringe {
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "Spectral" => Self::Spectral,
            "RGB Split" => Self::RgbSplit,
            "Red/Cyan" => Self::RedCyan,
            "Green/Magenta" => Self::GreenMagenta,
            "Blue/Yellow" => Self::BlueYellow,
            _ => return None,
        })
    }

    /// Where red, green and blue are centred, as a fraction of Amount, and how far each colour is
    /// smeared either side of its centre at zero softness.
    fn centres(self) -> ([f32; 3], f32) {
        match self {
            // Overlapping smears are what turn three colours into a continuous rainbow.
            Self::Spectral => ([0.5, 0.0, -0.5], 0.5),
            Self::RgbSplit => ([1.0, 0.0, -1.0], 0.0),
            Self::RedCyan => ([1.0, 0.0, 0.0], 0.0),
            Self::GreenMagenta => ([1.0, 0.0, 1.0], 0.0),
            Self::BlueYellow => ([0.0, 0.0, 1.0], 0.0),
        }
    }
}

/// The dialog's settings, converted to the units the renderer uses.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    pub mode: Mode,
    pub fringe: Fringe,
    /// Pixels the outermost colour moves (at the corners in Lens mode). Negative swaps sides.
    pub amount: f32,
    /// 0–1.
    pub softness: f32,
    /// Fraction of the centre-to-corner distance left untouched in Lens mode, 0–1.
    pub clear_center: f32,
    /// How quickly the effect builds from the clear centre to the corners: 1 is even, higher
    /// keeps it near the corners.
    pub falloff: f32,
    /// Lens centre as a fraction of the canvas width and height.
    pub center: [f32; 2],
    /// Directional mode's direction in degrees, counter-clockwise from pointing right.
    pub angle: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            mode: Mode::Lens,
            fringe: Fringe::Spectral,
            amount: 12.0,
            softness: 0.3,
            clear_center: 0.25,
            falloff: 1.5,
            center: [0.5, 0.5],
            angle: 0.0,
        }
    }
}

impl Settings {
    /// Reads the settings, clamping again in case they didn't come through PhotoCraft's checks.
    pub fn from_params(p: &Params) -> Self {
        let d = Settings::default();
        let num = |key: &str, default: f32, lo: f32, hi: f32| (p.number(key, f64::from(default)) as f32).clamp(lo, hi);
        Settings {
            mode: if p.text("mode") == Some("Directional") { Mode::Directional } else { Mode::Lens },
            fringe: p.text("fringe").and_then(Fringe::from_name).unwrap_or(d.fringe),
            amount: num("amount", d.amount, -128.0, 128.0),
            softness: num("softness", d.softness * 100.0, 0.0, 100.0) / 100.0,
            clear_center: num("clearCenter", d.clear_center * 100.0, 0.0, 100.0) / 100.0,
            falloff: num("falloff", d.falloff, 0.25, 4.0),
            center: [num("centerX", 50.0, 0.0, 100.0) / 100.0, num("centerY", 50.0, 0.0, 100.0) / 100.0],
            angle: num("angle", d.angle, -180.0, 180.0),
        }
    }
}

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
struct Plan {
    taps: Vec<(f32, [f32; 3])>,
    /// True when taps read segments (smeared colours), false for a hard split.
    segmented: bool,
    /// Segment half-length as a fraction of Amount: the gap between neighbouring taps.
    segment: f32,
    /// Most samples per segment.
    segment_taps: usize,
    /// The furthest a tap reads, as a fraction of Amount.
    reach: f32,
}

impl Plan {
    /// The furthest any read can go (a tap plus the segment around it), as a fraction of Amount,
    /// whichever tap count the plan ends up with. Needed before the plan, to cap the
    /// displacement.
    fn worst_reach(fringe: Fringe, softness: f32) -> f32 {
        (MIN_TAPS..=MAX_TAPS).map(|n| Taps::new(fringe, softness, n)).fold(0.0, |a, t| a.max(t.reach() + t.gap))
    }

    /// The furthest a tap alone can read, whichever tap count the plan ends up with.
    fn worst_tap_reach(fringe: Fringe, softness: f32) -> f32 {
        (MIN_TAPS..=MAX_TAPS).map(|n| Taps::new(fringe, softness, n)).fold(0.0, |a, t| a.max(t.reach()))
    }

    /// Picks the tap count and segment samples for displacements up to `longest` pixels, within
    /// the image's work budget.
    fn new(fringe: Fringe, softness: f32, longest: f32, g: &Geometry, tap_margin: usize) -> Plan {
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

/// Entries in Lens mode's table of displacement against distance from the centre.
const LENS_TABLE: usize = 4096;

/// Where each pixel's colours get pushed.
struct Field {
    mode: Mode,
    /// Directional mode's displacement, already capped.
    dir: [f32; 2],
    centre: [f32; 2],
    /// Lens mode: signed displacement in pixels at evenly spaced distances from the centre, so the
    /// pixel loop looks it up instead of calling `powf` (slow in an interpreter) for every pixel.
    lens: Vec<f32>,
    /// Table entries per pixel of distance.
    lens_scale: f32,
}

impl Field {
    fn new(s: &Settings, g: &Geometry, max_shift: f32) -> Result<Field, Error> {
        let (cw, ch) = (g.canvas_width.max(1) as f32, g.canvas_height.max(1) as f32);
        let a = s.angle.to_radians();
        let capped = s.amount.clamp(-max_shift, max_shift);
        let centre = [s.center[0] * cw, s.center[1] * ch];
        let mut lens = Vec::new();
        let mut lens_scale = 0.0;
        if s.mode == Mode::Lens {
            // Amount is reached at the corners of a centred lens: half the canvas diagonal away.
            let reference = 0.5 * (cw * cw + ch * ch).sqrt();
            let inner = s.clear_center.min(0.99);
            // Far enough for every canvas corner from wherever the centre is; pixels beyond
            // (layer content off the canvas) use the last entry.
            let far = (centre[0].max(cw - centre[0]).powi(2) + centre[1].max(ch - centre[1]).powi(2)).sqrt() + 2.0;
            lens_scale = (LENS_TABLE - 2) as f32 / far;
            lens = try_vec(LENS_TABLE, 0.0f32)?;
            for (i, m) in lens.iter_mut().enumerate() {
                let rn = i as f32 / lens_scale / reference;
                if rn > inner {
                    let u = (rn - inner) / (1.0 - inner);
                    *m = (s.amount * u.powf(s.falloff)).clamp(-max_shift, max_shift);
                }
            }
        }
        Ok(Field {
            mode: s.mode,
            // Image rows run downwards, so a counter-clockwise angle means negative y.
            dir: [a.cos() * capped, -a.sin() * capped],
            centre,
            lens,
            lens_scale,
        })
    }

    /// The longest displacement anywhere, in pixels.
    fn longest(&self) -> f32 {
        match self.mode {
            Mode::Directional => (self.dir[0] * self.dir[0] + self.dir[1] * self.dir[1]).sqrt(),
            Mode::Lens => self.lens.iter().fold(0.0, |a, m| a.max(m.abs())),
        }
    }

    /// The displacement in pixels at document position `(x, y)`, or `None` where there is none.
    #[inline(always)]
    fn at(&self, x: f32, y: f32) -> Option<[f32; 2]> {
        if self.mode == Mode::Directional {
            return Some(self.dir);
        }
        let (dx, dy) = (x - self.centre[0], y - self.centre[1]);
        let r = (dx * dx + dy * dy).sqrt();
        let pos = r * self.lens_scale;
        let i = (pos as usize).min(LENS_TABLE - 2);
        let (Some(&m0), Some(&m1)) = (self.lens.get(i), self.lens.get(i + 1)) else { return None };
        let m = m0 + (m1 - m0) * (pos - i as f32).min(1.0);
        if m == 0.0 || r < 1e-6 {
            return None;
        }
        let k = m / r;
        Some([dx * k, dy * k])
    }
}

/// A block of pixels copied out of the band: `width` pixels per row, its top-left at
/// (`corner`, `corner`) in band coordinates.
struct Plane<'a> {
    data: &'a [f32],
    width: usize,
    corner: usize,
}

/// Filters one band.
pub fn render(image: &mut Image<'_>, s: &Settings) -> Result<(), Error> {
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
    let mut src = Vec::new();
    src.try_reserve_exact(sw * sh * n).map_err(|_| Error::OutOfMemory)?;
    for row in src_corner..src_corner + sh {
        let start = (row * w + src_corner) * n;
        let line = image.data.get(start..start + sw * n).ok_or(Error::BadInput)?;
        for p in line.chunks_exact(n) {
            let lin = if linear { [decode(p[0]), decode(p[1]), decode(p[2])] } else { [p[0], p[1], p[2]] };
            if alpha {
                let a = p[3];
                src.extend_from_slice(&[lin[0] * a, lin[1] * a, lin[2] * a, a]);
            } else {
                src.extend_from_slice(&lin);
            }
        }
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

/// Gamma 2 to linear, keeping the sign of out-of-range values.
#[inline(always)]
fn decode(v: f32) -> f32 {
    v * v.abs()
}

/// Linear back to gamma 2.
#[inline(always)]
fn encode(v: f32) -> f32 {
    if v >= 0.0 { v.sqrt() } else { -(-v).sqrt() }
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
                if let Some([vx, vy]) = self.field.at(x, y) {
                    let len = (vx * vx + vy * vy).sqrt();
                    let half = self.plan.segment * len;
                    let k = ((2.0 * half / SEGMENT_SPACING).ceil() as usize).clamp(1, self.plan.segment_taps);
                    if half >= 0.25 && k > 1 {
                        let step = 2.0 / k as f32;
                        // Samples further apart than a pixel and a quarter can catch thin lines
                        // unevenly. Sliding each pixel's samples by a different amount turns that
                        // into fine grain, which the main taps then average away.
                        let phase = if 2.0 * half / k as f32 > 1.25 { hash01(x, y) } else { 0.5 };
                        let (ux, uy) = (vx / len * half, vy / len * half);
                        let mut total = 0.0f32;
                        for i in 0..k {
                            let u = (i as f32 + phase) * step - 1.0;
                            let wt = 1.0 - u.abs();
                            if let Some(p) = bilinear::<N, M>(src.data, src.width, sx + u * ux, sy + u * uy) {
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

    /// Second pass: the output. Each pixel reads its taps from `plane`: the segments, or for a
    /// hard split the band itself.
    ///
    /// Written once per layout (`N` channels, last one alpha or not; `M` is always `2 * N`) so the
    /// compiler can unroll the channel loops and drop bounds checks; the interpreter rewards
    /// every instruction saved.
    fn run<const N: usize, const M: usize, const ALPHA: bool>(&self, plane: &Plane<'_>, dst: &mut [f32]) {
        let (w, h, ov) = (self.w, self.h, self.ov);
        let taps = self.plan.taps.as_slice();
        for row in ov..h - ov {
            let y = (self.origin[1] + row as i64) as f32 + 0.5;
            let fy = (row - plane.corner) as f32;
            for col in ov..w - ov {
                let x = (self.origin[0] + col as i64) as f32 + 0.5;
                let Some([vx, vy]) = self.field.at(x, y) else { continue };
                // Under a hundredth of a pixel: nothing visible would change.
                if (vx * vx + vy * vy).sqrt() * self.plan.reach < 0.01 {
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
