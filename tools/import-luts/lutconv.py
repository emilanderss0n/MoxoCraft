"""Reads 3D LUTs (.cube files and HaldCLUT images) and writes them as .cube files.

Used by import_luts.py. Tables are numpy arrays of shape (n, n, n, 3), indexed
[blue][green][red], the order a .cube file lists them in (red changing fastest).
"""

from pathlib import Path

import numpy as np

# LUTs larger than this are resampled down to it: 33 points is what grading software exports by
# default, and finer tables only cost plug-in size (PhotoCraft caps a plug-in at 32 MiB).
MAX_SIZE = 33


def read_cube(path: Path) -> np.ndarray:
    size = None
    lo, hi = np.zeros(3), np.ones(3)
    values = []
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        word = line.split()[0].upper()
        if word == "LUT_3D_SIZE":
            size = int(line.split()[1])
        elif word == "LUT_1D_SIZE":
            raise ValueError(f"{path}: 1D LUTs aren't supported")
        elif word == "DOMAIN_MIN":
            lo = np.array([float(v) for v in line.split()[1:4]])
        elif word == "DOMAIN_MAX":
            hi = np.array([float(v) for v in line.split()[1:4]])
        elif word[0].isdigit() or word[0] in "-.":
            values.append([float(v) for v in line.split()[:3]])
    if size is None or len(values) != size**3:
        raise ValueError(f"{path}: expected {size}^3 entries, found {len(values)}")
    if not (np.allclose(lo, 0) and np.allclose(hi, 1)):
        raise ValueError(f"{path}: input domain {lo}..{hi} isn't 0..1")
    return np.array(values).reshape(size, size, size, 3)


def read_hald(path: Path) -> np.ndarray:
    """A HaldCLUT image: level L holds an L^2-point LUT as an L^3 x L^3 picture."""
    if path.suffix.lower() in (".tif", ".tiff"):
        import tifffile

        img = tifffile.imread(path)
    else:
        from PIL import Image

        img = np.asarray(Image.open(path).convert("RGB"))
    h, w = img.shape[:2]
    level = round(w ** (1 / 3))
    if h != w or level**3 != w:
        raise ValueError(f"{path}: {w}x{h} isn't a HaldCLUT size")
    n = level * level
    scale = 65535.0 if img.dtype == np.uint16 else 255.0
    return img[..., :3].reshape(n, n, n, 3).astype(np.float64) / scale


def read_lut(path: Path) -> np.ndarray:
    return read_cube(path) if path.suffix.lower() == ".cube" else read_hald(path)


def apply(table: np.ndarray, rgb: np.ndarray) -> np.ndarray:
    """Looks up colors (..., 3) in 0..1 with tetrahedral interpolation."""
    n = table.shape[0]
    p = np.clip(rgb, 0.0, 1.0) * (n - 1)
    i = np.minimum(np.floor(p).astype(np.int64), n - 2)
    f = p - i
    r, g, b = i[..., 0], i[..., 1], i[..., 2]
    fr, fg, fb = f[..., 0:1], f[..., 1:2], f[..., 2:3]

    def at(dr, dg, db):
        return table[b + db, g + dg, r + dr]

    c000, c111 = at(0, 0, 0), at(1, 1, 1)
    out = np.empty(rgb.shape, dtype=np.float64)
    # The six tetrahedra of the unit cube, picked by the order of the fractions.
    cases = [
        ((fr >= fg) & (fg >= fb), (1, 0, 0), (1, 1, 0), fr, fg, fb),
        ((fr >= fb) & (fb > fg), (1, 0, 0), (1, 0, 1), fr, fb, fg),
        ((fb > fr) & (fr >= fg), (0, 0, 1), (1, 0, 1), fb, fr, fg),
        ((fg > fr) & (fr >= fb), (0, 1, 0), (1, 1, 0), fg, fr, fb),
        ((fg >= fb) & (fb > fr), (0, 1, 0), (0, 1, 1), fg, fb, fr),
        ((fb > fg) & (fg > fr), (0, 0, 1), (0, 1, 1), fb, fg, fr),
    ]
    done = np.zeros(rgb.shape[:-1] + (1,), dtype=bool)
    for mask, a, bb, x, y, z in cases:
        mask = mask & ~done
        v = (1 - x) * c000 + (x - y) * at(*a) + (y - z) * at(*bb) + z * c111
        out = np.where(mask, v, out)
        done |= mask
    return out


def resample(table: np.ndarray, size: int) -> np.ndarray:
    """The LUT evaluated on a size^3 grid."""
    if table.shape[0] == size:
        return table
    t = np.linspace(0.0, 1.0, size)
    b, g, r = np.meshgrid(t, t, t, indexing="ij")
    return apply(table, np.stack([r, g, b], axis=-1))


def fit(table: np.ndarray) -> np.ndarray:
    """Resampled to MAX_SIZE when larger; smaller tables are kept exactly as they are."""
    return resample(table, MAX_SIZE) if table.shape[0] > MAX_SIZE else table


def write_cube(path: Path, table: np.ndarray, title: str, header: list[str]) -> None:
    n = table.shape[0]
    lines = [f"# {h}" if h else "#" for h in header]
    lines += [f'TITLE "{title}"', f"LUT_3D_SIZE {n}", "DOMAIN_MIN 0.0 0.0 0.0", "DOMAIN_MAX 1.0 1.0 1.0"]
    # Five decimals: steps of 1/100000, finer than the plug-in's 16-bit storage.
    flat = table.reshape(-1, 3)
    lines += [f"{r:.5f} {g:.5f} {b:.5f}" for r, g, b in flat]
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
