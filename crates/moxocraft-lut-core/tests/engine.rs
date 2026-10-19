//! Tests of the LUT engine itself, on made-up tables. Each LUT plug-in tests its own baked-in
//! LUTs with `pack_tests!`.

use moxocraft_lut_core::testing::{colours, identity};
use moxocraft_lut_core::{Lut, color, cube};
use moxocraft_sdk::{ColorMode, Format};

#[test]
fn identity_lut_changes_nothing() {
    for size in [2, 5, 17] {
        let bytes = identity(size);
        let lut = Lut::from_bytes(&bytes, size).unwrap();
        for c in colours(500) {
            let out = lut.lookup(c);
            for i in 0..3 {
                assert!((out[i] - c[i]).abs() < 1e-4, "size {size}: {c:?} -> {out:?}");
            }
        }
    }
}

#[test]
fn tables_of_the_wrong_size_are_refused() {
    assert!(Lut::from_bytes(&[0; 10], 2).is_none());
    assert!(Lut::from_bytes(&identity(3), 4).is_none());
    assert!(Lut::from_bytes(&identity(1), 1).is_none());
}

#[test]
fn davinci_round_trip_with_a_neutral_lut_changes_nothing() {
    // Into DaVinci Wide Gamut / Intermediate and back out must land where it started, or every
    // DaVinci-space LUT would shift colors on top of its own look.
    let size = 65;
    let mut values = Vec::new();
    for b in 0..size {
        for g in 0..size {
            for r in 0..size {
                values.extend([r, g, b].map(|v| v as f32 / (size - 1) as f32));
            }
        }
    }
    let neutral = cube::CubeData { size, values };
    for c in colours(500) {
        let out = cube::through_davinci(&neutral, c);
        for i in 0..3 {
            assert!((out[i] - c[i]).abs() < 3.0 / 255.0, "{c:?} -> {out:?}");
        }
    }
}

#[test]
fn davinci_intermediate_hits_the_reference_points() {
    // Blackmagic's published values: 18% grey at 0.336, linear 1 at about 0.514, black at 0, and
    // the toe and log parts meet without a jump.
    assert!((color::davinci_intermediate(0.18) - 0.336).abs() < 0.001);
    assert!((color::davinci_intermediate(1.0) - 0.5139).abs() < 0.001);
    assert_eq!(color::davinci_intermediate(0.0), 0.0);
    let t = 0.002_624_09;
    assert!((color::davinci_intermediate(t) - color::davinci_intermediate(t + 1e-6)).abs() < 1e-4);
    for v in [0.0, 0.01, 0.18, 1.0, 4.0] {
        assert!((color::davinci_intermediate_to_linear(color::davinci_intermediate(v)) - v).abs() < 1e-4 * (1.0 + v));
    }
}

#[test]
fn float_documents_are_graded_in_display_space() {
    // A 32-bit (linear) pixel must come out as the LUT's answer for its sRGB-encoded value,
    // converted back to linear.
    let bytes = identity(9);
    let lut = Lut::from_bytes(&bytes, 9).unwrap();
    let f32fmt = Format { depth: 32, alpha: false, mode: ColorMode::Rgb };
    let px = vec![0.18, 0.5, 0.9, 2.5, 0.0, 1.0];
    let mut data = px.clone();
    let mut image = moxocraft_sdk::Image {
        data: &mut data,
        width: 2,
        height: 1,
        channels: 3,
        format: f32fmt,
        geometry: moxocraft_sdk::Geometry { x: 0, y: 0, overlap: 0, canvas_width: 2, canvas_height: 1 },
    };
    moxocraft_lut_core::render(&mut image, &lut, 1.0).unwrap();
    for (a, b) in data.iter().zip(&px) {
        assert!((a - b).abs() < 2e-3, "identity LUT round trip: {b} -> {a}");
    }
}
