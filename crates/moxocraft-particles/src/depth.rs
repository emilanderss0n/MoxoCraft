//! The Depth list every particle plug-in's dialog has, and reading a depth map.
//!
//! A plug-in can't tell what's near or far in a photo, so the user tells it, one of two ways:
//! - Near Only / Far Only split the particles at Subject Depth, for two layers made with the same
//!   Variation. Masking the far layer with PhotoCraft's Select › Subject puts those particles
//!   behind the subject.
//! - Depth Map: the plug-in runs on a layer holding a depth map of the photo and replaces it with
//!   the particles, each hidden wherever the scene is nearer than it. Plug-ins can only read the
//!   layer they run on, so the map has to be that layer.

use moxocraft_sdk::{Error, Image, try_vec};

/// How far in front of the scene (in depth, 0–1) a particle has to be to show fully. Closer than
/// that it fades out, so particles don't end in hard edges where they meet the scene.
pub(crate) const EDGE: f32 = 0.04;

/// Which particles to draw, and against what.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Depth {
    /// All of them, over the layer.
    All,
    /// Only those nearer than Subject Depth, over the layer.
    NearOnly,
    /// Only those at Subject Depth or further, over the layer.
    FarOnly,
    /// The layer is a depth map (white near, or black near when `inverted`); replace it with the
    /// particles, hidden where the scene is nearer.
    Map { inverted: bool },
}

impl Depth {
    /// The dialog's names for the choices, in the order the manifests list them.
    pub const NAMES: [&str; 5] = ["None", "Near Only", "Far Only", "Depth Map", "Depth Map (Inverted)"];

    /// The choice a dialog option names (see [`NAMES`](Self::NAMES)).
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "None" => Self::All,
            "Near Only" => Self::NearOnly,
            "Far Only" => Self::FarOnly,
            "Depth Map" => Self::Map { inverted: false },
            "Depth Map (Inverted)" => Self::Map { inverted: true },
            _ => return None,
        })
    }

    /// Whether a particle at `depth` is drawn at all, with the subject at `subject`.
    #[inline]
    pub fn keeps(self, depth: f32, subject: f32) -> bool {
        match self {
            Self::NearOnly => depth < subject,
            Self::FarOnly => depth >= subject,
            Self::All | Self::Map { .. } => true,
        }
    }
}

/// Reads the band as a depth map: for each pixel, the depth of the scene there, on the particles'
/// scale (0 at the lens, 1 the farthest particles). The brightest white is right at the lens and
/// black is just beyond the farthest particles, so a black map hides nothing; `inverted` swaps
/// them. Transparent pixels count as far away.
pub(crate) fn read_map(image: &Image<'_>, inverted: bool) -> Result<Vec<f32>, Error> {
    let n = image.channels;
    let alpha = image.format.alpha;
    let linear = image.format.depth == 32;
    let mut out = try_vec(image.width * image.height, 0.0f32)?;
    for (d, p) in out.iter_mut().zip(image.data.chunks_exact(n)) {
        let mut v = 0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2];
        // 32-bit documents hold linear light; the map's grays are meant as they look on screen
        // (roughly gamma 2, as Chromatic Aberration assumes).
        if linear {
            v = v.max(0.0).sqrt();
        }
        let v = v.clamp(0.0, 1.0);
        let far = if inverted { v } else { 1.0 - v };
        let depth = far * (1.0 + EDGE);
        *d = if alpha { (1.0 + EDGE) + (depth - (1.0 + EDGE)) * p[n - 1].clamp(0.0, 1.0) } else { depth };
    }
    Ok(out)
}
