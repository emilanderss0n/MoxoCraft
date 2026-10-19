//! Renders every LUT of MoxoCraft's LUT plug-ins on an image, as PNGs.
//!
//!   cargo run --release -p moxocraft-lut-preview [photo.png] [out-dir]
//!
//! Without a photo it uses a test card: grey ramp, color sweep, skin tones and a sky gradient.
//! Writes `original.png` and one PNG per LUT into a folder per plug-in under `out-dir` (default
//! `target/preview-lut`). Bands are fed the same way PhotoCraft feeds them, so these match what the
//! plug-ins do in the app.

use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use moxocraft_lut_core::LutInfo;
use moxocraft_sdk::host::{self, Bands};
use moxocraft_sdk::{Filter, Format};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (px, w, h) = match args.first().filter(|a| a.to_ascii_lowercase().ends_with(".png")) {
        Some(path) => load(Path::new(path)),
        None => test_card(1200, 800),
    };
    let out_dir = args.iter().find(|a| !a.to_ascii_lowercase().ends_with(".png")).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("target/preview-lut"));
    std::fs::create_dir_all(&out_dir).expect("create output folder");
    save(&out_dir.join("original.png"), &px, w, h);
    render::<moxocraft_lut::LutFilter>(&out_dir.join("film-luts"), moxocraft_lut::LUTS, &px, w, h);
    render::<moxocraft_lut_cinematic::LutFilter>(&out_dir.join("cinematic-luts"), moxocraft_lut_cinematic::LUTS, &px, w, h);
    render::<moxocraft_lut_creative::LutFilter>(&out_dir.join("creative-luts"), moxocraft_lut_creative::LUTS, &px, w, h);
    println!("written to {}", out_dir.display());
}

/// One PNG per LUT of plug-in `F` in `dir`.
fn render<F: Filter>(dir: &Path, luts: &[LutInfo], px: &[f32], w: usize, h: usize) {
    std::fs::create_dir_all(dir).expect("create output folder");
    for info in luts {
        let params = format!(r#"{{"lut":"{}"}}"#, info.name);
        let start = std::time::Instant::now();
        let out = host::apply::<F>(px, w, h, Format::RGB8, &params, Bands::PHOTOCRAFT).expect("filter runs");
        println!("{:<48} {:>5.0} ms", info.name, start.elapsed().as_secs_f64() * 1000.0);
        save(&dir.join(format!("{}.png", info.name)), &out, w, h);
    }
}

/// Rows of the things a grade shows most clearly on.
fn test_card(w: usize, h: usize) -> (Vec<f32>, usize, usize) {
    let mut px = Vec::with_capacity(w * h * 3);
    for y in 0..h {
        for x in 0..w {
            let u = x as f32 / (w - 1) as f32;
            let band = y * 5 / h;
            let rgb = match band {
                // Grey ramp, black to white.
                0 => [u, u, u],
                // Full-saturation color sweep.
                1 => hue(u, 1.0, 1.0),
                // The same sweep, soft and darker.
                2 => hue(u, 0.45, 0.7),
                // Skin tones, from pale to deep.
                3 => {
                    let t = u;
                    [0.95 - 0.55 * t, 0.80 - 0.55 * t, 0.70 - 0.52 * t]
                }
                // Sky to sunset gradient.
                _ => {
                    let v = (y - 4 * h / 5) as f32 / (h / 5) as f32;
                    [0.25 + 0.7 * u * (1.0 - v * 0.5), 0.45 + 0.25 * (1.0 - u), 0.85 - 0.6 * u]
                }
            };
            px.extend_from_slice(&rgb);
        }
    }
    (px, w, h)
}

fn hue(t: f32, sat: f32, val: f32) -> [f32; 3] {
    let h = t * 6.0;
    let f = |n: f32| {
        let k = (n + h) % 6.0;
        val - val * sat * (k.min(4.0 - k).clamp(0.0, 1.0))
    };
    [f(5.0), f(3.0), f(1.0)]
}

fn load(path: &Path) -> (Vec<f32>, usize, usize) {
    let mut decoder = png::Decoder::new(File::open(path).expect("open the photo"));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().expect("read the PNG");
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).expect("decode the PNG");
    let (w, h) = (info.width as usize, info.height as usize);
    let step = info.color_type.samples();
    let mut px = Vec::with_capacity(w * h * 3);
    for p in buf[..info.buffer_size()].chunks_exact(step) {
        let rgb = if step >= 3 { [p[0], p[1], p[2]] } else { [p[0]; 3] };
        px.extend(rgb.map(|v| f32::from(v) / 255.0));
    }
    (px, w, h)
}

fn save(path: &Path, px: &[f32], w: usize, h: usize) {
    let file = BufWriter::new(File::create(path).expect("create png"));
    let mut enc = png::Encoder::new(file, w as u32, h as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let bytes: Vec<u8> = px.iter().map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8).collect();
    enc.write_header().and_then(|mut wr| wr.write_image_data(&bytes)).expect("write png");
}
