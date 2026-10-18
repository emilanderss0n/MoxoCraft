//! Reading the settings JSON PhotoCraft passes with each band.

use crate::image::Geometry;
use crate::{Error, Json};

/// The settings the user picked, already checked and clamped by PhotoCraft against the manifest.
pub struct Params {
    root: Json,
}

impl Params {
    pub fn new(root: Json) -> Self {
        Params { root }
    }

    pub fn parse(text: &str) -> Result<Self, Error> {
        Json::parse(text).filter(|j| matches!(j, Json::Object(_))).map(Params::new).ok_or(Error::BadInput)
    }

    /// A number setting, or `default` when it's missing or not a number.
    pub fn number(&self, key: &str, default: f64) -> f64 {
        self.root.get(key).and_then(Json::as_f64).unwrap_or(default)
    }

    /// A choice or text setting.
    pub fn text(&self, key: &str) -> Option<&str> {
        self.root.get(key).and_then(Json::as_str)
    }

    /// A checkbox setting.
    pub fn flag(&self, key: &str, default: bool) -> bool {
        self.root.get(key).and_then(Json::as_bool).unwrap_or(default)
    }

    /// The band's geometry from `_image`, if PhotoCraft sent it.
    pub fn geometry(&self) -> Option<Geometry> {
        let g = self.root.get("_image")?;
        let int = |k: &str| g.get(k).and_then(Json::as_f64).filter(|v| v.is_finite());
        let size = |k: &str| int(k).map(|v| v.clamp(0.0, 1e9) as usize);
        Some(Geometry {
            x: int("x")? as i64,
            y: int("y")? as i64,
            overlap: size("overlap").unwrap_or(0),
            canvas_width: size("canvasWidth")?,
            canvas_height: size("canvasHeight")?,
        })
    }
}
