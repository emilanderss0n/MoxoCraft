# Developing MoxoCraft

MoxoCraft's filters are PhotoCraft plug-ins: WebAssembly modules that PhotoCraft runs in a sandbox
(its plug-in ABI v1, described in PhotoCraft's `docs/plugins.md`). PhotoCraft cuts the layer into
bands, hands each band to the plug-in with the dialog's settings, and writes back what the plug-in
returns. Plug-ins get no file access, no network and no windows of their own; PhotoCraft draws the
settings dialog from each plug-in's manifest. It runs them in wasmi, an interpreter roughly 100×
slower than normal compiled code, and stops a run after 60 seconds.

The LUT files, where they come from and how to add or remove them are covered in
[`luts.md`](luts.md).

## Common commands

```text
./build.ps1                                                      build every plug-in into dist/
cargo test                                                       run all the tests
cargo run --release -p moxocraft-chromatic-aberration --example preview   Chromatic Aberration previews
cargo run --release -p moxocraft-snow --example preview-snow     Snow previews
cargo run --release -p moxocraft-floating-particles --example preview-floating-particles   Floating Particles previews
cargo run --release -p moxocraft-lut-preview -- photo.png        every LUT on a photo
cargo run --release -p moxocraft-bench -- dist/<plugin>.wasm '<settings JSON>'   measure speed
./tools/release.ps1 -Version <x.y.z> -NotesFile ../notes.md -DryRun   try a release
```

## Building

You need Rust; `build.ps1` adds the WebAssembly target itself. It builds every crate in
`filters/` into a `.wasm` in `dist/` and copies the LUTs' licenses, sources and changelog into
`dist/luts/`, which must travel with the plug-ins. `dist/` and `target/` are output only: the
build replaces what is in them.

## Tests

`cargo test` runs everything. The tests feed images through `moxocraft_sdk::host`, which cuts
them into bands exactly as PhotoCraft does, so they check band seams, edges and transparency
without PhotoCraft running.

## Previews

- `cargo run --release -p moxocraft-chromatic-aberration --example preview [out-dir] [width height]`
  renders a test scene and several looks into `target/preview/`.
- `cargo run --release -p moxocraft-snow --example preview-snow [out-dir] [photo.png [depth.png] | 6000x4000]`
  renders Snow at several settings into `target/preview-snow/`: on a test street scene with its
  own depth map (1500 × 1000 unless a size is given; full size shows the flakes' shapes), or on a
  PNG of your own. It also puts together the layered results the README describes: near and far
  layers, and a depth map turned into snow.
- `cargo run --release -p moxocraft-floating-particles --example preview-floating-particles [out-dir] [photo.png [depth.png] | 6000x4000]`
  does the same for Floating Particles into `target/preview-floating-particles/`, on a test scene
  of a figure among backlit trees.
- `cargo run --release -p moxocraft-lut-preview -- photo.png` renders every LUT of all three LUT
  plug-ins on a PNG of your own (or a test card) into `target/preview-lut/`, a folder per plug-in.

The LUT previews match PhotoCraft's output exactly; the Chromatic Aberration, Snow and Floating
Particles previews match it to within one level in 255.

## Measuring speed

`moxocraft-bench` runs a built plug-in in wasmi, the interpreter PhotoCraft uses, on a spread of
bands from a 24-megapixel RGBA canvas (PhotoCraft always sends an alpha channel), and estimates a
whole-image run on 1, 4 and 16 threads. Check it after changing a pixel loop.

It also reports instructions per pixel (PhotoCraft's "fuel"). That count is the same on every run,
but it doesn't reliably track time: wasmi charges for instructions it later optimizes away, so a
change can use fewer instructions and still run slower. Confirm speed changes with timings,
comparing against an unchanged build in the same session.

## Project layout

```text
crates/moxocraft-sdk/          shared plug-in plumbing every filter uses
crates/moxocraft-lut-core/     the LUT engine the three LUT plug-ins share
crates/moxocraft-particles/    the particle engine the overlay plug-ins share
filters/chromatic-aberration/  MoxoCraft - Chromatic Aberration
filters/floating-particles/    MoxoCraft - Floating Particles
filters/lut/                   MoxoCraft - Film LUTs      (its LUTs are in luts/film-luts)
filters/lut-cinematic/         MoxoCraft - Cinematic LUTs (luts/cinematic-luts)
filters/lut-creative/          MoxoCraft - Creative LUTs  (luts/creative-luts)
filters/snow/                  MoxoCraft - Snow
luts/                          the LUT files, licenses, sources and changelog (see luts.md)
tools/import-luts/             Python: imports the third-party LUTs into luts/ (see luts.md)
tools/bake-luts/               Python: makes the Film category from spektrafilm (see luts.md)
tools/lut-preview/             renders every LUT on a photo, as PNGs
tools/bench/                   runs a built .wasm in PhotoCraft's interpreter and reports its cost
tools/release.ps1              publishes a GitHub release (see Releasing)
build.ps1                      builds every filter into dist/
```

Dependencies only point one way: filters depend on the SDK and at most one engine, the engines
depend on the SDK, and the SDK depends on nothing. The tools depend on the crates and plug-ins
they work with; no plug-in depends on a tool.

### `moxocraft-sdk`

A filter implements the `Filter` trait (its manifest and an `apply` function) and calls
`export_filter!`. That is its whole connection to PhotoCraft.

| File | What it does |
|---|---|
| `lib.rs` | the filter-facing API: `Filter`, `Error`, `try_vec`, `export_filter!` |
| `image.rs` | `Image`, `Format`, `ColorMode`, `Geometry`: the band and where it sits in the document |
| `params.rs` | `Params`: reading the dialog's settings from PhotoCraft's JSON |
| `abi.rs` | the four exports PhotoCraft calls; the only `unsafe` code |
| `json.rs` | a small JSON reader and writer, so plug-ins carry no extra dependency |
| `host.rs` | a stand-in for PhotoCraft that cuts an image into bands as PhotoCraft does, for tests, previews and the benchmark |

### `moxocraft-lut-core`

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
| `bake.rs` | build | reads a pack's folder, orders and names the LUTs, writes the tables, their index and the manifest |
| `cube.rs` | build | reads `.cube` files and bakes in the DaVinci conversion where asked |
| `color.rs` | both | sRGB and DaVinci Wide Gamut / Intermediate color math |
| `lut.rs` | both | the baked table format and `Lut::lookup` |
| `render.rs` | run | applies the chosen LUT to a band, blended by Intensity |
| `lib.rs` | run | `lut_pack!`, which puts a plug-in together from its baked files |
| `testing.rs` | tests | `pack_tests!`, the tests every LUT plug-in runs on its own LUTs |

`lut_pack!` is a macro because the baked files sit in each plug-in crate's own `OUT_DIR`, and
`include!` can only reach them from code written in that crate. Where each LUT setting lives is
in [`luts.md`](luts.md#where-each-fact-lives).

### `moxocraft-particles`

The overlay plug-ins (Snow and Floating Particles) draw particles. Each plug-in decides what its particles look
like; the engine places them, draws them and lays them over the band. Positions and sizes are
measured in units of the picture's longer side, so a plug-in looks the same at any size and on
PhotoCraft's reduced live preview, which tells plug-ins the reduced size.

| File | What it does |
|---|---|
| `scatter.rs` | where the particles sit (depth slices of hashed grid cells), and the work budget that keeps the heaviest settings within PhotoCraft's time limit |
| `lens.rs` | how depth changes a particle's size and blur |
| `draw.rs` | `Dab`, one particle's shape (a soft disc or oval, or a streak along a `Motion` path), and `Coverage`, the band-sized buffer it's drawn into |
| `depth.rs` | the Depth list every particle plug-in shares, and reading a depth map |
| `render.rs` | one band from start to finish |
| `hash.rs` | random numbers that come out the same in every band |

A plug-in implements `Population` (how many particles there are at each depth, how far they
reach and roughly how much drawing they take) and calls `render` with a function that turns each
`Spot` from `Scatter::for_each` into a `Dab`. Particle manifests ask for `"area": "canvas"`, so
the plug-ins also run on an empty layer, and for no overlap, since no band needs its neighbors'
pixels.

Two rules keep bands seamless: every position is worked out in document pixels the same way in
every band, and `Population::reach` must never be smaller than how far a particle really
reaches, or a band skips a particle that pokes into it. The seam tests run long streaks across
narrow bands to catch that.

PhotoCraft divides settings named like pixel lengths (`size`, `blur`, `speed`, `radius`…) by the
preview's reduction, so particle plug-ins must not use those names; their tests check it.

### The filters

- **Chromatic Aberration**: `settings.rs` reads the dialog, `field.rs` works out how far each
  pixel's colors move, `taps.rs` decides where each pixel reads and how many reads the image can
  afford, and `render.rs` does the pixel passes.
- **Snow**: `settings.rs` reads the dialog, and `flake.rs` decides what a flake looks like and
  holds Snow's `Population`.
- **Floating Particles**: `settings.rs` reads the dialog and turns Hue, Saturation and
  Brightness into a color (PhotoCraft's dialogs have no color picker), and `particle.rs` decides
  what a particle looks like, gathers particles into clouds for Clustering and holds the
  plug-in's `Population`.

In each, `lib.rs` holds the manifest and the `Filter` entry point.

## Adding a filter

1. Copy `filters/chromatic-aberration` (or `filters/snow`, for a particle filter) to a new folder
   in `filters/` and rename the package in its `Cargo.toml`.
2. Give the manifest a new `id` and a name starting "MoxoCraft - ", implement
   `moxocraft_sdk::Filter`, and call `export_filter!`.
3. Add the folder to the workspace `members` in the root `Cargo.toml`.
4. Add its `.wasm` file name to `$Plugins` at the top of `tools/release.ps1` to ship it.

`build.ps1` picks up every crate in `filters/` by itself.

## Releasing

`tools/release.ps1` makes a GitHub release from a version number.

1. Set the new version in the root `Cargo.toml` (`[workspace.package]`); every plug-in's manifest
   takes it from there. Run `cargo build` so `Cargo.lock` picks it up, then commit and push to
   main.
2. Write the release notes in a file outside the repo folder (the script refuses to run with
   stray files in it). Describe the whole release as it now stands: every plug-in, current
   install steps and full credits, not only what changed. Keep the Credits section with the
   spektrafilm CC BY-SA 4.0 credit and the trademark note. The latest release's notes are a good
   starting point; save them from Git Bash, as Windows PowerShell garbles characters like ›:

   ```bash
   gh release view --json body --jq .body > ../notes.md
   ```

3. Try the release without publishing, then publish once that looks right:

   ```powershell
   ./tools/release.ps1 -Version <x.y.z> -NotesFile ../notes.md -DryRun
   ./tools/release.ps1 -Version <x.y.z> -NotesFile ../notes.md
   ```

The script stops with a message if main has changes or isn't pushed, the version doesn't match,
the release already exists, the notes are missing the spektrafilm credit or still name an older
zip, the Chromatic Aberration guide links to a file that won't be in the zip, or the README links
to a file that isn't committed. Otherwise it runs the tests, rebuilds the plug-ins from scratch and
packs `MoxoCraft-v<x.y.z>.zip` (the plug-ins, the Chromatic Aberration guide and its pictures, and
the LUTs' licenses, sources and changelog). The README and its pictures aren't in the zip, as
people read them on GitHub. It writes `SHA256SUMS.txt` with Unix line endings, since `sha256sum -c` rejects
Windows ones, uploads the zip, the `.wasm` files and the checksums as a release of the commit it
built, then downloads them again and checks each one. `-DryRun` stops before uploading and prints
the folder holding the files.

Three lists at the top of the script say what ships: `$Plugins` (the `.wasm` files `build.ps1`
built) and `$LutNotices` (the `.txt` files in `luts/`), which must match what is on disk, and
`$Docs` (the guide and its pictures), which must hold everything the guide links to. The guide
links to the README and the developer docs on GitHub instead, as those aren't in the zip.
