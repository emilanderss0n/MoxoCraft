//! Behaviour tests, run through `moxocraft_sdk::host`, which feeds bands exactly as PhotoCraft does.

use moxocraft_particles::Depth;
use moxocraft_sdk::host::{self, Bands};
use moxocraft_sdk::{ColorMode, Error, Format, Json};
use moxocraft_snow::{MANIFEST, Snow, Tint};

const RGBA8: Format = Format::RGBA8;

fn run_in(pixels: &[f32], w: usize, h: usize, format: Format, params: &str, bands: Bands) -> Result<Vec<f32>, Error> {
    host::apply::<Snow>(pixels, w, h, format, params, bands)
}

fn run(pixels: &[f32], w: usize, h: usize, params: &str) -> Vec<f32> {
    run_in(pixels, w, h, RGBA8, params, Bands::PHOTOCRAFT).unwrap()
}

/// An opaque RGBA picture: a dark gradient with a brighter patch, so flakes show everywhere.
fn photo(w: usize, h: usize) -> Vec<f32> {
    (0..w * h)
        .flat_map(|i| {
            let (x, y) = ((i % w) as f32 / w as f32, (i / w) as f32 / h as f32);
            let patch = if (0.3..0.6).contains(&x) && (0.2..0.5).contains(&y) { 0.3 } else { 0.0 };
            [0.1 + 0.2 * x + patch, 0.12 + 0.1 * y + patch, 0.2 + patch, 1.0]
        })
        .collect()
}

fn empty(w: usize, h: usize) -> Vec<f32> {
    vec![0.0; w * h * 4]
}

/// A depth map as an opaque RGBA layer, from one grey value per pixel.
fn map(w: usize, h: usize, grey: impl Fn(usize, usize) -> f32) -> Vec<f32> {
    (0..w * h)
        .flat_map(|i| {
            let g = grey(i % w, i / w);
            [g, g, g, 1.0]
        })
        .collect()
}

/// Lays `layer` (straight alpha) over the opaque `base` in Normal mode, as PhotoCraft composites
/// a layer above another.
fn over(base: &[f32], layer: &[f32]) -> Vec<f32> {
    base.chunks_exact(4)
        .zip(layer.chunks_exact(4))
        .flat_map(|(b, l)| {
            let a = l[3];
            [l[0] * a + b[0] * (1.0 - a), l[1] * a + b[1] * (1.0 - a), l[2] * a + b[2] * (1.0 - a), 1.0]
        })
        .collect()
}

fn alphas(px: &[f32]) -> Vec<f32> {
    px.chunks_exact(4).map(|p| p[3]).collect()
}

fn bits(v: &[f32]) -> Vec<u32> {
    v.iter().map(|x| x.to_bits()).collect()
}

fn max_diff(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len());
    a.iter().zip(b).fold(0.0, |m, (x, y)| m.max((x - y).abs()))
}

fn mean(v: &[f32]) -> f32 {
    v.iter().sum::<f32>() / v.len() as f32
}

#[test]
fn manifest_is_valid_for_photocraft() {
    let m = Json::parse(MANIFEST).expect("manifest is JSON");
    let id = m.get("id").and_then(Json::as_str).unwrap();
    assert!((1..=64).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_alphanumeric() || b".-_".contains(&b)));
    assert_eq!(m.get("kind").and_then(Json::as_str), Some("filter"));
    let name = m.get("name").and_then(Json::as_str).unwrap();
    assert!(!name.trim().is_empty() && name.chars().count() <= 64);
    let description = m.get("description").and_then(Json::as_str).unwrap();
    assert!(description.len() <= 1024, "description is {} bytes", description.len());
    assert_eq!(m.get("overlap").and_then(Json::as_f64), Some(0.0));
    assert_eq!(m.get("area").and_then(Json::as_str), Some("canvas"), "must also run on an empty layer");
    let params = m.get("params").and_then(Json::as_object).unwrap();
    // Names PhotoCraft treats as pixel lengths and shrinks on its reduced live preview
    // (`is_pixel_param` in its `crates/ui-egui/src/filter_dialog.rs`). Snow's are all relative.
    let pixel_names = [
        "radius",
        "distance",
        "cellSize",
        "horizontal",
        "vertical",
        "height",
        "wavelengthMin",
        "wavelengthMax",
        "amplitudeMin",
        "amplitudeMax",
        "maxRadius",
        "size",
        "blur",
        "speed",
        "shadowRadius",
        "highlightRadius",
    ];
    for (key, spec) in params {
        assert!(key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') && !key.starts_with('_') && key != "id" && key != "layer", "{key}");
        assert!(!pixel_names.contains(&key.as_str()), "{key}: PhotoCraft would scale it on previews");
        match spec.get("type").and_then(Json::as_str).unwrap() {
            "number" | "int" => {
                let [lo, hi, d] = ["min", "max", "default"].map(|k| spec.get(k).and_then(Json::as_f64).unwrap());
                assert!(lo <= d && d <= hi, "{key}: default outside its range");
            }
            "choice" => {
                let options: Vec<&str> = spec.get("options").and_then(Json::as_array).unwrap().iter().filter_map(Json::as_str).collect();
                assert!((1..=64).contains(&options.len()) && options.iter().all(|o| !o.is_empty() && !o.contains(['|', '"'])));
                assert!(options.contains(&spec.get("default").and_then(Json::as_str).unwrap()), "{key}: default isn't an option");
            }
            other => panic!("{key}: unexpected type {other}"),
        }
    }
    let options = |key: &str| -> Vec<String> {
        let spec = &params.iter().find(|(k, _)| k == key).unwrap().1;
        spec.get("options").and_then(Json::as_array).unwrap().iter().map(|o| o.as_str().unwrap().to_string()).collect()
    };
    // Every choice the dialog offers is one the filter knows, and Depth offers them all.
    assert_eq!(options("depth"), Depth::NAMES);
    assert!(options("tint").iter().all(|t| Tint::from_name(t).is_some()));
}

#[test]
fn bands_join_without_seams() {
    let (w, h) = (700, 520);
    let pic = photo(w, h);
    let depth = map(w, h, |x, y| if x > w / 3 && y > h / 4 { 0.8 } else { 0.1 });
    for (pixels, params) in [
        (&pic, r#"{"amount":80,"flakeSize":80,"defocus":70}"#),
        (&pic, r#"{"amount":60,"motion":70,"angle":35}"#),
        (&pic, r#"{"amount":100,"motion":100,"angle":85,"flakeSize":0,"defocus":0}"#),
        (&pic, r#"{"depth":"Near Only","subjectDepth":60,"defocus":100}"#),
        (&depth, r#"{"depth":"Depth Map","amount":70,"motion":20}"#),
    ] {
        let whole = run_in(pixels, w, h, RGBA8, params, Bands::WHOLE).unwrap();
        for bands in [Bands::PHOTOCRAFT, Bands { width: 97, height: 61 }, Bands { width: 1, height: 520 }, Bands { width: 700, height: 3 }] {
            let banded = run_in(pixels, w, h, RGBA8, params, bands).unwrap();
            assert!(bits(&whole) == bits(&banded), "{params} differs in {}×{} bands", bands.width, bands.height);
        }
    }
}

#[test]
fn the_reduced_live_preview_looks_like_the_result() {
    // PhotoCraft previews large documents on a copy reduced 2 or 4 times and tells the plug-in
    // the reduced size. Shrunk to the same size, the full result should look the same.
    let (w, h) = (1200, 800);
    let k = 4;
    let black = |w: usize, h: usize| (0..w * h).flat_map(|_| [0.0, 0.0, 0.0, 1.0]).collect::<Vec<f32>>();
    for params in [r#"{}"#, r#"{"amount":90,"flakeSize":20,"defocus":0}"#, r#"{"amount":50,"motion":50,"defocus":60}"#] {
        let full = run(&black(w, h), w, h, params);
        let small = run(&black(w / k, h / k), w / k, h / k, params);
        // Average the full result down, then compare 50 × 50 blocks of the reduced size.
        let (sw, sh) = (w / k, h / k);
        let shrunk: Vec<f32> = (0..sw * sh)
            .map(|i| {
                let (x, y) = (i % sw * k, i / sw * k);
                (0..k * k).map(|j| full[((y + j / k) * w + x + j % k) * 4]).sum::<f32>() / (k * k) as f32
            })
            .collect();
        let small: Vec<f32> = small.chunks_exact(4).map(|p| p[0]).collect();
        let (a, b) = (mean(&shrunk), mean(&small));
        assert!((a - b).abs() < 0.12 * a.max(b), "{params}: overall brightness {a} on the full result, {b} on the preview");
        let block = |v: &[f32], bx: usize, by: usize| (0..25 * 25).map(|j| v[(by * 25 + j / 25) * sw + bx * 25 + j % 25]).sum::<f32>() / 625.0;
        for by in 0..sh / 25 {
            for bx in 0..sw / 25 {
                let (a, b) = (block(&shrunk, bx, by), block(&small, bx, by));
                assert!((a - b).abs() < 0.03 + 0.25 * a.max(b), "{params}: block {bx},{by}: {a} full, {b} preview");
            }
        }
    }
}

#[test]
fn nothing_to_draw_leaves_the_layer_exactly_as_it_was() {
    let (w, h) = (300, 200);
    let pic = photo(w, h);
    for params in [r#"{"amount":0}"#, r#"{"opacity":0}"#, r#"{"depth":"Near Only","subjectDepth":0}"#] {
        assert!(bits(&run(&pic, w, h, params)) == bits(&pic), "{params}");
    }
}

#[test]
fn an_empty_layer_gives_the_same_picture_as_drawing_on_the_photo() {
    let (w, h) = (400, 300);
    let pic = photo(w, h);
    for params in [r#"{}"#, r#"{"amount":90,"opacity":55,"tint":"Warm","defocus":80}"#, r#"{"motion":40,"angle":-30}"#] {
        let direct = run(&pic, w, h, params);
        let layer = run(&empty(w, h), w, h, params);
        assert!(max_diff(&direct, &over(&pic, &layer)) < 1e-5, "{params}");
        assert!(alphas(&layer).iter().any(|&a| a > 0.2), "{params}: no flakes drawn");
    }
}

#[test]
fn near_and_far_layers_add_up_to_all_the_snow() {
    let (w, h) = (400, 300);
    let pic = photo(w, h);
    for subject in [10, 30, 75] {
        let all = run(&pic, w, h, &format!(r#"{{"amount":80,"subjectDepth":{subject}}}"#));
        let near = run(&empty(w, h), w, h, &format!(r#"{{"amount":80,"subjectDepth":{subject},"depth":"Near Only"}}"#));
        let far = run(&empty(w, h), w, h, &format!(r#"{{"amount":80,"subjectDepth":{subject},"depth":"Far Only"}}"#));
        assert!(max_diff(&all, &over(&over(&pic, &far), &near)) < 1e-4, "subject depth {subject}");
        // Flakes in front of the subject are fewer but bigger than those behind it.
        assert!(alphas(&near).iter().any(|&a| a > 0.0) && alphas(&far).iter().any(|&a| a > 0.0), "subject depth {subject}");
    }
}

#[test]
fn a_depth_map_hides_snow_behind_nearer_parts_of_the_scene() {
    let (w, h) = (400, 300);
    let params = |depth: &str| format!(r#"{{"amount":90,"motion":10,"depth":"{depth}"}}"#);
    let snow = run(&empty(w, h), w, h, &params("None"));
    let white = map(w, h, |_, _| 1.0);
    let black = map(w, h, |_, _| 0.0);
    // White is right at the lens, so nothing is in front of it; black hides nothing.
    for (pixels, mode, shows) in
        [(&white, "Depth Map", false), (&black, "Depth Map", true), (&white, "Depth Map (Inverted)", true), (&black, "Depth Map (Inverted)", false)]
    {
        let out = run(pixels, w, h, &params(mode));
        if shows {
            assert!(bits(&alphas(&out)) == bits(&alphas(&snow)), "{mode}: the layer should hold all the snow");
        } else {
            assert!(alphas(&out).iter().all(|&a| a == 0.0), "{mode}: the layer should be empty");
        }
        // The map is replaced by the snow in its color, whatever shows.
        assert!(out.chunks_exact(4).all(|p| p[..3] == [1.0, 1.0, 1.0]), "{mode}");
    }
    // The nearer the scene, the more it hides, pixel by pixel.
    let uniform = |grey: f32| alphas(&run(&map(w, h, |_, _| grey), w, h, &params("Depth Map")));
    let steps: Vec<Vec<f32>> = [0.0, 0.3, 0.5, 0.8, 1.0].into_iter().map(uniform).collect();
    for pair in steps.windows(2) {
        assert!(pair[0].iter().zip(&pair[1]).all(|(far, near)| near <= &(far + 1e-6)), "a nearer scene showed more snow");
        assert!(mean(&pair[1]) < mean(&pair[0]));
    }
    // Each pixel only looks at the map under it: a subject in the middle, half way back, gets
    // exactly what a whole map at that depth gives, and nothing is hidden around it.
    let subject = map(w, h, |x, _| if (150..250).contains(&x) { 0.5 } else { 0.0 });
    let out = alphas(&run(&subject, w, h, &params("Depth Map")));
    for (i, &a) in out.iter().enumerate() {
        let expected = if (150..250).contains(&(i % w)) { steps[2][i] } else { steps[0][i] };
        assert_eq!(a.to_bits(), expected.to_bits(), "pixel {i}");
    }
}

#[test]
fn more_snow_only_adds_flakes() {
    let (w, h) = (400, 300);
    let less = alphas(&run(&empty(w, h), w, h, r#"{"amount":30}"#));
    let more = alphas(&run(&empty(w, h), w, h, r#"{"amount":60}"#));
    assert!(less.iter().zip(&more).all(|(a, b)| *b >= a - 1e-6), "a flake disappeared when Amount went up");
    assert!(mean(&more) > 1.5 * mean(&less));
    let other = alphas(&run(&empty(w, h), w, h, r#"{"amount":30,"variation":2}"#));
    assert!(max_diff(&less, &other) > 0.5, "Variation should change the layout");
}

#[test]
fn works_in_every_rgb_format() {
    let (w, h) = (200, 150);
    for depth in [8, 16, 32] {
        for alpha in [false, true] {
            let format = Format { depth, alpha, mode: ColorMode::Rgb };
            let n = format.channels();
            let px: Vec<f32> = (0..w * h).flat_map(|_| [0.2, 0.25, 0.3, 1.0][..n].to_vec()).collect();
            let out = run_in(&px, w, h, format, r#"{"amount":80,"tint":"Cool"}"#, Bands::PHOTOCRAFT).unwrap();
            assert!(out.iter().all(|v| (0.0..=1.0).contains(v)), "{format:?}");
            assert!(max_diff(&px, &out) > 0.3, "{format:?}: no flakes");
            // Where flakes cover the most, the picture has moved towards the flake color:
            // gamma-encoded, or linear light in 32-bit.
            let colour = if depth == 32 { [0.86 * 0.86, 0.93 * 0.93, 1.0] } else { [0.86, 0.93, 1.0] };
            let p = out.chunks_exact(n).max_by(|a, b| a[2].total_cmp(&b[2])).unwrap();
            let covered = (p[2] - 0.3) / (colour[2] - 0.3);
            assert!(covered > 0.3, "{format:?}: flakes cover at most {covered}");
            for (c, base) in [0.2, 0.25].into_iter().enumerate() {
                let expected = base + (colour[c] - base) * covered;
                assert!((p[c] - expected).abs() < 1e-4, "{format:?}: channel {c} is {}, expected {expected}", p[c]);
            }
        }
    }
}

#[test]
fn other_colour_modes_are_refused() {
    for mode in [ColorMode::Grayscale, ColorMode::Cmyk, ColorMode::Lab] {
        let format = Format { depth: 8, alpha: false, mode };
        let px = vec![0.5; 16 * 16 * format.channels()];
        assert_eq!(run_in(&px, 16, 16, format, "{}", Bands::PHOTOCRAFT), Err(Error::UnsupportedMode), "{mode:?}");
    }
}
