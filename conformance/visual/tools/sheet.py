#!/usr/bin/env python3
"""VIS1 contact sheet: per probe, the VS Code crop next to ours, with the failed checks.

Usage: sheet.py <theme> <ours dir> <out.png>
  <ours dir>: target/vis1/<theme> (or target/vis1-stock/<theme>), written by
  `cargo nextest run -p svelters-testkit --features snapshots -E 'test(default_look_matches_vscode)'`
  (crops need the `snapshots` feature); its ../report.md gives the per-check results.
"""
import json
import sys
from pathlib import Path
from PIL import Image, ImageDraw

theme, ours_dir, out = sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3])
root = Path(__file__).resolve().parents[1]
ref_dir = root / 'vscode' / theme
ours = json.loads((ours_dir / 'metrics.json').read_text())
report = (ours_dir.parent / 'report.md').read_text() if (ours_dir.parent / 'report.md').exists() else ''
fails = {}
for line in report.splitlines():
    if line.startswith('| FAIL '):
        cid = line.split('|')[1].split()[1]
        t, node, state, check = cid.split('/')
        if t == theme:
            fails.setdefault((node, state), []).append(check)

ROW_H, COL_W, LABEL_W, PAD = 44, 300, 260, 6
bg = (128, 128, 128)
rows = []
seen = set()
for it in ours['items']:
    key = (it['element'], it['state'])
    if key in seen or not it.get('metrics'):
        continue
    seen.add(key)
    rows.append(it)
H = PAD + len(rows) * (ROW_H + PAD) + 20
sheet = Image.new('RGB', (LABEL_W + 2 * COL_W + 4 * PAD, H), bg)
d = ImageDraw.Draw(sheet)
d.text((PAD, 4), f'{theme}: VS Code 1.133 (left) | svelters-egui (right)', fill=(0, 0, 0))


def fit(im):
    im = im.convert('RGB')
    if im.width > COL_W or im.height > ROW_H:
        im.thumbnail((COL_W, ROW_H))
    return im


y = 20
for it in rows:
    node, state = it['element'], it['state']
    f = fails.get((node, state), [])
    label = f"{node} {state}\n-> {it.get('reference')}\n" + ('FAIL ' + ' '.join(f) if f else 'ok')
    d.text((PAD, y + 2), label, fill=(120, 0, 0) if f else (0, 0, 0))
    ref = ref_dir / f"{it.get('reference')}.png"
    if ref.exists():
        sheet.paste(fit(Image.open(ref)), (LABEL_W + PAD, y))
    mine = ours_dir / it['png']
    if mine.exists():
        sheet.paste(fit(Image.open(mine)), (LABEL_W + COL_W + 2 * PAD, y))
    y += ROW_H + PAD
sheet.save(out, optimize=True)
print(out, sheet.size, out.stat().st_size, 'bytes')
