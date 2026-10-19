//! Floating Particles: a PhotoCraft filter plug-in.
//!
//! Lays dust, pollen or specks of light hanging in the air over the picture, with depth:
//! particles far behind the subject are small and many, particles near the lens few, large and
//! out of focus. The particle engine (`moxocraft-particles`) places and draws them; this crate
//! decides what a particle looks like (`particle.rs`) and reads the dialog (`settings.rs`).
//!
//! The manifest asks for the whole canvas (`"area": "canvas"`), so the plug-in also runs on an
//! empty layer and gives the particles on their own there, and for no overlap: every particle is
//! worked out from its place in the document, so a band never needs to see its neighbors.

mod particle;
mod settings;

use moxocraft_sdk::{Error, Filter, Image, Params, export_filter};

pub use settings::hsb;

/// The manifest PhotoCraft reads at install time. Its settings become the dialog, in this order.
/// The version is the workspace's, from the root `Cargo.toml`.
///
/// PhotoCraft shrinks settings named like pixel lengths (`size`, `blur`, `speed`…) on its reduced
/// live preview, so none of these use those names: every length here is already measured against
/// the picture's size. PhotoCraft's dialogs have no color picker, so the color is set with Hue,
/// Saturation and Brightness sliders.
pub const MANIFEST: &str = concat!(
    r#"{
  "id": "moxocraft.floating-particles",
  "name": "MoxoCraft - Floating Particles",
  "version": ""#,
    env!("CARGO_PKG_VERSION"),
    r#"",
  "kind": "filter",
  "author": "MoxoCraft",
  "description": "Dust or specks of light floating in the air, with depth: far particles small and sharp, near ones large and out of focus. Sizes follow the picture, so settings suit any photo size. Run it on an empty layer to keep the particles separate. Size Variety mixes small and large particles. Defocus blurs particles away from Subject Depth (where your subject is: 0 at the lens, 100 far away). Glow adds a soft halo around sharp particles. Clustering gathers them into loose clouds. Hue, Saturation and Brightness set their color (Saturation 0 for white). Depth: Near Only and Far Only draw only the particles in front of or behind Subject Depth; make one layer of each with the same Variation and mask the far one with Select > Subject to put particles behind your subject. Depth Map replaces a layer holding a depth map of the photo (white near; Inverted: black near) with particles hidden behind nearer parts of the scene. Variation picks another random layout.",
  "params": {
    "amount": {"type": "number", "min": 0, "max": 100, "default": 40},
    "particleSize": {"type": "number", "min": 0, "max": 100, "default": 50},
    "sizeVariety": {"type": "number", "min": 0, "max": 100, "default": 60},
    "defocus": {"type": "number", "min": 0, "max": 100, "default": 40},
    "glow": {"type": "number", "min": 0, "max": 100, "default": 30},
    "clustering": {"type": "number", "min": 0, "max": 100, "default": 30},
    "hue": {"type": "number", "min": 0, "max": 360, "default": 40},
    "saturation": {"type": "number", "min": 0, "max": 100, "default": 30},
    "brightness": {"type": "number", "min": 0, "max": 100, "default": 100},
    "opacity": {"type": "number", "min": 0, "max": 100, "default": 100},
    "depth": {"type": "choice", "options": ["None", "Near Only", "Far Only", "Depth Map", "Depth Map (Inverted)"], "default": "None"},
    "subjectDepth": {"type": "number", "min": 0, "max": 100, "default": 30},
    "variation": {"type": "int", "min": 1, "max": 9999, "default": 1}
  },
  "overlap": 0,
  "area": "canvas"
}"#
);

pub struct FloatingParticles;

export_filter!(FloatingParticles);

impl Filter for FloatingParticles {
    const MANIFEST: &'static str = MANIFEST;

    fn apply(image: &mut Image<'_>, params: &Params) -> Result<(), Error> {
        particle::render(image, &settings::Settings::from_params(params))
    }
}
