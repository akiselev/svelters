#!/usr/bin/env python3
"""IC1 icon toolchain: lint, build and contact sheets for a custom icon set (plans/research/ICONS.md §3, §5).

    tools/icons/icons.py lint  <set>                       # the §3 rules; exit 1 on any finding
    tools/icons/icons.py build <set> <out> [--only ids]    # badges composed, white copies, manifest, id code
    tools/icons/icons.py sheet <set> <out.png> [--codicons <dir>] [--zoom <zoom.png>] [--only ids] [--sizes 12,16]
    tools/icons/icons.py hash  <set> [ids]                 # approval hashes for icons.toml

A set is a directory with `icons.toml` (see tools/icons/SPEC.md) and one `<px>/<id>.svg` directory of
masters per optical size. The per-size rules (stroke widths and alignment, minimum gap, dash, badge box)
come from tools/icons/sizes.toml, or the set's own sizes.toml. Badges come from
tools/icons/badges/<px>/<name>.svg. lint, build and hash use
the standard library only (Python 3.11+); sheet needs Pillow and rsvg-convert (or resvg with
--renderer resvg).
"""
import argparse
import hashlib
import io
import json
import math
import os
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib
import xml.etree.ElementTree as ET

HERE = os.path.dirname(os.path.abspath(__file__))
BADGE_DIR = os.path.join(HERE, 'badges')
SVG_NS = 'http://www.w3.org/2000/svg'
ET.register_namespace('', SVG_NS)

SIZES_TOML = os.path.join(HERE, 'sizes.toml')
OPACITIES = {1.0, 0.4}
FRINGE_OPACITIES = {0.2, 0.45}         # the results fringe, allowed only in `<domain>.result.*` ids
BADGES = ('add', 'remove', 'run', 'check', 'error', 'settings', 'link', 'lock', 'queued', 'stale')
KNOCKOUT_ID = 'badge-knockout'
MAX_BYTES = 2048
DEFAULT_DOMAINS = ('chrome', 'vcs', 'status', 'doc', 'mcad', 'ecad', 'sim', 'select')
ID_RE = re.compile(r'^[a-z][a-z0-9]*(\.[a-z0-9]+(-[a-z0-9]+)*)+$')

ELEMENTS = {'svg', 'g', 'defs', 'path', 'rect', 'circle', 'ellipse', 'line', 'polyline', 'polygon', 'mask',
            'clipPath', 'title', 'desc'}
SHAPES = {'path', 'rect', 'circle', 'ellipse', 'line', 'polyline', 'polygon'}
ATTRIBUTES = {'xmlns', 'width', 'height', 'viewBox', 'fill', 'stroke', 'stroke-width', 'stroke-linecap',
              'stroke-linejoin', 'stroke-dasharray', 'stroke-miterlimit', 'fill-rule', 'clip-rule', 'opacity',
              'fill-opacity', 'stroke-opacity', 'd', 'x', 'y', 'x1', 'y1', 'x2', 'y2', 'cx', 'cy', 'r', 'rx', 'ry',
              'points', 'id', 'mask', 'clip-path', 'maskUnits', 'maskContentUnits', 'clipPathUnits'}
INHERITED = ('fill', 'stroke', 'stroke-width', 'stroke-linecap', 'stroke-linejoin', 'stroke-dasharray')
MASK_COLOURS = {'#fff', '#ffffff', 'white', '#000', '#000000', 'black'}
EPS = 1e-6


def local(tag):
    return tag.rsplit('}', 1)[-1]


# ---------------------------------------------------------------- path geometry

NUM_RE = re.compile(r'[+-]?(?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?')
ARGS = {'M': 2, 'L': 2, 'H': 1, 'V': 1, 'C': 6, 'S': 4, 'Q': 4, 'T': 2, 'A': 7, 'Z': 0}


class _Cursor:
    def __init__(self, d):
        self.d, self.i = d, 0

    def skip(self):
        while self.i < len(self.d) and self.d[self.i] in ' \t\r\n,':
            self.i += 1

    def done(self):
        self.skip()
        return self.i >= len(self.d)

    def at_command(self):
        self.skip()
        return self.i < len(self.d) and self.d[self.i].upper() in ARGS

    def number(self):
        self.skip()
        m = NUM_RE.match(self.d, self.i)
        if not m:
            raise ValueError(f'bad number at {self.i} in {self.d!r}')
        self.i = m.end()
        return float(m.group())

    def flag(self):
        self.skip()
        if self.i >= len(self.d) or self.d[self.i] not in '01':
            raise ValueError(f'bad arc flag at {self.i} in {self.d!r}')
        self.i += 1
        return self.d[self.i - 1] == '1'


def _bezier(points, n=12):
    out = []
    for k in range(1, n + 1):
        t = k / n
        pts = list(points)
        while len(pts) > 1:
            pts = [(a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t) for a, b in zip(pts, pts[1:])]
        out.append(pts[0])
    return out


def _arc(p0, rx, ry, phi, large, sweep, p1, n=24):
    """Sample an SVG elliptical arc (endpoint parametrisation, SVG 1.1 F.6.5)."""
    if rx == 0 or ry == 0 or p0 == p1:
        return [p1]
    rx, ry = abs(rx), abs(ry)
    c, s = math.cos(math.radians(phi)), math.sin(math.radians(phi))
    dx, dy = (p0[0] - p1[0]) / 2, (p0[1] - p1[1]) / 2
    x1, y1 = c * dx + s * dy, -s * dx + c * dy
    lam = x1 * x1 / (rx * rx) + y1 * y1 / (ry * ry)
    if lam > 1:
        rx, ry = rx * math.sqrt(lam), ry * math.sqrt(lam)
    num = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1
    den = rx * rx * y1 * y1 + ry * ry * x1 * x1
    k = math.sqrt(max(0.0, num / den)) * (-1 if large == sweep else 1)
    cx1, cy1 = k * rx * y1 / ry, -k * ry * x1 / rx
    cx = c * cx1 - s * cy1 + (p0[0] + p1[0]) / 2
    cy = s * cx1 + c * cy1 + (p0[1] + p1[1]) / 2

    def ang(ux, uy, vx, vy):
        a = math.atan2(ux * vy - uy * vx, ux * vx + uy * vy)
        return a

    t1 = ang(1, 0, (x1 - cx1) / rx, (y1 - cy1) / ry)
    dt = ang((x1 - cx1) / rx, (y1 - cy1) / ry, (-x1 - cx1) / rx, (-y1 - cy1) / ry)
    if not sweep and dt > 0:
        dt -= 2 * math.pi
    elif sweep and dt < 0:
        dt += 2 * math.pi
    out = []
    for i in range(1, n + 1):
        t = t1 + dt * i / n
        ex, ey = rx * math.cos(t), ry * math.sin(t)
        out.append((c * ex - s * ey + cx, s * ex + c * ey + cy))
    return out


def path_segments(d):
    """Segments of a path: ('line', p0, p1) and ('curve', p0, samples)."""
    cur = _Cursor(d)
    segs = []
    pos = start = (0.0, 0.0)
    cmd = None
    prev_ctrl = None
    while not cur.done():
        if cur.at_command():
            cmd = cur.d[cur.i]
            cur.i += 1
        elif cmd is None:
            raise ValueError(f'path does not start with a command: {d!r}')
        up, rel = cmd.upper(), cmd.islower()
        if up == 'Z':
            if pos != start:
                segs.append(('line', pos, start))
            pos, prev_ctrl = start, None
            continue
        ox, oy = pos if rel else (0.0, 0.0)
        if up == 'M':
            pos = start = (ox + cur.number(), oy + cur.number())
            cmd = 'l' if rel else 'L'          # further pairs are implicit linetos
            prev_ctrl = None
            continue
        if up in 'LHV':
            if up == 'L':
                p = (ox + cur.number(), oy + cur.number())
            elif up == 'H':
                p = (cur.number() + (pos[0] if rel else 0.0), pos[1])
            else:
                p = (pos[0], cur.number() + (pos[1] if rel else 0.0))
            segs.append(('line', pos, p))
            pos, prev_ctrl = p, None
        elif up in 'CSQT':
            if up == 'C':
                c1 = (ox + cur.number(), oy + cur.number())
                c2 = (ox + cur.number(), oy + cur.number())
                p = (ox + cur.number(), oy + cur.number())
                ctrl = [pos, c1, c2, p]
            elif up == 'S':
                c1 = (2 * pos[0] - prev_ctrl[0], 2 * pos[1] - prev_ctrl[1]) if prev_ctrl else pos
                c2 = (ox + cur.number(), oy + cur.number())
                p = (ox + cur.number(), oy + cur.number())
                ctrl = [pos, c1, c2, p]
            elif up == 'Q':
                c1 = (ox + cur.number(), oy + cur.number())
                p = (ox + cur.number(), oy + cur.number())
                ctrl = [pos, c1, p]
            else:
                c1 = (2 * pos[0] - prev_ctrl[0], 2 * pos[1] - prev_ctrl[1]) if prev_ctrl else pos
                p = (ox + cur.number(), oy + cur.number())
                ctrl = [pos, c1, p]
            segs.append(('curve', pos, _bezier(ctrl)))
            prev_ctrl = ctrl[-2]
            pos = p
        elif up == 'A':
            rx, ry, phi = cur.number(), cur.number(), cur.number()
            large, sweep = cur.flag(), cur.flag()
            p = (ox + cur.number(), oy + cur.number())
            segs.append(('curve', pos, _arc(pos, rx, ry, phi, large, sweep, p)))
            pos, prev_ctrl = p, None
    return segs


def _num(el, name, default=0.0):
    v = el.get(name)
    return default if v is None else float(v)


def shape_segments(el):
    """Segments of any basic shape, as path_segments returns them."""
    tag = local(el.tag)
    if tag == 'path':
        return path_segments(el.get('d', ''))
    if tag == 'line':
        return [('line', (_num(el, 'x1'), _num(el, 'y1')), (_num(el, 'x2'), _num(el, 'y2')))]
    if tag in ('polyline', 'polygon'):
        vals = [float(v) for v in NUM_RE.findall(el.get('points', ''))]
        pts = list(zip(vals[::2], vals[1::2]))
        if tag == 'polygon' and pts:
            pts.append(pts[0])
        return [('line', a, b) for a, b in zip(pts, pts[1:])]
    if tag == 'rect':
        x, y, w, h = _num(el, 'x'), _num(el, 'y'), _num(el, 'width'), _num(el, 'height')
        rx = el.get('rx') or el.get('ry')
        r = min(float(rx), w / 2, h / 2) if rx else 0.0
        segs = [('line', (x + r, y), (x + w - r, y)), ('line', (x + w, y + r), (x + w, y + h - r)),
                ('line', (x + w - r, y + h), (x + r, y + h)), ('line', (x, y + h - r), (x, y + r))]
        if r:
            for cx, cy in ((x + r, y + r), (x + w - r, y + r), (x + w - r, y + h - r), (x + r, y + h - r)):
                segs.append(('curve', (cx, cy), [(cx + r * math.cos(t / 8 * math.pi), cy + r * math.sin(t / 8 * math.pi))
                                                  for t in range(16)]))
        return segs
    if tag in ('circle', 'ellipse'):
        cx, cy = _num(el, 'cx'), _num(el, 'cy')
        rx = _num(el, 'r') if tag == 'circle' else _num(el, 'rx')
        ry = _num(el, 'r') if tag == 'circle' else _num(el, 'ry')
        samples = [(cx + rx * math.cos(t / 16 * math.pi), cy + ry * math.sin(t / 16 * math.pi)) for t in range(32)]
        return [('curve', (cx - rx, cy), samples)]
    return []


def frac(v):
    return v - math.floor(v)


def on_phase(v, phase):
    f = frac(v)
    return abs(f - phase) < EPS or (phase == 0.0 and abs(f - 1.0) < EPS)


# ---------------------------------------------------------------- lint

class Finding:
    def __init__(self, icon, size, rule, msg):
        self.icon, self.size, self.rule, self.msg = icon, size, rule, msg

    def __str__(self):
        where = f'{self.size}/{self.icon}' if self.size else self.icon
        return f'{where}: [{self.rule}] {self.msg}'

    def __repr__(self):
        return f'Finding({self})'


def _fmt(v):
    return f'{v:g}'


class SizeSpec:
    """One optical size's rules, from a [size.<px>] table of sizes.toml."""

    def __init__(self, px, t):
        self.px = px
        self.required = bool(t.get('required', False))
        centre = {'half': 0.5, 'whole': 0.0}
        self.strokes = {float(s['width']): centre[s['centre']] for s in t['strokes']}
        h = t.get('hatch')
        self.hatch = {float(h['width']): centre[h['centre']]} if h else {}
        self.min_gap = float(t.get('min_gap', 0))
        self.dash = [float(v) for v in t.get('dash', [])]
        self.badge = float(t['badge'])

    def __repr__(self):
        return f'SizeSpec({self.px})'


def load_sizes(path=SIZES_TOML):
    with open(path, 'rb') as f:
        t = tomllib.load(f)
    return {int(px): SizeSpec(int(px), v) for px, v in sorted(t['size'].items(), key=lambda kv: int(kv[0]))}


def lint_svg(text, icon, spec, *, badge=False, domains=DEFAULT_DOMAINS):
    """Lint one SVG master against its size's rules. Returns a list of Findings (empty when clean)."""
    out = []
    size = spec.px

    def bad(rule, msg):
        out.append(Finding(icon, size, rule, msg))

    if len(text.encode()) > MAX_BYTES:
        bad('bytes', f'{len(text.encode())} bytes, budget {MAX_BYTES}')
    if 'data:' in text:
        bad('raster', 'data: URI (embedded raster or font)')
    try:
        root = ET.fromstring(text)
    except ET.ParseError as e:
        bad('parse', str(e))
        return out
    if local(root.tag) != 'svg':
        bad('root', f'root element is <{local(root.tag)}>')
        return out
    vb = [float(v) for v in NUM_RE.findall(root.get('viewBox', ''))]
    if vb != [0, 0, size, size]:
        bad('root', f'viewBox must be "0 0 {size} {size}", got {root.get("viewBox")!r}')
    for dim in ('width', 'height'):
        if root.get(dim) is not None and root.get(dim) != str(size):
            bad('root', f'{dim}={root.get(dim)!r}, expected {size} (or none)')
    if not badge:
        if '+' in icon:
            bad('id', 'a source id has no "+": badge variants are composed by build')
        elif not ID_RE.match(icon):
            bad('id', 'ids are <domain>.<group>.<name>: lower-case dot-separated segments, words joined by "-"')
        elif icon.split('.')[0] not in domains:
            bad('id', f'domain {icon.split(".")[0]!r} is not one of {", ".join(domains)}')
    result_id = '.result.' in f'.{icon}.' or icon.split('.')[-1] == 'result'
    ids = set()
    widths = spec.strokes
    runs = []          # axis-aligned stroked segments, for the min_gap check: (axis, at, lo, hi, width, tag)

    def walk(el, inh, in_defs, clipped, in_mask):
        tag = local(el.tag)
        if tag not in ELEMENTS:
            if tag == 'image':
                bad('raster', '<image> (raster images are not allowed)')
            else:
                bad('element', f'<{tag}> is not allowed')
            return
        if tag == 'svg' and el is not root:
            bad('element', 'nested <svg>')
        for name, value in el.attrib.items():
            name = local(name) if name.startswith('{') else name
            if name == 'transform' or name.endswith('Transform'):
                bad('attribute', f'{name} on <{tag}> (no transforms: draw on the grid)')
            elif name in ('style', 'class'):
                bad('attribute', f'{name} on <{tag}> (presentation attributes only)')
            elif name in ('href', 'filter') or name.startswith('on'):
                bad('attribute', f'{name} on <{tag}> is not allowed')
            elif name not in ATTRIBUTES:
                bad('attribute', f'{name} on <{tag}> is not allowed')
        if el.get('id'):
            if el.get('id') in ids:
                bad('ref-id', f'duplicate id {el.get("id")!r}')
            if el.get('id') == KNOCKOUT_ID:
                bad('ref-id', f'id {KNOCKOUT_ID!r} is reserved for badge composition')
            ids.add(el.get('id'))
        for name in ('fill', 'stroke', 'color'):
            v = el.get(name)
            if v is None or v in ('none', 'currentColor'):
                continue
            if in_mask and v.lower() in MASK_COLOURS:
                continue
            bad('colour', f'{name}={v!r} on <{tag}>: currentColor only'
                + (' (#fff/#000 inside <mask>)' if not in_mask else ''))
        for name in ('opacity', 'fill-opacity', 'stroke-opacity'):
            v = el.get(name)
            if v is None:
                continue
            try:
                f = float(v)
            except ValueError:
                bad('opacity', f'{name}={v!r} is not a number')
                continue
            ok = any(abs(f - o) < EPS for o in OPACITIES)
            if not ok and result_id and any(abs(f - o) < EPS for o in FRINGE_OPACITIES):
                ok = True
            if not ok:
                bad('opacity', f'{name}={v} on <{tag}>: 1 or .4 only'
                    + (' (.2/.45 for the results fringe)' if result_id else ''))
        if badge and tag in ('defs', 'mask', 'clipPath') or badge and (el.get('mask') or el.get('clip-path')):
            bad('badge', 'badges are plain shapes: no defs, mask or clip-path')
        inh = dict(inh)
        for k in INHERITED:
            if el.get(k) is not None:
                inh[k] = el.get(k)
        here_clipped = clipped or el.get('clip-path') is not None
        if tag in ('defs', 'mask', 'clipPath'):
            if tag in ('mask', 'clipPath') and not in_defs:
                bad('defs', f'<{tag}> outside <defs>')
            for ch in el:
                if tag == 'defs' and local(ch.tag) not in ('mask', 'clipPath'):
                    bad('defs', f'<defs> holds <{local(ch.tag)}>: masks and clip paths only')
                walk(ch, inh, True, here_clipped, in_mask or tag == 'mask')
            return
        if tag in SHAPES and not in_defs:
            check_shape(el, tag, inh, here_clipped)
        for ch in el:
            walk(ch, inh, in_defs, here_clipped, in_mask)

    def check_shape(el, tag, inh, clipped):
        try:
            segs = shape_segments(el)
        except (ValueError, TypeError) as e:
            bad('parse', f'<{tag}>: {e}')
            return
        stroked = inh.get('stroke', 'none') != 'none'
        lo = size - spec.badge if badge else 0.0
        if not clipped:
            pts = [p for s in segs for p in ([s[1], s[2]] if s[0] == 'line' else s[2])]
            for x, y in pts:
                if not (lo - EPS <= x <= size + EPS and lo - EPS <= y <= size + EPS):
                    bad('badge' if badge else 'bounds',
                        f'<{tag}> reaches ({_fmt(x)}, {_fmt(y)}), outside '
                        + (f'the badge area {_fmt(lo)}..{size}' if badge else f'0..{size}')
                        + ('' if badge else ' (clip it, or keep it on the canvas)'))
                    break
        if not stroked:
            return
        try:
            w = float(inh.get('stroke-width', '1'))
        except ValueError:
            bad('stroke-width', f'stroke-width={inh.get("stroke-width")!r} is not a number')
            return
        allowed = dict(widths)
        if clipped:
            allowed.update({k: v for k, v in spec.hatch.items() if k not in allowed})
        match = [a for a in allowed if abs(w - a) < EPS]
        if not match:
            hatch = ''.join(f' ({_fmt(a)}px hatching inside a clip-path)' for a in spec.hatch if a not in widths)
            bad('stroke-width', f'<{tag}> strokes {_fmt(w)}px; {size} masters use '
                + ', '.join(_fmt(a) for a in sorted(widths)) + 'px' + hatch)
            return
        phase = allowed[match[0]]
        dash = inh.get('stroke-dasharray')
        if dash and dash != 'none':
            vals = [float(v) for v in NUM_RE.findall(dash)]
            if vals != spec.dash:
                bad('dash', f'stroke-dasharray={dash!r}: centre lines use "{" ".join(_fmt(v) for v in spec.dash)}"')
            if inh.get('stroke-linecap', 'butt') != 'butt':
                bad('dash', 'dashed strokes use butt caps')
        if tag in ('circle', 'ellipse'):
            cx, cy = _num(el, 'cx'), _num(el, 'cy')
            rx = _num(el, 'r') if tag == 'circle' else _num(el, 'rx')
            ry = _num(el, 'r') if tag == 'circle' else _num(el, 'ry')
            for v, what in ((cx - rx, 'left'), (cx + rx, 'right'), (cy - ry, 'top'), (cy + ry, 'bottom')):
                if not on_phase(v, phase):
                    bad('half-pixel', f'<{tag}> {what} edge at {_fmt(v)}: a {_fmt(w)}px stroke centres on '
                        + ('.5' if phase else 'whole pixels'))
                    break
            return
        for s in segs:
            if s[0] != 'line':
                continue
            (x0, y0), (x1, y1) = s[1], s[2]
            dx, dy = abs(x1 - x0), abs(y1 - y0)
            if dx < EPS and dy < EPS:
                continue
            if dy < EPS or dx < EPS:
                v, axis = (y0, 'y') if dy < EPS else (x0, 'x')
                if not on_phase(v, phase):
                    bad('half-pixel', f'<{tag}> {"horizontal" if axis == "y" else "vertical"} stroke at '
                        f'{axis}={_fmt(v)}: a {_fmt(w)}px stroke centres on ' + ('.5' if phase else 'whole pixels'))
                a, b = (x0, x1) if axis == 'y' else (y0, y1)
                runs.append((axis, v, min(a, b), max(a, b), w, tag))
            elif not (abs(dx - dy) < EPS or abs(dx - 2 * dy) < EPS):
                bad('diagonal-angle', f'<{tag}> segment ({_fmt(x0)},{_fmt(y0)})->({_fmt(x1)},{_fmt(y1)}): '
                    'diagonals are 45 degrees or 2:1 (dx 2 : dy 1)')

    base = {'fill': root.get('fill', 'black'), 'stroke': 'none', 'stroke-width': '1'}
    if root.get('fill') is None:
        base['fill'] = 'black'
        bad('colour', 'the root sets no fill: set fill="none" or fill="currentColor"')
    walk(root, base, False, False, False)
    # min_gap: two parallel strokes that run side by side for more than 1px keep a clear gap (a line
    # ending in a hollow point marker touches the marker for 1px, which is the convention)
    seen = set()
    for i, (ax, at, lo, hi, w, tag) in enumerate(runs):
        for ax2, at2, lo2, hi2, w2, tag2 in runs[i + 1:]:
            d = abs(at - at2)
            if ax2 != ax or d < EPS or min(hi, hi2) - max(lo, lo2) < 1 + EPS:
                continue
            gap = d - (w + w2) / 2
            key = (ax, min(at, at2), max(at, at2))
            if gap < spec.min_gap - EPS and key not in seen:
                seen.add(key)
                bad('gap', f'{"horizontal" if ax == "y" else "vertical"} strokes at {ax}={_fmt(min(at, at2))} and '
                    f'{_fmt(max(at, at2))} leave {_fmt(gap)}px clear; {size} masters keep {_fmt(spec.min_gap)}px')
    return out


class IconSet:
    """A set directory: icons.toml plus one <px>/ directory of masters per optical size."""

    def __init__(self, path):
        self.path = path
        with open(os.path.join(path, 'icons.toml'), 'rb') as f:
            self.toml = tomllib.load(f)
        meta = self.toml.get('set', {})
        self.name = meta.get('name', os.path.basename(os.path.abspath(path)))
        self.licence = meta.get('licence')
        self.domains = tuple(meta.get('domains', DEFAULT_DOMAINS))
        self.icons = self.toml.get('icons', {})
        own = os.path.join(path, 'sizes.toml')
        self.specs = load_sizes(own if os.path.exists(own) else SIZES_TOML)

    @property
    def sizes(self):
        """The sizes this set draws: required ones plus every size with a master directory."""
        return [px for px in self.specs if self.specs[px].required or os.path.isdir(os.path.join(self.path, str(px)))]

    def source(self, icon, size):
        p = os.path.join(self.path, str(size), icon + '.svg')
        return p if os.path.exists(p) else None

    def files(self, size):
        d = os.path.join(self.path, str(size))
        return sorted(f[:-4] for f in os.listdir(d) if f.endswith('.svg')) if os.path.isdir(d) else []

    def entry(self, icon):
        return self.icons.get(icon, {})

    def required(self, icon):
        return sorted({px for px in self.specs if self.specs[px].required} | set(self.entry(icon).get('sizes', [])))

    def outputs(self):
        """Every id build emits, in sheet order: each icon, then its badge variants."""
        ids = []
        for icon in self.icons:
            ids.append(icon)
            ids += [f'{icon}+{b}' for b in self.entry(icon).get('badges', [])]
        return ids

    def approval_hash(self, icon):
        h = hashlib.sha256()
        e = self.entry(icon)
        if 'compose' in e:
            h.update(json.dumps(e['compose'], sort_keys=True).encode())
        for size in self.specs:
            p = self.source(icon, size)
            if p:
                with open(p, 'rb') as f:
                    h.update(f'{size}:'.encode() + f.read())
        return 'sha256:' + h.hexdigest()[:16]


def badge_source(name, size):
    p = os.path.join(BADGE_DIR, str(size), name + '.svg')
    return p if os.path.exists(p) else None


def lint_badges(specs):
    """Every size with a badges/<px>/ directory has all the badges, each inside its badge box."""
    out = []
    for size, spec in specs.items():
        if not os.path.isdir(os.path.join(BADGE_DIR, str(size))):
            continue
        for name in BADGES:
            p = badge_source(name, size)
            if not p:
                out.append(Finding('badge.' + name, size, 'badge', f'no {size} drawing in {BADGE_DIR}'))
                continue
            with open(p) as f:
                out += lint_svg(f.read(), 'badge.' + name, spec, badge=True)
    return out


MANIFEST_KEYS = {'metaphor', 'codicon', 'sizes', 'badges', 'compose', 'allow', 'approved'}


def lint_set(s):
    """Lint every master, the manifest and the waivers. Returns the remaining Findings."""
    raw = []
    if not s.licence:
        raw.append(Finding('icons.toml', None, 'manifest', '[set] licence is missing (the custom set is CC-BY-4.0)'))
    for d in os.listdir(s.path):
        if os.path.isdir(os.path.join(s.path, d)) and d.isdigit() and int(d) not in s.specs:
            raw.append(Finding(d + '/', None, 'size', f'{d}px has no [size.{d}] rules in sizes.toml'))
    for size in s.sizes:
        for icon in s.files(size):
            if icon not in s.icons:
                raw.append(Finding(icon, size, 'orphan', 'no [icons."<id>"] entry in icons.toml'))
            with open(s.source(icon, size)) as f:
                raw += lint_svg(f.read(), icon, s.specs[size], domains=s.domains)
    known = set(BADGES)
    for icon, e in s.icons.items():
        unknown = set(e) - MANIFEST_KEYS
        if unknown:
            raw.append(Finding(icon, None, 'manifest', f'unknown keys {sorted(unknown)}'))
        for px in e.get('sizes', []):
            if px not in s.specs:
                raw.append(Finding(icon, None, 'size', f'sizes lists {px}, which sizes.toml does not define'))
        comp = e.get('compose')
        if comp:
            if any(s.source(icon, z) for z in s.specs):
                raw.append(Finding(icon, None, 'manifest', 'a composed icon has no SVG of its own'))
            if comp.get('base') not in s.icons or 'compose' in s.entry(comp.get('base')):
                raw.append(Finding(icon, None, 'manifest', f'compose.base {comp.get("base")!r} is not a drawn icon'))
            if comp.get('badge') not in known:
                raw.append(Finding(icon, None, 'badge', f'compose.badge {comp.get("badge")!r} is not one of {BADGES}'))
            if not ID_RE.match(icon) or icon.split('.')[0] not in s.domains:
                raw.append(Finding(icon, None, 'id', 'ids are <domain>.<group>.<name> in a known domain'))
        else:
            for px in s.required(icon):
                if px in s.specs and not s.source(icon, px):
                    raw.append(Finding(icon, px, 'master', f'{px}/{icon}.svg is missing (a required size)'))
        # a badge variant exists at every size its base is drawn at, so the badge needs a drawing there
        pairs = [(b, icon) for b in e.get('badges', [])] + ([(comp.get('badge'), comp.get('base'))] if comp else [])
        for b, base in pairs:
            if b not in known:
                raw.append(Finding(icon, None, 'badge', f'badge {b!r} is not one of {BADGES}'))
                continue
            for px in s.sizes:
                if s.source(base, px) and not badge_source(b, px):
                    raw.append(Finding(icon, px, 'badge', f'badge {b!r} has no {px} drawing in {BADGE_DIR}'))
        if e.get('approved') and e['approved'] != s.approval_hash(icon):
            raw.append(Finding(icon, None, 'frozen', f'approved icon changed (now {s.approval_hash(icon)}); a '
                               'changed icon goes back to review: drop `approved` and say why in the batch notes'))
    # waivers: allow = [{ rule = "...", size = 16 (optional), reason = "..." }]
    out, used = [], set()
    for f in raw:
        waivers = s.entry(f.icon).get('allow', []) if f.icon in s.icons else []
        hit = next((w for w in waivers if w.get('rule') == f.rule and w.get('size', f.size) == f.size), None)
        if hit:
            used.add((f.icon, hit['rule'], hit.get('size')))
        else:
            out.append(f)
    for icon, e in s.icons.items():
        for w in e.get('allow', []):
            if not w.get('reason'):
                out.append(Finding(icon, None, 'waiver', f'waiver for {w.get("rule")!r} has no reason'))
            elif (icon, w.get('rule'), w.get('size')) not in used:
                out.append(Finding(icon, None, 'waiver-unused', f'waiver for {w.get("rule")!r} matches nothing'))
    rust = {}
    for icon in s.outputs():
        rust.setdefault(rust_const(icon), []).append(icon)
    for name, ids in rust.items():
        if len(ids) > 1:
            out.append(Finding(ids[0], None, 'id', f'ids {ids} share the Rust const {name}'))
    return out




# ---------------------------------------------------------------- build

def _tree(text):
    return ET.fromstring(text)


def _presentation(el):
    return {k: el.get(k) for k in INHERITED if el.get(k) is not None}


def _dilated(el, inh, out):
    """Copy the badge shapes into the knockout mask, black and 1px wider on every side."""
    tag = local(el.tag)
    inh = dict(inh)
    inh.update(_presentation(el))
    if tag in SHAPES:
        copy = ET.Element(el.tag, {k: v for k, v in el.attrib.items()
                                   if k not in INHERITED and k not in ('opacity', 'fill-opacity', 'stroke-opacity')})
        filled = inh.get('fill', 'black') != 'none'
        stroked = inh.get('stroke', 'none') != 'none'
        w = float(inh.get('stroke-width', '1')) if stroked else 0.0
        cap = inh.get('stroke-linecap', 'butt')
        copy.set('fill', '#000' if filled else 'none')
        copy.set('stroke', '#000')
        copy.set('stroke-width', _fmt(w + 2))
        copy.set('stroke-linejoin', 'round')
        copy.set('stroke-linecap', 'square' if cap == 'butt' and stroked else 'round')
        out.append(copy)
    for ch in el:
        _dilated(ch, inh, out)


def compose(base_text, badge_text, size, *, base_tone=False):
    """The base icon with a badge in the bottom-right corner and a 1px knockout under it (ICONS.md §3.2)."""
    base, badge = _tree(base_text), _tree(badge_text)
    root = ET.Element(f'{{{SVG_NS}}}svg', {k: v for k, v in base.attrib.items()})
    defs = ET.SubElement(root, f'{{{SVG_NS}}}defs')
    mask = ET.SubElement(defs, f'{{{SVG_NS}}}mask', {'id': KNOCKOUT_ID, 'maskUnits': 'userSpaceOnUse', 'x': '0',
                                                      'y': '0', 'width': str(size), 'height': str(size)})
    ET.SubElement(mask, f'{{{SVG_NS}}}rect', {'width': str(size), 'height': str(size), 'fill': '#fff'})
    knock = []
    badge_inh = {'fill': badge.get('fill', 'black'), 'stroke': badge.get('stroke', 'none'),
                 'stroke-width': badge.get('stroke-width', '1')}
    badge_inh.update(_presentation(badge))
    for ch in badge:
        _dilated(ch, badge_inh, knock)
    mask.extend(knock)
    body = []
    for ch in base:
        if local(ch.tag) == 'defs':
            defs.extend(list(ch))
        else:
            body.append(ch)
    g = ET.SubElement(root, f'{{{SVG_NS}}}g', {'mask': f'url(#{KNOCKOUT_ID})'})
    if base_tone:
        g.set('opacity', '.4')
    g.extend(body)
    # the badge keeps its own presentation defaults, whatever the base root sets
    bg = ET.SubElement(root, f'{{{SVG_NS}}}g', {k: v for k, v in badge.attrib.items()
                                                if k in INHERITED or k in ('fill-rule',)})
    bg.extend(list(badge))
    return ET.tostring(root, encoding='unicode') + '\n'


def white(text):
    """A tint-ready copy: egui_extras' SvgLoader renders currentColor black, so tinting needs white ink."""
    return text.replace('currentColor', '#fff')


def rust_const(icon):
    return icon.upper().replace('+', '__').replace('.', '_').replace('-', '_')


def render_outputs(s, only=None):
    """{(id, size): svg text} for every output id (sources, badge variants and compositions)."""
    out = {}

    def read(p):
        with open(p) as f:
            return f.read()

    for icon in s.outputs():
        if only and icon not in only:
            continue
        base, _, badge = icon.partition('+')
        e = s.entry(base)
        comp = e.get('compose')
        for size in s.sizes:
            if badge:
                src, b, tone = s.source(base, size), badge_source(badge, size), False
            elif comp:
                src, b, tone = s.source(comp['base'], size), badge_source(comp['badge'], size), comp.get('base_tone', False)
            else:
                src = s.source(icon, size)
                if src:
                    out[(icon, size)] = read(src)
                continue
            if src and b:
                out[(icon, size)] = compose(read(src), read(b), size, base_tone=tone)
    return out


def build(s, out_dir, only=None):
    outs = render_outputs(s, only)
    ids = [i for i in s.outputs() if any((i, z) in outs for z in s.sizes)]
    for (icon, size), text in outs.items():
        for sub, t in ((str(size), text), (os.path.join('white', str(size)), white(text))):
            os.makedirs(os.path.join(out_dir, sub), exist_ok=True)
            with open(os.path.join(out_dir, sub, icon + '.svg'), 'w') as f:
                f.write(t)
    manifest = {'set': s.name, 'licence': s.licence, 'icons': {}}
    for icon in ids:
        base, _, badge = icon.partition('+')
        e = s.entry(base)
        m = {'sizes': [z for z in s.sizes if (icon, z) in outs]}
        if e.get('codicon'):
            m['codicon'] = e['codicon']
        if badge:
            m['composed'] = {'base': base, 'badge': badge}
        elif e.get('compose'):
            m['composed'] = dict(e['compose'])
        m['approved'] = bool(e.get('approved'))
        manifest['icons'][icon] = m
    with open(os.path.join(out_dir, 'manifest.json'), 'w') as f:
        json.dump(manifest, f, indent=2)
        f.write('\n')
    head = f'Generated by tools/icons/icons.py build from {s.name!r} ({s.licence}); do not edit.'
    with open(os.path.join(out_dir, 'icon_ids.d.ts'), 'w') as f:
        f.write(f'// {head}\nexport type IconId =\n' + ''.join(f"  | '{i}'\n" for i in ids).rstrip('\n') + ';\n')
        f.write('export const ICON_IDS: readonly IconId[] = [\n' + ''.join(f"  '{i}',\n" for i in ids) + '];\n')
    with open(os.path.join(out_dir, 'icon_ids.rs'), 'w') as f:
        f.write(f'// {head}\n')
        for i in ids:
            f.write(f'pub const {rust_const(i)}: &str = "{i}";\n')
        f.write('pub const ALL: &[&str] = &[\n' + ''.join(f'    {rust_const(i)},\n' for i in ids) + '];\n')
    return ids


# ---------------------------------------------------------------- contact sheet

THEMES = [('Dark Modern', '#1F1F1F', '#CCCCCC'), ('Light Modern', '#F8F8F8', '#3B3B3B')]
CHROME = ['files', 'search', 'source-control', 'debug-alt', 'extensions', 'layers', 'filter', 'settings-gear',
          'error', 'warning', 'pinned', 'eye']


class Renderer:
    def __init__(self, kind):
        self.kind = kind
        self.cache = {}
        tool = 'rsvg-convert' if kind == 'rsvg' else 'resvg'
        if not shutil.which(tool):
            sys.exit(f'sheet: {tool} is not on PATH (--renderer rsvg needs librsvg2-bin; resvg needs `cargo install resvg`)')

    def __call__(self, text, px, fg):
        from PIL import Image
        key = (text, px, fg)
        if key not in self.cache:
            svg = text.replace('currentColor', fg).encode()
            if self.kind == 'rsvg':
                png = subprocess.run(['rsvg-convert', '-w', str(px), '-h', str(px)], input=svg,
                                     capture_output=True, check=True).stdout
            else:
                with tempfile.TemporaryDirectory() as td:
                    src, dst = os.path.join(td, 'i.svg'), os.path.join(td, 'i.png')
                    with open(src, 'wb') as f:
                        f.write(svg)
                    subprocess.run(['resvg', '-w', str(px), '-h', str(px), src, dst], check=True, capture_output=True)
                    with open(dst, 'rb') as f:
                        png = f.read()
            self.cache[key] = Image.open(io.BytesIO(png)).convert('RGBA')
        return self.cache[key]


def nearest_master(outs, icon, px, sizes):
    """The SVG for (icon, px): its own master, else the nearest drawn size (scaled). Returns (text, own)."""
    if (icon, px) in outs:
        return outs[(icon, px)], True
    have = [z for z in sizes if (icon, z) in outs] or [z for (i, z) in outs if i == icon]
    z = min(have, key=lambda z: (abs(z - px), -z))
    return outs[(icon, z)], False


def sheet(s, out_png, *, codicons=None, zoom=None, only=None, renderer='rsvg', sizes=None):
    """One row per icon, laid out across every optical size at 1x and 2x, in Dark and Light Modern."""
    from PIL import Image, ImageDraw, ImageFont
    render = Renderer(renderer)
    font = ImageFont.load_default(size=11)
    sizes = sorted(sizes or s.sizes)
    outs = render_outputs(s, only)
    ids = [i for i in s.outputs() if any(k[0] == i for k in outs)]

    def codicon(name):
        if not (codicons and name):
            return None
        p = os.path.join(codicons, name + '.svg')
        if not os.path.exists(p):
            return None
        with open(p) as f:
            return f.read()

    cells = [(px, px, px + 8) for px in sizes] + [(px, 2 * px, 2 * px + 8) for px in sizes]   # (master, pixels, slot)
    ccells = [(16, 16, 24), (16, 32, 40)]                                                        # codicons are 16
    set_w = sum(c[2] for c in cells)
    label_w, gap, cname_w = 290, 16, 110
    panel_w = 8 + set_w + gap + sum(c[2] for c in ccells) + cname_w
    row_h = max(2 * max(sizes) + 12, 48)
    head_h, strip_h = 72, 104
    badge_rows = [(b, {z: open(badge_source(b, z)).read() for z in sizes if badge_source(b, z)}) for b in BADGES]
    badge_row = max(sizes) + 26
    badge_h = 32 + badge_row * ((len(badge_rows) + 4) // 5)
    H = head_h + strip_h + row_h * len(ids) + badge_h + 10
    W = label_w + 2 * panel_w
    img = Image.new('RGBA', (W, H), '#FFFFFF')
    d = ImageDraw.Draw(img)
    d.text((8, 6), f'{s.name}: {len(ids)} icons, {s.licence}', fill='#000', font=font)
    d.text((8, 22), 'sizes ' + ', '.join(map(str, sizes)) + ' at 1x, then at 2x', fill='#444', font=font)
    d.text((8, 38), 'underlined cell: no master at that size\n(the nearest master, scaled)', fill='#888', font=font)
    for t, (name, bg, fg) in enumerate(THEMES):
        x0 = label_w + t * panel_w
        d.rectangle([x0, 0, x0 + panel_w - 1, H], fill=bg)
        d.text((x0 + 8, 6), name, fill=fg, font=font)
        x = x0 + 8
        for master, px, slot in cells:
            d.text((x, head_h - 16), f'{master}', fill=fg, font=font)
            if px == master == sizes[0] or px == 2 * master and master == sizes[0]:
                d.text((x, head_h - 32), '@1x' if px == master else '@2x', fill=fg, font=font)
            x += slot
        d.text((x + gap, head_h - 16), 'nearest codicon', fill=fg, font=font)
        # the toolbar strip: codicons chrome, then the set, at 16@1x and 16@2x
        own = [('i', i) for i in ids if '+' not in i]
        for k, scale in enumerate((1, 2)):
            x, y = x0 + 8, head_h + k * 46
            chrome = [('c', c) for c in CHROME[:(8 if scale == 1 else 4)]] if codicons else []
            for kind, name_ in chrome + own:
                text = codicon(name_) if kind == 'c' else nearest_master(outs, name_, 16, sizes)[0]
                if text is None:
                    continue
                if x + 16 * scale > x0 + panel_w - 8:
                    break
                img.alpha_composite(render(text, 16 * scale, fg), (x, y + (40 - 16 * scale) // 2))
                x += 16 * scale + (6 if scale == 1 else 8)
    d.text((8, head_h + 4), 'toolbar strip @1x: codicons\nchrome, then the set at 16'
           + ('' if codicons else '\n(no --codicons: set only)'), fill='#000', font=font)
    d.text((8, head_h + 58), 'same @2x', fill='#000', font=font)
    y = head_h + strip_h
    for n, icon in enumerate(ids, 1):
        base, _, badge = icon.partition('+')
        e = s.entry(base)
        drawn = [z for z in sizes if (icon, z) in outs]
        state = 'approved' if e.get('approved') else 'draft'
        d.text((8, y + 6), f'{n:>3}  {icon}', fill='#000', font=font)
        d.text((8, y + 21), f'     {state}; masters ' + ', '.join(map(str, drawn)), fill='#666', font=font)
        note = (f'= {base} + {badge} badge' if badge else
                '= {base} + {badge} badge'.format(**e['compose']) if e.get('compose') else e.get('metaphor', ''))
        d.text((8, y + 35), '     ' + note[:46], fill='#666', font=font)
        cname = e.get('codicon', '')
        ctext = codicon(cname)
        for t, (name, bg, fg) in enumerate(THEMES):
            x = label_w + t * panel_w + 8
            for master, px, slot in cells:
                text, own = nearest_master(outs, icon, master, sizes)
                top = y + (row_h - px) // 2
                img.alpha_composite(render(text, px, fg), (x + (slot - px) // 2, top))
                if not own:
                    d.line([x + 2, top + px + 2, x + slot - 4, top + px + 2], fill='#C05050', width=1)
                x += slot
            x += gap
            if ctext:
                for master, px, slot in ccells:
                    img.alpha_composite(render(ctext, px, fg), (x + (slot - px) // 2, y + (row_h - px) // 2))
                    x += slot
            else:
                x += sum(c[2] for c in ccells) if cname else 0
            if cname:
                lx = x if ctext else x - sum(c[2] for c in ccells)
                d.text((lx, y + row_h // 2 - 6), cname if ctext else f'{cname}\n(not vendored)', fill=fg, font=font)
        d.line([0, y, W, y], fill='#88888860')
        y += row_h
    d.line([0, y, W, y], fill='#88888860')
    d.text((8, y + 8), 'badges (composed by build, 1x)', fill='#000', font=font)
    y += 28
    for k, (b, srcs) in enumerate(badge_rows):
        col, row = k % 5, k // 5
        for t, (name, bg, fg) in enumerate(THEMES):
            x = label_w + t * panel_w + 8 + col * (panel_w - 16) // 5
            d.text((x, y + row * badge_row), b, fill=fg, font=font)
            xx = x
            for master in sizes:
                if master in srcs:
                    img.alpha_composite(render(srcs[master], master, fg), (xx, y + row * badge_row + 16))
                    xx += master + 6
    img.convert('RGB').save(out_png, optimize=True)
    if zoom:
        Z = 6
        block = sum(px * Z + 8 for px in sizes) + 16
        cols = max(1, 1600 // block)
        zrows = (len(ids) + cols - 1) // cols
        cell_h = max(sizes) * Z + 22
        zi = Image.new('RGB', (cols * block, zrows * cell_h), '#1F1F1F')
        zd = ImageDraw.Draw(zi)
        for n, icon in enumerate(ids):
            cx, cy = (n % cols) * block, (n // cols) * cell_h
            x = cx + 4
            for px in sizes:
                text, own = nearest_master(outs, icon, px, sizes)
                ic = render(text, px, '#CCCCCC')
                bgc = Image.new('RGBA', ic.size, '#1F1F1F')
                bgc.alpha_composite(ic)
                zi.paste(bgc.resize((px * Z, px * Z), Image.NEAREST), (x, cy + 16))
                if not own:
                    zd.line([x, cy + 16 + px * Z + 2, x + px * Z, cy + 16 + px * Z + 2], fill='#C05050')
                x += px * Z + 8
            zd.text((cx + 4, cy + 2), f'{n + 1} {icon}', fill='#CCCCCC', font=font)
        zi.save(zoom, optimize=True)
    return ids


# ---------------------------------------------------------------- CLI

def main(argv=None):
    ap = argparse.ArgumentParser(prog='icons.py', description=__doc__.split('\n')[0])
    sub = ap.add_subparsers(dest='cmd', required=True)
    p = sub.add_parser('lint', help='check a set against the ICONS.md §3 rules')
    p.add_argument('set')
    p = sub.add_parser('build', help='compose badges, write white copies, manifest.json and id code')
    p.add_argument('set')
    p.add_argument('out')
    p.add_argument('--only', help='comma-separated output ids')
    p = sub.add_parser('sheet', help='contact sheet PNG for review')
    p.add_argument('set')
    p.add_argument('out')
    p.add_argument('--codicons', default=os.environ.get('CODICONS_DIR'),
                   help='a directory of codicons <name>.svg (vscode-codicons src/icons); default $CODICONS_DIR')
    p.add_argument('--zoom', help='also write every size at 1x upscaled 6x (nearest) here')
    p.add_argument('--only', help='comma-separated output ids')
    p.add_argument('--sizes', help='comma-separated sizes to lay out (default: the sizes the set draws)')
    p.add_argument('--renderer', choices=('rsvg', 'resvg'), default='rsvg')
    p = sub.add_parser('hash', help='approval hashes (the `approved` value in icons.toml)')
    p.add_argument('set')
    p.add_argument('ids', nargs='*')
    a = ap.parse_args(argv)
    s = IconSet(a.set)
    only = set(a.only.split(',')) if getattr(a, 'only', None) else None
    if a.cmd == 'lint':
        findings = lint_badges(s.specs) + lint_set(s)
        for f in findings:
            print(f)
        n = sum(len(s.files(z)) for z in s.sizes)
        print(f'{s.name}: {n} masters, {len(findings)} finding(s)', file=sys.stderr)
        return 1 if findings else 0
    if a.cmd == 'build':
        ids = build(s, a.out, only)
        print(f'{a.out}: {len(ids)} icons', file=sys.stderr)
    elif a.cmd == 'sheet':
        ids = sheet(s, a.out, codicons=a.codicons, zoom=a.zoom, only=only, renderer=a.renderer,
                    sizes=[int(z) for z in a.sizes.split(',')] if a.sizes else None)
        print(f'{a.out}: {len(ids)} icons', file=sys.stderr)
    elif a.cmd == 'hash':
        for icon in a.ids or list(s.icons):
            print(f'{icon} = "{s.approval_hash(icon)}"')
    return 0


if __name__ == '__main__':
    sys.exit(main())
