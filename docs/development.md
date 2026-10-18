# Developing MoxoCraft

MoxoCraft's filters are PhotoCraft plug-ins: WebAssembly modules that PhotoCraft runs in a sandbox
(its plug-in ABI v1, described in PhotoCraft's `docs/plugins.md`). PhotoCraft cuts the layer into
bands, hands each band to the plug-in with the dialog's settings, and writes back what the plug-in
returns. Plug-ins get no file access, no network and no windows of their own; PhotoCraft draws the
settings dialog from each plug-in's manifest.

## Building

You need Rust; `build.ps1` adds the WebAssembly target itself.

```powershell
./build.ps1
```

This builds every filter into `dist/` and copies the LUT licence, credits and changelog into
`dist/luts/`, which must travel with the plug-ins.

## Project layout

```text
crates/moxocraft-sdk/          shared plug-in plumbing: the PhotoCraft exports, settings JSON,
                               and a stand-in for PhotoCraft (`host`) for tests and previews
filters/chromatic-aberration/  MoxoCraft - Chromatic Aberration
filters/lut/                   MoxoCraft - LUT (build.rs bakes in the LUTs from luts/)
luts/                          the .cube files, their licence, credits and changelog
tools/bake-luts/               the recipe that bakes luts/ from spektrafilm
tools/bench/                   runs a built .wasm in the interpreter PhotoCraft uses and reports
                               the cost per pixel and the expected time for a whole image
build.ps1                      builds every filter into dist/
```

## Tests, previews and benchmarks

```bash
cargo test
```

The tests feed images through `moxocraft_sdk::host`, which cuts them into bands exactly as
PhotoCraft does, so they check band seams, edges and transparency without PhotoCraft running.

```bash
cargo run --release -p moxocraft-chromatic-aberration --example preview
```

Renders a test scene and several looks into `target/preview/` (add `out-dir width height` to
change the folder or size). These match PhotoCraft's own output to within one level in 255.

```bash
cargo run --release -p moxocraft-lut --example lut_preview -- photo.png
```

Renders every LUT on a PNG of your own (or a test card, without one) into `target/preview-lut/`.
These match PhotoCraft's output exactly.

```bash
cargo run --release -p moxocraft-bench -- dist/moxocraft_chromatic_aberration.wasm "{\"amount\":28}"
```

Measures a built filter in wasmi, the interpreter PhotoCraft runs plug-ins in. It is roughly 100×
slower than normal compiled code, and PhotoCraft stops a plug-in run after 60 seconds, so check
this on a 24-megapixel canvas (the default) after changing a pixel loop.

## Adding a filter

Copy `filters/chromatic-aberration`, give it a new manifest `id` and a name starting
"MoxoCraft - ", implement `moxocraft_sdk::Filter`, call `export_filter!`, and add it to the
workspace members in `Cargo.toml`.

## LUTs

The LUT plug-in's `build.rs` bakes every `.cube` file in `luts/` into the `.wasm` (PhotoCraft gives
plug-ins no file access) and writes the manifest, so the dialog's list always matches. Each LUT
appears under its file name, with underscores shown as spaces, in natural order.
`luts/default.txt` names the one the dialog opens on, and `luts/CREDITS.txt` is added to the
plug-in's description, so the credit travels inside the `.wasm`. Everything must fit PhotoCraft's
32 MB plug-in limit: roughly 18 LUTs of 65 points, or over 100 of 33.

**Adding a LUT.** Drop a 3D `.cube` file into `luts/` and run `./build.ps1`. Check that its licence
allows redistribution first, and add its credit to `luts/CREDITS.txt`.

**DaVinci Resolve LUTs.** LUTs made inside Resolve's *DaVinci Wide Gamut / DaVinci Intermediate*
colour space, rather than for normal images, come out milky and break saturated colours into flat
blocks when applied straight to a photo. List those in a `luts/davinci-intermediate.txt` file (one
file name per line): the build then bakes the conversion into that space and back into the LUT, so
it works on normal photos at full speed.

**Re-baking the film stocks.** The included LUTs come from
[spektrafilm](https://github.com/andreavolpato/spektrafilm) (CC BY-SA 4.0).
[`tools/bake-luts/bake_spektrafilm.py`](../tools/bake-luts/bake_spektrafilm.py) holds the exact
recipe (stocks, prints and settings) and the one-time setup of spektrafilm in its own Python
environment. It rewrites the `.cube` files and `luts/CHANGELOG.txt`; then run `./build.ps1`. Slide
films (Kodachrome, Ektachrome, Provia, Velvia) are left out for now: spektrafilm's LUT tool always
sends film through a print, while spektrafilm itself shows slides scanned directly.

Licence conditions to keep: `luts/SPEKTRAFILM_LICENSE.txt` must stay byte-identical to upstream
(`luts/.gitattributes` stops Git changing its line endings), every change to the LUTs goes in
`luts/CHANGELOG.txt`, spektrafilm's name mustn't be used in product branding, and the LUTs mustn't
be sold as a LUT pack.
