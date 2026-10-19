//! The dialog's settings: what each one means, and reading them from PhotoCraft's JSON.

use moxocraft_particles::Depth;
use moxocraft_sdk::Params;

/// The radius of an in-focus particle at Particle Size 50, in units of the picture's longer side
/// (about 4 px on a 6000 px photo). Every 25 of Particle Size doubles it.
const PARTICLE_RADIUS: f32 = 0.0006;
/// How blurred particles far behind the subject get at Defocus 100, in units of the longer side.
const MAX_APERTURE: f32 = 0.006;

/// The dialog's settings, converted to the units the drawing uses.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Settings {
    /// How many particles, 0–1.
    pub amount: f32,
    /// An average in-focus particle's radius, in units of the longer side.
    pub radius: f32,
    /// How much particle sizes differ, 0 (all the same) to 1.
    pub variety: f32,
    /// Lens blur far behind the subject, in units of the longer side.
    pub aperture: f32,
    /// How strong the halo around sharp particles is, 0–1.
    pub glow: f32,
    /// How much the particles gather into clouds, 0 (spread evenly) to 1.
    pub clustering: f32,
    /// Gamma-encoded RGB, 0–1.
    pub color: [f32; 3],
    /// 0–1.
    pub opacity: f32,
    pub depth: Depth,
    /// Where the subject is, 0 at the lens to 1 at the farthest particles: particles there are
    /// sharpest, and Near Only / Far Only split there.
    pub subject: f32,
    /// Picks the random layout.
    pub seed: u32,
}

impl Settings {
    /// Reads the settings, clamping again in case they didn't come through PhotoCraft's checks.
    pub fn from_params(p: &Params) -> Self {
        let num = |key: &str, default: f32, lo: f32, hi: f32| {
            let v = p.number(key, f64::from(default)) as f32;
            if v.is_finite() { v.clamp(lo, hi) } else { default }
        };
        let percent = |key: &str, default: f32| num(key, default, 0.0, 100.0) / 100.0;
        Settings {
            amount: percent("amount", 40.0),
            radius: PARTICLE_RADIUS * ((num("particleSize", 50.0, 0.0, 100.0) - 50.0) / 25.0).exp2(),
            variety: percent("sizeVariety", 60.0),
            aperture: MAX_APERTURE * percent("defocus", 40.0),
            glow: percent("glow", 30.0),
            clustering: percent("clustering", 30.0),
            color: hsb(num("hue", 40.0, 0.0, 360.0), percent("saturation", 30.0), percent("brightness", 100.0)),
            opacity: percent("opacity", 100.0),
            depth: p.text("depth").and_then(Depth::from_name).unwrap_or(Depth::All),
            subject: percent("subjectDepth", 30.0),
            seed: num("variation", 1.0, 1.0, 9999.0).round() as u32,
        }
    }
}

/// A color from its hue (degrees around the color wheel, 0 red, 120 green, 240 blue), saturation
/// and brightness (0–1), as the color pickers in photo editors set it: gamma-encoded RGB, 0–1.
pub fn hsb(hue: f32, saturation: f32, brightness: f32) -> [f32; 3] {
    let (s, v) = (saturation.clamp(0.0, 1.0), brightness.clamp(0.0, 1.0));
    let h = (hue / 60.0).rem_euclid(6.0);
    // The strongest channel is `v`, the weakest `v - chroma`, and the middle one moves between
    // them across each sixth of the wheel.
    let chroma = v * s;
    let middle = chroma * (1.0 - ((h % 2.0) - 1.0).abs());
    let low = v - chroma;
    let [hi, mid] = [v, low + middle];
    match h as u32 {
        0 => [hi, mid, low],
        1 => [mid, hi, low],
        2 => [low, hi, mid],
        3 => [low, mid, hi],
        4 => [mid, low, hi],
        _ => [hi, low, mid],
    }
}
