//! Tests of the engine's parts. The drawing itself is tested through the plug-ins (see Snow's
//! tests), which run it the way PhotoCraft does.

use moxocraft_particles::{BUDGET, Depth, Lens, Population, Scatter};

/// Every particle `scatter` visits for `rect`, as (x, y, depth) bit patterns, in visiting order.
fn visit(scatter: &Scatter, rect: [f32; 4]) -> Vec<[u32; 3]> {
    let mut seen = Vec::new();
    scatter.for_each(rect, |s| seen.push([s.x.to_bits(), s.y.to_bits(), s.depth.to_bits()]));
    seen
}

/// Particles like Snow's: sizes shrink with distance, blur grows away from the subject.
struct Snowlike {
    lens: Lens,
    radius: f32,
}

impl Population for Snowlike {
    fn full(&self) -> f32 {
        15_000.0
    }
    fn weight(&self, depth: f32) -> f32 {
        1.0 / self.lens.scale(depth)
    }
    fn reach(&self, depth: f32) -> f32 {
        self.radius * self.lens.scale(depth) + self.lens.blur(depth)
    }
    fn work(&self, depth: f32) -> f32 {
        core::f32::consts::PI * self.reach(depth).powi(2)
    }
}

fn snowlike(amount: f32) -> Scatter {
    Scatter::new(7, amount, &Snowlike { lens: Lens::new(0.3, 0.002), radius: 0.001 })
}

#[test]
fn every_depth_choice_has_a_name() {
    for name in Depth::NAMES {
        assert!(Depth::from_name(name).is_some(), "{name}");
    }
    let all: Vec<_> = Depth::NAMES.iter().filter_map(|n| Depth::from_name(n)).collect();
    assert!(all.iter().enumerate().all(|(i, a)| all[i + 1..].iter().all(|b| a != b)), "two names mean the same");
}

#[test]
fn the_lens_is_sharp_at_the_subject_and_blurs_away_from_it() {
    for subject in [0.0, 0.3, 0.7, 1.0] {
        let lens = Lens::new(subject, 0.01);
        assert!((lens.scale(subject) - 1.0).abs() < 1e-6 && lens.blur(subject) < 1e-7, "{subject}");
        // Nearer looks bigger, and blur grows the further from the subject, on both sides.
        let depths: Vec<f32> = (0..=40).map(|i| i as f32 / 40.0).collect();
        for pair in depths.windows(2) {
            assert!(lens.scale(pair[0]) > lens.scale(pair[1]));
            if pair[1] <= subject {
                assert!(lens.blur(pair[0]) >= lens.blur(pair[1]));
            } else if pair[0] >= subject {
                assert!(lens.blur(pair[0]) <= lens.blur(pair[1]));
            }
        }
        // Far behind the subject the blur levels off below the aperture; in front it keeps going.
        assert!(lens.blur(1.0) < 0.01);
    }
}

#[test]
fn every_band_sees_the_particles_it_shares_the_same_way() {
    let scatter = snowlike(0.7);
    let whole = visit(&scatter, [0.1, 0.1, 0.6, 0.4]);
    assert!(whole.len() > 500);
    // A band inside it meets a subset of the same particles, in the same relative order.
    let part = visit(&scatter, [0.25, 0.2, 0.35, 0.22]);
    assert!(!part.is_empty() && part.len() < whole.len());
    let mut rest = whole.iter();
    assert!(part.iter().all(|p| rest.any(|w| w == p)), "a band saw a particle differently or out of order");
}

#[test]
fn more_amount_adds_particles_without_moving_any() {
    let rect = [0.0, 0.0, 0.5, 0.5];
    let less = visit(&snowlike(0.3), rect);
    let more = visit(&snowlike(0.6), rect);
    assert!(more.len() > less.len() * 3 / 2);
    let mut rest = more.iter();
    assert!(less.iter().all(|p| rest.any(|m| m == p)), "a particle moved when Amount went up");
    assert!(visit(&snowlike(0.0), rect).is_empty());
}

#[test]
fn huge_particles_are_capped_to_the_budget() {
    let small = Scatter::new(7, 1.0, &Snowlike { lens: Lens::new(0.3, 0.002), radius: 0.001 });
    assert!(small.work() < BUDGET);
    let huge = |amount: f32| Scatter::new(7, amount, &Snowlike { lens: Lens::new(1.0, 0.02), radius: 0.01 });
    assert!((huge(1.0).work() - BUDGET).abs() < 1e-3);
    // Past the budget, more Amount changes nothing.
    let rect = [0.0, 0.0, 0.3, 0.3];
    assert_eq!(visit(&huge(0.8), rect), visit(&huge(1.0), rect));
}
