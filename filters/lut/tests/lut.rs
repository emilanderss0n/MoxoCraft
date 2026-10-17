//! Behaviour tests, run through `moxocraft_sdk::host`, which feeds bands exactly as PhotoCraft does.

use moxocraft_lut::{DATA, LUTS, Lut, LutFilter, MANIFEST, cube};
use moxocraft_sdk::host::{self, Bands};
use moxocraft_sdk::{ColorMode, Error, Format, Json};

fn run(pixels: &[f32], w: usize, h: usize, format: Format, params: &str) -> Result<Vec<f32>, Error> {
    host::apply::<LutFilter>(pixels, w, h, format, params, Bands::PHOTOCRAFT)
}

/// A LUT of `size` points per axis that maps every colour to itself.
fn identity(size: usize) -> Vec<u8> {
    let mut bytes = Vec::new();
    for b in 0..size {
        for g in 0..size {
            for r in 0..size {
                for v in [r, g, b] {
                    let q = cube::quantize(v as f32 / (size - 1) as f32);
                    bytes.extend_from_slice(&q.to_le_bytes());
                }
            }
        }
    }
    bytes
}

/// A smooth spread of colours covering the cube.
fn colours(n: usize) -> Vec<[f32; 3]> {
    (0..n).map(|i| {
        let t = i as f32 / n as f32;
        [(t * 7.3).fract(), (t * 3.1 + 0.2).fract(), (t * 11.7 + 0.5).fract()]
    })
    .collect()
}

fn image(w: usize, h: usize, channels: usize) -> Vec<f32> {
    let mut px = Vec::with_capacity(w * h * channels);
    for y in 0..h {
        for x in 0..w {
            px.extend_from_slice(&[x as f32 / w as f32, y as f32 / h as f32, ((x + y) % 7) as f32 / 6.0]);
            if channels == 4 {
                px.push(((x * 3 + y) % 5) as f32 / 4.0);
            }
        }
    }
    px
}

#[test]
fn manifest_lists_exactly_the_baked_in_luts() {
    let m = Json::parse(MANIFEST).expect("manifest is JSON");
    assert_eq!(m.get("id").and_then(Json::as_str), Some("moxocraft.lut"));
    assert!(m.get("name").and_then(Json::as_str).is_some_and(|n| n.chars().count() <= 64));
    let lut = m.get("params").and_then(|p| p.get("lut")).unwrap();
    let options: Vec<&str> = lut.get("options").and_then(Json::as_array).unwrap().iter().filter_map(Json::as_str).collect();
    assert_eq!(options, LUTS.iter().map(|l| l.name).collect::<Vec<_>>());
    assert!(options.iter().all(|o| !o.is_empty() && !o.contains(['|', '"'])));
    assert!(options.contains(&lut.get("default").and_then(Json::as_str).unwrap()));
    assert!(DATA.len() < 30 << 20, "fits PhotoCraft's 32 MiB module limit");
}

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
fn lookups_are_continuous_across_cell_edges() {
    // Neighbouring colours must give neighbouring results, whichever tetrahedron each lands in.
    for info in LUTS {
        let lut = Lut::new(info).unwrap();
        for c in colours(300) {
            let a = lut.lookup(c);
            let b = lut.lookup(c.map(|v| (v + 1e-4).min(1.0)));
            for i in 0..3 {
                assert!((a[i] - b[i]).abs() < 0.01, "{}: jump at {c:?}", info.name);
            }
        }
    }
}

#[test]
fn baked_values_match_the_cube_files() {
    // Re-read each .cube file stored as-is and compare grid points with what the plug-in looks
    // up there. (DaVinci-space LUTs are checked against the exact conversion instead.)
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../luts");
    for info in LUTS.iter().filter(|l| !l.davinci) {
        let path = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .find(|p| p.file_stem().is_some_and(|s| s.to_string_lossy().replace('_', " ") == info.name))
            .unwrap_or_else(|| panic!("no file for {}", info.name));
        let rows: Vec<[f32; 3]> = std::fs::read_to_string(&path)
            .unwrap()
            .lines()
            .filter_map(|l| {
                let v: Vec<f32> = l.split_whitespace().map_while(|p| p.parse().ok()).collect();
                <[f32; 3]>::try_from(v).ok()
            })
            .collect();
        let n = info.size;
        assert_eq!(rows.len(), n * n * n, "{}", info.name);
        let lut = Lut::new(info).unwrap();
        let step = (n / 7).max(1);
        for b in (0..n).step_by(step) {
            for g in (0..n).step_by(step) {
                for r in (0..n).step_by(step) {
                    let grid = [r, g, b].map(|v| v as f32 / (n - 1) as f32);
                    let got = lut.lookup(grid);
                    let want = rows[(b * n + g) * n + r];
                    for i in 0..3 {
                        assert!((got[i] - want[i]).abs() < 4e-5, "{} at {r},{g},{b}: {got:?} vs {want:?}", info.name);
                    }
                }
            }
        }
    }
}

#[test]
fn colour_sweeps_dont_collapse_into_flat_blocks() {
    // DaVinci-space film LUTs fed a normal image without conversion turned saturated colours into
    // flat blocks: about a fifth of a colour sweep came out as one unchanging colour. Converted
    // properly, no stretch is flat for more than a sliver (the Rec709 LUT itself has about 1%).
    let n = 1200;
    let mut px = Vec::new();
    for i in 0..n {
        let h = i as f32 / n as f32 * 6.0;
        let f = |k: f32| {
            let k = (k + h) % 6.0;
            0.85 - 0.85 * 0.8 * k.min(4.0 - k).clamp(0.0, 1.0)
        };
        px.extend_from_slice(&[f(5.0), f(3.0), f(1.0)]);
    }
    for info in LUTS {
        let out = run(&px, n, 1, Format::RGB8, &format!(r#"{{"lut":"{}"}}"#, info.name)).unwrap();
        let (mut run_len, mut longest) = (0usize, 0usize);
        for p in out.chunks_exact(3).collect::<Vec<_>>().windows(2) {
            let d = (0..3).fold(0.0f32, |m, c| m.max((p[1][c] - p[0][c]).abs()));
            run_len = if d < 0.0004 { run_len + 1 } else { 0 };
            longest = longest.max(run_len);
        }
        let share = longest as f32 / n as f32;
        assert!(share < 0.05, "{}: {:.0}% of the sweep is one flat colour", info.name, share * 100.0);
    }
}

#[test]
fn davinci_round_trip_with_a_neutral_lut_changes_nothing() {
    // Into DaVinci Wide Gamut / Intermediate and back out must land where it started, or every
    // DaVinci-space LUT would shift colours on top of its own look.
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
fn baked_davinci_luts_match_the_exact_conversion() {
    // The baked table must give what converting every pixel into DaVinci space and back would,
    // across the whole colour cube. A sliver of extreme cyans (red near zero, where the film LUT
    // compresses colour hardest) comes out a few steps off, because the table is interpolated
    // twice there; everything else lands within a step or two.
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../luts");
    let mut checked = 0;
    for info in LUTS.iter().filter(|l| l.davinci) {
        let path = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .find(|p| p.file_stem().is_some_and(|s| s.to_string_lossy().replace('_', " ") == info.name))
            .unwrap();
        let source = cube::parse_cube(&std::fs::read_to_string(path).unwrap()).unwrap();
        let baked = Lut::new(info).unwrap();
        let n = 24;
        let mut errors = Vec::new();
        for b in 0..=n {
            for g in 0..=n {
                for r in 0..=n {
                    let c = [r, g, b].map(|v| v as f32 / n as f32);
                    let (want, got) = (cube::through_davinci(&source, c), baked.lookup(c));
                    errors.push((0..3).fold(0.0f32, |m, i| m.max((want[i] - got[i]).abs())) * 255.0);
                }
            }
        }
        let share_over = |steps: f32| errors.iter().filter(|&&e| e > steps).count() as f32 / errors.len() as f32;
        let average = errors.iter().sum::<f32>() / errors.len() as f32;
        let worst = errors.iter().fold(0.0f32, |a, &b| a.max(b));
        assert!(average < 0.5, "{}: average error {average} / 255", info.name);
        assert!(share_over(2.0) < 0.01, "{}: {:.2}% of colours more than 2/255 off", info.name, share_over(2.0) * 100.0);
        assert!(share_over(4.0) < 0.005, "{}: {:.2}% of colours more than 4/255 off", info.name, share_over(4.0) * 100.0);
        assert!(worst < 20.0, "{}: worst error {worst} / 255", info.name);
        checked += 1;
    }
    assert!(checked > 0, "some LUTs are DaVinci-space");
}

#[test]
fn davinci_intermediate_hits_the_reference_points() {
    // Blackmagic's published values: 18% grey at 0.336, linear 1 at about 0.514, black at 0, and
    // the toe and log parts meet without a jump.
    assert!((cube::davinci_intermediate(0.18) - 0.336).abs() < 0.001);
    assert!((cube::davinci_intermediate(1.0) - 0.5139).abs() < 0.001);
    assert_eq!(cube::davinci_intermediate(0.0), 0.0);
    let t = 0.002_624_09;
    assert!((cube::davinci_intermediate(t) - cube::davinci_intermediate(t + 1e-6)).abs() < 1e-4);
    for v in [0.0, 0.01, 0.18, 1.0, 4.0] {
        assert!((cube::davinci_intermediate_to_linear(cube::davinci_intermediate(v)) - v).abs() < 1e-4 * (1.0 + v));
    }
    assert!(LUTS.iter().any(|l| l.davinci) && LUTS.iter().any(|l| !l.davinci));
}

#[test]
fn intensity_blends_between_original_and_graded() {
    let (w, h) = (64, 48);
    let px = image(w, h, 3);
    let lut = LUTS[0].name;
    let none = run(&px, w, h, Format::RGB8, &format!(r#"{{"lut":"{lut}","intensity":0}}"#)).unwrap();
    assert_eq!(none, px);
    let full = run(&px, w, h, Format::RGB8, &format!(r#"{{"lut":"{lut}","intensity":100}}"#)).unwrap();
    let half = run(&px, w, h, Format::RGB8, &format!(r#"{{"lut":"{lut}","intensity":50}}"#)).unwrap();
    assert!(full.iter().zip(&px).any(|(a, b)| (a - b).abs() > 0.02), "the LUT changes the image");
    for i in 0..px.len() {
        assert!((half[i] - (px[i] + full[i]) / 2.0).abs() < 1e-5);
    }
}

#[test]
fn every_lut_runs_and_alpha_is_untouched() {
    let (w, h) = (40, 30);
    let px = image(w, h, 4);
    for info in LUTS {
        let out = run(&px, w, h, Format::RGBA8, &format!(r#"{{"lut":"{}"}}"#, info.name)).unwrap();
        for (o, p) in out.chunks_exact(4).zip(px.chunks_exact(4)) {
            assert_eq!(o[3], p[3], "{}", info.name);
            assert!(o.iter().all(|v| (0.0..=1.0).contains(v)));
        }
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
    moxocraft_lut::render(&mut image, &lut, 1.0).unwrap();
    for (a, b) in data.iter().zip(&px) {
        assert!((a - b).abs() < 2e-3, "identity LUT round trip: {b} -> {a}");
    }
}

#[test]
fn unknown_lut_falls_back_and_bad_input_never_crashes() {
    let (w, h) = (16, 16);
    let px = image(w, h, 3);
    let first = run(&px, w, h, Format::RGB8, &format!(r#"{{"lut":"{}"}}"#, LUTS[0].name)).unwrap();
    assert_eq!(run(&px, w, h, Format::RGB8, r#"{"lut":"Nope"}"#).unwrap(), first);
    assert_eq!(run(&px, w, h, Format::RGB8, r#"{"lut":5,"intensity":"x"}"#).unwrap(), first);
    let mut odd = px.clone();
    odd[0] = f32::NAN;
    odd[1] = -3.0;
    odd[2] = 40.0;
    assert!(run(&odd, w, h, Format::RGB8, "{}").is_ok());
    for format in [Format { depth: 8, alpha: false, mode: ColorMode::Grayscale }, Format { depth: 16, alpha: false, mode: ColorMode::Cmyk }] {
        let px = vec![0.5; w * h * format.channels()];
        assert_eq!(run(&px, w, h, format, "{}"), Err(Error::UnsupportedMode));
    }
    assert!(Lut::from_bytes(&[0; 10], 2).is_none());
}
