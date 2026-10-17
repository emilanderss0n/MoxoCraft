# MoxoCraft

Filters for [PhotoCraft](https://github.com/storytold/photocraft), the open-source Rust image
editor. Each filter is a PhotoCraft plug-in: a small sandboxed WebAssembly file that works the same
on Windows, macOS, Linux and PhotoCraft's web version.

## Filters

### Chromatic Aberration

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

## Install in PhotoCraft

```powershell
./build.ps1
```

That builds every filter into `dist/`. Then, in PhotoCraft, either:

- **Filter › Plug-ins › Install Plug-in…** and pick `dist/moxocraft_chromatic_aberration.wasm`, or
- **Edit › Preferences › Plug-ins**: turn on *Use Additional Plug-ins Folder* and point it at
  `dist/`. Every filter there loads at startup.

The filter then appears under **Filter › Plug-ins** with a settings dialog and live preview.

From the command line (handy for batch work):

```bash
photocraft-cli run photo.jpg --cmd plugin.install --params '{"path":"dist/moxocraft_chromatic_aberration.wasm"}' --cmd plugin.run --params '{"id":"moxocraft.chromatic-aberration","params":{"amount":20,"fringe":"Spectral"}}' --out photo-ca.jpg
```

## Project layout

```text
crates/moxocraft-sdk/          shared plug-in plumbing: the PhotoCraft exports, settings JSON,
                               and a stand-in for PhotoCraft (`host`) for tests and previews
filters/chromatic-aberration/  the Chromatic Aberration filter
tools/bench/                   runs a built .wasm in the interpreter PhotoCraft uses and reports
                               the cost per pixel and the expected time for a whole image
build.ps1                      builds every filter into dist/
```

To add a filter, copy `filters/chromatic-aberration`, give it a new manifest `id`, implement
`moxocraft_sdk::Filter`, call `export_filter!`, and add it to the workspace members in
`Cargo.toml`.

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
cargo run --release -p moxocraft-bench -- dist/moxocraft_chromatic_aberration.wasm "{\"amount\":28}"
```

Measures a built filter in wasmi, the interpreter PhotoCraft runs plug-ins in. That interpreter
is roughly 100× slower than normal compiled code, so check this after changing a pixel loop.
