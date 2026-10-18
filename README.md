# MoxoCraft

Creative filters for [PhotoCraft](https://github.com/storytold/photocraft), the free open-source
photo editor. MoxoCraft adds two filters under **Filter › Plug-ins**:

- **MoxoCraft - Chromatic Aberration**: coloured lens fringes, from a subtle edge fringe to a
  rainbow smear or an RGB-split glitch.
- **MoxoCraft - LUT**: the colour and contrast of classic Kodak and Fujifilm film stocks.

Each filter opens a settings dialog with a live preview on your image. Nothing changes until you
press **OK**, and every run is a single undo step. The filters work the same on Windows, macOS,
Linux and PhotoCraft in the browser.

## Install

You need the plug-in files `moxocraft_chromatic_aberration.wasm` and `moxocraft_lut.wasm`.
Ready-made downloads aren't published yet, so for now build them with one command (see
[Development](#development)); they end up in the `dist` folder.

Then, in PhotoCraft, either:

- go to **Filter › Plug-ins › Install Plug-in…** and pick each `.wasm` file, or
- go to **Edit › Preferences › Plug-ins**, turn on **Use Additional Plug-ins Folder** and point it
  at the `dist` folder. PhotoCraft then loads every MoxoCraft filter each time it starts, including
  new versions after an update.

The filters appear under **Filter › Plug-ins**. To update a filter, install the new `.wasm` again;
it replaces the old one.

## MoxoCraft - Chromatic Aberration

Real lenses bend red, green and blue light by slightly different amounts, which leaves coloured
fringes along high-contrast edges. This filter recreates that, from barely there to bold.

| Setting | What it does |
|---|---|
| **Mode** | *Lens*: colours spread outward from a centre point, strongest towards the corners, like a real lens. *Directional*: colours shift the same way across the whole image. |
| **Fringe** | Which colours separate. *Spectral* gives a smooth rainbow, *RGB Split* sends red and blue opposite ways, and *Red/Cyan*, *Green/Magenta* (purple fringing) and *Blue/Yellow* give two-colour fringes. |
| **Amount** | How far the colours move, in pixels (at the corners, in Lens mode). Negative values swap which colour lands on which side. |
| **Softness** | 0 gives crisp separate copies of each colour; higher values blend them into smooth gradients. |
| **Clear Center** | Lens mode: how much of the middle stays untouched. |
| **Falloff** | Lens mode: how quickly the effect builds towards the edges. 1 is even; higher keeps it near the corners, lower spreads it inward. |
| **Center X / Y** | Lens mode: where the lens centre sits, as a percentage of the image width and height. |
| **Angle** | Directional mode: the direction of the shift in degrees, counter-clockwise from pointing right. |

Tips:

- For a natural lens look, keep **Mode** on *Lens* with a small **Amount** (try 3–10 px on a
  24-megapixel photo).
- For a glitch look, use *Directional*, *RGB Split* and **Softness** 0.
- The effect shows most on bright edges against dark backgrounds. Fringes around highlights glow
  rather than turning muddy.
- The largest possible shift is about 60–125 px, depending on Softness.

## MoxoCraft - LUT

Gives your photo the colour and contrast of a film stock, printed the way that film was meant to be
printed.

| Setting | What it does |
|---|---|
| **LUT** | Which film stock to simulate. |
| **Intensity** | How strongly it applies, from 0 (your original) to 100 (the full look). |

Included film stocks:

- **Kodak**, printed on Portra Endura paper: Portra 160, Portra 400, Portra 800, Portra 800 pushed
  one and two stops, Gold 200, Ultramax 400, Ektar 100.
- **Fujifilm**, printed on Crystal Archive paper: C200, Pro 400H, X-Tra 400.
- **Kodak cinema negatives**, printed on Kodak 2383 print film for the classic movie look: Vision3
  50D, 250D, 200T and 500T.

Tips:

- The looks keep your photo's overall brightness; they change colour and contrast. Whites settle
  at the slightly creamy white of a real photographic print. If that's too much, lower
  **Intensity**.
- For more control, apply it to a duplicate layer and lower that layer's opacity or paint a
  mask.

## Good to know

- Both filters work on RGB images at 8, 16 and 32 bits per channel, with or without transparency.
  Other colour modes (Grayscale, CMYK, Lab) show an error instead.
- On a 24-megapixel photo, Chromatic Aberration takes roughly 10–15 seconds on a fast 16-thread
  computer and up to about 35 seconds on a 4-core one; the LUT filter takes 2–7 seconds.
  PhotoCraft runs plug-ins in a safe, sandboxed way that is slower than its built-in filters.
- Make a selection first to apply a filter to part of the image only; feathered selections blend
  smoothly, as with PhotoCraft's own filters.
- You can also run the filters from PhotoCraft's command line, for example on a batch of photos:

  ```bash
  photocraft-cli run photo.jpg --cmd plugin.install --params '{"path":"dist/moxocraft_lut.wasm"}' --cmd plugin.run --params '{"id":"moxocraft.lut","params":{"lut":"Kodak Portra 400","intensity":80}}' --out photo-graded.jpg
  ```

## Credits

The film-stock LUTs are derived from [spektrafilm](https://github.com/andreavolpato/spektrafilm)
by Andrea Volpato, which simulates film from measured film data. They are licensed
[CC BY-SA 4.0](luts/SPEKTRAFILM_LICENSE.txt); how they were made and what was changed is in
[`luts/CHANGELOG.txt`](luts/CHANGELOG.txt). The licence covers the LUTs themselves, not the photos
you make with them. If you share the MoxoCraft plug-ins, keep the `dist/luts` folder (licence,
credits and changelog) with them.

Kodak, Fujifilm and the film names are trademarks of their owners. They are used only to describe
the film stocks simulated; Kodak and Fujifilm don't endorse MoxoCraft.

## Development

MoxoCraft is written in Rust. Install [Rust](https://rustup.rs), then build the plug-ins into
`dist/`:

```powershell
./build.ps1
```

Run the tests with:

```bash
cargo test
```

[`docs/development.md`](docs/development.md) covers the project layout, previews and benchmarks,
adding filters and LUTs, and re-baking the film stocks.
