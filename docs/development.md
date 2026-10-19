# Developing MoxoCraft

MoxoCraft's filters are PhotoCraft plug-ins: WebAssembly modules that PhotoCraft runs in a sandbox
(its plug-in ABI v1, described in PhotoCraft's `docs/plugins.md`). PhotoCraft cuts the layer into
bands, hands each band to the plug-in with the dialog's settings, and writes back what the plug-in
returns. Plug-ins get no file access, no network and no windows of their own; PhotoCraft draws the
settings dialog from each plug-in's manifest.

The LUTs, where they come from and how to add or remove them are covered in
[`luts.md`](luts.md).

## Building

You need Rust; `build.ps1` adds the WebAssembly target itself.

```powershell
./build.ps1
```

This builds every crate in `filters/` into a `.wasm` in `dist/` and copies the LUTs' licences,
sources and changelog into `dist/luts/`, which must travel with the plug-ins. `dist/` and
`target/` are output only: the build replaces what is in them, so never edit files there.

## Project layout

```text
crates/moxocraft-sdk/          shared plug-in plumbing every filter uses
crates/moxocraft-lut-core/     the LUT engine the three LUT plug-ins share
crates/moxocraft-particles/    the particle engine the overlay plug-ins (Snow) share
filters/chromatic-aberration/  MoxoCraft - Chromatic Aberration
filters/lut/                   MoxoCraft - Film LUTs      (its LUTs are in luts/film-luts)
filters/lut-cinematic/         MoxoCraft - Cinematic LUTs (luts/cinematic-luts)
filters/lut-creative/          MoxoCraft - Creative LUTs  (luts/creative-luts)
filters/snow/                  MoxoCraft - Snow
luts/                          the LUT files, one folder per LUT plug-in, plus their licences,
                               sources and changelog (see luts.md)
tools/import-luts/             Python: imports the third-party LUTs into luts/ (see luts.md)
tools/bake-luts/               Python: makes the Film category from spektrafilm (see luts.md)
tools/lut-preview/             renders every LUT on a photo, as PNGs
tools/bench/                   runs a built .wasm in PhotoCraft's interpreter and reports its cost
tools/release.ps1              publishes a GitHub release (see Releasing)
build.ps1                      builds every filter into dist/
```

Dependencies only point one way: filters depend on `moxocraft-lut-core`, `moxocraft-particles`
and `moxocraft-sdk`, the two engines depend on `moxocraft-sdk`, and the SDK depends on nothing. The tools depend
on the crates and plug-ins they work with; no plug-in depends on a tool.

### `moxocraft-sdk`: what every filter is built on

A filter implements the `Filter` trait (its manifest and an `apply` function) and calls
`export_filter!`. That is its whole connection to PhotoCraft.

| File | What it does |
|---|---|
| `lib.rs` | the filter-facing API: `Filter`, `Error`, `try_vec`, `export_filter!`, re-exports |
| `image.rs` | `Image`, `Format`, `ColorMode`, `Geometry`: the band and where it sits in the document |
| `params.rs` | `Params`: reading the dialog's settings from PhotoCraft's JSON |
| `abi.rs` | the four exports PhotoCraft calls; the only `unsafe` code. Filters never call it |
| `json.rs` | a small JSON reader and writer, so plug-ins carry no extra dependency |
| `host.rs` | a stand-in for PhotoCraft that cuts an image into bands exactly as PhotoCraft does; used by tests, previews and the benchmark |

### `moxocraft-lut-core`: everything the LUT plug-ins share

Each LUT plug-in crate is configuration only: its `build.rs` holds a `Pack` (its folder under
`luts/`, manifest id, menu name and description) and its `lib.rs` is `lut_pack!();`. The work
happens in two stages:

```text
build time (the plug-in's build.rs, native code)       run time (inside the .wasm)
  luts/<pack>/  ──bake.rs──▶  OUT_DIR/luts.bin   ──┐
  (.cube files,  cube.rs     OUT_DIR/luts.rs    ──┼─ lut_pack! ─▶ LutFilter ─▶ render.rs ─▶ lut.rs
   *.txt config) color.rs    OUT_DIR/manifest.json┘
```

| File | Runs | What it does |
|---|---|---|
| `bake.rs` | build | reads a pack's folder and config files, orders and names the LUTs, writes the baked tables, their index (`LUTS`) and the manifest |
| `cube.rs` | build | reads `.cube` files; full-precision maths on them, such as baking in the DaVinci conversion |
| `color.rs` | both | sRGB and DaVinci Wide Gamut / Intermediate colour maths |
| `lut.rs` | both | the baked table format (shared by `bake.rs` and the plug-in) and `Lut::lookup` |
| `render.rs` | run | picks the LUT the dialog chose and applies it to a band, blended by Intensity |
| `lib.rs` | run | `lut_pack!`, which puts a plug-in together from its baked files |
| `testing.rs` | tests | `pack_tests!`, the tests every LUT plug-in runs on its own LUTs |

`lut_pack!` is a macro because the baked files sit in each plug-in crate's own `OUT_DIR`, and
`include!` can only reach them from code written in that crate.

### `moxocraft-particles`: everything the particle plug-ins share

The overlay plug-ins draw particles (snowflakes now; raindrops, dust and embers next). Each one
decides what its particles look like; the engine places them, draws them and lays them over the
band. Everything is measured in units of the picture's longer side, so a plug-in looks the same
at any size and on PhotoCraft's reduced live preview, which tells plug-ins the reduced size.

| File | What it does |
|---|---|
| `scatter.rs` | where the particles sit: depth slices from the lens (0) to far away (1), each a grid of cells whose particles come from a hash of the cell, so every band agrees on every particle; also the work budget that keeps the heaviest settings inside PhotoCraft's time limit |
| `lens.rs` | how a particle's depth changes its size and blur, focused at Subject Depth |
| `draw.rs` | `Dab`, one particle's shape (a soft disc, round or oval, or a streak along a `Motion` path that can curve and flicker; fainter when spread out), and `Coverage`, the band-sized buffer the particles are drawn into |
| `depth.rs` | the Depth list every particle plug-in shares, and reading a depth map |
| `render.rs` | one band from start to finish: check it, draw, then lay the particles over it (or replace it, with a depth map) |
| `hash.rs` | random-looking numbers that come out the same in every band |

A plug-in implements `Population` (how many particles there are at each depth, how far they
reach and roughly how much drawing they take) and calls `render` with a function that turns each
`Spot` from `Scatter::for_each` into a `Dab`. Manifests ask for `"area": "canvas"`, so the
plug-ins also run on an empty layer, and for no overlap: no band needs its neighbours' pixels.

Two rules keep bands seamless: every position is worked out in document pixels the same way in
every band, and `Population::reach` must never be smaller than how far a particle really
reaches, or a band skips a particle that pokes into it. The seam tests run long streaks across
narrow bands to catch that.

PhotoCraft divides settings named like pixel lengths (`size`, `blur`, `speed`, `radius`…) by the
preview's reduction, so particle plug-ins must not use those names; their tests check it.

### `filters/snow`

`lib.rs` holds the manifest and the `Filter` entry point, `settings.rs` reads the dialog, and
`flake.rs` decides what a flake looks like (an oval clump of up to three blobs when sharp, a
drifting, curving, flickering streak when moving) and holds Snow's `Population`.

### `filters/chromatic-aberration`

`lib.rs` holds the manifest and the `Filter` entry point; the rest follows a band through the
filter: `settings.rs` reads the dialog, `field.rs` works out how far each pixel's colours move,
`taps.rs` decides where each pixel reads and how many reads the image can afford, and `render.rs`
does the pixel passes.

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
cargo run --release -p moxocraft-snow --example preview-snow [out-dir] [photo.png [depth.png] | 6000x4000]
```

Renders Snow at several settings into `target/preview-snow/`: on a test street scene with its own
depth map (1500 × 1000, or the size given, to judge flakes at full size), or on a PNG of your own
(with its depth map, if you have one). It also builds the two
layered results the README describes, near and far layers and a depth map turned into snow.
These match PhotoCraft's output to within one level in 255.

```bash
cargo run --release -p moxocraft-lut-preview -- photo.png
```

Renders every LUT of all three LUT plug-ins on a PNG of your own (or a test card, without one)
into `target/preview-lut/`, a folder per plug-in. These match PhotoCraft's output exactly.

```bash
cargo run --release -p moxocraft-bench -- dist/moxocraft_chromatic_aberration.wasm "{\"amount\":28}"
```

Measures a built filter in wasmi, the interpreter PhotoCraft runs plug-ins in. It is roughly 100×
slower than normal compiled code, and PhotoCraft stops a plug-in run after 60 seconds, so check
this on a 24-megapixel canvas (the default) after changing a pixel loop. The instructions-per-pixel
figure doesn't vary between runs, so it shows even small changes.

## Adding a filter

1. Copy `filters/chromatic-aberration` to a new folder in `filters/` and rename the package in
   its `Cargo.toml`.
2. Give the manifest a new `id` and a name starting "MoxoCraft - ", implement
   `moxocraft_sdk::Filter`, and call `export_filter!`.
3. Add the folder to the workspace `members` in the root `Cargo.toml`.
4. Add its `.wasm` file name to `$Plugins` at the top of `tools/release.ps1` to ship it.

`build.ps1` picks up every crate in `filters/` by itself.

## Releasing

`tools/release.ps1` makes a GitHub release from a version number. First set the new version in
the root `Cargo.toml` (`[workspace.package]`); every plug-in's manifest takes it from there. Run
`cargo build` so `Cargo.lock` picks it up, then commit and push to main.

Write the release notes in a file outside the repo folder (the script refuses to run with stray
files in it). The previous release's notes are a good starting point; keep the Credits section
with the spektrafilm CC BY-SA 4.0 credit and the trademark note. Save it from Git Bash, as Windows
PowerShell garbles characters like › on the way into the file:

```bash
gh release view v0.1.0 --json body --jq .body > ../notes.md
```

Then try it without publishing, and publish once that looks right:

```powershell
./tools/release.ps1 -Version 0.2.0 -NotesFile ../notes.md -DryRun
./tools/release.ps1 -Version 0.2.0 -NotesFile ../notes.md
```

The script stops with a message if main has changes, isn't pushed, the version doesn't match, the
release already exists, or the notes are missing the spektrafilm credit or still name an older
zip. Otherwise it runs the tests, rebuilds the plug-ins from scratch with `build.ps1` and packs
`MoxoCraft-v0.2.0.zip` (the plug-ins, the README and the LUTs' licences, sources and changelog in
`luts/`). It also writes `SHA256SUMS.txt` with Unix line endings, since `sha256sum -c` rejects
Windows ones. Then it uploads the zip, the `.wasm` files and the checksums as a release of the
commit it built, downloads them again and checks each one. `-DryRun` stops before uploading and
prints the folder holding the files.

Two lists at the top of the script say what ships, and it stops until each matches what is on
disk: `$Plugins` (the `.wasm` files `build.ps1` built) and `$LutNotices` (the `.txt` files in
`luts/`).
