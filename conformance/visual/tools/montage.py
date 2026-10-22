#!/usr/bin/env python3
"""VIS1: a labelled grid of every crop in a directory (for reviewing a capture). Usage:
montage.py <dir with *.png> <out.png> [max_w]"""
import sys
from pathlib import Path
from PIL import Image, ImageDraw

src, out = Path(sys.argv[1]), Path(sys.argv[2])
max_w = int(sys.argv[3]) if len(sys.argv) > 3 else 360
files = sorted(p for p in src.glob('*.png'))
tiles = []
for p in files:
    im = Image.open(p).convert('RGB')
    if im.width > max_w or im.height > 220:
        im.thumbnail((max_w, 220))
    tiles.append((p.stem, im))
cols, pad, label = 4, 8, 14
col_w = max_w + pad
rows = [tiles[i:i + cols] for i in range(0, len(tiles), cols)]
H = sum(max(t[1].height for t in r) + label + pad for r in rows) + pad
sheet = Image.new('RGB', (cols * col_w + pad, H), (128, 128, 128))
d = ImageDraw.Draw(sheet)
y = pad
for r in rows:
    for i, (name, im) in enumerate(r):
        x = pad + i * col_w
        d.text((x, y), name, fill=(0, 0, 0))
        sheet.paste(im, (x, y + label))
    y += max(t[1].height for t in r) + label + pad
sheet.save(out)
print(out, sheet.size)
