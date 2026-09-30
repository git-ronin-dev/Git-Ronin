#!/usr/bin/env python3
"""Recolours the logo master into the app's dark palette.

The master (`logo.png` in the owner's design files) is a two-tone drawing:
a light figure and ring on a dark disc. Each pixel is placed on the line
between those two tones and redrawn between the palette's colours: the disc
becomes sumi ink (`--rn-canvas`), the figure vermilion (`--rn-accent`) and
the outer ring gold (`--rn-gold`). Colours are read from the dark block of
ui/src/ui/tokens.css, so the logo follows palette changes.

    scripts/brand/recolor-logo.py logo.png src-tauri/icons/source.png
    npx tauri icon src-tauri/icons/source.png -o <tmp>   # then copy the sizes

Needs Pillow and NumPy.
"""

import re
import sys
from pathlib import Path

import numpy as np
from PIL import Image

TOKENS = Path(__file__).resolve().parents[2] / "ui/src/ui/tokens.css"
# The ring starts outside this fraction of the disc's radius; the figure
# stays inside it.
RING_FROM = 0.89


def palette():
    dark = re.search(r"^:root\s*\{([^}]*)\}", TOKENS.read_text(), re.M).group(1)
    tokens = dict(re.findall(r"--rn-([\w-]+):\s*#([0-9a-f]{6})", dark, re.I))
    rgb = lambda name: np.array([int(tokens[name][i : i + 2], 16) for i in (0, 2, 4)], float)
    return rgb("canvas"), rgb("accent"), rgb("gold")


def main(src, dst):
    img = np.asarray(Image.open(src).convert("RGBA")).astype(float)
    rgb, alpha = img[..., :3], img[..., 3]
    opaque = alpha > 250

    # The two tones of the master: darkest and lightest opaque pixels.
    lum = rgb.mean(axis=2)
    dark = rgb[opaque & (lum < np.percentile(lum[opaque], 20))].mean(axis=0)
    light = rgb[opaque & (lum > np.percentile(lum[opaque], 90))].mean(axis=0)
    axis = light - dark
    t = np.clip(((rgb - dark) @ axis) / (axis @ axis), 0, 1)[..., None]

    ink, vermilion, gold = palette()
    h, w = alpha.shape
    ys, xs = np.nonzero(alpha > 0)
    cy, cx = (ys.min() + ys.max()) / 2, (xs.min() + xs.max()) / 2
    radius = max(ys.max() - ys.min(), xs.max() - xs.min()) / 2
    yy, xx = np.mgrid[0:h, 0:w]
    ring = (np.hypot(yy - cy, xx - cx) > RING_FROM * radius)[..., None]

    fg = np.where(ring, gold, vermilion)
    out = ink * (1 - t) + fg * t
    Image.fromarray(np.dstack([out, alpha]).round().astype(np.uint8), "RGBA").save(dst)


if __name__ == "__main__":
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    main(sys.argv[1], sys.argv[2])
