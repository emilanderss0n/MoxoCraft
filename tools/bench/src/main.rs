//! Measures a built plug-in in wasmi, the WebAssembly interpreter PhotoCraft runs plug-ins in.
//!
//!   cargo run --release -p moxocraft-bench -- <plugin.wasm> [params-json] [width height]
//!
//! Runs a spread of bands from an RGB 8-bit canvas (6000 × 4000 by default), cut and padded the way
//! PhotoCraft cuts them, and reports instructions per pixel (PhotoCraft's "fuel") and time. From
//! those it estimates a whole-image run, to compare against PhotoCraft's 60-second limit.

use std::time::Instant;

use moxocraft_sdk::{Format, Json};
use wasmi::{Config, Engine, Linker, Memory, Module, Store, TypedFunc};

type FilterFn = TypedFunc<(i32, i32, i32, i32, i32, i32, i32, i32), i32>;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(path) = args.first() else {
        eprintln!("usage: moxocraft-bench <plugin.wasm> [params-json] [width height]");
        std::process::exit(2);
    };
    let params = args.get(1).map_or("{}", String::as_str);
    let size = |i: usize, d: usize| args.get(i).and_then(|a| a.parse().ok()).unwrap_or(d);
    let (cw, ch) = (size(2, 6000), size(3, 4000));
    let Some(Json::Object(fields)) = Json::parse(params) else { return Err("params must be a JSON object".into()) };

    let mut config = Config::default();
    config.consume_fuel(true);
    let engine = Engine::new(&config);
    let module = Module::new(&engine, &std::fs::read(path)?)?;
    let overlap = {
        let (mut store, instance, memory) = instantiate(&engine, &module)?;
        let packed = instance.get_typed_func::<(), i64>(&store, "pc_manifest")?.call(&mut store, ())? as u64;
        let mut text = vec![0u8; (packed >> 32) as usize];
        memory.read(&store, (packed & 0xffff_ffff) as usize, &mut text)?;
        moxocraft_sdk::host::manifest_overlap(std::str::from_utf8(&text)?)
    };

    // PhotoCraft's bands: one 256-pixel tile row, as many tiles across as fit in 4 MiB of f32.
    let format = Format::RGB8;
    let n = format.channels();
    let across = ((4usize << 20) / (256 * 256 * n * 4)).clamp(1, 1024) * 256;
    let mut bands = Vec::new();
    for y in (0..ch).step_by(256) {
        for x in (0..cw).step_by(across) {
            bands.push((x, y, across.min(cw - x), 256.min(ch - y)));
        }
    }
    // A spread of about eight bands, corners and centre included.
    // A stride that doesn't divide the number of bands across, so the sample moves across columns too.
    let stride = (bands.len() / 8).max(1) | 1;
    let sample: Vec<_> = bands.iter().copied().enumerate().filter(|(i, _)| i % stride == 0 || *i == bands.len() - 1).map(|(_, b)| b).collect();

    println!("{path}\n{params}\ncanvas {cw} x {ch}, {} bands of up to {across} x 256, overlap {overlap}; measuring {}", bands.len(), sample.len());
    let (mut fuel_total, mut px_total, mut secs_total) = (0u64, 0usize, 0f64);
    for &(bx, by, w, h) in &sample {
        let (iw, ih) = (w + 2 * overlap, h + 2 * overlap);
        let (x0, y0) = (bx as i64 - overlap as i64, by as i64 - overlap as i64);
        let mut data = Vec::with_capacity(iw * ih * n);
        for row in 0..ih as i64 {
            let y = (y0 + row).clamp(0, ch as i64 - 1) as f32;
            for col in 0..iw as i64 {
                let x = (x0 + col).clamp(0, cw as i64 - 1) as f32;
                // Busy detail everywhere, so no pixel gets an easy way out.
                let v = ((x * 0.37).sin() * (y * 0.23).cos() * 0.5 + 0.5).clamp(0.0, 1.0);
                data.extend_from_slice(&[v, 1.0 - v, (v * 2.0).fract()]);
            }
        }
        let mut with_image = fields.clone();
        let num = |v: f64| Json::Number(v);
        with_image.push((
            "_image".into(),
            Json::Object(vec![
                ("x".into(), num(x0 as f64)),
                ("y".into(), num(y0 as f64)),
                ("width".into(), num(iw as f64)),
                ("height".into(), num(ih as f64)),
                ("overlap".into(), num(overlap as f64)),
                ("canvasWidth".into(), num(cw as f64)),
                ("canvasHeight".into(), num(ch as f64)),
            ]),
        ));
        let text = Json::Object(with_image).to_text();

        let (mut store, instance, memory) = instantiate(&engine, &module)?;
        let alloc = instance.get_typed_func::<i32, i32>(&store, "pc_alloc")?;
        let filter: FilterFn = instance.get_typed_func(&store, "pc_filter")?;
        let bytes: Vec<u8> = data.iter().flat_map(|v| v.to_le_bytes()).collect();
        let buf = alloc.call(&mut store, bytes.len() as i32)?;
        memory.write(&mut store, buf as u32 as usize, &bytes)?;
        let pbuf = alloc.call(&mut store, text.len() as i32)?;
        memory.write(&mut store, pbuf as u32 as usize, text.as_bytes())?;
        store.set_fuel(u64::MAX / 2)?;
        let before = store.get_fuel()?;
        let start = Instant::now();
        let code =
            filter.call(&mut store, (buf, bytes.len() as i32, iw as i32, ih as i32, n as i32, format.code() as i32, pbuf, text.len() as i32))?;
        let secs = start.elapsed().as_secs_f64();
        let fuel = before - store.get_fuel()?;
        if code != 0 {
            return Err(format!("pc_filter returned {code}").into());
        }
        let budget = 50_000_000 + 4_000 * (iw * ih * n) as u64;
        println!(
            "  band at ({bx:>4}, {by:>4})  {:>6.0} instr/px  {:>6.2} s  ({:.0}% of the band's instruction budget)",
            fuel as f64 / (w * h) as f64,
            secs,
            100.0 * fuel as f64 / budget as f64
        );
        fuel_total += fuel;
        px_total += w * h;
        secs_total += secs;
    }
    let per_px = fuel_total as f64 / px_total as f64;
    let rate = fuel_total as f64 / secs_total;
    let whole = per_px * (cw * ch) as f64 / rate;
    println!("average {per_px:.0} instructions per pixel, {:.0} M instructions/s on one thread", rate / 1e6);
    println!("whole image, estimated: {whole:.0} s on one thread; {:.1} s on 16 threads; {:.1} s on 4 threads (limit 60 s)", whole / 16.0, whole / 4.0);
    Ok(())
}

fn instantiate(engine: &Engine, module: &Module) -> Result<(Store<()>, wasmi::Instance, Memory), Box<dyn std::error::Error>> {
    let mut store = Store::new(engine, ());
    store.set_fuel(u64::MAX / 2)?;
    let instance = Linker::<()>::new(engine).instantiate_and_start(&mut store, module)?;
    let memory = instance.get_memory(&store, "memory").ok_or("the module exports no memory")?;
    Ok((store, instance, memory))
}
