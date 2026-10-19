//! How a particle's depth changes the way it looks: things further away look smaller, and things
//! away from the subject's distance are out of focus, as with a real lens.

/// How many times further away the farthest particles are than the nearest ones.
pub const FAR: f32 = 16.0;

/// The camera: where it's focused and how wide open the lens is.
#[derive(Clone, Copy, Debug)]
pub struct Lens {
    /// The subject's distance, in units of the nearest particles' distance.
    subject: f32,
    /// The blur radius, in units of the picture's longer side, of something infinitely far
    /// behind the subject.
    aperture: f32,
}

impl Lens {
    /// A lens focused on `subject_depth` (0 at the lens, 1 the farthest particles) whose blur
    /// grows up to `aperture` (in units of the picture's longer side) far behind the subject.
    pub fn new(subject_depth: f32, aperture: f32) -> Self {
        Lens { subject: distance(subject_depth.clamp(0.0, 1.0)), aperture: aperture.max(0.0) }
    }

    /// How big something at `depth` looks, compared with the same thing at the subject's
    /// distance: above 1 in front of the subject, below 1 behind it.
    #[inline]
    pub fn scale(&self, depth: f32) -> f32 {
        self.subject / distance(depth)
    }

    /// The radius of the blur disc at `depth`, in units of the picture's longer side. It's 0 at
    /// the subject's distance, levels off toward `aperture` behind it and keeps growing in front
    /// of it, the way a real lens's blur does.
    #[inline]
    pub fn blur(&self, depth: f32) -> f32 {
        self.aperture * (1.0 - self.scale(depth)).abs()
    }
}

/// The distance of `depth`, in units of the nearest particles' distance. Evenly spaced, so each
/// slice of depth holds the same thickness of space.
#[inline]
fn distance(depth: f32) -> f32 {
    1.0 + (FAR - 1.0) * depth
}
