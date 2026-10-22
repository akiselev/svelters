"""Contact sheet for the icon prototype (ICONS.md section 4).

    python3 render.py <proto-dir> <lib-svg-dir> <out.png> [--zoom <zoom.png>]

<proto-dir> holds 16/<id>.svg masters and optional 24/<id>.svg masters. <lib-svg-dir> holds
<library>/<name>.svg downloads of the comparison icons (not committed; see REFS). Each icon is
rendered with rsvg-convert at 16 and 24 px, at 1x and 2x, with currentColor replaced by VS Code
Dark Modern / Light Modern `icon.foreground` on the matching surface. A missing 24 master is the
16 master scaled. Comparison icons come from
https://raw.githubusercontent.com/{microsoft/vscode-codicons/main/src/icons,iconoir-icons/iconoir/main/icons/regular,
tabler/tabler-icons/main/icons/outline,lucide-icons/lucide/main/icons}/<name>.svg. --zoom also writes 16@1x and 24@1x upscaled 6x (nearest) for pixel review.
"""
import io, os, subprocess, sys
from PIL import Image, ImageDraw, ImageFont

THEMES = [('dark', '#1F1F1F', '#CCCCCC'), ('light', '#F8F8F8', '#3B3B3B')]
SIZES = [16, 24, 32, 48]          # 16@1x, 24@1x, 16@2x, 24@2x
SLOT = {16: 22, 24: 30, 32: 38, 48: 54}
G = 'gi' + 't'
# prototype id -> nearest icon in a base library (library/name)
REFS = {
    'mcad.extrude': 'iconoir/extrude', 'mcad.revolve': 'lucide/rotate-3d', 'mcad.fillet': 'iconoir/fillet-3d',
    'mcad.sketch': 'iconoir/design-pencil', 'mcad.constraint.coincident': 'tabler/vector-triangle',
    'mcad.mate': 'iconoir/union', 'mcad.section': 'iconoir/cube-cut-with-curve', 'ecad.via': 'lucide/circuit-board',
    'ecad.trace': 'tabler/route', 'ecad.footprint': 'tabler/cpu', 'ecad.net-label': 'lucide/cable',
    'ecad.copper-pour': 'tabler/grain', 'ecad.drc': 'tabler/ruler-measure', 'ecad.layer-stack': 'codicons/layers',
    'sim.mesh': 'tabler/mesh', 'sim.bc.fixed': 'lucide/anchor', 'sim.load.force': 'tabler/arrow-down-bar',
    'sim.probe': 'codicons/target', 'sim.solve': 'codicons/run-all', 'sim.result.contour': 'lucide/thermometer',
}
ORDER = list(REFS)
CHROME = ['files', 'search', 'source-control', 'debug-alt', 'extensions', 'layers', 'filter', 'settings-gear',
          'error', 'warning', 'pinned', 'eye']
STRIP = ['mcad.sketch', 'mcad.extrude', 'mcad.revolve', 'mcad.fillet', 'mcad.section', 'ecad.trace', 'ecad.via',
         'ecad.drc', 'sim.mesh', 'sim.load.force', 'sim.solve', 'sim.result.contour']
FONT = ImageFont.load_default(size=11)


def render(path, px, fg):
    svg = open(path).read().replace('currentColor', fg)
    png = subprocess.run(['rsvg-convert', '-w', str(px), '-h', str(px)], input=svg.encode(),
                         capture_output=True, check=True).stdout
    return Image.open(io.BytesIO(png)).convert('RGBA')


def paste_set(img, x, y, paths, bg, fg, h):
    for s in SIZES:
        p = paths[24] if s in (24, 48) else paths[16]
        if p:
            ic = render(p, s, fg)
            img.alpha_composite(ic, (x + (SLOT[s] - s) // 2, y + (h - s) // 2))
        x += SLOT[s]
    return x


def main():
    proto, libdir, out = sys.argv[1], sys.argv[2], sys.argv[3]
    zoom = sys.argv[sys.argv.index('--zoom') + 1] if '--zoom' in sys.argv else None
    m = lambda i, sz: (os.path.join(proto, str(sz), i + '.svg')
                       if os.path.exists(os.path.join(proto, str(sz), i + '.svg')) else None)
    label_w, row_h, set_w = 200, 60, sum(SLOT.values())
    panel_w = 2 * set_w + 24
    strip_h = 2 * 40 + 24
    W = label_w + 2 * panel_w
    H = 40 + strip_h + row_h * len(ORDER) + 10
    img = Image.new('RGBA', (W, H), '#FFFFFF')
    d = ImageDraw.Draw(img)
    for t, (name, bg, fg) in enumerate(THEMES):
        x0 = label_w + t * panel_w
        d.rectangle([x0, 0, x0 + panel_w - 1, H], fill=bg)
        d.text((x0 + 8, 6), f'{name}: prototype 16,24 @1x | 16,24 @2x', fill=fg, font=FONT)
        d.text((x0 + set_w + 16, 20), 'base library 16,24 @1x | @2x', fill=fg, font=FONT)
        # toolbar context strip: codicons chrome, then the prototype set, at 16@1x and 16@2x
        for k, scale in enumerate((1, 2)):
            x, y = x0 + 8, 40 + k * 44
            for c in CHROME[:6] + STRIP[:6] if scale == 2 else CHROME + STRIP:
                p = os.path.join(libdir, 'codicons', c + '.svg') if '.' not in c else m(c, 16)
                ic = render(p, 16 * scale, fg)
                img.alpha_composite(ic, (x, y + (36 - 16 * scale) // 2))
                x += 16 * scale + (6 if scale == 1 else 8)
    d.text((8, 46), 'toolbar strip @1x:\ncodicons chrome,\nthen prototype', fill='#000', font=FONT)
    d.text((8, 90), 'same @2x (subset)', fill='#000', font=FONT)
    y = 40 + strip_h
    for i in ORDER:
        lib = REFS[i]
        m24 = m(i, 24)
        d.text((8, y + 14), i, fill='#000', font=FONT)
        d.text((8, y + 30), f'ref {lib}' + ('   [24 master]' if m24 else ''), fill='#666', font=FONT)
        for t, (name, bg, fg) in enumerate(THEMES):
            x = label_w + t * panel_w + 8
            x = paste_set(img, x, y, {16: m(i, 16), 24: m24 or m(i, 16)}, bg, fg, row_h)
            ref = os.path.join(libdir, lib + '.svg')
            paste_set(img, x + 8, y, {16: ref, 24: ref}, bg, fg, row_h)
        d.line([0, y, W, y], fill='#88888840')
        y += row_h
    img.convert('RGB').save(out, optimize=True)
    if zoom:
        Z, cols = 6, 4
        cell = 24 * Z + 16
        rows = (len(ORDER) + cols - 1) // cols
        zi = Image.new('RGB', (cols * cell * 2, rows * (cell + 14)), '#1F1F1F')
        zd = ImageDraw.Draw(zi)
        for n, i in enumerate(ORDER):
            cx, cy = (n % cols) * cell * 2, (n // cols) * (cell + 14)
            for k, sz in enumerate((16, 24)):
                p = (m(i, 24) or m(i, 16)) if sz == 24 else m(i, 16)
                ic = render(p, sz, '#CCCCCC')
                bgc = Image.new('RGBA', ic.size, '#1F1F1F')
                bgc.alpha_composite(ic)
                zi.paste(bgc.resize((sz * Z, sz * Z), Image.NEAREST), (cx + k * cell + 4, cy + 14))
            zd.text((cx + 4, cy), i, fill='#CCCCCC', font=FONT)
        zi.save(zoom, optimize=True)
    print('wrote', out, os.path.getsize(out), 'bytes')


if __name__ == '__main__':
    main()
