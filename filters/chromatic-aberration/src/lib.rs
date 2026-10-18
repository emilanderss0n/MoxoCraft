//! Chromatic Aberration: a PhotoCraft filter plug-in.
//!
//! Real lenses bend red, green and blue light by slightly different amounts, so the three colours
//! land a little apart and bright edges pick up coloured fringes. This filter recreates that.
//!
//! How it works, per output pixel:
//! 1. Work out a displacement: in Lens mode it points away from the centre and grows towards the
//!    corners; in Directional mode it's the same everywhere (`field.rs`).
//! 2. Read the picture at several points ("taps") along that displacement. Each tap adds to red,
//!    green and blue with its own weights, so each colour ends up shifted by a different amount.
//!    With Softness at 0 a colour comes from a single tap (a hard split); with more softness, or
//!    in Spectral mode, each colour is spread along the displacement and the fringes become smooth
//!    rainbow gradients (`taps.rs`).
//! 3. Mix in linear light (8- and 16-bit pixels are roughly gamma 2, so we square them first and
//!    take the square root at the end). Fringes on bright highlights then glow the way real ones
//!    do instead of turning muddy (`render.rs`).
//!
//! PhotoCraft runs plug-ins in an interpreter (roughly 100× slower than native code) and stops a
//! run after 60 seconds, so the tap count is budgeted per image and the pixel loops are written to
//! save every instruction they can. Check changes with `moxocraft-bench`.
//!
//! Transparent layers work too: colours are mixed premultiplied by alpha, and fringes may spill
//! out past the layer's edge into transparent pixels.
//!
//! Modules, in the order a band goes through them: `settings` reads the dialog, `field` and
//! `taps` plan the reads, `render` does the pixel work.

mod field;
mod render;
mod settings;
mod taps;

use moxocraft_sdk::{Error, Filter, Image, Params, export_filter};

pub use settings::Fringe;
pub use taps::{MAX_TAPS, Taps};

/// The manifest PhotoCraft reads at install time. Its settings become the dialog, in this order.
///
/// `overlap` is how many pixels of surrounding picture PhotoCraft hands us around each band; it's
/// also the furthest any colour can move, so displacements are capped to it. The version is the
/// workspace's, from the root `Cargo.toml`.
pub const MANIFEST: &str = concat!(
    r#"{
  "id": "moxocraft.chromatic-aberration",
  "name": "MoxoCraft - Chromatic Aberration",
  "version": ""#,
    env!("CARGO_PKG_VERSION"),
    r#"",
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
}"#
);

pub struct ChromaticAberration;

export_filter!(ChromaticAberration);

impl Filter for ChromaticAberration {
    const MANIFEST: &'static str = MANIFEST;

    fn apply(image: &mut Image<'_>, params: &Params) -> Result<(), Error> {
        render::render(image, &settings::Settings::from_params(params))
    }
}
