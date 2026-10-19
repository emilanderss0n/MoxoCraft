//! The dialog's settings: what each one means, and reading them from PhotoCraft's JSON.

use moxocraft_particles::Depth;
use moxocraft_sdk::Params;

/// The radius of an in-focus flake at Flake Size 50, in units of the picture's longer side (about
/// 5 px on a 6000 px photo). Every 25 of Flake Size doubles it.
const FLAKE_RADIUS: f32 = 0.0008;
/// How blurred flakes far behind the subject get at Defocus 100, in units of the longer side.
const MAX_APERTURE: f32 = 0.006;
/// How long flakes at the subject's distance streak at Motion 100, in units of the longer side.
const MAX_STREAK: f32 = 0.03;

/// The flakes' colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tint {
    White,
    /// A cold blue-white, as in shade or at dusk.
    Cool,
    /// Lit by warm light, such as streetlights or a low sun.
    Warm,
}

impl Tint {
    /// The tint a dialog option names (the `tint` options in the manifest).
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "White" => Self::White,
            "Cool" => Self::Cool,
            "Warm" => Self::Warm,
            _ => return None,
        })
    }

    /// Gamma-encoded RGB, 0–1.
    pub(crate) fn colour(self) -> [f32; 3] {
        match self {
            Self::White => [1.0, 1.0, 1.0],
            Self::Cool => [0.86, 0.93, 1.0],
            Self::Warm => [1.0, 0.93, 0.82],
        }
    }
}

/// The dialog's settings, converted to the units the drawing uses.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Settings {
    /// How much snow, 0–1.
    pub amount: f32,
    /// An average in-focus flake's radius, in units of the longer side.
    pub flake_radius: f32,
    /// Lens blur far behind the subject, in units of the longer side.
    pub aperture: f32,
    /// How far flakes at the subject's distance streak, in units of the longer side.
    pub streak: f32,
    /// Which way flakes fall, in radians from straight down; positive leans right.
    pub angle: f32,
    /// How much falling flakes wander from that direction and curve, 0–1.
    pub swirl: f32,
    pub tint: Tint,
    /// 0–1.
    pub opacity: f32,
    pub depth: Depth,
    /// Where the subject is, 0 at the lens to 1 at the farthest flakes: flakes there are
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
        let motion = percent("motion", 0.0);
        Settings {
            amount: percent("amount", 40.0),
            flake_radius: FLAKE_RADIUS * ((num("flakeSize", 50.0, 0.0, 100.0) - 50.0) / 25.0).exp2(),
            aperture: MAX_APERTURE * percent("defocus", 30.0),
            // Squared, so the low end, where real snow streaks sit, gets more of the slider.
            streak: MAX_STREAK * motion * motion,
            angle: num("angle", 0.0, -90.0, 90.0).to_radians(),
            swirl: percent("swirl", 35.0),
            tint: p.text("tint").and_then(Tint::from_name).unwrap_or(Tint::White),
            opacity: percent("opacity", 100.0),
            depth: p.text("depth").and_then(Depth::from_name).unwrap_or(Depth::All),
            subject: percent("subjectDepth", 30.0),
            seed: num("variation", 1.0, 1.0, 9999.0).round() as u32,
        }
    }
}
