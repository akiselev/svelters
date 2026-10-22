#!/usr/bin/env python3
"""IC1 icon toolchain: lint, build and contact sheets for cadatomic's icon style (plans/research/ICON-STYLE.md).

    tools/icons/icons.py lint  <set>                          # ICON-STYLE §10 L1-L13; exit 1 on any finding
    tools/icons/icons.py build <set> <out> [--only ids]       # badges, HC tone, white copies, manifest, id code
    tools/icons/icons.py sheet <set> <out.png> [--codicons <dir>] [--zoom <zoom.png>] [--only ids] [--sizes 12,16]
    tools/icons/icons.py hash  <set> [ids]                    # approval hashes for icons.toml

A set is a directory with `icons.toml` (keys: tools/icons/SPEC.md) and one `<px>/<id>.svg` directory of masters
per optical size. The rules (per-size strokes, gaps, counters, arrowheads, pads, badge cells, required masters,
the accent allowlist) come from tools/icons/style.toml, or from the set's own style.toml. Badge glyphs come from
tools/icons/badges/<px>/<name>.svg. lint, build and hash use the standard library only (Python 3.11+); sheet
needs Pillow and rsvg-convert (or resvg with --renderer resvg).
"""
import argparse
import fnmatch
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
STYLE_TOML = os.path.join(HERE, 'style.toml')
SVG_NS = 'http://www.w3.org/2000/svg'
ET.register_namespace('', SVG_NS)

KNOCKOUT_ID = 'badge-knockout'
CLASSES = ('subject', 'accent', 'badge')
DEFAULT_DOMAINS = ('chrome', 'vcs', 'status', 'doc', 'mcad', 'ecad', 'sim', 'select')
ID_RE = re.compile(r'^[a-z][a-z0-9]*(\.[a-z0-9]+(-[a-z0-9]+)*)+$')

# L2 (ICON-STYLE §3.1): the element and attribute allowlists.
ELEMENTS = {'svg', 'g', 'defs', 'path', 'rect', 'circle', 'clipPath', 'mask'}
SHAPES = {'path', 'rect', 'circle'}
ATTRIBUTES = {'xmlns', 'width', 'height', 'viewBox', 'fill', 'stroke', 'stroke-width', 'stroke-linecap',
              'stroke-linejoin', 'stroke-dasharray', 'fill-rule', 'clip-rule', 'opacity', 'fill-opacity',
              'stroke-opacity', 'd', 'x', 'y', 'cx', 'cy', 'r', 'rx', 'ry', 'id', 'mask', 'clip-path', 'maskUnits',
              'class'}
INHERITED = ('fill', 'stroke', 'stroke-width', 'stroke-linecap', 'stroke-linejoin', 'stroke-dasharray')
MASK_COLOURS = {'#fff', '#ffffff', 'white', '#000', '#000000', 'black'}
# O1 (user, 2026-10-22): round terminals; butt for dashed lines, copper, gear teeth and ends on a pad or edge.
CAPS = {'round', 'butt'}
JOINS = {'round', 'miter'}                        # miter for copper traces (§3.1)
EPS = 1e-6
ANGLE_TOL = 0.01                                  # L7


def local(tag):
    return tag.rsplit('}', 1)[-1]


def phase(width):
    """Where a stroke of this width centres to rasterise crisply: even whole widths on integers, others on .5."""
    return 0.0 if abs(width - round(width)) < EPS and round(width) % 2 == 0 else 0.5


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


def on_phase(v, ph):
    f = frac(v)
    return abs(f - ph) < EPS or (ph == 0.0 and abs(f - 1.0) < EPS)


def _fmt(v):
    return f'{v:g}'


def triangle(segs):
    """The three vertices of a closed 3-point polygon path, else None."""
    if len(segs) != 3 or any(s[0] != 'line' for s in segs):
        return None
    pts = [s[1] for s in segs]
    if any(abs(segs[i][2][0] - segs[(i + 1) % 3][1][0]) > EPS or abs(segs[i][2][1] - segs[(i + 1) % 3][1][1]) > EPS
           for i in range(3)):
        return None
    return pts


def arrowhead(pts):
    """(half_base, length) of an isosceles triangle with an axis-aligned base, else None."""
    for i in range(3):
        a, b, apex = pts[i], pts[(i + 1) % 3], pts[(i + 2) % 3]
        if abs(a[1] - b[1]) < EPS:                                 # horizontal base
            hb, mid, ln = abs(a[0] - b[0]) / 2, (a[0] + b[0]) / 2, abs(apex[1] - a[1])
            if abs(apex[0] - mid) < EPS and ln > EPS:
                return hb, ln
        if abs(a[0] - b[0]) < EPS:                                 # vertical base
            hb, mid, ln = abs(a[1] - b[1]) / 2, (a[1] + b[1]) / 2, abs(apex[0] - a[0])
            if abs(apex[1] - mid) < EPS and ln > EPS:
                return hb, ln
    return None


# ---------------------------------------------------------------- style and lint

class SizeSpec:
    """One optical size's rules, from a [size.<px>] table of style.toml."""

    def __init__(self, px, t):
        self.px = px
        self.stroke = float(t['stroke'])
        self.copper = float(t['copper']) if 'copper' in t else None
        self.hatch = float(t['hatch']) if 'hatch' in t else None
        self.pad = float(t.get('pad', 0))
        self.min_gap = float(t.get('min_gap', 0))
        self.min_counter = float(t.get('min_counter', 0))
        self.arrowhead = float(t['arrowhead']) if 'arrowhead' in t else None
        self.cut = t.get('cut')
        self.tone = bool(t.get('tone', True))
        self.badge = float(t.get('badge', 0))
        self.dash = [float(v) for v in t.get('dash', [])]

    def widths(self, clipped):
        w = {self.stroke} | ({self.copper} if self.copper else set())
        return w | ({self.hatch} if clipped and self.hatch else set())

    def __repr__(self):
        return f'SizeSpec({self.px})'


class Style:
    """style.toml: the per-size table and the set-wide rules."""

    def __init__(self, path=STYLE_TOML):
        with open(path, 'rb') as f:
            t = tomllib.load(f)
        self.path = path
        self.specs = {int(px): SizeSpec(int(px), v) for px, v in sorted(t['size'].items(), key=lambda kv: int(kv[0]))}
        self.max_bytes = int(t.get('max_bytes', 1536))
        self.badges = tuple(t.get('badges', ()))
        self.accent_ids = list(t.get('accent_ids', []))
        self.fringe_ids = list(t.get('fringe_ids', []))
        self.subject_outline = bool(t.get('subject_outline', False))
        self.hc_tone = float(t.get('hc_tone', 0.4))
        m = t.get('masters', {})
        self.masters_all = [int(v) for v in m.get('all', [])]
        self.masters_domains = {k: [int(x) for x in v] for k, v in m.get('domains', {}).items()}
        self.masters_uses = {k: [int(x) for x in v] for k, v in m.get('uses', {}).items()}


def globbed(icon, patterns):
    return any(fnmatch.fnmatchcase(icon, p) for p in patterns)


class Finding:
    def __init__(self, icon, size, rule, msg):
        self.icon, self.size, self.rule, self.msg = icon, size, rule, msg

    def __str__(self):
        where = f'{self.size}/{self.icon}' if self.size else self.icon
        return f'{where}: [{self.rule}] {self.msg}'

    def __repr__(self):
        return f'Finding({self})'


def lint_svg(text, icon, spec, style, *, badge=False, built=False, free_angles=False, domains=DEFAULT_DOMAINS):
    """Lint one SVG master against ICON-STYLE §10 for its size. Returns a list of Findings (empty when clean).

    badge: a badge glyph (tools/icons/badges), which must stay inside the badge cell.
    built: build output, which may carry the `badge` class and the knockout mask."""
    out = []
    size = spec.px

    def bad(rule, msg):
        out.append(Finding(icon, size, rule, msg))

    if len(text.encode()) > style.max_bytes:
        bad('L13', f'{len(text.encode())} bytes, budget {style.max_bytes}')
    if 'data:' in text:
        bad('L2', 'data: URI (embedded raster or font)')
    try:
        root = ET.fromstring(text)
    except ET.ParseError as e:
        bad('parse', str(e))
        return out
    if local(root.tag) != 'svg':
        bad('L1', f'root element is <{local(root.tag)}>')
        return out
    # L1: the root
    vb = [float(v) for v in NUM_RE.findall(root.get('viewBox', ''))]
    if vb != [0, 0, size, size]:
        bad('L1', f'viewBox must be "0 0 {size} {size}", got {root.get("viewBox")!r}')
    want = {'width': str(size), 'height': str(size), 'fill': 'none', 'stroke': 'currentColor',
            'stroke-width': _fmt(spec.stroke), 'stroke-linecap': 'round', 'stroke-linejoin': 'round'}
    for k, v in want.items():
        got = root.get(k)
        same = got is not None and (got == v or (k == 'stroke-width' and _isnum(got) and abs(float(got) - spec.stroke) < EPS))
        if not same:
            bad('L1', f'root {k}={got!r}, expected {v!r}')
    if not badge:
        if '+' in icon and not built:
            bad('id', 'a source id has no "+": badge variants are composed by build')
        elif not ID_RE.match(icon.split('+')[0]):
            bad('id', 'ids are <domain>.<group>.<name>: lower-case dot-separated segments, words joined by "-"')
        elif icon.split('.')[0] not in domains:
            bad('id', f'domain {icon.split(".")[0]!r} is not one of {", ".join(domains)}')
    fringe = globbed(icon.split('+')[0], style.fringe_ids)
    accent_ok = globbed(icon.split('+')[0], style.accent_ids)
    ids = set()
    runs = []          # axis-aligned stroked segments for L10: (axis, at, lo, hi, width)

    def walk(el, inh, classes, in_defs, clipped, in_mask):
        tag = local(el.tag)
        if tag not in ELEMENTS:
            bad('L2', f'<{tag}> is not allowed' + (' (no raster images)' if tag == 'image' else ''))
            return
        if tag == 'svg' and el is not root:
            bad('L2', 'nested <svg>')
        for name in el.attrib:
            short = local(name) if name.startswith('{') else name
            if name.startswith('{') or short not in ATTRIBUTES:
                why = {'transform': ' (no transforms: draw on the grid)', 'style': ' (presentation attributes only)',
                       'href': ' (no references)'}.get(short, '')
                bad('L2', f'{short} on <{tag}> is not allowed{why}')
        if el.get('id'):
            if el.get('id') in ids:
                bad('L2', f'duplicate id {el.get("id")!r}')
            if el.get('id') == KNOCKOUT_ID and not built:
                bad('L2', f'id {KNOCKOUT_ID!r} is reserved for badge composition')
            ids.add(el.get('id'))
        # L3: colours
        for name in ('fill', 'stroke'):
            v = el.get(name)
            if v is None or v in ('none', 'currentColor') or (in_mask and v.lower() in MASK_COLOURS):
                continue
            bad('L3', f'{name}={v!r} on <{tag}>: currentColor or none' + ('' if in_mask else ' (#fff/#000 inside <mask>)'))
        # L4: classes
        here = set(classes)
        for c in (el.get('class') or '').split():
            if c not in CLASSES:
                bad('L4', f'class {c!r}: only subject, accent and badge')
            elif c == 'badge' and not built:
                bad('L4', 'class "badge" is added by build, never drawn')
            elif c == 'accent' and not accent_ok:
                bad('L4', f'class "accent" on {icon!r}, which is not on the accent allowlist (state glyphs only)')
            here.add(c)
        # L5: opacities and corners
        for name in ('opacity', 'fill-opacity', 'stroke-opacity'):
            v = el.get(name)
            if v is None:
                continue
            if not _isnum(v):
                bad('L5', f'{name}={v!r} is not a number')
                continue
            f = float(v)
            if abs(f - 1) < EPS or (fringe and any(abs(f - o) < EPS for o in (0.2, 0.45))):
                continue
            if abs(f - 0.4) < EPS:
                if 'subject' not in here:
                    bad('L5', f'{name}=.4 on <{tag}> outside class "subject"')
                elif not spec.tone:
                    bad('L5', f'no subject tone at {size}: draw the subject solid or as an outline')
                continue
            bad('L5', f'{name}={v} on <{tag}>: 1, or .4 on the subject' + (' (.2/.45 for the fringe)' if fringe else ''))
        if tag == 'rect' and (el.get('rx') or el.get('ry')):
            bad('L5', 'rx/ry: corners are square (O2: the 45-degree cut is drawn as a path)')
        # L1 per element: terminals and joins (O1)
        if el.get('stroke-linecap') and el.get('stroke-linecap') not in CAPS:
            bad('L1', f'stroke-linecap={el.get("stroke-linecap")!r}: round, or butt for dashes, copper and teeth')
        if el.get('stroke-linejoin') and el.get('stroke-linejoin') not in JOINS:
            bad('L1', f'stroke-linejoin={el.get("stroke-linejoin")!r}: round, or miter for copper traces')
        if badge and (tag in ('defs', 'mask', 'clipPath') or el.get('mask') or el.get('clip-path')):
            bad('badge', 'badge glyphs are plain shapes: no defs, mask or clip-path')
        inh = dict(inh)
        for k in INHERITED:
            if el.get(k) is not None:
                inh[k] = el.get(k)
        here_clipped = clipped or el.get('clip-path') is not None
        if tag in ('defs', 'mask', 'clipPath'):
            if tag in ('mask', 'clipPath') and not in_defs:
                bad('L2', f'<{tag}> outside <defs>')
            for ch in el:
                if tag == 'defs' and local(ch.tag) not in ('mask', 'clipPath'):
                    bad('L2', f'<defs> holds <{local(ch.tag)}>: masks and clip paths only')
                walk(ch, inh, here, True, here_clipped, in_mask or tag == 'mask')
            return
        if tag in SHAPES and not in_defs:
            check_shape(el, tag, inh, here, here_clipped)
        for ch in el:
            walk(ch, inh, here, in_defs, here_clipped, in_mask)

    def check_shape(el, tag, inh, classes, clipped):
        try:
            segs = shape_segments(el)
        except (ValueError, TypeError, ZeroDivisionError) as e:
            bad('parse', f'<{tag}>: {e}')
            return
        stroked = inh.get('stroke', 'none') != 'none'
        filled = inh.get('fill', 'none') != 'none'
        w = 0.0
        if stroked:
            if not _isnum(inh.get('stroke-width', '1')):
                bad('L9', f'stroke-width={inh.get("stroke-width")!r} is not a number')
                return
            w = float(inh.get('stroke-width', '1'))
        tri = triangle(segs) if tag == 'path' and filled and not stroked else None
        # L4: a classed part is fill-only or stroke-only (a subject may keep its outline, pending ICR1)
        if classes and stroked and filled and not ('subject' in classes and style.subject_outline):
            bad('L4', f'<{tag}> in class {"/".join(sorted(classes))} is both filled and stroked: split the outline')
        # L8: stroke centres within pad..size-pad, fills within half a pixel more; the ink stays on the canvas.
        # Exempt (ICON-STYLE §3.2): heads (solid triangles), clipped parts, and the ends of open strokes (round
        # caps may touch the edge; butt ends may end on it). A composed badge was linted as its own glyph.
        in_badge = 'badge' in classes
        if not clipped and not tri and not in_badge:
            if badge:
                lo, hi = size - spec.badge, size
            else:
                lo = spec.pad if stroked else spec.pad - 0.5
                hi = size - lo
            ends = {}
            for sg in segs:
                for p in (sg[1], sg[2] if sg[0] == 'line' else sg[2][-1]):
                    k = (round(p[0], 4), round(p[1], 4))
                    ends[k] = ends.get(k, 0) + 1
            pts = [p for sg in segs for p in ([sg[1], sg[2]] if sg[0] == 'line' else [sg[1]] + sg[2])]
            for x, y in pts:
                end = stroked and tag == 'path' and ends.get((round(x, 4), round(y, 4))) == 1
                if end:
                    if not (-EPS <= x <= size + EPS and -EPS <= y <= size + EPS):
                        bad('L8', f'<{tag}> ends at ({_fmt(x)}, {_fmt(y)}), off the canvas')
                        break
                    continue
                if not (lo - EPS <= x <= hi + EPS and lo - EPS <= y <= hi + EPS):
                    bad('badge' if badge else 'L8', f'<{tag}> reaches ({_fmt(x)}, {_fmt(y)}), outside '
                        + (f'the badge cell {_fmt(lo)}..{size}' if badge else f'the live area {_fmt(lo)}..{_fmt(hi)}'))
                    break
                if stroked and not (-EPS <= x - w / 2 and x + w / 2 <= size + EPS
                                    and -EPS <= y - w / 2 and y + w / 2 <= size + EPS):
                    bad('L8', f'<{tag}> ink at ({_fmt(x)}, {_fmt(y)}) leaves the canvas')
                    break
        # L7: diagonals 45 degrees or 2:1, strokes and fills
        if not free_angles:
            for s in segs:
                if s[0] != 'line':
                    continue
                dx, dy = abs(s[2][0] - s[1][0]), abs(s[2][1] - s[1][1])
                if dx < EPS or dy < EPS:
                    continue
                r = dx / dy
                if not any(abs(r - k) <= ANGLE_TOL * k for k in (1, 2, 0.5)):
                    bad('L7', f'<{tag}> segment ({_fmt(s[1][0])},{_fmt(s[1][1])})->({_fmt(s[2][0])},{_fmt(s[2][1])}): '
                        'diagonals are 45 degrees or 2:1')
                    break
        # L11: a solid head with 45-degree sides has half-base = length = the size's arrowhead
        if tri and spec.arrowhead:
            ah = arrowhead(tri)
            if ah and abs(ah[0] - ah[1]) < 0.05 and abs(ah[0] - spec.arrowhead) > EPS:
                bad('L11', f'arrowhead half-base {_fmt(ah[0])}: {size} heads are {_fmt(spec.arrowhead)} (§5.1)')
        if not stroked:
            # L6: fill-only edges on whole pixels
            if filled and not clipped:
                for s in segs:
                    if s[0] != 'line':
                        continue
                    (x0, y0), (x1, y1) = s[1], s[2]
                    if abs(y1 - y0) < EPS and abs(x1 - x0) > EPS and not on_phase(y0, 0.0):
                        bad('L6', f'<{tag}> horizontal fill edge at y={_fmt(y0)}: fill edges lie on whole pixels')
                        break
                    if abs(x1 - x0) < EPS and abs(y1 - y0) > EPS and not on_phase(x0, 0.0):
                        bad('L6', f'<{tag}> vertical fill edge at x={_fmt(x0)}: fill edges lie on whole pixels')
                        break
            return
        # L9: stroke widths
        allowed = spec.widths(clipped)
        if not any(abs(w - a) < EPS for a in allowed):
            bad('L9', f'<{tag}> strokes {_fmt(w)}px; {size} masters use {_fmt(spec.stroke)}'
                + (f', copper {_fmt(spec.copper)}' if spec.copper else '')
                + (f', hatching {_fmt(spec.hatch)} inside a clip-path' if spec.hatch else ''))
            return
        ph = phase(w)
        dash = inh.get('stroke-dasharray')
        if dash and dash != 'none':
            vals = [float(v) for v in NUM_RE.findall(dash)]
            if not spec.dash:
                bad('L1', f'no dashed lines at {size}: draw the centre line solid')
            elif vals != spec.dash:
                bad('L1', f'stroke-dasharray={dash!r}: centre lines use "{" ".join(_fmt(v) for v in spec.dash)}"')
            if inh.get('stroke-linecap', 'butt') != 'butt':
                bad('L1', 'dashed strokes use butt caps')
        if tag == 'circle':
            cx, cy, r = _num(el, 'cx'), _num(el, 'cy'), _num(el, 'r')
            for v, what in ((cx - r, 'left'), (cx + r, 'right'), (cy - r, 'top'), (cy + r, 'bottom')):
                if not on_phase(v, ph):
                    bad('L6', f'<circle> {what} extreme at {_fmt(v)}: a {_fmt(w)}px stroke centres on '
                        + ('.5' if ph else 'whole pixels'))
                    break
            if 2 * r - w < spec.min_counter - EPS:
                bad('L10', f'<circle> r={_fmt(r)} leaves a {_fmt(2 * r - w)}px counter; {size} keeps {_fmt(spec.min_counter)}')
            return
        if tag == 'rect':
            cw, ch = _num(el, 'width') - w, _num(el, 'height') - w
            if min(cw, ch) < spec.min_counter - EPS:
                bad('L10', f'<rect> leaves a {_fmt(cw)}x{_fmt(ch)} counter; {size} keeps {_fmt(spec.min_counter)}')
        for s in segs:
            if s[0] != 'line':
                continue
            (x0, y0), (x1, y1) = s[1], s[2]
            dx, dy = abs(x1 - x0), abs(y1 - y0)
            if (dx < EPS) == (dy < EPS):
                continue
            v, axis = (y0, 'y') if dy < EPS else (x0, 'x')
            if not on_phase(v, ph):
                bad('L6', f'<{tag}> {"horizontal" if axis == "y" else "vertical"} stroke at {axis}={_fmt(v)}: '
                    f'a {_fmt(w)}px stroke centres on ' + ('.5' if ph else 'whole pixels'))
            a, b = (x0, x1) if axis == 'y' else (y0, y1)
            if 'badge' not in classes:               # the base under the badge cell is masked away
                runs.append((axis, v, min(a, b), max(a, b), w))

    walk(root, {'fill': root.get('fill', 'black'), 'stroke': root.get('stroke', 'none'),
                'stroke-width': root.get('stroke-width', '1')}, set(), False, False, False)
    # L10: parallel strokes running side by side for longer than a counter keep the size's clear gap. (A line
    # that ends in a hollow point marker runs inside the marker for half its span: that is the convention.)
    seen = set()
    reach = max(1.0, spec.min_counter)
    for i, (ax, at, lo, hi, w) in enumerate(runs):
        for ax2, at2, lo2, hi2, w2 in runs[i + 1:]:
            d = abs(at - at2)
            if ax2 != ax or d < EPS or min(hi, hi2) - max(lo, lo2) < reach + EPS:
                continue
            gap = d - (w + w2) / 2
            key = (ax, min(at, at2), max(at, at2))
            if gap < spec.min_gap - EPS and key not in seen:
                seen.add(key)
                bad('L10', f'{"horizontal" if ax == "y" else "vertical"} strokes at {ax}={_fmt(min(at, at2))} and '
                    f'{_fmt(max(at, at2))} leave {_fmt(gap)}px clear; {size} keeps {_fmt(spec.min_gap)}')
    return out


def _isnum(v):
    try:
        float(v)
        return True
    except (TypeError, ValueError):
        return False


# ---------------------------------------------------------------- sets

MANIFEST_KEYS = {'metaphor', 'codicon', 'sizes', 'uses', 'badges', 'compose', 'allow', 'approved', 'free_angles'}


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
        self.set_allow = meta.get('allow', [])
        self.icons = self.toml.get('icons', {})
        own = os.path.join(path, 'style.toml')
        self.style = Style(own if os.path.exists(own) else STYLE_TOML)
        self.specs = self.style.specs

    @property
    def sizes(self):
        """The sizes this set draws: every size with a master directory, plus the ones every id needs."""
        return [px for px in self.specs
                if px in self.style.masters_all or os.path.isdir(os.path.join(self.path, str(px)))]

    def source(self, icon, size):
        p = os.path.join(self.path, str(size), icon + '.svg')
        return p if os.path.exists(p) else None

    def files(self, size):
        d = os.path.join(self.path, str(size))
        return sorted(f[:-4] for f in os.listdir(d) if f.endswith('.svg')) if os.path.isdir(d) else []

    def entry(self, icon):
        return self.icons.get(icon, {})

    def required(self, icon):
        """L12: the masters an id must have (ICON-STYLE §5.2)."""
        st, e = self.style, self.entry(icon)
        req = set(st.masters_all) | set(st.masters_domains.get(icon.split('.')[0], []))
        for u in e.get('uses', []):
            req |= set(st.masters_uses.get(u, []))
        return sorted(req | set(e.get('sizes', [])))

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


def lint_badges(style):
    """Every size with a badge cell and a badges/<px>/ directory has every badge, each inside the cell."""
    out = []
    for size, spec in style.specs.items():
        if not spec.badge or not os.path.isdir(os.path.join(BADGE_DIR, str(size))):
            continue
        for name in style.badges:
            p = badge_source(name, size)
            if not p:
                out.append(Finding('badge.' + name, size, 'badge', f'no {size} drawing in {BADGE_DIR}'))
                continue
            with open(p) as f:
                out += lint_svg(f.read(), 'badge.' + name, spec, style, badge=True)
    return out


def lint_set(s):
    """Lint every master, the manifest and the waivers. Returns the remaining Findings."""
    raw = []
    st = s.style
    if not s.licence:
        raw.append(Finding('icons.toml', None, 'manifest', '[set] licence is missing (the custom set is CC-BY-4.0)'))
    for d in sorted(os.listdir(s.path)):
        if os.path.isdir(os.path.join(s.path, d)) and d.isdigit() and int(d) not in s.specs:
            raw.append(Finding(d + '/', None, 'size', f'{d}px has no [size.{d}] table in style.toml'))
    for size in s.sizes:
        for icon in s.files(size):
            if icon not in s.icons:
                raw.append(Finding(icon, size, 'orphan', 'no [icons."<id>"] entry in icons.toml'))
            with open(s.source(icon, size)) as f:
                raw += lint_svg(f.read(), icon, s.specs[size], st, domains=s.domains,
                                free_angles=bool(s.entry(icon).get('free_angles')))
    known = set(st.badges)
    for icon, e in s.icons.items():
        unknown = set(e) - MANIFEST_KEYS
        if unknown:
            raw.append(Finding(icon, None, 'manifest', f'unknown keys {sorted(unknown)}'))
        for u in e.get('uses', []):
            if u not in st.masters_uses:
                raw.append(Finding(icon, None, 'manifest', f'uses {u!r} is not one of {sorted(st.masters_uses)}'))
        for px in e.get('sizes', []):
            if px not in s.specs:
                raw.append(Finding(icon, None, 'size', f'sizes lists {px}, which style.toml does not define'))
        comp = e.get('compose')
        if comp:
            if any(s.source(icon, z) for z in s.specs):
                raw.append(Finding(icon, None, 'manifest', 'a composed icon has no SVG of its own'))
            if comp.get('base') not in s.icons or 'compose' in s.entry(comp.get('base')):
                raw.append(Finding(icon, None, 'manifest', f'compose.base {comp.get("base")!r} is not a drawn icon'))
            if comp.get('badge') not in known:
                raw.append(Finding(icon, None, 'badge', f'compose.badge {comp.get("badge")!r} is not one of {st.badges}'))
            if not ID_RE.match(icon) or icon.split('.')[0] not in s.domains:
                raw.append(Finding(icon, None, 'id', 'ids are <domain>.<group>.<name> in a known domain'))
        else:
            for px in s.required(icon):
                if px in s.specs and not s.source(icon, px):
                    raw.append(Finding(icon, px, 'L12', f'{px}/{icon}.svg is missing (a required master, §5.2)'))
        for b in e.get('badges', []):
            if b not in known:
                raw.append(Finding(icon, None, 'badge', f'badge {b!r} is not one of {st.badges}'))
        if e.get('approved') and e['approved'] != s.approval_hash(icon):
            raw.append(Finding(icon, None, 'frozen', f'approved icon changed (now {s.approval_hash(icon)}); a '
                               'changed icon goes back to review: drop `approved` and say why in the batch notes'))
    # waivers: per icon `allow = [{ rule, size?, reason }]`, per set `[[set.allow]]` with `ids` globs
    out, used = [], set()
    for f in raw:
        own = [(('icon', f.icon, k), w) for k, w in enumerate(s.entry(f.icon).get('allow', []))] if f.icon in s.icons else []
        sets = [(('set', k), w) for k, w in enumerate(s.set_allow) if globbed(f.icon, w.get('ids', []))]
        hit = next((key for key, w in own + sets
                    if w.get('rule') == f.rule and w.get('size', f.size) == f.size), None)
        if hit:
            used.add(hit)
        else:
            out.append(f)
    for icon, e in s.icons.items():
        for k, w in enumerate(e.get('allow', [])):
            if not w.get('reason'):
                out.append(Finding(icon, None, 'waiver', f'waiver for {w.get("rule")!r} has no reason'))
            elif ('icon', icon, k) not in used:
                out.append(Finding(icon, None, 'waiver-unused', f'waiver for {w.get("rule")!r} matches nothing'))
    for k, w in enumerate(s.set_allow):
        if not w.get('reason'):
            out.append(Finding('icons.toml', None, 'waiver', f'set waiver for {w.get("rule")!r} has no reason'))
        elif ('set', k) not in used:
            out.append(Finding('icons.toml', None, 'waiver-unused', f'set waiver for {w.get("rule")!r} matches nothing'))
    rust = {}
    for icon in s.outputs():
        rust.setdefault(rust_const(icon), []).append(icon)
    for name, ids in rust.items():
        if len(ids) > 1:
            out.append(Finding(ids[0], None, 'id', f'ids {ids} share the Rust const {name}'))
    return out


# ---------------------------------------------------------------- build

def compose(base_text, badge_text, size, cell, *, base_tone=False):
    """ICON-STYLE §7.2: the base in a mask that clears the bottom-right cell, then the glyph in class "badge"."""
    base, badge = ET.fromstring(base_text), ET.fromstring(badge_text)
    root = ET.Element(f'{{{SVG_NS}}}svg', dict(base.attrib))
    defs = ET.SubElement(root, f'{{{SVG_NS}}}defs')
    mask = ET.SubElement(defs, f'{{{SVG_NS}}}mask', {'id': KNOCKOUT_ID, 'maskUnits': 'userSpaceOnUse', 'x': '0',
                                                      'y': '0', 'width': str(size), 'height': str(size)})
    # The mask's rects set stroke="none": they would inherit the root's currentColor stroke, which (white in
    # the tint-ready copies) draws a half-strength seam along the cell's edge.
    ET.SubElement(mask, f'{{{SVG_NS}}}rect', {'width': str(size), 'height': str(size), 'fill': '#fff', 'stroke': 'none'})
    c = _fmt(cell)
    ET.SubElement(mask, f'{{{SVG_NS}}}rect', {'x': _fmt(size - cell), 'y': _fmt(size - cell), 'width': c,
                                              'height': c, 'fill': '#000', 'stroke': 'none'})
    body = []
    for ch in base:
        (defs.extend(list(ch)) if local(ch.tag) == 'defs' else body.append(ch))
    g = ET.SubElement(root, f'{{{SVG_NS}}}g', {'mask': f'url(#{KNOCKOUT_ID})'})
    if base_tone:
        g.set('class', 'subject')
        g.set('opacity', '.4')
    g.extend(body)
    bg = ET.SubElement(root, f'{{{SVG_NS}}}g', {'class': 'badge', **{k: v for k, v in badge.attrib.items()
                                                                     if k in INHERITED and v != root.get(k)}})
    bg.extend(list(badge))
    return ET.tostring(root, encoding='unicode') + '\n'


def white(text):
    """A tint-ready copy: egui_extras' SvgLoader renders currentColor black, so tinting needs white ink."""
    return text.replace('currentColor', '#fff')


def high_contrast(text, tone):
    """The HC copy: the subject tone rises from .4 (L5 guarantees .4 appears only on the subject)."""
    return re.sub(r'((?:fill-|stroke-)?opacity)="0?\.4"', lambda m: f'{m.group(1)}="{_fmt(tone)}"', text)


def rust_const(icon):
    return icon.upper().replace('+', '__').replace('.', '_').replace('-', '_')


def render_outputs(s, only=None):
    """{(id, size): svg text} for every output id: sources, badge variants and compositions."""
    out = {}

    def read(p):
        with open(p) as f:
            return f.read()

    for icon in s.outputs():
        if only and icon not in only:
            continue
        base, _, badge = icon.partition('+')
        comp = s.entry(base).get('compose')
        for size in s.sizes:
            cell = s.specs[size].badge
            if badge:
                src, b, tone = s.source(base, size), badge_source(badge, size), False
            elif comp:
                src, b, tone = s.source(comp['base'], size), badge_source(comp['badge'], size), comp.get('base_tone', False)
            else:
                if s.source(icon, size):
                    out[(icon, size)] = read(s.source(icon, size))
                continue
            if src and b and cell:            # no badge cell (12, 14) or no glyph drawn yet: no variant there
                out[(icon, size)] = compose(read(src), read(b), size, cell, base_tone=tone)
    return out


def build(s, out_dir, only=None):
    outs = render_outputs(s, only)
    ids = [i for i in s.outputs() if any((i, z) in outs for z in s.sizes)]
    tone = s.style.hc_tone
    for (icon, size), text in outs.items():
        variants = ((str(size), text), (os.path.join('hc', str(size)), high_contrast(text, tone)),
                    (os.path.join('white', str(size)), white(text)),
                    (os.path.join('white', 'hc', str(size)), white(high_contrast(text, tone))))
        for sub, t in variants:
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


def pick_master(p, masters):
    """ICON-STYLE §5.3 Z2: the master for a box of p physical pixels. Returns (master, drawn_px, offset, mode):
    draw `master` at `drawn_px` pixels, `offset` px in from the box's top-left (Z1: the box is whole pixels)."""
    cands = {m: (m, 1) for m in masters}
    for m in masters:
        cands.setdefault(2 * m, (m, 2))                  # virtual masters: m at x2 is crisp
    if p in cands:
        m, k = cands[p]
        return m, p, 0, 'exact' if k == 1 else 'x2'
    below = [c for c in cands if c <= p and p - c <= 2]
    if below:
        c = max(below)
        return cands[c][0], c, (p - c) // 2, 'padded'
    c = min(cands, key=lambda c: (abs(c - p), -c))
    return cands[c][0], p, 0, 'scaled'


# ---------------------------------------------------------------- contact sheet
# Absorbs plans/research/icon-style/sheet.py (IC0): root `color` instead of replacing currentColor, so the slot
# classes resolve as ICR1's injected stylesheet will; the slot and fractional-zoom sections.

# name, background (sideBar), icon, warning, error, success
THEMES = [('Dark Modern', '#181818', '#CCCCCC', '#CCA700', '#F14C4C', '#73C991'),
          ('Light Modern', '#F8F8F8', '#3B3B3B', '#BF8803', '#E51400', '#388A34')]
CHROME = ['files', 'search', 'source-control', 'debug-alt', 'extensions', 'layers', 'filter', 'settings-gear']
ZOOMS = (1.0, 1.1, 1.25, 1.5)


def with_color(svg, fg, style=None):
    svg = re.sub(r'<svg ', f'<svg color="{fg}" ', svg, count=1)
    if style:
        svg = re.sub(r'(<svg [^>]*>)', lambda m: m.group(1) + '<style>' + style + '</style>', svg, count=1)
    return svg


def placed(svg, box, size, ox, oy):
    """The master drawn at `size` px with its origin at (ox, oy) inside a `box` px square (fractional ok)."""
    inner = re.sub(r'<svg ([^>]*?)width="[^"]+" height="[^"]+"',
                   lambda m: f'<svg {m.group(1)}x="{ox}" y="{oy}" width="{size}" height="{size}"', svg, count=1)
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="{box}" height="{box}" viewBox="0 0 {box} {box}">'
            + inner + '</svg>')


class Renderer:
    def __init__(self, kind):
        self.kind = kind
        self.cache = {}
        tool = 'rsvg-convert' if kind == 'rsvg' else 'resvg'
        if not shutil.which(tool):
            sys.exit(f'sheet: {tool} is not on PATH (--renderer rsvg needs librsvg2-bin; resvg needs `cargo install resvg`)')

    def __call__(self, text, px, fg, style=None):
        from PIL import Image
        key = (text, px, fg, style)
        if key not in self.cache:
            svg = with_color(text, fg, style).encode()
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


def slot_style(icon, text, theme):
    """The slot colours the sheet demonstrates for an icon with an accent or a badge (ICON-STYLE §6)."""
    _, _, warning, error, success = theme[1:]
    rules = []
    if 'class="accent"' in text:
        c = success if re.search(r'\.(run|pass)', icon) else error if re.search(r'\.(error|stop|fail)', icon) else warning
        rules.append(f'.accent{{color:{c}}}')
    if 'class="badge"' in text:
        b = icon.partition('+')[2]
        c = {'warning': warning, 'remove': error, 'error': error, 'add': success, 'check': success,
             'run': success}.get(b, warning)
        rules.append(f'.badge{{color:{c}}}')
    return ''.join(rules) or None


def sheet(s, out_png, *, codicons=None, zoom=None, only=None, renderer='rsvg', sizes=None):
    """The approval sheet: one row per icon across every optical size at 1x and 2x, in Dark and Light Modern,
    with the nearest codicon; then the badges, the colour slots and fractional zoom (ICON-STYLE §5.3)."""
    from PIL import Image, ImageDraw, ImageFont
    render = Renderer(renderer)
    font = ImageFont.load_default(size=11)
    sizes = sorted(sizes or s.sizes)
    outs = render_outputs(s, only)
    ids = [i for i in s.outputs() if any(k[0] == i for k in outs)]

    def codicon(name):
        p = os.path.join(codicons, name + '.svg') if codicons and name else None
        if not (p and os.path.exists(p)):
            return None
        with open(p) as f:
            return f.read()

    cells = [(px, px, px + 8) for px in sizes] + [(px, 2 * px, 2 * px + 8) for px in sizes]   # (master, pixels, slot)
    ccells = [(16, 16, 24), (16, 32, 40)]                                                        # codicons are 16
    set_w = sum(c[2] for c in cells)
    label_w, gap, cname_w = 290, 16, 110
    panel_w = 8 + set_w + gap + sum(c[2] for c in ccells) + cname_w
    W = label_w + 2 * panel_w
    row_h = max(2 * max(sizes) + 12, 48)
    slotted = [i for i in ids if any(slot_style(i, outs[k], THEMES[0]) for k in outs if k[0] == i)]
    zoomed = [i for i in ids if '+' not in i and sum((i, z) in outs for z in sizes) >= 3][:8]
    zoom_ps = [round(16 * z) for z in ZOOMS]
    sections = [('head', 72 + 104), ('rows', row_h * len(ids)), ('badges', 32 + (max(sizes) + 26) * 3)]
    if slotted:
        sections.append(('slots', 16 + 70 * ((len(slotted) + 1) // 2)))
    if zoomed:
        sections.append(('zoom', 40 + 36 * len(zoomed)))
    H = sum(h for _, h in sections) + 10
    img = Image.new('RGBA', (W, H), '#FFFFFF')
    d = ImageDraw.Draw(img)
    for t, th in enumerate(THEMES):
        d.rectangle([label_w + t * panel_w, 0, label_w + (t + 1) * panel_w - 1, H], fill=th[1])

    def ic(text, px, x, y, fg, style=None):
        img.alpha_composite(render(text, px, fg, style), (int(x), int(y)))

    y = 0
    for name, h in sections:
        d.line([0, y, W, y], fill='#88888860')
        if name == 'head':
            d.text((8, 6), f'{s.name}: {len(ids)} icons, {s.licence}', fill='#000', font=font)
            d.text((8, 22), 'sizes ' + ', '.join(map(str, sizes)) + ' at 1x, then at 2x', fill='#444', font=font)
            d.text((8, 38), 'red underline: no master at that size\n(the nearest master, scaled)', fill='#888', font=font)
            d.text((8, 76), 'toolbar strip @1x: codicons\nchrome, then the set at 16'
                   + ('' if codicons else '\n(no --codicons: set only)'), fill='#000', font=font)
            d.text((8, 130), 'same @2x', fill='#000', font=font)
            own = [i for i in ids if '+' not in i]
            for t, th in enumerate(THEMES):
                x0 = label_w + t * panel_w
                d.text((x0 + 8, 6), th[0], fill=th[2], font=font)
                x = x0 + 8
                for master, px, slot in cells:
                    d.text((x, 56), str(master), fill=th[2], font=font)
                    if master == sizes[0]:
                        d.text((x, 40), '@1x' if px == master else '@2x', fill=th[2], font=font)
                    x += slot
                d.text((x + gap, 56), 'nearest codicon', fill=th[2], font=font)
                for k, scale in enumerate((1, 2)):
                    x, yy = x0 + 8, 72 + k * 46
                    chrome = [codicon(c) for c in CHROME[:(8 if scale == 1 else 4)]] if codicons else []
                    for text in [c for c in chrome if c] + [nearest_master(outs, i, 16, sizes)[0] for i in own]:
                        if x + 16 * scale > x0 + panel_w - 8:
                            break
                        ic(text, 16 * scale, x, yy + (40 - 16 * scale) // 2, th[2])
                        x += 16 * scale + (6 if scale == 1 else 8)
        elif name == 'rows':
            for n, icon in enumerate(ids, 1):
                yy = y + (n - 1) * row_h
                base, _, badge = icon.partition('+')
                e = s.entry(base)
                drawn = [z for z in sizes if (icon, z) in outs]
                d.text((8, yy + 6), f'{n:>3}  {icon}', fill='#000', font=font)
                d.text((8, yy + 21), f'     {"approved" if e.get("approved") else "draft"}; masters '
                       + ', '.join(map(str, drawn)), fill='#666', font=font)
                note = (f'= {base} + {badge} badge' if badge else
                        '= {base} + {badge} badge'.format(**e['compose']) if e.get('compose') else e.get('metaphor', ''))
                d.text((8, yy + 35), '     ' + note[:46], fill='#666', font=font)
                cname = e.get('codicon', '')
                ctext = codicon(cname)
                for t, th in enumerate(THEMES):
                    x = label_w + t * panel_w + 8
                    for master, px, slot in cells:
                        text, mine = nearest_master(outs, icon, master, sizes)
                        top = yy + (row_h - px) // 2
                        ic(text, px, x + (slot - px) // 2, top, th[2])
                        if not mine:
                            d.line([x + 2, top + px + 2, x + slot - 4, top + px + 2], fill='#C05050')
                        x += slot
                    x += gap
                    if ctext:
                        for master, px, slot in ccells:
                            ic(ctext, px, x + (slot - px) // 2, yy + (row_h - px) // 2, th[2])
                            x += slot
                    if cname:
                        d.text((x, yy + row_h // 2 - 6), cname if ctext else f'{cname}\n(not vendored)',
                               fill=th[2], font=font)
                d.line([0, yy, W, yy], fill='#88888840')
        elif name == 'badges':
            d.text((8, y + 8), 'badges (composed by build into\nthe bottom-right cell, 1x)', fill='#000', font=font)
            bh = max(sizes) + 26
            for k, b in enumerate(s.style.badges):
                col, row = k % 4, k // 4
                for t, th in enumerate(THEMES):
                    x = label_w + t * panel_w + 8 + col * (panel_w - 16) // 4
                    d.text((x, y + 8 + row * bh), b, fill=th[2], font=font)
                    xx = x
                    for master in sizes:
                        if badge_source(b, master):
                            with open(badge_source(b, master)) as f:
                                ic(f.read(), master, xx, y + 24 + row * bh, th[2])
                            xx += master + 6
        elif name == 'slots':
            d.text((8, y + 8), 'colour slots (ICON-STYLE 6):\ndefault, then the slot set;\n16@1x, 24@1x, 16@2x',
                   fill='#000', font=font)
            for k, icon in enumerate(slotted):
                col, row = k % 2, k // 2
                for t, th in enumerate(THEMES):
                    x = label_w + t * panel_w + 8 + col * (panel_w - 16) // 2
                    yy = y + 8 + row * 70
                    d.text((x, yy), icon, fill=th[2], font=font)
                    for st in (None, 'set'):
                        for px, master, dx, dy in ((16, 16, 0, 18), (24, 24, 22, 14), (32, 16, 50, 14)):
                            text = outs.get((icon, master))
                            if text:
                                ic(text, px, x + dx, yy + dy, th[2], slot_style(icon, text, th) if st else None)
                        x += 90
        elif name == 'zoom':
            d.text((8, y + 8), 'zoom, 16pt: naive | rule (Z1, Z2)', fill='#000', font=font)
            for t, th in enumerate(THEMES):
                x = label_w + t * panel_w + 8
                for z, p in zip(ZOOMS, zoom_ps):
                    d.text((x, y + 8), f'{z}x  P={p}', fill=th[2], font=font)
                    x += 2 * p + 26
                for n, icon in enumerate(zoomed):
                    x, yy = label_w + t * panel_w + 8, y + 26 + n * 36
                    if t == 0:
                        d.text((8, yy + 10), icon, fill='#000', font=font)
                    masters = [z for z in sizes if (icon, z) in outs]
                    for z, p in zip(ZOOMS, zoom_ps):
                        naive = placed(outs[(icon, 16)], p + 1, 16 * z, 0.35, 0.35)
                        ic(naive, p + 1, x, yy, th[2])
                        m, drawn, off, mode = pick_master(p, masters)
                        ic(placed(outs[(icon, m)], p, drawn, off, off), p, x + p + 8, yy, th[2])
                        x += 2 * p + 26
        y += h
    img.convert('RGB').save(out_png, optimize=True)
    if zoom:
        Z = 6
        block = sum(px * Z + 8 for px in sizes) + 16
        cols = max(1, 1600 // block)
        cell_h = max(sizes) * Z + 22
        zi = Image.new('RGB', (cols * block, ((len(ids) + cols - 1) // cols) * cell_h), THEMES[0][1])
        zd = ImageDraw.Draw(zi)
        for n, icon in enumerate(ids):
            cx, cy = (n % cols) * block, (n // cols) * cell_h
            x = cx + 4
            for px in sizes:
                text, mine = nearest_master(outs, icon, px, sizes)
                im = render(text, px, THEMES[0][2])
                bgc = Image.new('RGBA', im.size, THEMES[0][1])
                bgc.alpha_composite(im)
                zi.paste(bgc.resize((px * Z, px * Z), Image.NEAREST), (x, cy + 16))
                if not mine:
                    zd.line([x, cy + 16 + px * Z + 2, x + px * Z, cy + 16 + px * Z + 2], fill='#C05050')
                x += px * Z + 8
            zd.text((cx + 4, cy + 2), f'{n + 1} {icon}', fill=THEMES[0][2], font=font)
        zi.save(zoom, optimize=True)
    return ids


# ---------------------------------------------------------------- CLI

def main(argv=None):
    ap = argparse.ArgumentParser(prog='icons.py', description=__doc__.split('\n')[0])
    sub = ap.add_subparsers(dest='cmd', required=True)
    p = sub.add_parser('lint', help='check a set against ICON-STYLE §10 (L1-L13)')
    p.add_argument('set')
    p = sub.add_parser('build', help='compose badges; write HC and white copies, manifest.json and id code')
    p.add_argument('set')
    p.add_argument('out')
    p.add_argument('--only', help='comma-separated output ids')
    p = sub.add_parser('sheet', help='the approval contact sheet (PNG)')
    p.add_argument('set')
    p.add_argument('out')
    p.add_argument('--codicons', default=os.environ.get('CODICONS_DIR'),
                   help='a directory of codicons <name>.svg (vscode-codicons src/icons); default $CODICONS_DIR')
    p.add_argument('--zoom', help='also write every size at 1x, enlarged 6x (nearest), here')
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
        findings = lint_badges(s.style) + lint_set(s)
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
