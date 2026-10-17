//! Renders a test scene and the filter's look at a few settings, as PNGs.
//!
//!   cargo run --release -p moxocraft-chromatic-aberration --example preview [out-dir] [width height]
//!
//! Writes `scene.png` (the untouched input, also handy for trying the plug-in inside PhotoCraft)
//! and one PNG per preset into `out-dir` (default `target/preview`). Bands are fed the same way
//! PhotoCraft feeds them, so these match what the plug-in does in the app.

use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use moxocraft_chromatic_aberration::ChromaticAberration;
use moxocraft_sdk::Format;
use moxocraft_sdk::host::{self, Bands};

const PRESETS: &[(&str, &str)] = &[
    ("lens-spectral", r#"{}"#),
    ("lens-strong", r#"{"amount":28,"softness":45,"clearCenter":15,"falloff":1.2}"#),
    ("lens-red-cyan", r#"{"fringe":"Red/Cyan","amount":16,"softness":20}"#),
    ("lens-purple", r#"{"fringe":"Green/Magenta","amount":-10,"softness":35}"#),
    ("rgb-split", r#"{"mode":"Directional","fringe":"RGB Split","amount":10,"softness":0}"#),
    ("directional-spectral", r#"{"mode":"Directional","amount":14,"angle":30,"softness":60}"#),
    ("extreme", r#"{"amount":128,"softness":100,"clearCenter":0,"falloff":0.5}"#),
];

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out_dir = args.first().map(PathBuf::from).unwrap_or_else(|| PathBuf::from("target/preview"));
    let size = |i: usize, default: usize| args.get(i).and_then(|a| a.parse().ok()).unwrap_or(default);
    let (w, h) = (size(1, 1200), size(2, 800));
    std::fs::create_dir_all(&out_dir).expect("create output folder");
    let scene = scene(w, h);
    save(&out_dir.join("scene.png"), &scene, w, h);
    for (name, params) in PRESETS {
        let start = std::time::Instant::now();
        let out = host::apply::<ChromaticAberration>(&scene, w, h, Format::RGB8, params, Bands::PHOTOCRAFT).expect("filter runs");
        println!("{name:<22} {:>6.0} ms  {params}", start.elapsed().as_secs_f64() * 1000.0);
        save(&out_dir.join(format!("{name}.png")), &out, w, h);
    }
    println!("written to {}", out_dir.display());
}

/// A night scene full of the edges chromatic aberration shows up on: soft bokeh lights, a bright
/// ring, thin lines and a frame of white bars, over a deep blue gradient.
fn scene(w: usize, h: usize) -> Vec<f32> {
    // Laid out for 1200 × 800 and scaled to the requested width.
    let k = w as f32 / 1200.0;
    let mut seed = 0x2545_f491_u32;
    let mut rand = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed as f32 / u32::MAX as f32
    };
    let lights: Vec<[f32; 6]> = (0..70)
        .map(|_| {
            let hue = rand();
            let a = hue * std::f32::consts::TAU;
            let colour = [0.5 + 0.5 * a.cos(), 0.5 + 0.5 * (a + 2.1).cos(), 0.5 + 0.5 * (a + 4.2).cos()];
            [rand() * w as f32, rand() * h as f32, (6.0 + rand() * 28.0) * k, 0.6 + colour[0] * 0.4, 0.6 + colour[1] * 0.4, 0.6 + colour[2] * 0.4]
        })
        .collect();
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    let bar_size = ((40.0 * k) as usize).max(1);
    let mut px = Vec::with_capacity(w * h * 3);
    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let t = fy / h as f32;
            let mut c = [0.02 + 0.05 * t, 0.03 + 0.06 * t, 0.10 + 0.12 * (1.0 - t)];
            let mut add = |col: [f32; 3], k: f32| {
                for i in 0..3 {
                    c[i] = c[i] + (col[i] - c[i]) * k.clamp(0.0, 1.0);
                }
            };
            for l in &lights {
                let d = ((fx - l[0]).powi(2) + (fy - l[1]).powi(2)).sqrt();
                add([l[3], l[4], l[5]], (l[2] - d).clamp(0.0, 1.5) / 1.5 * 0.85);
            }
            // A bright ring around the centre.
            let r = ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt();
            add([1.0, 1.0, 1.0], 1.0 - ((r - 300.0 * k).abs() - 6.0 * k).clamp(0.0, 1.0));
            // Thin spokes out from the centre.
            let a = (fy - cy).atan2(fx - cx);
            let spoke = ((a * 8.0).sin().abs() * r).clamp(0.0, 2.0);
            if r > 60.0 * k {
                add([0.95, 0.95, 1.0], 1.0 - spoke);
            }
            // White bars along the frame edges.
            let edge = bar_size * 3 / 4;
            let bar = (x / bar_size).is_multiple_of(2) && (y < edge || y >= h - edge) || (y / bar_size).is_multiple_of(2) && (x < edge || x >= w - edge);
            if bar {
                add([1.0, 1.0, 1.0], 1.0);
            }
            px.extend_from_slice(&c);
        }
    }
    px
}

fn save(path: &Path, px: &[f32], w: usize, h: usize) {
    let file = BufWriter::new(File::create(path).expect("create png"));
    let mut enc = png::Encoder::new(file, w as u32, h as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let bytes: Vec<u8> = px.iter().map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8).collect();
    enc.write_header().and_then(|mut wr| wr.write_image_data(&bytes)).expect("write png");
}
