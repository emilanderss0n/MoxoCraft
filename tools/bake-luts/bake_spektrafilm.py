"""Bakes MoxoCraft's film-stock LUTs from spektrafilm and writes luts/CHANGELOG.txt.

spektrafilm (https://github.com/andreavolpato/spektrafilm, by Andrea Volpato) simulates film
stocks from measured data. Its profiles and every LUT made from them are licensed CC BY-SA 4.0
(luts/SPEKTRAFILM_LICENSE.txt), which requires credit, the licence notice, and a record of
changes to travel with the LUTs. This script is that record: it bakes each LUT with the settings
below, renames it, and writes the changes into luts/CHANGELOG.txt.

spektrafilm's own code is GPLv3. It is only run here, on your machine, to produce the LUT files;
none of it is copied into MoxoCraft.

Setup (once, in its own folder outside this repository):

    git clone https://github.com/andreavolpato/spektrafilm
    cd spektrafilm && git checkout 3bb2c2d2801ff68b92019cf1dbcbb133d60832bc
    py -3.13 -m venv ../venv && ../venv/Scripts/python -m pip install .

Then, from the MoxoCraft folder:

    python tools/bake-luts/bake_spektrafilm.py --tool <venv>/Scripts/spektrafilm-lut.exe --source <spektrafilm clone>

and rebuild the plug-ins (./build.ps1).
"""

import argparse
import datetime
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

# Film profile, print profile, menu name. Each film is printed on the paper (or print film)
# spektrafilm lists as its own target print. Slide films are left out: spektrafilm's LUT tool
# always sends the film through a print, but spektrafilm itself shows slides scanned directly.
STOCKS = [
    ("kodak_portra_160", "kodak_portra_endura", "Kodak Portra 160"),
    ("kodak_portra_400", "kodak_portra_endura", "Kodak Portra 400"),
    ("kodak_portra_800", "kodak_portra_endura", "Kodak Portra 800"),
    ("kodak_portra_800_push1", "kodak_portra_endura", "Kodak Portra 800 (Push 1)"),
    ("kodak_portra_800_push2", "kodak_portra_endura", "Kodak Portra 800 (Push 2)"),
    ("kodak_gold_200", "kodak_portra_endura", "Kodak Gold 200"),
    ("kodak_ultramax_400", "kodak_portra_endura", "Kodak Ultramax 400"),
    ("kodak_ektar_100", "kodak_portra_endura", "Kodak Ektar 100"),
    ("fujifilm_c200", "fujifilm_crystal_archive_typeii", "Fujifilm C200"),
    ("fujifilm_pro_400h", "fujifilm_crystal_archive_typeii", "Fujifilm Pro 400H"),
    ("fujifilm_xtra_400", "fujifilm_crystal_archive_typeii", "Fujifilm X-Tra 400"),
    ("kodak_vision3_50d", "kodak_2383", "Kodak Vision3 50D (2383 print)"),
    ("kodak_vision3_250d", "kodak_2383", "Kodak Vision3 250D (2383 print)"),
    ("kodak_vision3_200t", "kodak_2383", "Kodak Vision3 200T (2383 print)"),
    ("kodak_vision3_500t", "kodak_2383", "Kodak Vision3 500T (2383 print)"),
]

# Where sRGB white sits on the film, in stops above mid-grey. spektrafilm's default for sRGB
# input is 4.0, which brightens every photo by over a stop (mid-grey 0.46 comes out at 0.73).
# 2.47 is sRGB's own headroom (log2(1 / 0.18)): mid-grey stays put, so the LUT changes color
# and contrast, not exposure. (Film Lab, https://github.com/thedevmark/film-lab, made the same
# choice for its Kodak Gold 200 LUT.)
STOPS_ABOVE_MIDGRAY = 2.47

# Points per axis. 33 matches 65 to within 2/255 at worst (0.12/255 on average) for these LUTs,
# at an eighth of the size.
RESOLUTION = 33


def spec_toml(film: str, print_: str) -> str:
    # Set through a spec file: in spektrafilm 0.3.4 the --stops-above-gray command-line flag is
    # silently ignored, and the spec field is not.
    return (
        f'film_profile = "{film}"\n'
        f'print_profiles = ["{print_}"]\n'
        'input_color_space = "sRGB"\n'
        'output_color_space = "sRGB"\n'
        'topology = "1lut"\n'
        f"resolution = {RESOLUTION}\n"
        f"stops_above_midgray = {STOPS_ABOVE_MIDGRAY}\n"
    )


def main() -> int:
    repo = Path(__file__).resolve().parents[2]
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--tool", required=True, type=Path, help="spektrafilm-lut executable")
    ap.add_argument("--source", type=Path, help="spektrafilm clone, to record its exact commit")
    ap.add_argument("--out", type=Path, default=repo / "luts", help="LUT folder (default: luts/)")
    args = ap.parse_args()

    commit = "unknown"
    if args.source:
        commit = subprocess.run(["git", "-C", str(args.source), "rev-parse", "HEAD"], capture_output=True, text=True, check=True).stdout.strip()
    # The film stocks are the Film category of the Film LUTs plug-in.
    film_dir = args.out / "film-luts" / "Film"
    film_dir.mkdir(parents=True, exist_ok=True)
    version = None
    written = []
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        for film, print_, name in STOCKS:
            spec = tmp / f"{film}.toml"
            spec.write_text(spec_toml(film, print_), encoding="utf-8")
            out = tmp / film
            subprocess.run([str(args.tool), "build", "--from", str(spec), "--name", film, "--out", str(out)], check=True, capture_output=True, text=True)
            bundle = out / film
            meta = json.loads((bundle / "bundle.json").read_text(encoding="utf-8"))
            stops = meta["input_exposure"]["stops_above_midgray"]
            if abs(stops - STOPS_ABOVE_MIDGRAY) > 1e-6:
                sys.exit(f"{film}: the bake used {stops} stops above mid-grey, not {STOPS_ABOVE_MIDGRAY}")
            cubes = list(bundle.glob("*.cube"))
            if len(cubes) != 1:
                sys.exit(f"{film}: expected one .cube in the bundle, found {len(cubes)}")
            # spektrafilm writes its version into each .cube header ("# spektrafilm: 0.3.4").
            for line in cubes[0].read_text(encoding="utf-8").splitlines()[:40]:
                if line.startswith("# spektrafilm:"):
                    version = version or line.split(":", 1)[1].strip()
            # The .cube keeps spektrafilm's own header, which carries the credit and licence.
            shutil.copyfile(cubes[0], film_dir / f"{name}.cube")
            written.append((name, film, print_))
            print(f"baked {name}")

    today = datetime.date.today().isoformat()
    lines = [
        "MoxoCraft LUTs: changes from spektrafilm",
        "=" * 40,
        "",
        "The .cube files in film-luts/Film are derived from spektrafilm by Andrea Volpato",
        "(https://github.com/andreavolpato/spektrafilm) and licensed CC BY-SA 4.0; see",
        "SPEKTRAFILM_LICENSE.txt. Modified by MoxoCraft, as described below.",
        "",
        f"Baked {today} by tools/bake-luts/bake_spektrafilm.py with spektrafilm's LUT creator",
        f"(spektrafilm {version or 'version unknown'}, commit {commit}).",
        "",
        "Settings (spektrafilm's defaults unless noted):",
        "  - 1-LUT bundle, sRGB in and sRGB out, " + f"{RESOLUTION} points per axis.",
        "  - Each film printed on its own target print (listed below).",
        f"  - stops_above_midgray = {STOPS_ABOVE_MIDGRAY} (changed; spektrafilm's default for sRGB",
        "    input is 4.0). Keeps mid-grey where it was instead of brightening every photo by",
        "    over a stop, so the LUT changes colour and contrast, not exposure.",
        "",
        "Other changes:",
        "  - Files renamed from spektrafilm's bundle names to the film names below.",
        "  - The MoxoCraft - Film LUTs plug-in stores each LUT inside its .wasm at 16-bit precision",
        "    (steps of about 0.00003); the values are otherwise unchanged.",
        "",
        "LUTs (menu name: film profile -> print profile):",
    ]
    lines += [f"  - {name}: {film} -> {print_}" for name, film, print_ in written]
    # LF line endings on every system, like the rest of luts/ (luts/.gitattributes stops Git
    # changing them).
    (args.out / "CHANGELOG.txt").write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
    print(f"wrote {len(written)} LUTs to {film_dir} and CHANGELOG.txt to {args.out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
