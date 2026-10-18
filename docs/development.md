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

This builds every filter into `dist/` and copies the LUTs' licences, sources and changelog into
`dist/luts/`, which must travel with the plug-ins.

## Project layout

```text
crates/moxocraft-sdk/          shared plug-in plumbing: the PhotoCraft exports, settings JSON,
                               and a stand-in for PhotoCraft (`host`) for tests and previews
crates/moxocraft-lut-core/     the LUT engine the LUT plug-ins share: lookups, baking .cube files
                               at build time, and the tests every LUT plug-in runs
filters/chromatic-aberration/  MoxoCraft - Chromatic Aberration
filters/lut/                   MoxoCraft - Film LUTs (bakes in luts/film-luts)
filters/lut-cinematic/         MoxoCraft - Cinematic LUTs (bakes in luts/cinematic-luts)
filters/lut-creative/          MoxoCraft - Creative LUTs (bakes in luts/creative-luts)
luts/                          one folder of .cube files per LUT plug-in, plus the LUTs'
                               licences, sources and changelog
tools/bake-luts/               the recipe that bakes the Film category from spektrafilm
tools/import-luts/             the recipe that imports every other LUT from its source collection
tools/lut-preview/             renders every LUT on a photo, as PNGs
tools/bench/                   runs a built .wasm in the interpreter PhotoCraft uses and reports
                               the cost per pixel and the expected time for a whole image
tools/release.ps1              makes a GitHub release (see Releasing)
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
cargo run --release -p moxocraft-lut-preview -- photo.png
```

Renders every LUT of all three LUT plug-ins on a PNG of your own (or a test card, without one)
into `target/preview-lut/`, a folder per plug-in. These match PhotoCraft's output exactly.

```bash
cargo run --release -p moxocraft-bench -- dist/moxocraft_chromatic_aberration.wasm "{\"amount\":28}"
```

Measures a built filter in wasmi, the interpreter PhotoCraft runs plug-ins in. It is roughly 100×
slower than normal compiled code, and PhotoCraft stops a plug-in run after 60 seconds, so check
this on a 24-megapixel canvas (the default) after changing a pixel loop.

## Releasing

`tools/release.ps1` makes a GitHub release from a version number. First set the new version in
two places: `version` in the root `Cargo.toml` (the LUT plug-ins take theirs from there) and the
`"version"` in the manifest in `filters/chromatic-aberration/src/lib.rs`. Run `cargo build` so
`Cargo.lock` picks it up, then commit and push to main.

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

The script stops with a message if main has changes, isn't pushed, the version doesn't match in
both places, the release already exists, or the notes are missing the spektrafilm credit or
still name an older zip. Otherwise it runs the tests, rebuilds the plug-ins from scratch and packs
`MoxoCraft-v0.2.0.zip` (the plug-ins, the README and the LUTs' licences, sources and changelog in
`luts/`). It also writes `SHA256SUMS.txt` with Unix line endings, since `sha256sum -c` rejects
Windows ones. Then it uploads the zip, the `.wasm` files and the checksums as a release of the
commit it built, downloads them again and checks each one. `-DryRun` stops before uploading and
prints the folder holding the files. If you add a `.txt` notice to `luts/`, add it to
`$LutNotices` at the top of the script too; it stops until the two match.

## Adding a filter

Copy `filters/chromatic-aberration`, give it a new manifest `id` and a name starting
"MoxoCraft - ", implement `moxocraft_sdk::Filter`, call `export_filter!`, and add it to the
workspace members in `Cargo.toml`.

## LUTs

PhotoCraft gives plug-ins no file access, so each LUT plug-in's build script bakes its folder of
`.cube` files into the `.wasm` (`moxocraft_lut_core::bake`) and writes the manifest, so the
dialog's list always matches. PhotoCraft lists at most 64 choices in a dropdown, which is why the
LUTs are spread over three plug-ins:

```text
luts/film-luts/       MoxoCraft - Film LUTs:      Film, Negative, Slide, Instant, Fujifilm Style
luts/cinematic-luts/  MoxoCraft - Cinematic LUTs: Cinematic, Moody, Vintage, Black & White
luts/creative-luts/   MoxoCraft - Creative LUTs:  Colorful, Scenic, Creative
```

Each plug-in folder holds a folder per category and three small files: `categories.txt` (the
categories in menu order), `default.txt` (the LUT the dialog opens on) and `CREDITS.txt` (added to
the plug-in's description, so the credit travels inside the `.wasm`). A LUT appears as
"Category - File name", with underscores shown as spaces, in natural order within its category.
The build stops if a plug-in goes over 64 LUTs or PhotoCraft's 32 MB module limit (about 140 LUTs
of 33 points). The Film LUTs plug-in keeps v0.1.0's id, `moxocraft.lut`, and file name, so
installing it replaces the old "MoxoCraft - LUT".

**Adding a LUT.** Drop a 3D `.cube` file into a category folder and run `./build.ps1`. Check that
its licence allows redistribution first, and add it to `tools/import-luts/import_luts.py` (so its
source, licence and changes land in `luts/SOURCES.txt`) and its credit to the plug-in's
`CREDITS.txt`. A new category also needs a line in `categories.txt`.

**DaVinci Resolve LUTs.** LUTs made inside Resolve's *DaVinci Wide Gamut / DaVinci Intermediate*
colour space, rather than for normal images, come out milky and break saturated colours into flat
blocks when applied straight to a photo. List those in the plug-in folder's
`davinci-intermediate.txt` (one `Category/File name.cube` per line): the build then bakes the
conversion into that space and back into the LUT, so it works on normal photos at full speed.

**Re-making the LUTs.** Two scripts record exactly where every LUT came from and what was done to
it, which the CC BY and CC BY-SA licences ask for:

- [`tools/bake-luts/bake_spektrafilm.py`](../tools/bake-luts/bake_spektrafilm.py) bakes the Film
  category from [spektrafilm](https://github.com/andreavolpato/spektrafilm) (CC BY-SA 4.0): the
  stocks, prints and settings, and the one-time setup of spektrafilm in its own Python
  environment. It rewrites `luts/film-luts/Film` and `luts/CHANGELOG.txt`. Slide films are left
  out there: spektrafilm's LUT tool always sends film through a print, while spektrafilm itself
  shows slides scanned directly.
- [`tools/import-luts/import_luts.py`](../tools/import-luts/import_luts.py) imports the other 132
  LUTs from their collections at pinned commits (the setup is at the top of the script). It
  converts HaldCLUT images to `.cube`, resamples anything over 33 points per axis down to 33,
  writes a header into each file naming its source, author and licence, copies the licence texts
  byte for byte, and rewrites `luts/SOURCES.txt`. Each source's licence was checked by hand; the
  notes in the script say what was taken and what was left out.

Then run `./build.ps1`.

Licence conditions to keep: `luts/SPEKTRAFILM_LICENSE.txt` must stay byte-identical to upstream
(`luts/.gitattributes` stops Git changing its line endings), every change to the spektrafilm LUTs
goes in `luts/CHANGELOG.txt`, spektrafilm's name mustn't be used in product branding, and the LUTs
mustn't be sold as a LUT pack (spektrafilm and slay both ask this). The licence texts, `SOURCES.txt`
and `CHANGELOG.txt` must ship with the plug-ins.
