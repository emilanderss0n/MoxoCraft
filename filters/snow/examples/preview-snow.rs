//! Renders the snow on a picture at a few settings, as PNGs.
//!
//!   cargo run --release -p moxocraft-snow --example preview-snow [out-dir] [photo.png [depth.png] | WIDTHxHEIGHT]
//!
//! Without a photo it draws a test scene (a figure in a snowy street at dusk) with a matching
//! depth map, 1500 × 1000 unless a size such as `6000x4000` is given (to see flakes at full size). Writes `scene.png` and `depth.png` (the inputs) and one PNG per preset into
//! `out-dir` (default `target/preview-snow`). Bands are fed the way PhotoCraft feeds them.
//!
//! The Near Only / Far Only and depth-map presets are put together the way a user would in
//! PhotoCraft: snow made on empty layers, the far layer masked off the figure (with the test
//! scene's own mask, standing in for Select › Subject; skipped for a photo, which has none), and
//! the layers laid over the picture in Normal mode.

use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use moxocraft_sdk::Format;
use moxocraft_sdk::host::{self, Bands};
use moxocraft_snow::Snow;

const PRESETS: &[(&str, &str)] = &[
    ("default", r#"{}"#),
    ("light-sharp", r#"{"amount":25,"defocus":0}"#),
    ("heavy", r#"{"amount":85,"flakeSize":60,"defocus":45}"#),
    ("dreamy", r#"{"amount":50,"flakeSize":70,"defocus":80,"subjectDepth":20}"#),
    ("falling", r#"{"amount":60,"motion":35,"angle":20}"#),
    ("falling-straight", r#"{"amount":60,"motion":35,"swirl":0}"#),
    ("swirling", r#"{"amount":60,"motion":30,"swirl":100}"#),
    ("blizzard", r#"{"amount":100,"flakeSize":45,"motion":60,"angle":55,"defocus":40}"#),
    ("cool-faded", r#"{"tint":"Cool","opacity":60}"#),
    ("variation-2", r#"{"variation":2}"#),
];

/// A picture to put snow on: pixels (RGB, 0–1), size, and optionally a depth map and a subject
/// mask (one value per pixel, 0–1).
struct Picture {
    rgb: Vec<f32>,
    width: usize,
    height: usize,
    depth: Option<Vec<f32>>,
    subject: Option<Vec<f32>>,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out_dir = args.first().map(PathBuf::from).unwrap_or_else(|| PathBuf::from("target/preview-snow"));
    std::fs::create_dir_all(&out_dir).expect("create output folder");
    let size = args.get(1).and_then(|a| a.split_once('x')).and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)));
    let pic = match args.get(1) {
        Some(_) if size.is_some() => {
            let (w, h) = size.unwrap_or((1500, 1000));
            scene(w, h)
        }
        Some(photo) => {
            let (rgb, width, height) = load(Path::new(photo));
            let depth = args.get(2).map(|d| {
                let (map, w, h) = load(Path::new(d));
                assert_eq!((w, h), (width, height), "the depth map must be the photo's size");
                map.chunks_exact(3).map(|p| p[0]).collect()
            });
            Picture { rgb, width, height, depth, subject: None }
        }
        None => scene(1500, 1000),
    };
    let (w, h) = (pic.width, pic.height);
    save(&out_dir.join("scene.png"), &pic.rgb, w, h);
    if let Some(d) = &pic.depth {
        save(&out_dir.join("depth.png"), &grey_to_rgb(d), w, h);
    }

    // PhotoCraft always sends an alpha channel, so the previews do too.
    let opaque: Vec<f32> = pic.rgb.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 1.0]).collect();
    let empty = vec![0.0f32; w * h * 4];
    let run = |pixels: &[f32], params: &str| {
        let start = std::time::Instant::now();
        let out = host::apply::<Snow>(pixels, w, h, Format::RGBA8, params, Bands::PHOTOCRAFT).expect("filter runs");
        (out, start.elapsed().as_secs_f64() * 1000.0)
    };
    for (name, params) in PRESETS {
        let (out, ms) = run(&opaque, params);
        println!("{name:<14} {ms:>6.0} ms  {params}");
        save(&out_dir.join(format!("{name}.png")), &drop_alpha(&out), w, h);
    }

    // Near and far layers, the far one masked off the subject.
    let (near, _) = run(&empty, r#"{"depth":"Near Only"}"#);
    let (far, _) = run(&empty, r#"{"depth":"Far Only"}"#);
    let mut layered = opaque.clone();
    over(&mut layered, &far, pic.subject.as_deref());
    over(&mut layered, &near, None);
    println!("{:<14}         near and far layers, far one masked off the subject", "behind-subject");
    save(&out_dir.join("behind-subject.png"), &drop_alpha(&layered), w, h);

    if let Some(d) = &pic.depth {
        let map: Vec<f32> = d.iter().flat_map(|&v| [v, v, v, 1.0]).collect();
        let (snow, ms) = run(&map, r#"{"depth":"Depth Map"}"#);
        let mut layered = opaque.clone();
        over(&mut layered, &snow, None);
        println!("{:<14} {ms:>6.0} ms  depth map turned into a snow layer, laid over the picture", "depth-map");
        save(&out_dir.join("depth-map.png"), &drop_alpha(&layered), w, h);
        save(&out_dir.join("depth-map-layer.png"), &on_grey(&snow), w, h);
    }
    println!("written to {}", out_dir.display());
}

/// Lays `layer` (RGBA, straight alpha) over `base` (opaque RGBA) in Normal mode, hidden where
/// `hide` is 1.
fn over(base: &mut [f32], layer: &[f32], hide: Option<&[f32]>) {
    for (i, (b, l)) in base.chunks_exact_mut(4).zip(layer.chunks_exact(4)).enumerate() {
        let a = l[3] * hide.map_or(1.0, |m| 1.0 - m[i]);
        for c in 0..3 {
            b[c] = l[c] * a + b[c] * (1.0 - a);
        }
    }
}

/// A test scene at dusk: a figure in the middle of a street, buildings with lit windows and a
/// row of street lamps behind, all with a depth map (white near) and the figure's mask.
fn scene(w: usize, h: usize) -> Picture {
    let k = w as f32 / 1500.0;
    let (fw, fh) = (w as f32, h as f32);
    let horizon = fh * 0.58;
    let mut rgb = Vec::with_capacity(w * h * 3);
    let mut depth = Vec::with_capacity(w * h);
    let mut subject = Vec::with_capacity(w * h);
    // Buildings: left edge, width, height above the horizon.
    let buildings = [(0.0, 0.16, 0.42), (0.15, 0.12, 0.30), (0.26, 0.10, 0.36), (0.62, 0.14, 0.33), (0.75, 0.11, 0.27), (0.85, 0.15, 0.40)];
    let lamps = [0.08, 0.31, 0.69, 0.93];
    for y in 0..h {
        for x in 0..w {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let (u, v) = (px / fw, py / fh);
            // Dusk sky, then the street fading toward the camera.
            let mut c = if py < horizon {
                let t = py / horizon;
                [0.16 + 0.22 * t, 0.20 + 0.20 * t, 0.34 + 0.12 * t]
            } else {
                let t = (py - horizon) / (fh - horizon);
                [0.30 - 0.12 * t, 0.31 - 0.12 * t, 0.36 - 0.12 * t]
            };
            let mut d = if py < horizon { 0.0 } else { 0.12 + 0.88 * ((py - horizon) / (fh - horizon)).powf(1.5) };
            for &(left, width, tall) in &buildings {
                if u >= left && u < left + width && py < horizon && py > horizon - tall * fh {
                    c = [0.10, 0.11, 0.15];
                    d = 0.12;
                    // Lit windows.
                    let (wx, wy) = (((u - left) * fw / (26.0 * k)).fract(), ((horizon - py) / (32.0 * k)).fract());
                    if wx > 0.35 && wx < 0.75 && wy > 0.3 && wy < 0.75 && ((u * 37.0).floor() as i32 + (py / (32.0 * k)) as i32) % 3 != 0 {
                        c = [0.85, 0.66, 0.36];
                    }
                }
            }
            // Street lamps: a post and a warm glow.
            for &lx in &lamps {
                let (dx, dy) = (px - lx * fw, py - (horizon - 170.0 * k));
                if dx.abs() < 2.5 * k && py > horizon - 170.0 * k && py < horizon + 10.0 * k {
                    c = [0.06, 0.06, 0.07];
                    d = 0.14;
                }
                let glow = (-(dx * dx + dy * dy) / (2.0 * (22.0 * k).powi(2))).exp();
                for (ch, lamp) in c.iter_mut().zip([1.0, 0.78, 0.45]) {
                    *ch += (lamp - *ch) * glow;
                }
            }
            // The figure: a coat and a head, a little left of center.
            let (cx, top) = (0.46 * fw, horizon - 40.0 * k);
            let head = ((px - cx).powi(2) + (py - top).powi(2)).sqrt() < 34.0 * k;
            let body_w = 70.0 * k * (0.75 + 0.35 * ((py - top) / (fh - top)).clamp(0.0, 1.0));
            let body = py > top + 28.0 * k && (px - cx).abs() < body_w;
            let s = if head || body { 1.0 } else { 0.0 };
            if s > 0.0 {
                c = if head { [0.55, 0.40, 0.33] } else { [0.30, 0.10, 0.10] };
                d = 0.78;
            }
            rgb.extend_from_slice(&c);
            depth.push(d);
            subject.push(s);
            let _ = v;
        }
    }
    Picture { rgb, width: w, height: h, depth: Some(depth), subject: Some(subject) }
}

fn drop_alpha(px: &[f32]) -> Vec<f32> {
    px.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect()
}

/// A snow layer (RGBA) shown over mid gray, so its flakes and their transparency are visible.
fn on_grey(px: &[f32]) -> Vec<f32> {
    px.chunks_exact(4).flat_map(|p| [0, 1, 2].map(|c| p[c] * p[3] + 0.25 * (1.0 - p[3]))).collect()
}

fn grey_to_rgb(v: &[f32]) -> Vec<f32> {
    v.iter().flat_map(|&g| [g, g, g]).collect()
}

/// Reads a PNG as RGB, 0–1.
fn load(path: &Path) -> (Vec<f32>, usize, usize) {
    let mut decoder = png::Decoder::new(File::open(path).expect("open png"));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().expect("read png");
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).expect("decode png");
    let (w, h) = (info.width as usize, info.height as usize);
    let n = info.color_type.samples();
    let rgb = buf[..w * h * n]
        .chunks_exact(n)
        .flat_map(|p| match n {
            1 | 2 => [p[0]; 3],
            _ => [p[0], p[1], p[2]],
        })
        .map(|b| f32::from(b) / 255.0)
        .collect();
    (rgb, w, h)
}

fn save(path: &Path, px: &[f32], w: usize, h: usize) {
    let file = BufWriter::new(File::create(path).expect("create png"));
    let mut enc = png::Encoder::new(file, w as u32, h as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let bytes: Vec<u8> = px.iter().map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8).collect();
    enc.write_header().and_then(|mut wr| wr.write_image_data(&bytes)).expect("write png");
}
