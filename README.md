# MoxoCraft

Filters for [PhotoCraft](https://github.com/storytold/photocraft), the open-source Rust image
editor. Each filter is a PhotoCraft plug-in: a sandboxed WebAssembly file that works the same on
Windows, macOS, Linux and PhotoCraft's web version. They all appear under **Filter › Plug-ins**
with names starting "MoxoCraft -", each with PhotoCraft's settings dialog and live preview;
nothing changes in the document until you press OK.

## Filters

### MoxoCraft - Chromatic Aberration

Splits red, green and blue apart the way a real lens does, from a subtle lens-edge fringe to a
full rainbow smear or a hard RGB-split glitch.

| Setting | What it does |
|---|---|
| **Mode** | *Lens* pushes colours outward from a centre point, strongest at the corners. *Directional* shifts them the same way everywhere. |
| **Fringe** | Which colours separate: *Spectral* (a smooth rainbow), *RGB Split* (red one way, blue the other), *Red/Cyan*, *Green/Magenta* (purple fringing) or *Blue/Yellow*. |
| **Amount** | How far the colours move, in pixels (at the corners in Lens mode). Negative values swap which colour lands on which side. |
| **Softness** | 0 gives crisp, separate colour copies; higher values blend them into smooth gradients. |
| **Clear Center** | Lens mode: how much of the middle stays untouched, as a percentage of the centre-to-corner distance. |
| **Falloff** | Lens mode: how quickly the effect builds towards the corners. 1 is even, higher keeps it near the edges, lower spreads it inward. |
| **Center X / Y** | Lens mode: where the lens centre sits, as a percentage of the canvas. |
| **Angle** | Directional mode: the direction in degrees, counter-clockwise from pointing right. |

Notes:

- Works on RGB documents (and indexed ones, which plug-ins receive as RGB) at 8, 16 and 32 bits,
  with or without transparency. Other colour modes get a clear error.
- Colours are mixed in linear light, so fringes around bright highlights glow instead of going
  muddy.
- PhotoCraft lets a plug-in read 128 px around each pixel, which caps the shift at about 60–125 px
  depending on Softness.
- On a 24-megapixel photo it takes about 11–14 s on a 16-thread machine, and up to roughly 35 s on
  4 threads. Very large images with very long smears automatically use fewer samples, to stay well
  inside PhotoCraft's 60-second limit for a plug-in run.

### MoxoCraft - LUT

Gives the image the colour and contrast of a film stock, printed the way that film was meant to be
printed.

| Setting | What it does |
|---|---|
| **LUT** | Which film stock to simulate (see below). |
| **Intensity** | How strongly it applies, from 0 (original) to 100 (the full look). |

Included stocks, each on the print paper (or print film) it pairs with:

- **Kodak**, on Portra Endura paper: Portra 160, Portra 400, Portra 800, Portra 800 pushed one
  and two stops, Gold 200, Ultramax 400, Ektar 100.
- **Fujifilm**, on Crystal Archive Type II paper: C200, Pro 400H, X-Tra 400.
- **Kodak cinema negatives**, on Kodak 2383 print film (the classic film-release look): Vision3
  50D, 250D, 200T and 500T.

Notes:

- Mid-grey keeps its brightness, so the look changes colour and contrast, not exposure. White
  settles at the slightly creamy white of a photographic print; lower **Intensity** to keep more
  of the original's brightness.
- Works on RGB documents at 8, 16 and 32 bits. Alpha is left alone. 32-bit documents (linear
  light) are converted to sRGB for the lookup and back, and brighter-than-white values keep their
  extra brightness.
- Colours are looked up with tetrahedral interpolation, as grading software does.
- About 2 s for a 24-megapixel photo on a 16-thread machine, 7 s on 4 threads.

**Credits and licence.** The LUTs are derived from
[spektrafilm](https://github.com/andreavolpato/spektrafilm) by Andrea Volpato, which simulates film
stocks from measured film data. They are licensed CC BY-SA 4.0 ([`luts/SPEKTRAFILM_LICENSE.txt`](luts/SPEKTRAFILM_LICENSE.txt));
how they were made and what was changed is in [`luts/CHANGELOG.txt`](luts/CHANGELOG.txt). That
licence covers the LUT files and anything that re-encodes them (including the copy inside the
plug-in), not your photos and not the rest of MoxoCraft's code. The credit travels inside the
plug-in's description, and `build.ps1` copies the licence, credits and changelog into
`dist/luts/` next to the plug-ins: keep them with the plug-ins whenever you share them. spektrafilm
also asks that its name isn't used in product branding and that the LUTs aren't sold as a LUT
pack. Kodak, Fujifilm and the film names are trademarks of their owners, used only to describe the
stocks simulated; they don't endorse MoxoCraft.

**Re-baking the LUTs.** [`tools/bake-luts/bake_spektrafilm.py`](tools/bake-luts/bake_spektrafilm.py)
holds the exact recipe (stocks, prints and settings) and the one-time setup of spektrafilm in its
own Python environment. It rewrites the `.cube` files and `CHANGELOG.txt`; then run `./build.ps1`.
Slide films (Kodachrome, Ektachrome, Provia, Velvia) are left out for now: spektrafilm's LUT tool
always sends film through a print, while spektrafilm itself shows slides scanned directly.

**Adding other LUTs.** Drop a 3D `.cube` file into `luts/` and run `./build.ps1`; it appears in the
list under its file name (underscores shown as spaces). [`luts/default.txt`](luts/default.txt)
names the LUT the dialog opens on. Check that a LUT's licence allows redistribution before adding
it, and add its credit to `luts/CREDITS.txt`. LUTs are built into the plug-in because PhotoCraft
gives plug-ins no file access, and they must fit its 32 MB plug-in limit: roughly 18 LUTs of 65
points, or over 100 of 33.

LUTs made inside DaVinci Resolve's *DaVinci Wide Gamut / DaVinci Intermediate* colour space,
rather than for normal images, come out milky and break saturated colours into flat blocks when
applied straight to a photo. List those in a `luts/davinci-intermediate.txt` file (one file name
per line): the build then bakes the conversion into that space and back into the LUT, so it works
on normal photos at full speed.

## Install in PhotoCraft

```powershell
./build.ps1
```

That builds every filter into `dist/`. Then, in PhotoCraft, either:

- **Filter › Plug-ins › Install Plug-in…** and pick each `.wasm` in `dist/`, or
- **Edit › Preferences › Plug-ins**: turn on *Use Additional Plug-ins Folder* and point it at
  `dist/`. Every filter there loads at startup.

The filters then appear under **Filter › Plug-ins** with a settings dialog and live preview.

From the command line (handy for batch work):

```bash
photocraft-cli run photo.jpg --cmd plugin.install --params '{"path":"dist/moxocraft_chromatic_aberration.wasm"}' --cmd plugin.run --params '{"id":"moxocraft.chromatic-aberration","params":{"amount":20,"fringe":"Spectral"}}' --out photo-ca.jpg
photocraft-cli run photo.jpg --cmd plugin.install --params '{"path":"dist/moxocraft_lut.wasm"}' --cmd plugin.run --params '{"id":"moxocraft.lut","params":{"lut":"Kodak Portra 400","intensity":80}}' --out photo-graded.jpg
```

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

To add a filter, copy `filters/chromatic-aberration`, give it a new manifest `id` and a name
starting "MoxoCraft - ", implement `moxocraft_sdk::Filter`, call `export_filter!`, and add it to
the workspace members in `Cargo.toml`.

## Development

You need Rust with the WebAssembly target (`build.ps1` adds it).

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

Measures a built filter in wasmi, the interpreter PhotoCraft runs plug-ins in. That interpreter
is roughly 100× slower than normal compiled code, so check this after changing a pixel loop.
