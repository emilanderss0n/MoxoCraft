//! The particle engine behind MoxoCraft's overlay plug-ins (Snow, and the rain, dust and ember
//! plug-ins to come).
//!
//! Each plug-in decides what its particles look like; everything else lives here. A band goes
//! through it like this:
//!
//! 1. [`Scatter`] decides where every particle sits. Positions are measured in units of the
//!    picture's longer side, so the same settings give the same layout on a phone photo, a
//!    24-megapixel one and PhotoCraft's reduced live preview (which tells plug-ins the reduced
//!    size). Space is split into depth slices, from right at the lens (depth 0) to far behind the
//!    subject (depth 1); each slice is a grid of cells, and a cell's particles come from a hash of
//!    its coordinates and the Variation number. A band only visits the cells whose particles can
//!    reach it, and works out each particle exactly the way every other band does, so particles
//!    that cross band edges join without seams.
//! 2. [`Lens`] turns a particle's depth into how big it looks and how far out of focus it is.
//! 3. [`Coverage::dab`] draws each particle into a band-sized buffer as a soft disc (round or oval)
//!    or a soft streak along a [`Motion`] path, farthest first. [`Dab::new`] works out its shape
//!    from its size, blur and motion, keeping the amount of light it gives about the same however
//!    it's spread.
//! 4. [`render`] puts it together: it checks the band, reads the depth map when asked, lets the
//!    plug-in draw, then lays the particles over the band, or replaces the band with them.
//!
//! [`Depth`] is the dialog's Depth list, which every particle plug-in shares.

mod depth;
mod draw;
mod hash;
mod lens;
mod render;
mod scatter;

pub use depth::Depth;
pub use draw::{Coverage, Dab, Motion, PATH_POINTS};
pub use hash::Rng;
pub use lens::Lens;
pub use render::{Frame, render};
pub use scatter::{BUDGET, Population, STREAK_COST, Scatter, Spot};
