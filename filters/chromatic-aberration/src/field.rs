//! The displacement field: how far, and which way, each pixel's colors get pushed.

use moxocraft_sdk::{Error, Geometry, try_vec};

use crate::settings::{Mode, Settings};

/// Entries in Lens mode's table of displacement against distance from the centre.
const LENS_TABLE: usize = 4096;

/// Where each pixel's colors get pushed.
pub(crate) struct Field {
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
    /// The field for these settings on this canvas, with every displacement capped to
    /// `max_shift` pixels.
    pub fn new(s: &Settings, g: &Geometry, max_shift: f32) -> Result<Field, Error> {
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
    pub fn longest(&self) -> f32 {
        match self.mode {
            Mode::Directional => (self.dir[0] * self.dir[0] + self.dir[1] * self.dir[1]).sqrt(),
            Mode::Lens => self.lens.iter().fold(0.0, |a, m| a.max(m.abs())),
        }
    }

    /// The displacement when it is the same everywhere (Directional mode).
    pub fn uniform(&self) -> Option<[f32; 2]> {
        (self.mode == Mode::Directional).then_some(self.dir)
    }

    /// The displacement in pixels at document position `(x, y)`, or `None` where there is none.
    #[inline(always)]
    pub fn at(&self, x: f32, y: f32) -> Option<[f32; 2]> {
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
