//! Snow: a PhotoCraft filter plug-in.
//!
//! Lays falling snow over the picture, with depth: flakes far behind the subject are small and
//! many, flakes near the lens few, large and out of focus. The particle engine
//! (`moxocraft-particles`) places and draws them; this crate decides what a flake looks like
//! (`flake.rs`) and reads the dialog (`settings.rs`).
//!
//! The manifest asks for the whole canvas (`"area": "canvas"`), so the plug-in also runs on an
//! empty layer and gives the snow on its own there, and for no overlap: every flake is worked out
//! from its place in the document, so a band never needs to see its neighbours.

mod flake;
mod settings;

use moxocraft_sdk::{Error, Filter, Image, Params, export_filter};

pub use settings::Tint;

/// The manifest PhotoCraft reads at install time. Its settings become the dialog, in this order.
/// The version is the workspace's, from the root `Cargo.toml`.
///
/// PhotoCraft shrinks settings named like pixel lengths (`size`, `blur`, `speed`…) on its reduced
/// live preview, so none of these use those names: every length here is already measured against
/// the picture's size.
pub const MANIFEST: &str = concat!(
    r#"{
  "id": "moxocraft.snow",
  "name": "MoxoCraft - Snow",
  "version": ""#,
    env!("CARGO_PKG_VERSION"),
    r#"",
  "kind": "filter",
  "author": "MoxoCraft",
  "description": "Falling snow with depth: far flakes small and sharp, near ones large and out of focus. Sizes follow the picture, so settings suit any photo size. Run it on an empty layer to keep the snow separate. Defocus blurs flakes away from Subject Depth (where your subject is: 0 at the lens, 100 far away). Motion streaks the flakes in the Angle direction (degrees from straight down); Swirl makes them drift and curve. Depth: Near Only and Far Only draw only the flakes in front of or behind Subject Depth; make one layer of each with the same Variation and mask the far one with Select > Subject to put snow behind your subject. Depth Map replaces a layer holding a depth map of the photo (white near; Inverted: black near) with snow hidden behind nearer parts of the scene. Variation picks another random layout.",
  "params": {
    "amount": {"type": "number", "min": 0, "max": 100, "default": 40},
    "flakeSize": {"type": "number", "min": 0, "max": 100, "default": 50},
    "defocus": {"type": "number", "min": 0, "max": 100, "default": 30},
    "motion": {"type": "number", "min": 0, "max": 100, "default": 0},
    "angle": {"type": "number", "min": -90, "max": 90, "default": 0},
    "swirl": {"type": "number", "min": 0, "max": 100, "default": 35},
    "tint": {"type": "choice", "options": ["White", "Cool", "Warm"], "default": "White"},
    "opacity": {"type": "number", "min": 0, "max": 100, "default": 100},
    "depth": {"type": "choice", "options": ["None", "Near Only", "Far Only", "Depth Map", "Depth Map (Inverted)"], "default": "None"},
    "subjectDepth": {"type": "number", "min": 0, "max": 100, "default": 30},
    "variation": {"type": "int", "min": 1, "max": 9999, "default": 1}
  },
  "overlap": 0,
  "area": "canvas"
}"#
);

pub struct Snow;

export_filter!(Snow);

impl Filter for Snow {
    const MANIFEST: &'static str = MANIFEST;

    fn apply(image: &mut Image<'_>, params: &Params) -> Result<(), Error> {
        flake::render(image, &settings::Settings::from_params(params))
    }
}
