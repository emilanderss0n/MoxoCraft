# MoxoCraft

Creative filters for [PhotoCraft](https://github.com/storytold/photocraft), the free open-source
photo editor. MoxoCraft adds these filters under **Filter › Plug-ins**:

- **MoxoCraft - Chromatic Aberration**: coloured lens fringes, from a subtle edge fringe to a
  rainbow smear or an RGB-split glitch.
- **MoxoCraft - Film LUTs**: the colour and contrast of film stocks: colour negative, slide and
  instant films, and Fujifilm-style camera looks.
- **MoxoCraft - Cinematic LUTs**: movie-style grades, moody and vintage looks, and black-and-white
  films.
- **MoxoCraft - Creative LUTs**: colourful, scenic and experimental looks.

Together the three LUT filters hold 147 looks.

Each filter opens a settings dialog with a live preview on your image. Nothing changes until you
press **OK**, and every run is a single undo step. The filters work the same on Windows, macOS,
Linux and PhotoCraft in the browser.

## Install

1. Download `MoxoCraft-<version>.zip` from the latest
   [release](https://github.com/emilanderss0n/MoxoCraft/releases/latest) and unzip it somewhere
   you'll keep it. It holds the plug-in files (`moxocraft_chromatic_aberration.wasm`,
   `moxocraft_lut.wasm`, `moxocraft_lut_cinematic.wasm` and `moxocraft_lut_creative.wasm`), plus a
   `luts` folder with the LUTs' licences, credits and sources.
2. In PhotoCraft, either:
   - go to **Filter › Plug-ins › Install Plug-in…** and pick each `.wasm` file, or
   - go to **Edit › Preferences › Plug-ins**, turn on **Use Additional Plug-ins Folder** and point
     it at the unzipped folder. PhotoCraft then loads every MoxoCraft filter each time it starts;
     to update, unzip a newer release into the same folder.

The filters appear under **Filter › Plug-ins**. If you installed them from the menu, update by
installing the new `.wasm` files the same way; each replaces the old one. (Film LUTs replaces
the "MoxoCraft - LUT" filter from version 0.1.0.)

## MoxoCraft - Chromatic Aberration

Real lenses bend red, green and blue light by slightly different amounts, which leaves coloured
fringes along high-contrast edges. This filter recreates that, from barely there to bold.

The dialog always shows every setting, whichever **Mode** you pick. **Clear Center**,
**Falloff** and **Center X / Y** only do anything in *Lens* mode, and **Angle** only in
*Directional* mode; the other mode ignores them.

| Setting | Mode | What it does |
|---|---|---|
| **Mode** | Both | *Lens*: colours spread outward from a centre point, strongest towards the corners, like a real lens. *Directional*: colours shift the same way across the whole image. |
| **Fringe** | Both | Which colours separate. *Spectral* gives a smooth rainbow, *RGB Split* sends red and blue opposite ways, and *Red/Cyan*, *Green/Magenta* (purple fringing) and *Blue/Yellow* give two-colour fringes. |
| **Amount** | Both | How far the colours move, in pixels (at the corners, in Lens mode). Negative values swap which colour lands on which side. |
| **Softness** | Both | 0 gives crisp separate copies of each colour; higher values blend them into smooth gradients. |
| **Strength** | Both | How strongly the effect shows over your original, from 0 (unchanged) to 100 (the full effect). It fades the fringes without moving them. |
| **Clear Center** | Lens | How much of the middle stays untouched. |
| **Falloff** | Lens | How quickly the effect builds towards the edges. 1 is even; higher keeps it near the corners, lower spreads it inward. |
| **Center X / Y** | Lens | Where the lens centre sits, as a percentage of the image width and height. |
| **Angle** | Directional | The direction of the shift in degrees, counter-clockwise from pointing right. |

Tips:

- For a natural lens look, keep **Mode** on *Lens* with a small **Amount** (try 3–10 px on a
  24-megapixel photo).
- If the fringes are the right shape but too strong, lower **Strength** rather than **Amount**.
  Amount changes how far the colours move, so lowering it narrows the fringes as well as
  fading them. Strength only fades them.
- Strength isn't quite the same as putting the effect on a duplicate layer and lowering its
  opacity. It blends light the way a lens does, rather than the values you see on screen. At 50%,
  bright fringes against a dark background stay a little stronger, and dark fringes against a
  bright sky fade a little more, than they would on a layer at 50% opacity.
- For a glitch look, use *Directional*, *RGB Split* and **Softness** 0.
- The effect shows most on bright edges against dark backgrounds. Fringes around highlights glow
  rather than turning muddy.
- The largest possible shift is about 65–125 px, depending on Softness.

### Recipes

Four starting points. Settings not listed stay at their defaults. The amounts are for a photo
about 6000 px wide (24 megapixels). Amount is measured in pixels, so for other sizes scale it with
the width: half as much on a photo 3000 px wide, for example. Each picture is a 400 px crop near a
corner at 100%, before on the left and after on the right.

**Subtle Lens**: *Lens*, *Red/Cyan*, **Amount** 4, **Softness** 30, **Clear Center** 10,
**Falloff** 1.

Thin red and cyan edges like a real lens leaves towards the corners, without softening much
detail. Best on sharp, high-contrast subjects: architecture, window frames, branches. At
fit-to-screen size you'll hardly see it, which is the point.

![Subtle Lens before and after: a steel roof with thin red and cyan edges](docs/images/chromatic-aberration/subtle-lens.jpg)

**Purple Fringing**: *Lens*, *Green/Magenta*, **Amount** -4, **Softness** 50,
**Clear Center** 10, **Falloff** 1, **Strength** 60.

The purple outline cheaper lenses leave on dark edges against a bright sky. Best on backlit
scenes: branches, wires, window frames or silhouettes against the sky. Green/Magenta also leaves
a green edge on the other side of each line. Strength 60 fades that green more than the purple:
on the example photo, the purple came out about 30% stronger than the green, where at Strength
100 they are about even.

![Purple Fringing before and after: bare branches against a white sky with purple edges](docs/images/chromatic-aberration/purple-fringing.jpg)

**Dreamy Spectral**: *Lens*, *Spectral*, **Amount** 30, **Softness** 80, **Clear Center** 30,
**Falloff** 1.2.

Soft rainbow glows that grow towards the edges while the middle stays sharp. Best on soft,
backlit or night photos with bright highlights: sun on water, out-of-focus lights, a portrait
with the subject in the middle. It smears fine detail near the edges, so it doesn't suit sharp
architecture.

![Dreamy Spectral before and after: out-of-focus sun reflections with blue and orange glows](docs/images/chromatic-aberration/dreamy-spectral.jpg)

**RGB Glitch**: *Directional*, *RGB Split*, **Amount** 12, **Softness** 0.

Hard red, green and blue copies shifted sideways across the whole picture. Best on bold, graphic
images: architecture, neon and night scenes, posters. Raise **Amount** to 20–30 for a bigger
split, or change **Angle** to shift the copies another way.

![RGB Glitch before and after: a steel roof split into red, green and blue copies](docs/images/chromatic-aberration/rgb-glitch.jpg)

The example photos are public domain (CC0), from Wikimedia Commons:
[steel roof](https://commons.wikimedia.org/wiki/File:King_cross_roof_architecture_%28Unsplash%29.jpg),
[bare branches](https://commons.wikimedia.org/wiki/File:Bare_branches_against_a_white_sky,_Quebec_City.jpg)
and [surfer and sea](https://commons.wikimedia.org/wiki/File:Surfer_and_sea_reflecting_the_sun_%28Unsplash%29.jpg).
They were resized to 6000 px wide before applying the filter.

## LUT filters

Each LUT filter gives your photo a ready-made look. Pick one from the **LUT** list, which is
grouped by category ("Film - Kodak Portra 400", "Slide - Fujifilm Velvia 50", ...), and set how
strongly it applies.

| Setting | What it does |
|---|---|
| **LUT** | Which look to apply. |
| **Intensity** | How strongly it applies, from 0 (your original) to 100 (the full look). |

**MoxoCraft - Film LUTs**

- **Film**: Kodak Portra 160, 400 and 800 (plus 800 pushed one and two stops), Gold 200,
  Ultramax 400 and Ektar 100 on Portra Endura paper; Fujifilm C200, Pro 400H and X-Tra 400 on
  Crystal Archive paper; and Kodak Vision3 50D, 250D, 200T and 500T on 2383 print film for the
  classic movie look. Simulated from measured film data.
- **Negative**: more colour negative films, among them Agfa Vista, Fujifilm Superia and Pro
  160C/800Z, Kodak Portra NC/VC/UC, a cross-processed Kodak Elite and Lomography Redscale.
- **Slide**: Fujifilm Velvia, Provia, Astia and Sensia, Kodak Kodachrome 64, Ektachrome and Elite
  Chrome, Agfa Precisa, Polaroid Polachrome and a Lomography cross-process slide film.
- **Instant**: Polaroid 665, 669, 690, PX-70, PX-680, PX-100UV+ and an expired Time Zero, and
  Fujifilm FP-100C and FP-3000B.
- **Fujifilm Style**: the look of Fujifilm cameras' Acros, Classic Chrome, Provia Standard and
  Velvia Vivid modes.

**MoxoCraft - Cinematic LUTs**

- **Cinematic**: teal-and-orange and other blockbuster grades, plus warm, cool, neon and sunset
  looks.
- **Moody**: dark, dramatic grades: icy, emerald, noir, night and horror looks.
- **Vintage**: faded and retro film looks, and a grade for each decade from the 1920s to the
  1980s.
- **Black & White**: Ilford HP5, FP4, Pan F and Delta 3200, Kodak Tri-X, T-Max and HIE infrared,
  Agfa APX, Fujifilm Neopan and Rollei Retro.

**MoxoCraft - Creative LUTs**

- **Colorful**: bright and playful colour grades.
- **Scenic**: grades made for landscapes: greens, blues, fog and snow.
- **Creative**: bold, experimental looks, including false-colour infrared and Lomochrome Purple.

Tips:

- The Film category keeps your photo's overall brightness and changes colour and contrast; whites
  settle at the slightly creamy white of a real photographic print. Many of the other looks are
  bolder. If one is too strong, lower **Intensity**.
- For more control, apply it to a duplicate layer and lower that layer's opacity or paint a
  mask.

## Good to know

- All the filters work on RGB images at 8, 16 and 32 bits per channel, with or without
  transparency.
  Other colour modes (Grayscale, CMYK, Lab) show an error instead.
- On a 24-megapixel photo, Chromatic Aberration takes roughly 10–15 seconds on a fast 16-thread
  computer and up to about 35 seconds on a 4-core one; the LUT filters take 1–4 seconds.
  With **Strength** between 0 and 100, Chromatic Aberration takes up to about 20% longer.
  PhotoCraft runs plug-ins in a safe, sandboxed way that is slower than its built-in filters.
- Make a selection first to apply a filter to part of the image only; feathered selections blend
  smoothly, as with PhotoCraft's own filters.
- You can also run the filters from PhotoCraft's command line, for example on a batch of photos
  (run from the unzipped MoxoCraft folder):

  ```bash
  photocraft-cli run photo.jpg --cmd plugin.install --params '{"path":"moxocraft_lut.wasm"}' --cmd plugin.run --params '{"id":"moxocraft.lut","params":{"lut":"Film - Kodak Portra 400","intensity":80}}' --out photo-graded.jpg
  ```

## Credits

The LUTs come from these collections, each under a licence that allows sharing them. MoxoCraft
converted them to the same format and size and renamed them; [`luts/SOURCES.txt`](luts/SOURCES.txt)
lists every LUT with its author, source and licence.

- **Film**: derived from [spektrafilm](https://github.com/andreavolpato/spektrafilm) by Andrea
  Volpato, which simulates film from measured film data. Licensed
  [CC BY-SA 4.0](luts/SPEKTRAFILM_LICENSE.txt); how they were made is in
  [`luts/CHANGELOG.txt`](luts/CHANGELOG.txt).
- **Negative, Slide, Instant and Black & White**: Pat David's
  [film emulation presets](https://patdavid.net/2013/08/film-emulation-presets-in-gmic-gimp/), from
  the [NatronGitHub/clut](https://github.com/NatronGitHub/clut) collection. Licensed
  [CC BY-SA 4.0](luts/CC-BY-SA-4.0.txt).
- **Fujifilm Style**: [FilmSim](https://github.com/sguyader/FilmSim) by Sébastien Guyader, public
  domain (CC0). Community-made, not by Fujifilm.
- **Cinematic, Moody, Vintage and Colorful**: LUTs shared on [freshLUTs](https://freshluts.com)
  by their creators (CC0), as selected by [OpenShot](https://github.com/OpenShot/openshot-qt);
  [slay](https://www.slaycamera.com/) (Amber, Fade, Film, Neon and Portrait;
  [CC BY 4.0](luts/CC-BY-4.0.txt)); and Striped Purple.
- **Striped Purple** (in Vintage, Colorful, Scenic and Creative): by Austin Barrett,
  [color-grading-luts](https://github.com/stripedpurple/color-grading-luts), under the
  [MIT licence](luts/STRIPED_PURPLE_LICENSE.txt).
- **Scenic**: also Tension Warm, Tension Cold and Krakow by
  [Christoph Wurst](https://github.com/ChristophWurst/haldclut) ([CC BY-SA 4.0](luts/CC-BY-SA-4.0.txt)).

The licences cover the LUTs themselves, not the photos you make with them. If you share the
MoxoCraft plug-ins, keep the `luts` folder (licences, sources and changelog) with them.

Kodak, Fujifilm, Ilford, Agfa, Polaroid, Lomography, Rollei and the film names are trademarks of
their owners. They are used only to describe the looks simulated; their owners don't endorse
MoxoCraft.

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

[`docs/development.md`](docs/development.md) covers how the code is organised, previews and
benchmarks, adding a filter and releasing. [`docs/luts.md`](docs/luts.md) covers the LUT files:
adding and removing LUTs, re-making them from their sources, and their licences.
