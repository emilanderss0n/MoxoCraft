//! Random-looking numbers that always come out the same for the same inputs, so every band agrees
//! on every particle.

/// Mixes a 32-bit number so that nearby inputs give unrelated outputs (Chris Wellons' "lowbias32").
#[inline(always)]
fn mix(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

/// A stream of random-looking numbers, the same every time for the same starting values.
#[derive(Clone, Copy, Debug)]
pub struct Rng(u32);

impl Rng {
    /// A stream started from several numbers (a seed, cell coordinates, an index…).
    pub fn from(parts: &[u32]) -> Self {
        Rng(parts.iter().fold(0x2545_f491, |h, &p| mix(h ^ p)))
    }

    pub fn next_u32(&mut self) -> u32 {
        // Steps of the golden ratio visit every 32-bit value before repeating.
        self.0 = self.0.wrapping_add(0x9e37_79b9);
        mix(self.0)
    }

    /// A number from 0 up to (not including) 1.
    pub fn unit(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 * (1.0 / 16_777_216.0)
    }
}
