//! Behavior tests, run through `moxocraft_sdk::host`, which feeds bands exactly as PhotoCraft does.

use moxocraft_floating_particles::{FloatingParticles, MANIFEST, hsb};
use moxocraft_particles::Depth;
use moxocraft_sdk::host::{self, Bands};
use moxocraft_sdk::{ColorMode, Error, Format, Json};

const RGBA8: Format = Format::RGBA8;

fn run_in(pixels: &[f32], w: usize, h: usize, format: Format, params: &str, bands: Bands) -> Result<Vec<f32>, Error> {
    host::apply::<FloatingParticles>(pixels, w, h, format, params, bands)
}

fn run(pixels: &[f32], w: usize, h: usize, params: &str) -> Vec<f32> {
    run_in(pixels, w, h, RGBA8, params, Bands::PHOTOCRAFT).unwrap()
}

/// An opaque RGBA picture: a dark gradient with a brighter patch, so particles show everywhere.
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

/// A depth map as an opaque RGBA layer, from one gray value per pixel.
fn map(w: usize, h: usize, gray: impl Fn(usize, usize) -> f32) -> Vec<f32> {
    (0..w * h)
        .flat_map(|i| {
            let g = gray(i % w, i / w);
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
    // (`is_pixel_param` in its `crates/ui-egui/src/filter_dialog.rs`). These are all relative.
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
    // Every choice the dialog offers is one the filter knows, and Depth offers them all.
    let spec = &params.iter().find(|(k, _)| k == "depth").unwrap().1;
    let options: Vec<&str> = spec.get("options").and_then(Json::as_array).unwrap().iter().filter_map(Json::as_str).collect();
    assert_eq!(options, Depth::NAMES);
}

#[test]
fn bands_join_without_seams() {
    let (w, h) = (700, 520);
    let pic = photo(w, h);
    let depth = map(w, h, |x, y| if x > w / 3 && y > h / 4 { 0.8 } else { 0.1 });
    for (pixels, params) in [
        (&pic, r#"{"amount":80,"particleSize":80,"defocus":70}"#),
        (&pic, r#"{"amount":60,"glow":100,"defocus":10,"clustering":100}"#),
        (&pic, r#"{"amount":100,"particleSize":100,"sizeVariety":100,"defocus":100,"glow":100,"subjectDepth":0}"#),
        (&pic, r#"{"amount":100,"particleSize":0,"sizeVariety":0,"defocus":0,"glow":0,"clustering":0}"#),
        (&pic, r#"{"depth":"Near Only","subjectDepth":60,"defocus":100}"#),
        (&depth, r#"{"depth":"Depth Map","amount":70,"glow":60}"#),
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
    for params in
        [r#"{}"#, r#"{"amount":90,"particleSize":20,"defocus":0,"glow":0}"#, r#"{"amount":60,"glow":100,"defocus":60}"#, r#"{"amount":70,"clustering":100}"#]
    {
        let full = run(&black(w, h), w, h, params);
        let small = run(&black(w / k, h / k), w / k, h / k, params);
        // Average the full result down, then compare 25 × 25 blocks of the reduced size.
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
    for params in [r#"{}"#, r#"{"amount":90,"opacity":55,"hue":200,"saturation":70,"defocus":80}"#, r#"{"glow":100,"brightness":40}"#] {
        let direct = run(&pic, w, h, params);
        let layer = run(&empty(w, h), w, h, params);
        assert!(max_diff(&direct, &over(&pic, &layer)) < 1e-5, "{params}");
        assert!(alphas(&layer).iter().any(|&a| a > 0.2), "{params}: no particles drawn");
    }
}

#[test]
fn near_and_far_layers_add_up_to_all_the_particles() {
    let (w, h) = (400, 300);
    let pic = photo(w, h);
    for subject in [10, 30, 75] {
        let all = run(&pic, w, h, &format!(r#"{{"amount":80,"subjectDepth":{subject}}}"#));
        let near = run(&empty(w, h), w, h, &format!(r#"{{"amount":80,"subjectDepth":{subject},"depth":"Near Only"}}"#));
        let far = run(&empty(w, h), w, h, &format!(r#"{{"amount":80,"subjectDepth":{subject},"depth":"Far Only"}}"#));
        assert!(max_diff(&all, &over(&over(&pic, &far), &near)) < 1e-4, "subject depth {subject}");
        assert!(alphas(&near).iter().any(|&a| a > 0.0) && alphas(&far).iter().any(|&a| a > 0.0), "subject depth {subject}");
    }
}

#[test]
fn a_depth_map_hides_particles_behind_nearer_parts_of_the_scene() {
    let (w, h) = (400, 300);
    let params = |depth: &str| format!(r#"{{"amount":90,"glow":50,"depth":"{depth}"}}"#);
    let particles = run(&empty(w, h), w, h, &params("None"));
    let white = map(w, h, |_, _| 1.0);
    let black = map(w, h, |_, _| 0.0);
    let color = hsb(40.0, 0.3, 1.0);
    // White is right at the lens, so nothing is in front of it; black hides nothing.
    for (pixels, mode, shows) in
        [(&white, "Depth Map", false), (&black, "Depth Map", true), (&white, "Depth Map (Inverted)", true), (&black, "Depth Map (Inverted)", false)]
    {
        let out = run(pixels, w, h, &params(mode));
        if shows {
            assert!(bits(&alphas(&out)) == bits(&alphas(&particles)), "{mode}: the layer should hold all the particles");
        } else {
            assert!(alphas(&out).iter().all(|&a| a == 0.0), "{mode}: the layer should be empty");
        }
        // The map is replaced by the particles in their color, whatever shows.
        assert!(out.chunks_exact(4).all(|p| p[..3] == color), "{mode}");
    }
    // The nearer the scene, the more it hides, pixel by pixel.
    let uniform = |gray: f32| alphas(&run(&map(w, h, |_, _| gray), w, h, &params("Depth Map")));
    let steps: Vec<Vec<f32>> = [0.0, 0.3, 0.5, 0.8, 1.0].into_iter().map(uniform).collect();
    for pair in steps.windows(2) {
        assert!(pair[0].iter().zip(&pair[1]).all(|(far, near)| near <= &(far + 1e-6)), "a nearer scene showed more particles");
        assert!(mean(&pair[1]) < mean(&pair[0]));
    }
    // Each pixel only looks at the map under it.
    let subject = map(w, h, |x, _| if (150..250).contains(&x) { 0.5 } else { 0.0 });
    let out = alphas(&run(&subject, w, h, &params("Depth Map")));
    for (i, &a) in out.iter().enumerate() {
        let expected = if (150..250).contains(&(i % w)) { steps[2][i] } else { steps[0][i] };
        assert_eq!(a.to_bits(), expected.to_bits(), "pixel {i}");
    }
}

#[test]
fn more_particles_only_adds_particles() {
    let (w, h) = (400, 300);
    let less = alphas(&run(&empty(w, h), w, h, r#"{"amount":30}"#));
    let more = alphas(&run(&empty(w, h), w, h, r#"{"amount":60}"#));
    assert!(less.iter().zip(&more).all(|(a, b)| *b >= a - 1e-6), "a particle disappeared when Amount went up");
    assert!(mean(&more) > 1.5 * mean(&less));
    let other = alphas(&run(&empty(w, h), w, h, r#"{"amount":30,"variation":2}"#));
    assert!(max_diff(&less, &other) > 0.3, "Variation should change the layout");
}

#[test]
fn clustering_gathers_the_particles_without_changing_how_many() {
    let (w, h) = (900, 600);
    // Only the far particles, all one size, so each covers about as much as another and the
    // layout shows rather than a few big near ones.
    let layer = |clustering: u32| {
        let params = format!(r#"{{"amount":80,"sizeVariety":0,"defocus":0,"glow":0,"depth":"Far Only","subjectDepth":50,"clustering":{clustering}}}"#);
        alphas(&run(&empty(w, h), w, h, &params))
    };
    // How unevenly the particles cover the picture: the spread of 50 × 50 blocks' coverage.
    let unevenness = |a: &[f32]| {
        let blocks: Vec<f32> = (0..(h / 50) * (w / 50))
            .map(|b| (0..2500).map(|j| a[((b / (w / 50)) * 50 + j / 50) * w + (b % (w / 50)) * 50 + j % 50]).sum::<f32>() / 2500.0)
            .collect();
        let m = mean(&blocks);
        (blocks.iter().map(|v| (v - m) * (v - m)).sum::<f32>() / blocks.len() as f32).sqrt() / m
    };
    let (even, half, clustered) = (layer(0), layer(50), layer(100));
    let (m0, m100) = (mean(&even), mean(&clustered));
    assert!((m0 - m100).abs() < 0.25 * m0, "Clustering changed how much is covered: {m0} at 0, {m100} at 100");
    let (u0, u50, u100) = (unevenness(&even), unevenness(&half), unevenness(&clustered));
    assert!(u0 < u50 && u50 < u100 && u100 > 2.0 * u0, "unevenness {u0} at 0, {u50} at 50, {u100} at 100");
}

#[test]
fn glow_only_adds_a_halo() {
    let (w, h) = (400, 300);
    let plain = alphas(&run(&empty(w, h), w, h, r#"{"amount":60,"defocus":0,"glow":0}"#));
    let glowing = alphas(&run(&empty(w, h), w, h, r#"{"amount":60,"defocus":0,"glow":100}"#));
    assert!(plain.iter().zip(&glowing).all(|(a, b)| *b >= a - 1e-6), "Glow hid part of a particle");
    assert!(mean(&glowing) > 1.5 * mean(&plain));
}

#[test]
fn hue_saturation_and_brightness_give_the_expected_colors() {
    let near = |a: [f32; 3], b: [f32; 3]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6);
    for (hue, rgb) in [
        (0.0, [1.0, 0.0, 0.0]),
        (60.0, [1.0, 1.0, 0.0]),
        (120.0, [0.0, 1.0, 0.0]),
        (180.0, [0.0, 1.0, 1.0]),
        (240.0, [0.0, 0.0, 1.0]),
        (300.0, [1.0, 0.0, 1.0]),
        (360.0, [1.0, 0.0, 0.0]),
    ] {
        assert!(near(hsb(hue, 1.0, 1.0), rgb), "hue {hue}: {:?}", hsb(hue, 1.0, 1.0));
    }
    assert!(near(hsb(30.0, 1.0, 1.0), [1.0, 0.5, 0.0]));
    assert!(near(hsb(210.0, 0.5, 0.8), [0.4, 0.6, 0.8]));
    for hue in [0.0, 97.0, 333.0] {
        assert!(near(hsb(hue, 0.0, 1.0), [1.0; 3]), "Saturation 0 is white");
        assert!(near(hsb(hue, 0.7, 0.0), [0.0; 3]), "Brightness 0 is black");
    }
    // The sliders set the color of the particles on an empty layer.
    let (w, h) = (200, 150);
    let out = run(&empty(w, h), w, h, r#"{"amount":80,"hue":210,"saturation":50,"brightness":80}"#);
    let drawn: Vec<&[f32]> = out.chunks_exact(4).filter(|p| p[3] > 0.0).collect();
    assert!(!drawn.is_empty() && drawn.iter().all(|p| near([p[0], p[1], p[2]], [0.4, 0.6, 0.8])));
}

#[test]
fn works_in_every_rgb_format() {
    let (w, h) = (200, 150);
    let color = hsb(200.0, 0.4, 1.0);
    for depth in [8, 16, 32] {
        for alpha in [false, true] {
            let format = Format { depth, alpha, mode: ColorMode::Rgb };
            let n = format.channels();
            let px: Vec<f32> = (0..w * h).flat_map(|_| [0.2, 0.25, 0.3, 1.0][..n].to_vec()).collect();
            let out = run_in(&px, w, h, format, r#"{"amount":80,"hue":200,"saturation":40,"glow":60}"#, Bands::PHOTOCRAFT).unwrap();
            assert!(out.iter().all(|v| (0.0..=1.0).contains(v)), "{format:?}");
            assert!(max_diff(&px, &out) > 0.3, "{format:?}: no particles");
            // Where particles cover the most, the picture has moved toward their color:
            // gamma-encoded, or linear light in 32-bit.
            let color = if depth == 32 { color.map(|c| c * c) } else { color };
            let p = out.chunks_exact(n).max_by(|a, b| a[2].total_cmp(&b[2])).unwrap();
            let covered = (p[2] - 0.3) / (color[2] - 0.3);
            assert!(covered > 0.3, "{format:?}: particles cover at most {covered}");
            for (c, base) in [0.2, 0.25].into_iter().enumerate() {
                let expected = base + (color[c] - base) * covered;
                assert!((p[c] - expected).abs() < 1e-4, "{format:?}: channel {c} is {}, expected {expected}", p[c]);
            }
        }
    }
}

#[test]
fn other_color_modes_are_refused() {
    for mode in [ColorMode::Grayscale, ColorMode::Cmyk, ColorMode::Lab] {
        let format = Format { depth: 8, alpha: false, mode };
        let px = vec![0.5; 16 * 16 * format.channels()];
        assert_eq!(run_in(&px, 16, 16, format, "{}", Bands::PHOTOCRAFT), Err(Error::UnsupportedMode), "{mode:?}");
    }
}
