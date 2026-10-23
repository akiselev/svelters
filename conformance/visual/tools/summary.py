#!/usr/bin/env python3
"""VIS1: one line per reference item (size, radius, border, colours, font) of a metrics.json, colours
as #RRGGBB[AA]. Usage: summary.py conformance/visual/vscode/dark-modern/metrics.json [filter]"""
import json
import re
import sys


def hexc(c):
    if not c:
        return '-'
    m = re.match(r'rgba?\(([\d.]+), ([\d.]+), ([\d.]+)(?:, ([\d.]+))?\)', c)
    if not m:
        return c
    r, g, b = (int(float(x)) for x in m.groups()[:3])
    a = m.group(4)
    s = f'#{r:02X}{g:02X}{b:02X}'
    if a is not None:
        s += f'{round(float(a) * 255):02X}'
    return s


d = json.load(open(sys.argv[1]))
flt = sys.argv[2] if len(sys.argv) > 2 else ''
for it in d['items']:
    k = f"{it['element']}-{it['state']}"
    if flt not in k:
        continue
    m = it['metrics']
    if not m:
        print(k, 'MISSING')
        continue
    r = m.get('rect') or {}
    print(f"{k:32} {r.get('w')}x{r.get('h')} pad={m.get('padding')} r={m.get('radius')} b={m.get('border_width')} {hexc(m.get('border_color'))} "
          f"bg={hexc(m.get('bg'))} fg={hexc(m.get('fg'))} font={m.get('font_size')}/{m.get('font_weight')} lh={m.get('line_height')} "
          f"out={m.get('outline')} sh={m.get('box_shadow')} op={m.get('opacity')}")
