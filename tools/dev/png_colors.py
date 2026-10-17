#!/usr/bin/env python3
"""Fraction of a PNG's pixels that exactly match each given colour.

Usage: png_colors.py <file.png> <rrggbb> [<rrggbb> ...]   -> JSON {"rrggbb": fraction}
Used by the styling screenshot tests (examples/counter/tests/gallery.rs).
"""
import json
import sys

from PIL import Image

img = Image.open(sys.argv[1]).convert("RGB")
counts = {}
for px in img.getdata():
    counts[px] = counts.get(px, 0) + 1
total = img.width * img.height
out = {}
for hexcolor in sys.argv[2:]:
    key = tuple(int(hexcolor[i : i + 2], 16) for i in (0, 2, 4))
    out[hexcolor] = counts.get(key, 0) / total
print(json.dumps(out))
