//! The dialog's settings: what each choice means, and reading them from PhotoCraft's JSON.

use moxocraft_sdk::Params;

/// Which way the colours are pushed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
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
    /// The fringe a dialog option names (the `fringe` options in the manifest).
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
    pub(crate) fn centres(self) -> ([f32; 3], f32) {
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
pub(crate) struct Settings {
    pub mode: Mode,
    pub fringe: Fringe,
    /// Pixels the outermost colour moves (at the corners in Lens mode). Negative swaps sides.
    pub amount: f32,
    /// 0–1.
    pub softness: f32,
    /// How much of the effect is mixed over the original picture, 0–1.
    pub strength: f32,
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
            strength: 1.0,
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
            strength: num("strength", d.strength * 100.0, 0.0, 100.0) / 100.0,
            clear_center: num("clearCenter", d.clear_center * 100.0, 0.0, 100.0) / 100.0,
            falloff: num("falloff", d.falloff, 0.25, 4.0),
            center: [num("centerX", 50.0, 0.0, 100.0) / 100.0, num("centerY", 50.0, 0.0, 100.0) / 100.0],
            angle: num("angle", d.angle, -180.0, 180.0),
        }
    }
}
