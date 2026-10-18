# LUTs

PhotoCraft gives plug-ins no file access, so each LUT plug-in carries its LUTs inside its `.wasm`.
Its build script reads its folder under `luts/` and bakes every `.cube` file in it into the
plug-in, then writes the manifest from the same list, so the dialog always matches what is baked
in. PhotoCraft lists at most 64 choices in a dropdown, which is why the LUTs are spread over three
plug-ins:

```text
luts/film-luts/       MoxoCraft - Film LUTs:      Film, Negative, Slide, Instant, Fujifilm Style
luts/cinematic-luts/  MoxoCraft - Cinematic LUTs: Cinematic, Moody, Vintage, Black & White
luts/creative-luts/   MoxoCraft - Creative LUTs:  Colorful, Scenic, Creative
```

## Where each fact lives

Each fact is kept in one place, and the build works out the rest from it.

| What | Where |
|---|---|
| A plug-in's id, menu name, description, and which folder it bakes | `filters/<crate>/build.rs` |
| Which LUTs it has, and their category | the `.cube` files in `luts/<pack>/<Category>/` |
| A LUT's menu name | its file name: "Category - File name", underscores shown as spaces |
| The categories' menu order | `luts/<pack>/categories.txt` |
| Order within a category | natural order of the names ("50D" before "200T") |
| The LUT the dialog opens on | `luts/<pack>/default.txt` (optional; else the first LUT) |
| Credits shown in PhotoCraft's description of the plug-in | `luts/<pack>/CREDITS.txt` (optional) |
| LUTs made for DaVinci Resolve's working space | `luts/<pack>/davinci-intermediate.txt` (optional) |
| Where a LUT came from, its author, licence and changes | `luts/SOURCES.txt`, written by `tools/import-luts/import_luts.py` |
| How the Film category was made | `luts/CHANGELOG.txt`, written by `tools/bake-luts/bake_spektrafilm.py` |
| The licence texts | `luts/*.txt` (shipped with the plug-ins) |
| The plug-ins' version | the root `Cargo.toml` |

The build stops with a message if these disagree: a category folder missing from
`categories.txt`, a `.cube` file outside a category folder, a default that isn't a LUT, two LUTs
with the same menu name, more than 64 LUTs, more than PhotoCraft's 32 MB module limit (about 140
LUTs of 33 points), or a description over 1024 bytes once the credits are added.

What the build makes from them sits in each plug-in crate's `OUT_DIR` under `target/` and is
remade on every change: `luts.bin` (every table, back to back, at 16 bits per value), `luts.rs`
(the index of names, files and byte ranges) and `manifest.json`. `lut_pack!` puts them into the
plug-in. See [`development.md`](development.md) for the code.

The Film LUTs plug-in keeps v0.1.0's id, `moxocraft.lut`, and file name, so installing it
replaces the old "MoxoCraft - LUT".

## Adding or removing LUTs

**Adding a LUT.** To try one, drop a 3D `.cube` file into a category folder and run
`./build.ps1`. Before committing it, check its licence allows redistribution, add it to the recipe
in `tools/import-luts/import_luts.py` (so its source, licence and changes land in
`luts/SOURCES.txt`) and re-run the import (below), and add its credit to the plug-in's
`CREDITS.txt` if its source isn't credited there yet.

**Removing a LUT.** Take it out of the recipe in `import_luts.py` and re-run the import, which
removes the file and its `SOURCES.txt` entry. If it was the default, change `default.txt`, and drop
its source from `CREDITS.txt` if nothing else uses it.

**Adding or removing a category.** Create or delete the folder in `luts/<pack>/` and add or remove
its line in `categories.txt`; for imported LUTs, also add or remove its `"<pack>/<Category>"`
entry in the recipe.

**Adding a LUT plug-in.** Copy `filters/lut-creative` to a new folder, rename the package in its
`Cargo.toml`, and give its `build.rs` a new folder, id, name and description. Create the folder
in `luts/` with `categories.txt` and the category folders, update the crate's `tests/pack.rs` to
match, add the crate to the workspace `members` in the root `Cargo.toml`, and add it to
`tools/lut-preview` and to `$Plugins` in `tools/release.ps1`.

**DaVinci Resolve LUTs.** LUTs made inside Resolve's *DaVinci Wide Gamut / DaVinci Intermediate*
colour space, rather than for normal images, come out milky and break saturated colours into flat
blocks when applied straight to a photo. List those in the plug-in folder's
`davinci-intermediate.txt` (one `Category/File name.cube` per line): the build then bakes the
conversion into that space and back into the LUT, so it works on normal photos at full speed.

## Re-making the LUTs from their sources

Two scripts record exactly where every LUT came from and what was done to it, which the CC BY and
CC BY-SA licences ask for. They are kept separate on purpose: each source's licence was checked by
hand, and the notes in each script say what was taken and what was left out.

- [`tools/bake-luts/bake_spektrafilm.py`](../tools/bake-luts/bake_spektrafilm.py) bakes the Film
  category from [spektrafilm](https://github.com/andreavolpato/spektrafilm) (CC BY-SA 4.0): the
  stocks, prints and settings, and the one-time setup of spektrafilm in its own Python
  environment. It rewrites `luts/film-luts/Film` and `luts/CHANGELOG.txt`. Slide films are left
  out there: spektrafilm's LUT tool always sends film through a print, while spektrafilm itself
  shows slides scanned directly.
- [`tools/import-luts/import_luts.py`](../tools/import-luts/import_luts.py) imports the other 132
  LUTs from their collections at pinned commits (the setup is at the top of the script). It
  converts HaldCLUT images to `.cube` and resamples anything over 33 points per axis down to 33
  (both in `lutconv.py`), writes a header into each file naming its source, author and licence,
  copies the licence texts byte for byte, and rewrites `luts/SOURCES.txt`.

Then run `./build.ps1`.

## Licence conditions to keep

- `luts/SPEKTRAFILM_LICENSE.txt` must stay byte-identical to upstream (`luts/.gitattributes`
  stops Git changing its line endings, and `tools/release.ps1` checks it).
- Every change to the spektrafilm LUTs goes in `luts/CHANGELOG.txt`.
- spektrafilm's name mustn't be used in product branding, and the LUTs mustn't be sold as a LUT
  pack (spektrafilm and slay both ask this).
- The licence texts, `SOURCES.txt` and `CHANGELOG.txt` must ship with the plug-ins; `build.ps1`
  copies them to `dist/luts/` and `tools/release.ps1` puts them in the release zip.
