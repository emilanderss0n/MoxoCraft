//! Test helpers, and [`pack_tests!`](crate::pack_tests), the tests every LUT plug-in runs on its
//! own baked-in LUTs. Not part of the plug-ins themselves.

use crate::cube;

/// A LUT of `size` points per axis that maps every colour to itself, as stored bytes.
pub fn identity(size: usize) -> Vec<u8> {
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
pub fn colours(n: usize) -> Vec<[f32; 3]> {
    (0..n)
        .map(|i| {
            let t = i as f32 / n as f32;
            [(t * 7.3).fract(), (t * 3.1 + 0.2).fract(), (t * 11.7 + 0.5).fract()]
        })
        .collect()
}

/// A test image: gradients and a repeating pattern, with a varied alpha when `channels` is 4.
pub fn image(w: usize, h: usize, channels: usize) -> Vec<f32> {
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

/// The rows of a `.cube` file's table, in file order.
pub fn cube_rows(path: &std::path::Path) -> Vec<[f32; 3]> {
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .lines()
        .filter_map(|l| {
            let v: Vec<f32> = l.split_whitespace().map_while(|p| p.parse().ok()).collect();
            <[f32; 3]>::try_from(v).ok()
        })
        .collect()
}

/// The behaviour tests every LUT plug-in runs, through `moxocraft_sdk::host`, which feeds bands
/// exactly as PhotoCraft does. Call from the plug-in's `tests/` with its crate name, manifest id
/// and LUT folder (relative to the crate):
///
/// `moxocraft_lut_core::pack_tests!(moxocraft_lut, "moxocraft.lut", "../../luts/film-luts");`
#[macro_export]
macro_rules! pack_tests {
    ($pack:ident, $id:expr, $folder:expr) => {
        use $crate::sdk::host::{self, Bands};
        use $crate::sdk::{Error, Format, Json};
        use $crate::testing::{colours, cube_rows, image};
        use $crate::{Lut, cube};
        use $pack::{DATA, LUTS, LutFilter, MANIFEST};

        fn run(pixels: &[f32], w: usize, h: usize, format: Format, params: &str) -> Result<Vec<f32>, Error> {
            host::apply::<LutFilter>(pixels, w, h, format, params, Bands::PHOTOCRAFT)
        }

        fn folder() -> std::path::PathBuf {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join($folder)
        }

        #[test]
        fn manifest_lists_exactly_the_baked_in_luts() {
            let m = Json::parse(MANIFEST).expect("manifest is JSON");
            assert_eq!(m.get("id").and_then(Json::as_str), Some($id));
            assert!(m.get("name").and_then(Json::as_str).is_some_and(|n| n.chars().count() <= 64));
            assert!(m.get("description").and_then(Json::as_str).is_some_and(|d| d.len() <= 1024));
            let lut = m.get("params").and_then(|p| p.get("lut")).unwrap();
            let options: Vec<&str> = lut.get("options").and_then(Json::as_array).unwrap().iter().filter_map(Json::as_str).collect();
            assert_eq!(options, LUTS.iter().map(|l| l.name).collect::<Vec<_>>());
            // PhotoCraft's rules for a choice parameter.
            assert!((1..=64).contains(&options.len()), "{} choices; PhotoCraft allows 64", options.len());
            assert!(options.iter().all(|o| !o.is_empty() && o.chars().count() <= 64 && !o.contains(['|', '"'])));
            assert!(options.contains(&lut.get("default").and_then(Json::as_str).unwrap()));
            assert!(DATA.len() < 30 << 20, "fits PhotoCraft's 32 MiB module limit");
        }

        #[test]
        fn lookups_are_continuous_across_cell_edges() {
            // Neighbouring colours must give neighbouring results, whichever tetrahedron each
            // lands in.
            for info in LUTS {
                let lut = Lut::new(DATA, info).unwrap();
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
            // Re-read each .cube file stored as-is and compare grid points with what the plug-in
            // looks up there. (DaVinci-space LUTs are checked against the exact conversion.)
            for info in LUTS.iter().filter(|l| !l.davinci) {
                let rows = cube_rows(&folder().join(info.file));
                let n = info.size;
                assert_eq!(rows.len(), n * n * n, "{}", info.name);
                let lut = Lut::new(DATA, info).unwrap();
                let step = (n / 7).max(1);
                for b in (0..n).step_by(step) {
                    for g in (0..n).step_by(step) {
                        for r in (0..n).step_by(step) {
                            let grid = [r, g, b].map(|v| v as f32 / (n - 1) as f32);
                            let got = lut.lookup(grid);
                            // The plug-in clamps to 0-1 after looking up.
                            let want = rows[(b * n + g) * n + r].map(|v| v.clamp(0.0, 1.0));
                            for i in 0..3 {
                                assert!((got[i] - want[i]).abs() < 4e-5, "{} at {r},{g},{b}: {got:?} vs {want:?}", info.name);
                            }
                        }
                    }
                }
            }
        }

        #[test]
        fn baked_davinci_luts_match_the_exact_conversion() {
            // The baked table must give what converting every pixel into DaVinci space and back
            // would, across the whole colour cube. A sliver of extreme cyans (red near zero, where
            // a film LUT compresses colour hardest) comes out a few steps off, because the table
            // is interpolated twice there; everything else lands within a step or two. Only runs
            // when the folder's davinci-intermediate.txt lists some LUTs.
            for info in LUTS.iter().filter(|l| l.davinci) {
                let source = cube::parse_cube(&std::fs::read_to_string(folder().join(info.file)).unwrap()).unwrap();
                let baked = Lut::new(DATA, info).unwrap();
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
            }
        }

        #[test]
        fn intensity_blends_between_original_and_graded() {
            let (w, h) = (64, 48);
            let px = image(w, h, 3);
            for info in LUTS {
                let lut = info.name;
                let none = run(&px, w, h, Format::RGB8, &format!(r#"{{"lut":"{lut}","intensity":0}}"#)).unwrap();
                assert_eq!(none, px);
                let full = run(&px, w, h, Format::RGB8, &format!(r#"{{"lut":"{lut}","intensity":100}}"#)).unwrap();
                let half = run(&px, w, h, Format::RGB8, &format!(r#"{{"lut":"{lut}","intensity":50}}"#)).unwrap();
                assert!(full.iter().zip(&px).any(|(a, b)| (a - b).abs() > 0.02), "{lut} changes the image");
                for i in 0..px.len() {
                    assert!((half[i] - (px[i] + full[i]) / 2.0).abs() < 1e-5);
                }
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
            for format in [Format { depth: 8, alpha: false, mode: $crate::sdk::ColorMode::Grayscale }, Format { depth: 16, alpha: false, mode: $crate::sdk::ColorMode::Cmyk }] {
                let px = vec![0.5; w * h * format.channels()];
                assert_eq!(run(&px, w, h, format, "{}"), Err(Error::UnsupportedMode));
            }
        }
    };
}
