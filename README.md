<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/brand/moxocraft-logo-white.webp">
    <img alt="MoxoCraft" src="docs/brand/moxocraft-logo.webp" width="480">
  </picture>
</p>

Creative filters for [PhotoCraft](https://github.com/storytold/photocraft), the free open-source
photo editor. Once installed, they appear under **Filter › Plug-ins**:

- **MoxoCraft - Chromatic Aberration**: colored lens fringes, from a subtle edge fringe to a
  rainbow smear or an RGB-split glitch.
- **MoxoCraft - Film LUTs**: the color and contrast of film stocks: color negative, slide and
  instant films, and Fujifilm-style camera looks.
- **MoxoCraft - Cinematic LUTs**: movie-style grades, moody and vintage looks, and black-and-white
  films.
- **MoxoCraft - Creative LUTs**: colorful, scenic and experimental looks.
- **MoxoCraft - Snow**: falling snow with depth: small sharp flakes far away, large soft ones
  close to the camera, and ways to put the snow behind your subject.

Together the three LUT filters hold 147 looks.

## Install

1. Download `MoxoCraft-<version>.zip` from the latest
   [release](https://github.com/emilanderss0n/MoxoCraft/releases/latest) and unzip it somewhere
   you'll keep it. It holds the plug-in files (`moxocraft_chromatic_aberration.wasm`,
   `moxocraft_lut.wasm`, `moxocraft_lut_cinematic.wasm`, `moxocraft_lut_creative.wasm` and
   `moxocraft_snow.wasm`), a `luts` folder with the LUTs' licenses, credits and sources, and a
   `docs` folder with the Chromatic Aberration guide.
2. In PhotoCraft, either:
   - go to **Filter › Plug-ins › Install Plug-in…** and pick each `.wasm` file, or
   - go to **Edit › Preferences › Plug-ins**, turn on **Use Additional Plug-ins Folder** and point
     it at the unzipped folder. PhotoCraft then loads every MoxoCraft filter each time it starts;
     to update, unzip a newer release into the same folder.

The filters appear under **Filter › Plug-ins**. If you installed them from the menu, update by
installing the new `.wasm` files the same way; each replaces the old one. (Film LUTs replaces
the "MoxoCraft - LUT" filter from version 0.1.0.)

## Using the filters

Each filter opens a settings dialog with a live preview on your image. Nothing changes until you
press **OK**, and every run is a single undo step.

- Make a selection first to filter only part of the image; feathered selections blend smoothly,
  as with PhotoCraft's own filters.
- The filters work on RGB images at 8, 16 and 32 bits per channel, with or without transparency.
  Other color modes (Grayscale, CMYK, Lab) show an error instead.
- They work the same on Windows, macOS, Linux and PhotoCraft in the browser.
- You can also run them from PhotoCraft's command line, for example on a batch of photos (run
  from the unzipped MoxoCraft folder):

  ```bash
  photocraft-cli run photo.jpg --cmd plugin.install --params '{"path":"moxocraft_lut.wasm"}' --cmd plugin.run --params '{"id":"moxocraft.lut","params":{"lut":"Film - Kodak Portra 400","intensity":80}}' --out photo-graded.jpg
  ```

## MoxoCraft - Chromatic Aberration

Real lenses bend red, green and blue light by slightly different amounts, which leaves colored
fringes along high-contrast edges. This filter recreates that, from barely there to bold.

The dialog always shows every setting, whichever **Mode** you pick. **Clear Center**,
**Falloff** and **Center X / Y** only do anything in *Lens* mode, and **Angle** only in
*Directional* mode.

| Setting | Mode | What it does |
|---|---|---|
| **Mode** | Both | *Lens*: colors spread outward from a center point, strongest toward the corners, like a real lens. *Directional*: colors shift the same way across the whole image. |
| **Fringe** | Both | Which colors separate. *Spectral* gives a smooth rainbow, *RGB Split* sends red and blue opposite ways, and *Red/Cyan*, *Green/Magenta* (purple fringing) and *Blue/Yellow* give two-color fringes. |
| **Amount** | Both | How far the colors move, in pixels (at the corners, in Lens mode). Negative values swap which color lands on which side. |
| **Softness** | Both | 0 gives crisp separate copies of each color; higher values blend them into smooth gradients. |
| **Strength** | Both | How strongly the effect shows over your original, from 0 (unchanged) to 100 (the full effect). It fades the fringes without moving them. |
| **Clear Center** | Lens | How much of the middle stays untouched. |
| **Falloff** | Lens | How quickly the effect builds toward the edges. 1 is even; higher keeps it near the corners, lower spreads it inward. |
| **Center X / Y** | Lens | Where the lens center sits, as a percentage of the image width and height. |
| **Angle** | Directional | The direction of the shift in degrees, counter-clockwise from pointing right. |

Tips:

- For a natural lens look, keep **Mode** on *Lens* with a small **Amount** (try 3–10 px on a
  24-megapixel photo).
- If the fringes are the right shape but too strong, lower **Strength** rather than **Amount**.
  Amount changes how far the colors move, so lowering it narrows the fringes as well as fading
  them. Strength only fades them.
- For a glitch look, use *Directional*, *RGB Split* and **Softness** 0.

The [Chromatic Aberration guide](docs/chromatic-aberration.md) has four recipes with
before-and-after pictures (Subtle Lens, Purple Fringing, Dreamy Spectral and RGB Glitch), and
more on how Strength blends and how far the colors can move.

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
- **Negative**: more color negative films, among them Agfa Vista, Fujifilm Superia and Pro
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

- **Colorful**: bright and playful color grades.
- **Scenic**: grades made for landscapes: greens, blues, fog and snow.
- **Creative**: bold, experimental looks, including false-color infrared and Lomochrome Purple.

Tips:

- The Film category keeps your photo's overall brightness and changes color and contrast; whites
  settle at the slightly creamy white of a real photographic print. Many of the other looks are
  bolder. If one is too strong, lower **Intensity**.
- For more control, apply it to a duplicate layer and lower that layer's opacity or paint a
  mask.

## MoxoCraft - Snow

Falling snow with depth, the way it looks in a real photo: flakes far behind your subject are
small and many, flakes close to the camera few, large and out of focus. In-focus flakes are
irregular clumps, out-of-focus ones soft discs like a lens draws them, and falling flakes drift
and tumble rather than drop straight like rain. Sizes are measured against the picture rather
than in pixels, so the same settings give the same look on any photo, and the live preview
matches the result.

Run it on an empty layer (**Layer › New › Layer…**) to keep the snow on a layer of its own, where
you can mask, move or fade it. The result looks the same as running it on the photo.

| Setting | What it does |
|---|---|
| **Amount** | How much snow. Raising it adds flakes without moving the ones already there. |
| **Flake Size** | How big the flakes at your subject's distance are. Every 25 doubles their size. |
| **Defocus** | How blurry flakes get away from your subject, like a lens opened wide. Flakes in front of the subject blur the most; 0 keeps every flake sharp. |
| **Motion** | 0 freezes the flakes. Higher values streak them, as if they fell while the shutter was open; near flakes streak the most, and each streak flickers a little as its flake tumbles. |
| **Angle** | Which way the snow falls, in degrees from straight down; positive values lean it to the right. Only does anything when Motion is above 0. |
| **Swirl** | How much falling flakes wander from that direction and curve: 0 for heavy, wet snow falling straight, 100 for light snow swirling in the wind. Only does anything when Motion is above 0. |
| **Tint** | The flakes' color: *White*, *Cool* (blue, as in shade or at dusk) or *Warm* (lit by streetlights or a low sun). |
| **Opacity** | How strongly the snow shows, from 0 to 100. The same as lowering the opacity of a layer holding the snow. |
| **Depth** | *None* draws all the snow. *Near Only* and *Far Only* draw only the flakes in front of or behind Subject Depth. *Depth Map* and *Depth Map (Inverted)* turn a depth map into snow. Both are explained below. |
| **Subject Depth** | How far away your subject is, from 0 (at the lens) to 100 (as far as the snow goes). Flakes at that distance are the sharpest, and Near Only and Far Only split the snow there. |
| **Variation** | Picks a different random layout. The other settings don't move the flakes, so you can get the look right first and then try layouts. |

### Snow behind your subject

A plug-in can't tell what's near or far in a photo, so on its own Snow puts every flake in front
of everything. There are two ways to put some of it behind your subject.

**Near and far layers.** This needs nothing outside PhotoCraft.

1. Set **Subject Depth** to roughly where your subject stands.
2. Add an empty layer and run Snow with **Depth** set to *Far Only*.
3. Select your subject (**Select › Subject**, or any selection tool), choose **Select › Inverse**,
   then **Layer › Layer Mask › Reveal Selection**. The far snow now goes behind your subject.
4. Add another empty layer on top and run Snow again with the same settings and **Depth** set to
   *Near Only*.

Together, the two layers hold the same snow as *None*.

**A depth map.** This handles any scene, but you need a depth map: a grayscale picture of how far
away each part of your photo is. Depth-estimation tools such as Depth Anything make one from a
photo.

1. Put the depth map on a layer of its own, lined up with your photo and the same size.
2. Run Snow on that layer with **Depth** set to *Depth Map*. The layer turns into snow, each flake
   hidden wherever the scene is nearer than the flake.

*Depth Map* expects white for near and black for far, as most tools make them; use
*Depth Map (Inverted)* for a map the other way round. Duplicate the depth map first if you want
to use it again.

Tips:

- Snow shows best against darker backgrounds, as it does in real photos.
- For light snow, try **Amount** 20–30 and **Defocus** 20. For a blizzard, try **Amount** 80–100,
  **Motion** 40–60, **Angle** 30–60 and **Swirl** 60–80.
- For a dreamy look, raise **Defocus** to 70–90 and **Flake Size** to 60–70: near flakes turn into
  large, soft discs.
- **Motion** streaks look best in small amounts (10–40). Real falling snow rarely streaks far.
- Sharp flakes show their clumpy shapes best at full size. Zoomed out, every flake looks round.
- With very large, blurred and streaked flakes at a high **Amount**, Snow limits how many flakes
  it draws so it finishes in time. Past that point, raising **Amount** adds nothing more.

## Speed

PhotoCraft runs plug-ins in a sandbox that keeps them safe but makes them slower than its
built-in filters, and it stops any plug-in run that takes longer than 60 seconds. Times measured
on a 24-megapixel photo with a Ryzen 7 7800X3D, using all 16 threads and with PhotoCraft limited
to 4 cores running one thread each:

| Filter and settings | 16 threads | 4 cores |
|---|---|---|
| Snow, default settings | under 0.1 s | 0.2 s |
| Snow, heavy (Amount 100, Flake Size 60, Defocus 50, Motion 40) | 1 s | 4 s |
| Snow, every setting at its largest | 5 s | 22 s |
| Chromatic Aberration, default settings | 9 s | 41 s |
| Chromatic Aberration, heaviest settings (*Spectral*, Amount 128, Softness 100, Clear Center 0, Falloff 0.5) | 12–15 s | 48–55 s |

With 4 cores running two threads each, Chromatic Aberration's heaviest settings took 19–22
seconds. **Strength** between 0 and 100 adds about 20% at light settings, but only 1–2 seconds at
the heaviest. The LUT filters take about 1–4 seconds.

Chromatic Aberration's smooth fringes are the slow part. On a computer slower than this one with
only 4 threads, they can go past the 60-second limit; PhotoCraft then stops the filter and leaves
the image as it was. Crisp fringes (**Softness** 0) are much quicker: the RGB Glitch recipe took 2
seconds on 16 threads and 10 on 4 cores.

## Credits

The LUTs come from these collections, each under a license that allows sharing them. MoxoCraft
converted them to the same format and size and renamed them; [`luts/SOURCES.txt`](luts/SOURCES.txt)
lists every LUT with its author, source and license.

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
  [MIT license](luts/STRIPED_PURPLE_LICENSE.txt).
- **Scenic**: also Tension Warm, Tension Cold and Krakow by
  [Christoph Wurst](https://github.com/ChristophWurst/haldclut) ([CC BY-SA 4.0](luts/CC-BY-SA-4.0.txt)).

The licenses cover the LUTs themselves, not the photos you make with them. If you share the
MoxoCraft plug-ins, keep the `luts` folder (licenses, sources and changelog) with them.

Kodak, Fujifilm, Ilford, Agfa, Polaroid, Lomography, Rollei and the film names are trademarks of
their owners. They are used only to describe the looks simulated; their owners don't endorse
MoxoCraft.

## Development

MoxoCraft is written in Rust. Install [Rust](https://rustup.rs), then build the plug-ins into
`dist/` and run the tests:

```powershell
./build.ps1
cargo test
```

[`docs/development.md`](https://github.com/emilanderss0n/MoxoCraft/blob/main/docs/development.md)
covers how the code is organized, previews, benchmarks, adding a filter and releasing.
[`docs/luts.md`](https://github.com/emilanderss0n/MoxoCraft/blob/main/docs/luts.md) covers the LUT
files: adding and removing LUTs, re-making them from their sources, and their licenses.
