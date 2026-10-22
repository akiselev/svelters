"""IC0 approval contact sheet for ICON-STYLE.md.

    python3 sheet.py <svg-dir> <proto-dir> <codicons-dir> <out.png> [--preview <zoom.png>]

<svg-dir> is src.py's output; <proto-dir> is plans/research/icons-proto; <codicons-dir> holds codicons SVGs
(not committed: fetched from microsoft/vscode-codicons src/icons/<name>.svg). Rendering is rsvg-convert with
the root `color` set to the theme's icon foreground, so `currentColor` and the colour-slot classes
(ICON-STYLE section 6) resolve as they would with ICR1's injected stylesheet.
"""
import io, os, re, subprocess, sys
from PIL import Image, ImageDraw, ImageFont

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import src  # noqa: E402

THEMES = [  # name, bg (sideBar), fg (icon.foreground), selection bg, selection fg, warning, error, success
    ('Dark Modern', '#181818', '#CCCCCC', '#04395E', '#FFFFFF', '#CCA700', '#F14C4C', '#73C991'),
    ('Light Modern', '#F8F8F8', '#3B3B3B', '#E8E8E8', '#000000', '#BF8803', '#E51400', '#388A34'),
]
REF = {  # sample id -> comparison icon: codicons/<name> or proto/<id>
    'chrome.file': 'codicons/file', 'chrome.folder': 'codicons/folder', 'chrome.search': 'codicons/search',
    'chrome.settings': 'codicons/settings-gear', 'chrome.close': 'codicons/close',
    'chrome.chevron': 'codicons/chevron-right', 'chrome.add': 'codicons/add', 'chrome.run': 'codicons/play',
    'chrome.debug': 'codicons/debug-alt', 'chrome.save': 'codicons/save', 'chrome.undo': 'codicons/discard',
    'chrome.warning': 'codicons/warning', 'mcad.sketch': 'proto/mcad.sketch', 'mcad.extrude': 'proto/mcad.extrude',
    'mcad.fillet': 'proto/mcad.fillet', 'mcad.mate': 'proto/mcad.mate', 'mcad.section': 'proto/mcad.section',
    'ecad.trace': 'proto/ecad.trace', 'ecad.via': 'proto/ecad.via', 'ecad.footprint': 'proto/ecad.footprint',
    'ecad.net': 'proto/ecad.net-label', 'sim.mesh': 'proto/sim.mesh', 'sim.load': 'proto/sim.load.force',
    'sim.result': 'proto/sim.result.contour',
}
CHANGED = ['mcad.extrude', 'mcad.fillet', 'mcad.sketch', 'sim.load', 'ecad.net']  # before/after zooms
FONT = ImageFont.load_default(size=11)
BOLD = ImageFont.load_default(size=13)
W = 1250


def with_color(svg, fg, style=None):
    svg = re.sub(r'<svg ', f'<svg color="{fg}" ', svg, count=1)
    if style:
        svg = re.sub(r'(<svg [^>]*>)', r'\1<style>' + style + '</style>', svg, count=1)
    return svg


def raster(svg, w, h=None):
    png = subprocess.run(['rsvg-convert', '-w', str(w), '-h', str(h or w)], input=svg.encode(),
                         capture_output=True, check=True).stdout
    return Image.open(io.BytesIO(png)).convert('RGBA')


def placed(svg, box, size, ox, oy):
    """The master drawn at `size` px with its origin at (ox, oy) inside a `box` px square (fractional ok)."""
    inner = re.sub(r'<svg ([^>]*?)width="[^"]+" height="[^"]+"',
                   lambda m: f'<svg {m.group(1)}x="{ox}" y="{oy}" width="{size}" height="{size}"', svg, count=1)
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="{box}" height="{box}" viewBox="0 0 {box} {box}">'
            + inner + '</svg>')


class Sheet:
    def __init__(s, svgdir, proto, codi):
        s.svgdir, s.proto, s.codi = svgdir, proto, codi
        s.parts = []

    def svg(s, iid, size):
        p = os.path.join(s.svgdir, str(size), iid + '.svg')
        return open(p).read() if os.path.exists(p) else None

    def ref(s, iid):
        r = REF.get(iid)
        if not r:
            return None
        lib, name = r.split('/')
        p = os.path.join(s.codi, name + '.svg') if lib == 'codicons' else os.path.join(s.proto, '16', name + '.svg')
        return open(p).read() if os.path.exists(p) else None

    def section(s, title, h, note=''):
        img = Image.new('RGBA', (W, h + 30), '#FFFFFF')
        d = ImageDraw.Draw(img)
        d.text((8, 6), title, fill='#000', font=BOLD)
        if note:
            d.text((8 + d.textlength(title, font=BOLD) + 12, 8), note, fill='#555', font=FONT)
        s.parts.append(img)
        return img, d, 30

    def icon(s, img, svg, px, x, y, fg, style=None):
        img.alpha_composite(raster(with_color(svg, fg, style), px), (int(x), int(y)))

    def panels(s, img, d, y0, h, x0, cols):
        """Two theme panels side by side starting at x0, each `cols` wide; returns their x origins."""
        xs = []
        for t, th in enumerate(THEMES):
            x = x0 + t * cols
            d.rectangle([x, y0, x + cols - 4, y0 + h], fill=th[1])
            xs.append(x)
        return xs

    # -- sections ---------------------------------------------------------------------------------------------
    def context(s):
        chrome = [i for i in src.ORDER if i.startswith('chrome.')]
        dom = [i for i in src.ORDER if not i.startswith('chrome.')]
        img, d, y0 = s.section('1. In context: toolbar at 16@1x and 16@2x (ours, then codicons), tree rows',
                               250, 'Dark Modern sideBar #181818 / icon #CCCCCC; Light Modern #F8F8F8 / #3B3B3B')
        half = W // 2
        for t, th in enumerate(THEMES):
            x0 = t * half
            d.rectangle([x0, y0, x0 + half - 4, y0 + 245], fill=th[1])
            d.text((x0 + 8, y0 + 4), th[0], fill=th[2], font=FONT)
            x = x0 + 8
            for i in chrome + dom:
                s.icon(img, s.svg(i, 16), 16, x, y0 + 22, th[2]); x += 24
            x = x0 + 8
            for i in chrome:
                r = s.ref(i)
                s.icon(img, r, 16, x, y0 + 44, th[2]); x += 24
            d.text((x + 6, y0 + 46), '<- codicons', fill=th[2], font=FONT)
            x = x0 + 8
            for i in chrome[:7] + dom[:6]:
                s.icon(img, s.svg(i, 16), 32, x, y0 + 68, th[2]); x += 44
            # tree rows: rest, hover-ish, selected
            rows = [('Part1.cadpart', 'chrome.file', None), ('Sketch1', 'mcad.sketch', None),
                    ('Extrude1', 'mcad.extrude', 'sel'), ('Section view', 'mcad.section', None),
                    ('U3  SOIC-8', 'ecad.footprint', None), ('Von Mises stress', 'sim.result', None)]
            y = y0 + 112
            for label, i, st in rows:
                fg = th[2]
                if st == 'sel':
                    d.rectangle([x0, y, x0 + half - 4, y + 21], fill=th[3]); fg = th[4]
                s.icon(img, s.svg(i, 16), 16, x0 + 24, y + 3, fg)
                d.text((x0 + 46, y + 4), label, fill=fg, font=FONT)
                y += 22

    def table(s):
        n = len(src.ORDER)
        rows = (n + 1) // 2
        rh = 58
        img, d, y0 = s.section('2. All 24 samples: 16@1x, 24@1x, 16@2x, 24@2x | nearest codicon or prototype 16@1x, 16@2x',
                               rows * rh + 4)
        colw = W // 2
        for k, iid in enumerate(src.ORDER):
            cx, cy = (k // rows) * colw, y0 + (k % rows) * rh
            d.text((cx + 6, cy + 16), iid, fill='#000', font=FONT)
            r = REF.get(iid, '')
            d.text((cx + 6, cy + 31), 'ref ' + r, fill='#777', font=FONT)
            for t, th in enumerate(THEMES):
                x = cx + 150 + t * 232
                d.rectangle([x, cy + 1, x + 228, cy + rh - 2], fill=th[1])
                x += 6
                for px, sz in ((16, 16), (24, 24), (32, 16), (48, 24)):
                    s.icon(img, s.svg(iid, sz), px, x, cy + (rh - px) // 2, th[2]); x += px + 8
                x += 6
                d.line([x - 6, cy + 8, x - 6, cy + rh - 9], fill=th[2] + '40')
                rf = s.ref(iid)
                if rf:
                    for px in (16, 32):
                        s.icon(img, rf, px, x, cy + (rh - px) // 2, th[2]); x += px + 8

    def badges(s):
        img, d, y0 = s.section('3. Badges (composed by the build: bottom-right cell knocked out, glyph drawn in it)',
                               90, 'add, add, remove, warning, link, lock at 16@1x, 24@1x, 16@2x')
        xs = s.panels(img, d, y0, 86, 0, W // 2)
        for t, th in enumerate(THEMES):
            x = xs[t] + 8
            for iid, b in src.BADGED:
                nm = f'{iid}+{b}'
                s.icon(img, s.svg(nm, 16), 16, x, y0 + 8, th[2])
                s.icon(img, s.svg(nm, 24), 24, x + 22, y0 + 4, th[2])
                s.icon(img, s.svg(nm, 16), 32, x + 4, y0 + 40, th[2])
                d.text((x, y0 + 74), b, fill=th[2], font=FONT)
                x += 98

    def slots(s):
        img, d, y0 = s.section('4. Colour slots (section 6): accent and badge set from CSS', 96,
                               'default, then slot set: warning accent=warning; run accent=success; '
                               'sketch+warning badge=warning; extrude+remove badge=error; 16@1x, 24@1x, 16@2x')
        xs = s.panels(img, d, y0, 92, 0, W // 2)
        for t, th in enumerate(THEMES):
            cases = [('chrome.warning', f'.accent{{color:{th[5]}}}'), ('chrome.run', f'.accent{{color:{th[7]}}}'),
                     ('mcad.sketch+warning', f'.badge{{color:{th[5]}}}'),
                     ('mcad.extrude+remove', f'.badge{{color:{th[6]}}}')]
            x = xs[t] + 8
            for iid, style in cases:
                for st in (None, style):
                    s.icon(img, s.svg(iid, 16), 16, x, y0 + 10, th[2], st)
                    s.icon(img, s.svg(iid, 24), 24, x + 22, y0 + 6, th[2], st)
                    s.icon(img, s.svg(iid, 16), 32, x + 4, y0 + 40, th[2], st)
                    x += 70
                d.text((x - 130, y0 + 76), iid.split('.')[1], fill=th[2], font=FONT)
                x += 6

    def optical(s):
        sizes = [12, 14, 16, 20, 24, 32]
        img, d, y0 = s.section('5. Optical sizes at 1x: 12, 14, 16, 20, 24, 32 (= 16 master x2)', 430,
                               'each size is its own master; below: 12 and 14 enlarged x5 (nearest) for pixel review')
        xs = s.panels(img, d, y0, 256, 0, W // 2)
        for t, th in enumerate(THEMES):
            x = xs[t] + 10
            for sz in sizes:
                d.text((x, y0 + 4), str(sz), fill=th[2], font=FONT); x += sz + 14
            y = y0 + 22
            for iid in src.OPTICAL:
                x = xs[t] + 10
                for sz in sizes:
                    m = s.svg(iid, sz) if sz != 32 else s.svg(iid, 16)
                    s.icon(img, m, sz, x, y + (32 - sz) // 2, th[2]); x += sz + 14
                d.text((x + 4, y + 10), iid, fill=th[2], font=FONT)
                y += 38
            # x5 zooms of 12 and 14, one full-width row per theme
            yz = y0 + 262 + t * 84
            d.rectangle([0, yz, W, yz + 80], fill=th[1])
            x = 10
            for iid in src.OPTICAL:
                for sz in (12, 14):
                    ic = raster(with_color(s.svg(iid, sz), th[2]), sz)
                    bg = Image.new('RGBA', ic.size, th[1]); bg.alpha_composite(ic)
                    img.paste(bg.resize((sz * 5, sz * 5), Image.NEAREST), (x, yz + 76 - sz * 5))
                    d.text((x, yz + 2), f'{sz}', fill=th[2], font=FONT)
                    x += sz * 5 + 6
                x += 22

    def zoom(s):
        img, d, y0 = s.section('6. Fractional zoom (egui zoom_factor): 16px icon at 1.0, 1.1, 1.25, 1.5', 330,
                               'per zoom: left = naive (16 master scaled, box at a fractional origin); '
                               'right = rule (box snapped, master chosen per section 5.3)')
        xs = s.panels(img, d, y0, 326, 0, W // 2)
        steps = [(1.0, 16, None), (1.1, 18, 16), (1.25, 20, 20), (1.5, 24, 24)]
        for t, th in enumerate(THEMES):
            x = xs[t] + 10
            for z, P, m in steps:
                d.text((x, y0 + 4), f'{z}x {P}px', fill=th[2], font=FONT); x += 2 * P + 30 if m else P + 40
            y = y0 + 22
            for iid in src.OPTICAL:
                x = xs[t] + 10
                for z, P, m in steps:
                    base = s.svg(iid, 16)
                    if m is None:
                        s.icon(img, base, 16, x, y + 4, th[2]); x += P + 40; continue
                    naive = placed(base, P + 1, 16 * z, 0.35, 0.35)
                    s.icon(img, naive, P + 1, x, y, th[2])
                    rule = s.svg(iid, m)
                    s.icon(img, placed(rule, P, m, (P - m) // 2, (P - m) // 2), P, x + P + 8, y, th[2])
                    x += 2 * P + 30
                y += 34
            # x5 zoom of the 1.25 case for two icons: naive vs rule
            x = xs[t] + 10
            for iid in ('mcad.extrude', 'ecad.footprint'):
                for kind in ('naive', 'rule'):
                    base = s.svg(iid, 16)
                    sv = placed(base, 21, 20, 0.35, 0.35) if kind == 'naive' else placed(s.svg(iid, 20), 21, 20, 0, 0)
                    ic = raster(with_color(sv, th[2]), 21)
                    bg = Image.new('RGBA', ic.size, th[1]); bg.alpha_composite(ic)
                    img.paste(bg.resize((21 * 4, 21 * 4), Image.NEAREST), (x, y0 + 238))
                    d.text((x, y0 + 224), f'1.25x {kind}', fill=th[2], font=FONT)
                    x += 21 * 4 + 8
                x += 10

    def choices(s):
        sets = [
            ('O1 terminals', [('round (rec.)', dict(cap='round')), ('square', dict(cap='square'))],
             ['chrome.close', 'chrome.add', 'chrome.undo', 'chrome.search', 'mcad.extrude', 'mcad.sketch']),
            ('O2 corners', [('cut, all frames (rec.)', dict(corner='cut')),
                            ('cut on documents only', dict(corner='doc')),
                            ('rounded + dog-ear', dict(corner='round'))],
             ['chrome.file', 'chrome.folder', 'chrome.save']),
            ('O3 arrowheads', [('solid 90 (rec.)', dict(head='solid')), ('solid narrow', dict(head='narrow')),
                               ('open V', dict(head='open'))],
             ['chrome.undo', 'mcad.extrude', 'mcad.fillet', 'sim.load']),
        ]
        img, d, y0 = s.section('7. Open choices (section 12): each option at 16@1x and 16@2x', 3 * 96)
        for k, (name, opts, ids) in enumerate(sets):
            yy = y0 + k * 96
            xs = s.panels(img, d, yy, 92, 0, W // 2)
            for t, th in enumerate(THEMES):
                x = xs[t] + 8
                d.text((x, yy + 4), name, fill=th[2], font=FONT)
                x += 90
                for label, mode in opts:
                    if name.startswith('O2') and mode['corner'] == 'doc':
                        svgs = [src.variant(i, 16, corner='round' if i == 'chrome.folder' else 'cut') for i in ids]
                    else:
                        svgs = [src.variant(i, 16, **mode) for i in ids]
                    d.text((x, yy + 4), label, fill=th[2], font=FONT)
                    xx = x
                    for sv in svgs:
                        s.icon(img, sv, 16, xx, yy + 24, th[2]); xx += 22
                    xx = x
                    for sv in svgs:
                        s.icon(img, sv, 32, xx, yy + 48, th[2]); xx += 38
                    x += max(len(ids) * 38, d.textlength(label, font=FONT)) + 16

    def before_after(s):
        img, d, y0 = s.section('8. Before/after (prototype v3 -> IC0) at 16@1x, enlarged x6', 130)
        x = 8
        d.rectangle([0, y0, W, y0 + 126], fill=THEMES[0][1])
        for iid in s.CHANGED:
            ref = REF[iid].split('/')[1]
            for k, sv in enumerate((open(os.path.join(s.proto, '16', ref + '.svg')).read(), s.svg(iid, 16))):
                ic = raster(with_color(sv, THEMES[0][2]), 16)
                bg = Image.new('RGBA', ic.size, THEMES[0][1]); bg.alpha_composite(ic)
                img.paste(bg.resize((96, 96), Image.NEAREST), (x + k * 100, y0 + 22))
            d.text((x, y0 + 6), f'{iid}: before | after', fill=THEMES[0][2], font=FONT)
            x += 246

    CHANGED = CHANGED

    def save(s, out):
        H = sum(p.height for p in s.parts) + 40
        img = Image.new('RGB', (W, H), '#FFFFFF')
        d = ImageDraw.Draw(img)
        d.text((8, 10), 'cadatomic icon style IC0: approval sheet (ICON-STYLE.md). Rendered with rsvg-convert; '
               'egui renders with resvg 0.45. Comparison icons: Codicons (c) Microsoft, CC BY 4.0.', fill='#000', font=BOLD)
        y = 40
        for p in s.parts:
            img.paste(p.convert('RGB'), (0, y)); y += p.height
        img.save(out, optimize=True)
        if os.path.getsize(out) > 1_000_000:  # keep the sheet under 1 MB
            img.quantize(colors=256, method=Image.Quantize.MEDIANCUT, dither=Image.Dither.NONE).save(out, optimize=True)
        print('wrote', out, os.path.getsize(out), 'bytes', img.size)


def main():
    svgdir, proto, codi, out = sys.argv[1:5]
    sh = Sheet(svgdir, proto, codi)
    for f in (sh.context, sh.table, sh.badges, sh.slots, sh.optical, sh.zoom, sh.choices, sh.before_after):
        f()
    sh.save(out)


if __name__ == '__main__':
    main()
