//! Behaviour tests, run through `moxocraft_sdk::host`, which feeds bands exactly as PhotoCraft does.

use moxocraft_chromatic_aberration::{ChromaticAberration, Fringe, MANIFEST, MAX_TAPS, Taps};
use moxocraft_sdk::host::{self, Bands};
use moxocraft_sdk::{ColorMode, Error, Format, Json};

fn run(pixels: &[f32], w: usize, h: usize, format: Format, params: &str) -> Result<Vec<f32>, Error> {
    host::apply::<ChromaticAberration>(pixels, w, h, format, params, Bands::PHOTOCRAFT)
}

/// One band, margins included, straight through the plug-in's entry point. Unlike `run`, nothing
/// tidies up the result afterwards, so every returned sample can be compared bit for bit.
fn run_band(pixels: &[f32], w: usize, h: usize, format: Format, params: &str) -> Vec<f32> {
    let Some(Json::Object(fields)) = Json::parse(params) else { panic!("{params} isn't a JSON object") };
    let ov = host::manifest_overlap(MANIFEST);
    let text = host::band_params(&fields, (-(ov as i64), -(ov as i64)), (w, h), ov, (w - 2 * ov, h - 2 * ov), format);
    let mut data = pixels.to_vec();
    assert_eq!(moxocraft_sdk::abi::run::<ChromaticAberration>(&mut data, w, h, format.channels(), format.code(), text.as_bytes()), 0, "{params}");
    data
}

fn bits(v: &[f32]) -> Vec<u32> {
    v.iter().map(|x| x.to_bits()).collect()
}

/// RGB with and without alpha, at 8, 16 and 32 bits.
fn every_rgb_format() -> [Format; 6] {
    [8, 16, 32].map(|depth| [false, true].map(|alpha| Format { depth, alpha, mode: ColorMode::Rgb })).concat().try_into().unwrap()
}

/// An RGB image: black with one white vertical line at column `x`.
fn line(w: usize, h: usize, x: usize) -> Vec<f32> {
    let mut px = vec![0.0; w * h * 3];
    for y in 0..h {
        px[(y * w + x) * 3..(y * w + x) * 3 + 3].fill(1.0);
    }
    px
}

/// A small scene with edges in every direction: rings, spokes, dots and a soft gradient.
fn scene(w: usize, h: usize, channels: usize) -> Vec<f32> {
    let mut px = Vec::with_capacity(w * h * channels);
    for y in 0..h {
        for x in 0..w {
            let (u, v) = (x as f32 / w as f32 - 0.5, y as f32 / h as f32 - 0.5);
            let r = (u * u + v * v).sqrt();
            let ring = ((r * 40.0).sin() > 0.6) as u8 as f32;
            let spoke = ((v.atan2(u) * 12.0).sin() > 0.95) as u8 as f32;
            let base = [0.2 + 0.6 * (x as f32 / w as f32), 0.3, 0.6 - 0.4 * (y as f32 / h as f32)];
            let k = ring.max(spoke);
            for c in base {
                px.push(c + (1.0 - c) * k);
            }
            if channels == 4 {
                px.push(if r < 0.4 { 1.0 } else { 0.0 });
            }
        }
    }
    px
}

fn max_diff(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).fold(0.0, |m, (x, y)| m.max((x - y).abs()))
}

#[test]
fn manifest_is_valid_for_photocraft() {
    let m = Json::parse(MANIFEST).expect("manifest is JSON");
    let id = m.get("id").and_then(Json::as_str).unwrap();
    assert!((1..=64).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_alphanumeric() || b".-_".contains(&b)));
    assert_eq!(m.get("kind").and_then(Json::as_str), Some("filter"));
    let name = m.get("name").and_then(Json::as_str).unwrap();
    assert!(!name.trim().is_empty() && name.chars().count() <= 64);
    assert!(m.get("description").and_then(Json::as_str).unwrap().len() <= 1024);
    let overlap = m.get("overlap").and_then(Json::as_f64).unwrap();
    assert!((0.0..=256.0).contains(&overlap));
    let params = m.get("params").and_then(Json::as_object).unwrap();
    for (key, spec) in params {
        assert!(key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') && !key.starts_with('_') && key != "id" && key != "layer", "{key}");
        match spec.get("type").and_then(Json::as_str).unwrap() {
            "number" => {
                let [lo, hi, d] = ["min", "max", "default"].map(|k| spec.get(k).and_then(Json::as_f64).unwrap());
                assert!(lo <= d && d <= hi, "{key}: default outside its range");
            }
            "choice" => {
                let options: Vec<&str> = spec.get("options").and_then(Json::as_array).unwrap().iter().filter_map(Json::as_str).collect();
                assert!(options.iter().all(|o| !o.is_empty() && !o.contains(['|', '"'])));
                assert!(options.contains(&spec.get("default").and_then(Json::as_str).unwrap()), "{key}: default isn't an option");
            }
            other => panic!("{key}: unexpected type {other}"),
        }
    }
    // Every fringe the dialog offers is one the filter knows.
    let fringes = params.iter().find(|(k, _)| k == "fringe").unwrap().1.get("options").and_then(Json::as_array).unwrap();
    for f in fringes {
        assert!(Fringe::from_name(f.as_str().unwrap()).is_some());
    }
}

#[test]
fn each_colour_weighs_exactly_one_for_every_setting() {
    for fringe in [Fringe::Spectral, Fringe::RgbSplit, Fringe::RedCyan, Fringe::GreenMagenta, Fringe::BlueYellow] {
        for softness in [0.0, 0.001, 0.05, 0.3, 1.0] {
            for count in 2..=MAX_TAPS {
                let taps = Taps::new(fringe, softness, count);
                assert!(!taps.offset.is_empty() && taps.offset.len() <= MAX_TAPS);
                for c in 0..3 {
                    let sum: f32 = taps.weight.iter().map(|w| w[c]).sum();
                    assert!((sum - 1.0).abs() < 1e-5, "{fringe:?} {softness} {count}: channel {c} sums to {sum}");
                }
                assert!(taps.reach() <= 2.0);
                // Segments reach one `gap` either side of a tap, so no two taps of a smear may be
                // further apart than that (overlapping smears can bring them closer).
                if !Taps::is_hard(fringe, softness) {
                    let mut sorted = taps.offset.clone();
                    sorted.sort_by(f32::total_cmp);
                    let widest = sorted.windows(2).map(|p| p[1] - p[0]).fold(0.0f32, f32::max);
                    let spread_groups = fringe != Fringe::Spectral;
                    assert!(spread_groups || widest <= taps.gap + 1e-5, "{fringe:?} {softness} {count}: taps {widest} apart, gap {}", taps.gap);
                    assert!(taps.gap > 0.0);
                }
            }
        }
    }
}

#[test]
fn flat_colour_is_left_alone() {
    let (w, h) = (300, 300);
    let px: Vec<f32> = (0..w * h).flat_map(|_| [0.8, 0.4, 0.1]).collect();
    for params in [
        r#"{}"#,
        r#"{"mode":"Directional","amount":-40,"angle":33,"fringe":"RGB Split","softness":0}"#,
        r#"{"amount":128,"softness":100}"#,
        r#"{"amount":30,"strength":50}"#,
    ] {
        let out = run(&px, w, h, Format::RGB8, params).unwrap();
        assert!(max_diff(&px, &out) < 1e-5, "{params}");
    }
}

#[test]
fn zero_amount_changes_nothing() {
    let px = scene(200, 150, 3);
    assert_eq!(run(&px, 200, 150, Format::RGB8, r#"{"amount":0}"#).unwrap(), px);
}

#[test]
fn zero_strength_returns_every_sample_untouched() {
    let (w, h) = (300, 280);
    for format in every_rgb_format() {
        let mut px = scene(w, h, format.channels());
        // Values a normal document never holds, in color and alpha alike: they must come back
        // exactly as they went in, down to the sign of zero.
        let odd = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 1e20, -3.0, 40.0, -0.0, 1e-40];
        for i in 0..px.len() / 997 {
            px[i * 997] = odd[i % odd.len()];
        }
        for params in [
            r#"{"strength":0}"#,
            r#"{"strength":0,"mode":"Directional","fringe":"RGB Split","amount":-30,"softness":0}"#,
            r#"{"strength":0,"amount":128,"softness":100,"clearCenter":0}"#,
        ] {
            assert_eq!(bits(&run_band(&px, w, h, format, params)), bits(&px), "{format:?} {params}");
            // At full strength the same settings do change the band, so the check above is real.
            let full = params.replace(r#""strength":0"#, r#""strength":100"#);
            assert_ne!(bits(&run_band(&px, w, h, format, &full)), bits(&px), "{format:?} {full}");
        }
    }
}

#[test]
fn full_strength_is_the_effect_as_it_was() {
    let (w, h) = (300, 280);
    for format in every_rgb_format() {
        let px = scene(w, h, format.channels());
        for (without, with) in [
            (r#"{}"#, r#"{"strength":100}"#),
            (r#"{"mode":"Directional","fringe":"RGB Split","softness":0}"#, r#"{"mode":"Directional","fringe":"RGB Split","softness":0,"strength":100}"#),
        ] {
            assert_eq!(bits(&run_band(&px, w, h, format, with)), bits(&run_band(&px, w, h, format, without)), "{format:?} {with}");
        }
    }
}

#[test]
fn strength_mixes_the_effect_into_the_original_in_linear_light() {
    let (w, h) = (256, 192);
    let px = scene(w, h, 3);
    let full = run(&px, w, h, Format::RGB8, r#"{"amount":20,"softness":40}"#).unwrap();
    for strength in [1.0f32, 25.0, 50.0, 90.0, 99.9] {
        let out = run(&px, w, h, Format::RGB8, &format!(r#"{{"amount":20,"softness":40,"strength":{strength}}}"#)).unwrap();
        // 8-bit pixels are mixed as gamma 2: squared on the way in, square-rooted on the way out.
        let s = strength / 100.0;
        let worst = (0..px.len()).fold(0.0f32, |m, i| m.max((out[i] - (s * full[i] * full[i] + (1.0 - s) * px[i] * px[i]).sqrt()).abs()));
        assert!(worst < 1e-4, "strength {strength}: off by {worst}");
    }
}

#[test]
fn rgb_split_moves_red_and_blue_by_exactly_amount() {
    let (w, h) = (64, 8);
    let px = line(w, h, 30);
    let out = run(&px, w, h, Format::RGB8, r#"{"mode":"Directional","fringe":"RGB Split","amount":3,"softness":0,"angle":0}"#).unwrap();
    let at = |x: usize, c: usize| out[(4 * w + x) * 3 + c];
    // Red reads from 3 px to the right, so the line shows up in red 3 px to its left; blue the
    // other way; green stays.
    assert!(at(27, 0) > 0.99 && at(27, 1) < 0.01 && at(27, 2) < 0.01);
    assert!(at(33, 2) > 0.99 && at(33, 0) < 0.01 && at(33, 1) < 0.01);
    assert!(at(30, 1) > 0.99 && at(30, 0) < 0.01 && at(30, 2) < 0.01);
    // A negative amount swaps them.
    let flipped = run(&px, w, h, Format::RGB8, r#"{"mode":"Directional","fringe":"RGB Split","amount":-3,"softness":0}"#).unwrap();
    assert!(flipped[(4 * w + 33) * 3] > 0.99 && flipped[(4 * w + 27) * 3 + 2] > 0.99);
}

#[test]
fn long_smears_are_smooth_not_stepped() {
    // A white line smeared over ~100 px: far more ground than the 8 main taps cover on their own,
    // so this goes through the segment pass. Without it the smear breaks into a few copies of the
    // line with dark gaps between them.
    let (w, h) = (400, 4);
    let px = line(w, h, 200);
    let out = run(&px, w, h, Format::RGB8, r#"{"mode":"Directional","amount":60,"softness":60}"#).unwrap();
    let row: Vec<f32> = (0..w).map(|x| out[(2 * w + x) * 3 + 1]).collect();
    let lit: Vec<f32> = row[150..250].to_vec();
    let peak = lit.iter().fold(0.0f32, |a, &b| a.max(b));
    assert!(peak > 0.05, "the smear is visible");
    // Steps show up as big jumps between neighbouring pixels.
    let jump = lit.windows(2).fold(0.0f32, |a, p| a.max((p[1] - p[0]).abs()));
    assert!(jump < peak * 0.12, "jump {jump} against peak {peak}");
}

#[test]
fn angle_points_counter_clockwise() {
    let (w, h) = (16, 64);
    // A white horizontal line at row 30.
    let mut px = vec![0.0; w * h * 3];
    px[30 * w * 3..31 * w * 3].fill(1.0);
    let out = run(&px, w, h, Format::RGB8, r#"{"mode":"Directional","fringe":"Red/Cyan","amount":4,"softness":0,"angle":90}"#).unwrap();
    // 90° points up, so red reads from 4 px above and the red copy of the line lands 4 px below.
    assert!(out[(34 * w + 8) * 3] > 0.99);
    assert!(out[(30 * w + 8) * 3] < 0.01, "red left the original line");
    assert!(out[(30 * w + 8) * 3 + 1] > 0.99, "green stayed");
}

#[test]
fn bands_join_without_seams() {
    // Small bands and odd sizes, so lots of band edges cross the detail.
    let (w, h) = (333, 277);
    for (format, channels) in [(Format::RGB8, 3), (Format::RGBA8, 4)] {
        let px = scene(w, h, channels);
        for params in [
            r#"{"amount":40,"softness":50}"#,
            r#"{"mode":"Directional","amount":-25,"angle":135,"fringe":"Green/Magenta"}"#,
            r#"{"amount":30,"softness":20,"strength":40}"#,
        ] {
            let whole = host::apply::<ChromaticAberration>(&px, w, h, format, params, Bands::WHOLE).unwrap();
            let banded = host::apply::<ChromaticAberration>(&px, w, h, format, params, Bands { width: 64, height: 48 }).unwrap();
            // Positions are worked out from each band's own corner, so float rounding differs a
            // little; 1e-4 is still 40× finer than one 8-bit step.
            assert!(max_diff(&whole, &banded) < 1e-4, "{params}: seams of {}", max_diff(&whole, &banded));
            assert!(max_diff(&whole, &px) > 0.1, "{params}: the effect did something");
        }
    }
}

#[test]
fn lens_mode_leaves_the_centre_and_fringes_the_corners() {
    let (w, h) = (256, 256);
    let px = scene(w, h, 3);
    let out = run(&px, w, h, Format::RGB8, r#"{"amount":20,"clearCenter":30}"#).unwrap();
    let region = |x0: usize, y0: usize, s: usize| {
        let mut d: f32 = 0.0;
        for y in y0..y0 + s {
            for x in x0..x0 + s {
                let i = (y * w + x) * 3;
                d = d.max(max_diff(&px[i..i + 3], &out[i..i + 3]));
            }
        }
        d
    };
    assert_eq!(region(118, 118, 20), 0.0, "the clear centre is untouched");
    assert!(region(0, 0, 30) > 0.2, "the corners carry fringes");
}

#[test]
fn shapes_on_transparent_layers_get_coloured_fringes_not_dark_halos() {
    let (w, h) = (40, 40);
    // An opaque white square on a transparent layer whose hidden color is black.
    let mut px = vec![0.0; w * h * 4];
    for y in 10..30 {
        for x in 10..30 {
            px[(y * w + x) * 4..(y * w + x) * 4 + 4].fill(1.0);
        }
    }
    let out = run(&px, w, h, Format::RGBA8, r#"{"mode":"Directional","fringe":"RGB Split","amount":2,"softness":0}"#).unwrap();
    let p = |x: usize, y: usize| &out[(y * w + x) * 4..(y * w + x) * 4 + 4];
    // Left of the square: only the red copy arrives there. It's pure red, not darkened by the
    // black hidden behind the transparent pixels.
    let left = p(9, 20);
    assert!(left[3] > 0.99 && left[0] > 0.99 && left[1] < 0.01 && left[2] < 0.01, "{left:?}");
    // Inside, away from the edges, the square is still white and opaque.
    assert!(p(20, 20).iter().all(|&v| v > 0.999));
    // Far away it stays transparent.
    assert_eq!(p(2, 2)[3], 0.0);
}

#[test]
fn strength_fades_fringes_on_transparent_layers_without_dark_halos() {
    // The same white square as above, at half strength.
    let (w, h) = (40, 40);
    let mut px = vec![0.0; w * h * 4];
    for y in 10..30 {
        for x in 10..30 {
            px[(y * w + x) * 4..(y * w + x) * 4 + 4].fill(1.0);
        }
    }
    let out = run(&px, w, h, Format::RGBA8, r#"{"mode":"Directional","fringe":"RGB Split","amount":2,"softness":0,"strength":50}"#).unwrap();
    let p = |x: usize, y: usize| &out[(y * w + x) * 4..(y * w + x) * 4 + 4];
    let near = |a: &[f32], b: [f32; 4]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-5);
    // Left of the square the red copy arrives half as strongly: half as opaque, still pure red.
    assert!(near(p(9, 20), [1.0, 0.0, 0.0, 0.5]), "{:?}", p(9, 20));
    // On its left edge, blue has moved away: at full strength the pixel is yellow, at half it
    // keeps half its blue light (the square root of 0.5 in gamma 2) and stays opaque.
    assert!(near(p(10, 20), [1.0, 1.0, 0.5f32.sqrt(), 1.0]), "{:?}", p(10, 20));
    assert!(p(20, 20).iter().all(|&v| v > 0.999));
    assert_eq!(p(2, 2)[3], 0.0);
}

#[test]
fn float_documents_stay_linear_and_keep_bright_values() {
    let (w, h) = (64, 8);
    let mut px = line(w, h, 30);
    for v in px.iter_mut() {
        *v *= 4.0;
    }
    let f32fmt = Format { depth: 32, alpha: false, mode: ColorMode::Rgb };
    let out = run(&px, w, h, f32fmt, r#"{"mode":"Directional","fringe":"RGB Split","amount":2.5,"softness":0}"#).unwrap();
    // Half way between pixels: an even linear blend of 4.0 and 0.0, with nothing clipped at 1.
    let red = out[(4 * w + 27) * 3];
    assert!((red - 2.0).abs() < 1e-4, "{red}");
}

#[test]
fn other_colour_modes_are_refused_cleanly() {
    let (w, h) = (8, 8);
    for format in [
        Format { depth: 8, alpha: false, mode: ColorMode::Grayscale },
        Format { depth: 16, alpha: true, mode: ColorMode::Cmyk },
        Format { depth: 8, alpha: false, mode: ColorMode::Lab },
    ] {
        let px = vec![0.5; w * h * format.channels()];
        assert_eq!(run(&px, w, h, format, "{}"), Err(Error::UnsupportedMode), "{format:?}");
    }
    let indexed = Format { depth: 8, alpha: false, mode: ColorMode::Indexed };
    assert!(run(&scene(w, h, 3), w, h, indexed, "{}").is_ok(), "indexed arrives as RGB");
}

#[test]
fn odd_settings_never_crash() {
    let (w, h) = (50, 40);
    let px = scene(w, h, 4);
    for params in [
        r#"{"amount":"lots","fringe":7,"mode":null,"softness":-5}"#,
        r#"{"amount":1e30,"falloff":-1,"clearCenter":1000,"centerX":-50,"centerY":1e9}"#,
        r#"{"amount":-128,"softness":100,"clearCenter":100,"falloff":4}"#,
        r#"{"fringe":"Nope","mode":"Sideways"}"#,
        r#"{"strength":"half","amount":40}"#,
        r#"{"strength":-5,"amount":40}"#,
        r#"{"strength":1e30,"amount":40,"softness":0}"#,
    ] {
        let out = run(&px, w, h, Format::RGBA8, params).unwrap();
        assert!(out.iter().all(|v| v.is_finite()), "{params}");
    }
    // A buffer that doesn't match its size, or settings that aren't JSON, are refused, not a crash.
    assert_ne!(moxocraft_sdk::abi::run::<ChromaticAberration>(&mut [0.0; 10], 4, 4, 3, Format::RGB8.code(), b"{}"), 0);
    assert_ne!(moxocraft_sdk::abi::run::<ChromaticAberration>(&mut [0.0; 48], 4, 4, 3, Format::RGB8.code(), b"{nope"), 0);
    // Shifts far bigger than the margin are capped to it rather than reading outside the band.
    let tiny = host::apply::<ChromaticAberration>(&px, w, h, Format::RGBA8, r#"{"amount":128}"#, Bands { width: 7, height: 5 }).unwrap();
    assert!(tiny.iter().all(|v| v.is_finite()));
}
