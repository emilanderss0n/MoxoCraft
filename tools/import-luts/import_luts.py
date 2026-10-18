"""Imports MoxoCraft's third-party LUTs into luts/ and writes luts/SOURCES.txt.

Every LUT below comes from a collection whose licence allows redistribution (checked by hand;
see the notes on each source). This script is the record of what was taken, from which exact
commit, and what was changed: HaldCLUT images are converted to .cube files, and LUTs larger than
33 points per axis are resampled down to 33 (lutconv.py). Each .cube file gets a header naming
its source, author and licence, and luts/SOURCES.txt lists them all, which the CC BY and
CC BY-SA licences ask for. The licence texts are copied byte for byte from the source
repositories.

Setup (once): clone the sources outside this repository, at the commits in SOURCES below:

    git clone https://github.com/NatronGitHub/clut
    git clone https://github.com/sguyader/FilmSim
    git clone https://github.com/ChristophWurst/haldclut
    git clone https://github.com/slaycamera/luts slaycamera-luts
    git clone https://github.com/stripedpurple/color-grading-luts
    git clone --filter=blob:none --no-checkout https://github.com/OpenShot/openshot-qt
    cd openshot-qt && git checkout c58ce6e7f11d59872dffcabe9a09bf9dec16cf53 -- src/colors

and a Python with numpy, pillow and tifffile (`python -m venv venv`, then
`venv/Scripts/python -m pip install numpy pillow tifffile`). Then, from the MoxoCraft folder:

    python tools/import-luts/import_luts.py --sources <folder holding the clones>

and rebuild the plug-ins (./build.ps1). The spektrafilm film stocks in luts/film-luts/Film are
made by tools/bake-luts/bake_spektrafilm.py instead.
"""

import argparse
import re
import subprocess
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
import lutconv  # noqa: E402

# Collection key -> where it lives and on what terms. "licence_file" is copied into luts/ under
# "licence_copy" (from the repository's history, so its bytes are exactly upstream's).
SOURCES = {
    "natron": {
        "title": "HaldCLUT film emulation presets by Pat David",
        "author": "Pat David",
        "repo": "https://github.com/NatronGitHub/clut",
        "folder": "clut",
        "commit": "af7b50d4caf6244fb6895a647f5b6a84efe7931a",
        "licence": "CC BY-SA 4.0",
        "licence_copy": "CC-BY-SA-4.0.txt",
        # Pat David made these from scratch (patdavid.net/2013/08/film-emulation-presets-in-gmic-gimp)
        # and licenses his work CC BY-SA 4.0, as the repository's README states. Its print and
        # PictureFX folders (by other authors) and the uncredited "various" folder are not used.
        "notes": "From the bw, colorslide, instant_consumer, instant_pro and negative_* folders, "
        "which the repository's README credits to Pat David "
        "(https://patdavid.net/2013/08/film-emulation-presets-in-gmic-gimp/).",
    },
    "filmsim": {
        "title": "FilmSim by Sébastien Guyader",
        "author": "Sébastien Guyader (sguyader)",
        "repo": "https://github.com/sguyader/FilmSim",
        "folder": "FilmSim",
        "commit": "1453b2b55c48d99a889b1e455f91f6898ba2db41",
        "licence": "CC0 1.0 (public domain dedication)",
        "notes": "Community-made simulations of Fujifilm camera looks; not made or endorsed by Fujifilm.",
    },
    "wurst": {
        "title": "haldclut by Christoph Wurst",
        "author": "Christoph Wurst",
        "repo": "https://github.com/ChristophWurst/haldclut",
        "folder": "haldclut",
        "commit": "310aec52794c1d0080164c1cb87e5b6003a44d37",
        "licence": "CC BY-SA 4.0",
        "licence_file": "LICENSE",
        "licence_copy": "CC-BY-SA-4.0.txt",
    },
    "slay": {
        "title": "slay LUTs",
        "author": "slay (https://www.slaycamera.com/)",
        "repo": "https://github.com/slaycamera/luts",
        "folder": "slaycamera-luts",
        "commit": "a1709044efe115640dde1dee17d4185fc059af0d",
        "licence": "CC BY 4.0",
        "licence_file": "LICENSE",
        "licence_copy": "CC-BY-4.0.txt",
        "notes": "Credit as the author asks: LUTs: slay (https://www.slaycamera.com/) (CC BY 4.0).",
    },
    "stripedpurple": {
        "title": "color-grading-luts by Striped Purple (Austin Barrett)",
        "author": "Austin Barrett - Striped Purple (GitHub: stripedpurple / Nixua)",
        "repo": "https://github.com/stripedpurple/color-grading-luts",
        "folder": "color-grading-luts",
        "commit": "9757e8b5147693cab49c3fd674bf23dbe28d6c8e",
        "licence": "MIT",
        "licence_file": "LICENSE",
        "licence_copy": "STRIPED_PURPLE_LICENSE.txt",
        "notes": "The author asks that you spread the word about their work: "
        "https://www.instagram.com/stripedpurple/.",
    },
    "openshot": {
        "title": "freshLUTs community LUTs, as selected and renamed by OpenShot",
        "author": "the freshLUTs creators listed per LUT (via OpenShot's src/colors/AUTHORS.md)",
        "repo": "https://github.com/OpenShot/openshot-qt",
        "folder": "openshot-qt",
        "commit": "c58ce6e7f11d59872dffcabe9a09bf9dec16cf53",
        "licence": "CC0 1.0 (public domain dedication), as stated on each LUT's freshluts.com page",
    },
}

# "Plug-in folder/Category" -> [(source, path in the source, menu name)]. Each LUT plug-in lists
# its categories in the order of its folder's categories.txt and shows each LUT as
# "Category - Name". PhotoCraft lists at most 64 LUTs in one plug-in's dropdown.
RECIPE = {
    "film-luts/Negative": [
        ("natron", "negative_color/agfa_ultra_color_100.png", "Agfa Ultra Color 100"),
        ("natron", "negative_color/agfa_vista_200.png", "Agfa Vista 200"),
        ("natron", "negative_new/fuji_160c.png", "Fujifilm Pro 160C"),
        ("natron", "negative_new/fuji_800z.png", "Fujifilm Pro 800Z"),
        ("natron", "negative_old/fuji_superia_100.png", "Fujifilm Superia 100"),
        ("natron", "negative_old/fuji_superia_400.png", "Fujifilm Superia 400"),
        ("natron", "negative_old/fuji_superia_800.png", "Fujifilm Superia 800"),
        ("natron", "negative_old/fuji_superia_1600.png", "Fujifilm Superia 1600"),
        ("natron", "negative_color/fuji_superia_reala_100.png", "Fujifilm Superia Reala 100"),
        ("natron", "negative_color/kodak_elite_color_200.png", "Kodak Elite Color 200"),
        ("natron", "negative_color/kodak_elite_100_xpro.png", "Kodak Elite 100 (Cross-Processed)"),
        ("natron", "negative_color/kodak_portra_160_nc.png", "Kodak Portra 160 NC"),
        ("natron", "negative_color/kodak_portra_160_vc.png", "Kodak Portra 160 VC"),
        ("natron", "negative_old/kodak_portra_400_uc.png", "Kodak Portra 400 UC"),
        ("natron", "negative_old/kodak_portra_400_vc.png", "Kodak Portra 400 VC"),
        ("natron", "negative_color/lomography_redscale_100.png", "Lomography Redscale 100"),
    ],
    "film-luts/Slide": [
        ("natron", "colorslide/agfa_precisa_100.png", "Agfa Precisa 100"),
        ("natron", "colorslide/fuji_astia_100f.png", "Fujifilm Astia 100F"),
        ("natron", "colorslide/fuji_provia_100f.png", "Fujifilm Provia 100F"),
        ("natron", "colorslide/fuji_provia_400x.png", "Fujifilm Provia 400X"),
        ("natron", "colorslide/fuji_sensia_100.png", "Fujifilm Sensia 100"),
        ("natron", "colorslide/fuji_velvia_50.png", "Fujifilm Velvia 50"),
        ("natron", "colorslide/kodak_ektachrome_100_vs.png", "Kodak Ektachrome 100 VS"),
        ("natron", "colorslide/kodak_elite_chrome_200.png", "Kodak Elite Chrome 200"),
        ("natron", "colorslide/kodak_elite_extracolor_100.png", "Kodak Elite ExtraColor 100"),
        ("natron", "colorslide/kodak_kodachrome_64.png", "Kodak Kodachrome 64"),
        ("natron", "colorslide/lomography_x-pro_slide_200.png", "Lomography X-Pro Slide 200"),
        ("natron", "colorslide/polaroid_polachrome.png", "Polaroid Polachrome"),
    ],
    "film-luts/Instant": [
        ("natron", "instant_consumer/polaroid_px-100uv+_warm.png", "Polaroid PX-100UV+ Warm"),
        ("natron", "instant_consumer/polaroid_px-680.png", "Polaroid PX-680"),
        ("natron", "instant_consumer/polaroid_px-680_cold.png", "Polaroid PX-680 Cold"),
        ("natron", "instant_consumer/polaroid_px-70.png", "Polaroid PX-70"),
        ("natron", "instant_consumer/polaroid_px-70_warm.png", "Polaroid PX-70 Warm"),
        ("natron", "instant_consumer/polaroid_time_zero_(expired).png", "Polaroid Time Zero (Expired)"),
        ("natron", "instant_pro/fuji_fp-100c.png", "Fujifilm FP-100C"),
        ("natron", "instant_pro/fuji_fp-100c_negative.png", "Fujifilm FP-100C Negative"),
        ("natron", "instant_pro/fuji_fp-3000b.png", "Fujifilm FP-3000B"),
        ("natron", "instant_pro/polaroid_665.png", "Polaroid 665"),
        ("natron", "instant_pro/polaroid_669.png", "Polaroid 669"),
        ("natron", "instant_pro/polaroid_690.png", "Polaroid 690"),
    ],
    "cinematic-luts/Black & White": [
        ("natron", "bw/agfa_apx_100.png", "Agfa APX 100"),
        ("natron", "bw/fuji_neopan_1600.png", "Fujifilm Neopan 1600"),
        ("natron", "bw/ilford_delta_3200.png", "Ilford Delta 3200"),
        ("natron", "bw/ilford_fp4_plus_125.png", "Ilford FP4 Plus 125"),
        ("natron", "bw/ilford_hp5_plus_400.png", "Ilford HP5 Plus 400"),
        ("natron", "bw/ilford_pan_f_plus_50.png", "Ilford Pan F Plus 50"),
        ("natron", "bw/kodak_hie_(hs_infra).png", "Kodak HIE Infrared"),
        ("natron", "bw/kodak_t-max_400.png", "Kodak T-Max 400"),
        ("natron", "bw/kodak_tri-x_400.png", "Kodak Tri-X 400"),
        ("natron", "bw/rollei_retro_80s.png", "Rollei Retro 80S"),
    ],
    "film-luts/Fujifilm Style": [
        ("filmsim", "Acros.tif", "Acros"),
        ("filmsim", "Classic_Chrome.tif", "Classic Chrome"),
        ("filmsim", "Provia_Std.tif", "Provia Standard"),
        ("filmsim", "Velvia_Vivid_v2.tif", "Velvia Vivid"),
    ],
    "cinematic-luts/Cinematic": [
        ("openshot", "cinematic_&_blockbuster/city_neon_cinema.cube", "City Neon"),
        ("openshot", "cinematic_&_blockbuster/cool_cinema.cube", "Cool"),
        ("openshot", "cinematic_&_blockbuster/dreamy_cinema.cube", "Dreamy"),
        ("openshot", "cinematic_&_blockbuster/elegant_dark_cinema.cube", "Elegant Dark"),
        ("openshot", "cinematic_&_blockbuster/heroic_cinema.cube", "Heroic"),
        ("openshot", "cinematic_&_blockbuster/romantic_cinema.cube", "Romantic"),
        ("openshot", "cinematic_&_blockbuster/sunlit_cinema.cube", "Sunlit"),
        ("openshot", "cinematic_&_blockbuster/teal_&_orange_cinema.cube", "Teal & Orange"),
        ("openshot", "cinematic_&_blockbuster/teal_cinema.cube", "Teal"),
        ("openshot", "teal_&_orange_vibes/moonlight_orange.cube", "Moonlight Orange"),
        ("openshot", "teal_&_orange_vibes/signature_teal_&_orange.cube", "Teal & Orange Signature"),
        ("openshot", "teal_&_orange_vibes/sunset_orange.cube", "Sunset Orange"),
        ("openshot", "teal_&_orange_vibes/teal_punch.cube", "Teal Punch"),
        ("openshot", "teal_&_orange_vibes/western_sunset.cube", "Western Sunset"),
        ("slay", "luts/slay-01-amber.cube", "slay Amber"),
        ("slay", "luts/slay-03-neon.cube", "slay Neon"),
        ("slay", "luts/slay-04-film.cube", "slay Film"),
        ("slay", "luts/slay-05-portrait.cube", "slay Portrait"),
    ],
    "cinematic-luts/Moody": [
        ("openshot", "dark_&_moody/dramatic_warmth.cube", "Dramatic Warmth"),
        ("openshot", "dark_&_moody/icy_drama.cube", "Icy Drama"),
        ("openshot", "dark_&_moody/mystic_emerald_drama.cube", "Emerald Drama"),
        ("openshot", "dark_&_moody/night_glow.cube", "Night Glow"),
        ("openshot", "dark_&_moody/noir_era.cube", "Noir Era"),
        ("openshot", "dark_&_moody/spy_night.cube", "Spy Night"),
        ("openshot", "dark_&_moody/teal_horror.cube", "Teal Horror"),
        ("openshot", "dark_&_moody/woodland_drama.cube", "Woodland Drama"),
    ],
    "cinematic-luts/Vintage": [
        ("openshot", "film_stock_&_vintage/classic_film.cube", "Classic Film"),
        ("openshot", "film_stock_&_vintage/emerald_film.cube", "Emerald Film"),
        ("openshot", "film_stock_&_vintage/faded_memories.cube", "Faded Memories"),
        ("openshot", "film_stock_&_vintage/golden_years_film.cube", "Golden Years"),
        ("openshot", "film_stock_&_vintage/green_film_pop.cube", "Green Film Pop"),
        ("openshot", "film_stock_&_vintage/low_key_film.cube", "Low Key Film"),
        ("openshot", "film_stock_&_vintage/red_film.cube", "Red Film"),
        ("openshot", "film_stock_&_vintage/standard_film.cube", "Standard Film"),
        ("openshot", "film_stock_&_vintage/vintage_400_film.cube", "Vintage 400"),
        ("openshot", "film_stock_&_vintage/vintage_green_film.cube", "Vintage Green"),
        ("stripedpurple", "Cinema Through The Ages/Striped Purple - 1920s.cube", "1920s"),
        ("stripedpurple", "Cinema Through The Ages/Striped Purple - 1930s.cube", "1930s"),
        ("stripedpurple", "Cinema Through The Ages/Striped Purple - 1960s.cube", "1960s"),
        ("stripedpurple", "Cinema Through The Ages/Striped Purple - 1970s.cube", "1970s"),
        ("stripedpurple", "Cinema Through The Ages/Striped Purple - 1980_s.cube", "1980s"),
        ("slay", "luts/slay-02-fade.cube", "slay Fade"),
    ],
    "creative-luts/Colorful": [
        ("openshot", "vibrant_&_colorful/color_pop.cube", "Color Pop"),
        ("openshot", "vibrant_&_colorful/photo_contrast.cube", "Photo Contrast"),
        ("openshot", "vibrant_&_colorful/valentine_pop.cube", "Valentine Pop"),
        ("openshot", "vibrant_&_colorful/warm_pop.cube", "Warm Pop"),
        ("openshot", "vibrant_&_colorful/warm_to_cool.cube", "Warm to Cool"),
        ("stripedpurple", "Avant Garde/Striped Purple - Amber Haze.cube", "Amber Haze"),
        ("stripedpurple", "Avant Garde/Striped Purple - Arcade Dayz.cube", "Arcade Dayz"),
        ("stripedpurple", "Avant Garde/Striped Purple - Cherry Champagne.cube", "Cherry Champagne"),
        ("stripedpurple", "Avant Garde/Striped Purple - Pendleton.cube", "Pendleton"),
        ("stripedpurple", "Avant Garde/Striped Purple - Polaroid Mint Cream.cube", "Mint Cream"),
        ("stripedpurple", "Avant Garde/Striped Purple - Sugar Plums.cube", "Sugar Plums"),
        ("stripedpurple", "Avant Garde/Striped Purple - Turquoise.cube", "Turquoise"),
        ("stripedpurple", "Avant Garde/Striped Purple - Veggie Crisps.cube", "Veggie Crisps"),
        ("stripedpurple", "Avant Garde/Striped Purple - Woelfelly Neon.cube", "Woelfelly Neon"),
    ],
    "creative-luts/Scenic": [
        ("stripedpurple", "Scenic Moods/Striped Purple - December Greens.cube", "December Greens"),
        ("stripedpurple", "Scenic Moods/Striped Purple - Emerald Malaise.cube", "Emerald Malaise"),
        ("stripedpurple", "Scenic Moods/Striped Purple - Irish Uprising.cube", "Irish Uprising"),
        ("stripedpurple", "Scenic Moods/Striped Purple - Philly Fog.cube", "Philly Fog"),
        ("stripedpurple", "Scenic Moods/Striped Purple - Sea Turtle.cube", "Sea Turtle"),
        ("stripedpurple", "Scenic Moods/Striped Purple - Seagull Cove.cube", "Seagull Cove"),
        ("stripedpurple", "Scenic Moods/Striped Purple - Snowy Blues.cube", "Snowy Blues"),
        ("stripedpurple", "Scenic Moods/Striped Purple - Spring Water.cube", "Spring Water"),
        ("wurst", "Tension 1 warm.tif", "Tension Warm"),
        ("wurst", "Tension 2 cold.tif", "Tension Cold"),
        ("wurst", "v1/Krakow.tif", "Krakow"),
    ],
    "creative-luts/Creative": [
        ("stripedpurple", "Zoology/Striped Purple - Ara.cube", "Ara"),
        ("stripedpurple", "Zoology/Striped Purple - Dodo.cube", "Dodo"),
        ("stripedpurple", "Zoology/Striped Purple - Kraken.cube", "Kraken"),
        ("stripedpurple", "Zoology/Striped Purple - Lobster.cube", "Lobster"),
        ("stripedpurple", "Zoology/Striped Purple - Tiger.cube", "Tiger"),
        ("stripedpurple", "Drink it in!/Striped Purple - Aricha Blend.cube", "Aricha Blend"),
        ("stripedpurple", "Drink it in!/Striped Purple - Blue Lagoon.cube", "Blue Lagoon"),
        ("stripedpurple", "Drink it in!/Striped Purple - Horchata.cube", "Horchata"),
        ("stripedpurple", "Drink it in!/Striped Purple - Vanilla Bean.cube", "Vanilla Bean"),
        ("stripedpurple", "Film Simulations/Infrared/Striped Purple - Infrared.cube", "Infrared"),
        ("stripedpurple", "Film Simulations/Lomography/Lomochrome Purple 4.cube", "Lomochrome Purple"),
    ],
}

# OpenShot keeps its LUTs in src/colors; its AUTHORS.md maps each file to the freshLUTs original.
OPENSHOT_COLORS = "src/colors"


def git(repo: Path, *args: str) -> bytes:
    return subprocess.run(["git", "-C", str(repo), *args], capture_output=True, check=True).stdout


def openshot_authors(repo: Path) -> dict[str, dict[str, str]]:
    """File name -> freshLUTs author, original name and page, from OpenShot's AUTHORS.md."""
    table = {}
    for line in (repo / OPENSHOT_COLORS / "AUTHORS.md").read_text(encoding="utf-8").splitlines():
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if len(cells) >= 9 and cells[0].isdigit():
            table[cells[8]] = {"author": cells[1], "name": cells[2], "page": cells[7]}
    return table


def main() -> int:
    repo_root = Path(__file__).resolve().parents[2]
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--sources", required=True, type=Path, help="folder holding the source clones")
    ap.add_argument("--out", type=Path, default=repo_root / "luts", help="LUT folder (default: luts/)")
    args = ap.parse_args()

    roots = {}
    for key, src in SOURCES.items():
        root = args.sources / src["folder"]
        head = git(root, "rev-parse", "HEAD").decode().strip()
        if head != src["commit"]:
            sys.exit(f"{root} is at {head}; check out {src['commit']} (the commit reviewed for this import)")
        roots[key] = root
    authors = openshot_authors(roots["openshot"])

    # Licence texts, byte for byte from each repository's history (a Windows checkout may have
    # changed their line endings).
    for key, src in SOURCES.items():
        if "licence_file" in src:
            text = git(roots[key], "show", f"{src['commit']}:{src['licence_file']}")
            (args.out / src["licence_copy"]).write_bytes(text)

    report = []
    names = set()
    for place, entries in RECIPE.items():
        folder = args.out / place
        category = place.split("/")[1]
        # Start each category folder afresh, so LUTs dropped from the recipe disappear.
        if folder.exists():
            for old in folder.glob("*.cube"):
                old.unlink()
        for key, rel, name in entries:
            src = SOURCES[key]
            path = roots[key] / (OPENSHOT_COLORS if key == "openshot" else "") / rel
            original = lutconv.read_lut(path)
            table = lutconv.fit(original)
            n0, n = original.shape[0], table.shape[0]
            if path.suffix.lower() == ".cube":
                change = f"resampled from {n0} to {n} points per axis" if n0 != n else "values rounded to 5 decimal places"
            else:
                change = f"converted from a HaldCLUT image ({n0} points per axis) to a {n}-point .cube"
            author = src["author"]
            extra = ""
            if key == "openshot":
                a = authors[Path(rel).name]
                author = f"{a['author']} (freshLUTs)"
                extra = f'originally "{a["name"]}", {a["page"]}'
            full = f"{category} - {name}"
            assert full not in names, full
            names.add(full)
            licence_note = src["licence"] + (f"; full text in {src['licence_copy']}" if "licence_copy" in src else "")
            header = [
                f"MoxoCraft LUT: {full}",
                f"Source: {src['title']}, {src['repo']}",
                f"        commit {src['commit']}, file {rel}",
                f"Author: {author}" + (f"; {extra}" if extra else ""),
                f"Licence: {licence_note}",
                f"Changes by MoxoCraft: {change}; renamed; this header added.",
            ]
            lutconv.write_cube(folder / f"{name}.cube", table, name, header)
            report.append((full, key, rel, author, extra, change))
            print(f"{full:55s} {change}")

    write_sources(args.out, report)
    print(f"wrote {len(report)} LUTs and SOURCES.txt to {args.out}")
    return 0


def write_sources(out: Path, report) -> None:
    lines = [
        "MoxoCraft LUTs: sources, credits and changes",
        "=" * 44,
        "",
        "The LUTs in film-luts/Film are derived from spektrafilm; see CHANGELOG.txt and",
        "SPEKTRAFILM_LICENSE.txt. Every other LUT comes from one of the collections below. Kodak,",
        "Fujifilm, Ilford, Agfa, Polaroid, Lomography, Rollei and the film names are trademarks",
        "of their owners, used only to describe the looks simulated; they don't endorse MoxoCraft.",
        "",
        "Changes made to all of them by MoxoCraft (tools/import-luts/import_luts.py): HaldCLUT",
        "images converted to .cube files; LUTs over 33 points per axis resampled to 33 with",
        "tetrahedral interpolation; .cube values rounded to 5 decimal places; files renamed; a",
        "header naming the source added. The MoxoCraft LUT plug-ins store each LUT inside their",
        ".wasm at 16-bit precision.",
        "",
        "Collections",
        "-----------",
    ]
    for key, src in SOURCES.items():
        lines += ["", src["title"], f"  {src['repo']} (commit {src['commit']})", f"  Author: {src['author']}", f"  Licence: {src['licence']}"]
        if "licence_copy" in src:
            lines.append(f"  Licence text: {src['licence_copy']}")
        if "notes" in src:
            lines.append(f"  {src['notes']}")
    lines += ["", "LUTs", "----", ""]
    for full, key, rel, author, extra, change in report:
        lines.append(f"{full}")
        lines.append(f"  from {SOURCES[key]['title']}: {rel}")
        lines.append(f"  by {author}" + (f"; {extra}" if extra else ""))
        lines.append(f"  {SOURCES[key]['licence'].split(' (')[0]}; {change}")
    (out / "SOURCES.txt").write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")


if __name__ == "__main__":
    sys.exit(main())
