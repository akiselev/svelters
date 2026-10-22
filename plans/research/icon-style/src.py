"""IC0 sample icons in the ICON-STYLE.md style, written by hand as SVG path data.

    python3 src.py <out-dir>      # writes <out-dir>/<size>/<id>.svg, the badge and variant sets, and checks

Each icon is a function of the current MODE (used only for the open-choice strips). Sizes: 16 and 24 for all
24 samples; 12, 14 and 20 for the six optical-size samples. 32 and 48 are the 16 and 24 masters at 2x.
Classes (colour slots, ICON-STYLE §6): `subject` (40% tone), `accent` (state colour), `badge`.
"""
import math, os, re, sys

SW = {12: 1, 14: 1, 16: 1, 20: 1.25, 24: 1.5}      # stroke width per optical size
HEADH = {12: 2, 14: 2.5, 16: 3, 20: 3.5, 24: 4.5}  # arrowhead half-base (= length for the 45-degree head)
CUT = {12: 2, 14: 3, 16: 3, 20: 4, 24: 5}          # signature corner cut
MODE = {'corner': 'cut', 'head': 'solid', 'cap': 'round'}

F = 'fill="currentColor" stroke="none"'                                   # solid, primary
T = 'class="subject" fill="currentColor" fill-opacity=".4"'               # subject tone fill (keeps the stroke)
TS = 'class="subject" fill="currentColor" fill-opacity=".4" stroke="none"'  # subject tone, no stroke
A = 'class="accent"'


def head(size):
    cap = MODE['cap']
    join = 'round' if cap == 'round' else 'miter'
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="{size}" height="{size}" viewBox="0 0 {size} {size}" '
            f'fill="none" stroke="currentColor" stroke-width="{SW[size]}" stroke-linecap="{cap}" '
            f'stroke-linejoin="{join}">')


def n(v):
    s = f'{v:.3f}'.rstrip('0').rstrip('.')
    return '0' if s == '-0' else s


# ---- shared primitives -------------------------------------------------------------------------------------
def ah(x, y, d, s, cls=''):
    """Arrowhead with its apex at (x, y), pointing d in u/d/l/r. Solid 45-degree head by default."""
    h = HEADH[s]
    m = MODE['head']
    ux, uy = {'u': (0, -1), 'd': (0, 1), 'l': (-1, 0), 'r': (1, 0)}[d]
    px, py = -uy, ux
    if m == 'narrow':
        hb, ln = round(h * 2 / 3 * 2) / 2, round(h * 4 / 3 * 2) / 2
    else:
        hb, ln = h, h
    bx, by = x - ux * ln, y - uy * ln
    p1 = (bx + px * hb, by + py * hb)
    p2 = (bx - px * hb, by - py * hb)
    if m == 'open':
        return f'<path {cls} d="M{n(p1[0])} {n(p1[1])} L{n(x)} {n(y)} L{n(p2[0])} {n(p2[1])}"/>'
    return f'<path {cls} {F} d="M{n(x)} {n(y)} L{n(p1[0])} {n(p1[1])} L{n(p2[0])} {n(p2[1])}Z"/>'


def ah_ne(x, y, s, legs):
    """Arrowhead pointing up-right (45 degrees). Solid: a right triangle with axis-aligned legs."""
    m = MODE['head']
    if m == 'solid':
        return f'<path {F} d="M{n(x)} {n(y)} L{n(x - legs)} {n(y)} L{n(x)} {n(y + legs)}Z"/>'
    r2 = math.sqrt(2)
    u = (1 / r2, -1 / r2)
    p = (1 / r2, 1 / r2)
    h = legs / r2
    hb, ln = (round(h * 2 / 3 * 4) / 4, round(h * 4 / 3 * 4) / 4) if m == 'narrow' else (h, h)
    bx, by = x - u[0] * ln, y - u[1] * ln
    p1 = (bx + p[0] * hb, by + p[1] * hb)
    p2 = (bx - p[0] * hb, by - p[1] * hb)
    if m == 'open':
        return f'<path d="M{n(p1[0])} {n(p1[1])} L{n(x)} {n(y)} L{n(p2[0])} {n(p2[1])}"/>'
    return f'<path {F} d="M{n(x)} {n(y)} L{n(p1[0])} {n(p1[1])} L{n(p2[0])} {n(p2[1])}Z"/>'


def doc(x0, y0, x1, y1, s, extra=''):
    """A document or container frame. 'cut': the signature top-right cut, other corners square."""
    c = CUT[s]
    m = MODE['corner']
    if m in ('cut', 'doc'):
        return f'<path {extra} d="M{n(x0)} {n(y0)} H{n(x1 - c)} L{n(x1)} {n(y0 + c)} V{n(y1)} H{n(x0)}Z"/>'
    # 'round': codicons-like rounded corners plus a dog-ear fold line
    r = 1
    return (f'<path {extra} d="M{n(x0)} {n(y0 + r)} A{r} {r} 0 0 1 {n(x0 + r)} {n(y0)} H{n(x1 - c)} L{n(x1)} {n(y0 + c)} '
            f'V{n(y1 - r)} A{r} {r} 0 0 1 {n(x1 - r)} {n(y1)} H{n(x0 + r)} A{r} {r} 0 0 1 {n(x0)} {n(y1 - r)}Z"/>'
            f'<path d="M{n(x1 - c)} {n(y0)} V{n(y0 + c)} H{n(x1)}"/>')


I = {}


def icon(iid, *sizes):
    def reg(fn):
        for s in sizes:
            I[(iid, s)] = (lambda s=s: fn(s))
        return fn
    return reg


# ---- chrome ------------------------------------------------------------------------------------------------
@icon('chrome.file', 12, 14, 16, 20, 24)
def _(s):
    return doc(*{12: (2.5, 1.5, 9.5, 10.5), 14: (3.5, 1.5, 11.5, 12.5), 16: (3.5, 1.5, 12.5, 14.5),
                 20: (4.5, 1.5, 15.5, 18.5), 24: (5.5, 2.5, 18.5, 21.5)}[s], s)


@icon('chrome.folder', 16, 24)
def _(s):
    m = MODE['corner']
    if s == 16:
        if m == 'cut':
            return '<path d="M1.5 2.5H5.5L7.5 4.5H11.5L14.5 7.5V13.5H1.5Z"/>'
        return ('<path d="M1.5 3.5A1 1 0 0 1 2.5 2.5H5.5L7.5 4.5H13.5A1 1 0 0 1 14.5 5.5V12.5A1 1 0 0 1 13.5 13.5'
                'H2.5A1 1 0 0 1 1.5 12.5Z"/>')
    return '<path d="M2.5 3.5H8.5L11.5 6.5H16.5L21.5 11.5V20.5H2.5Z"/>'


@icon('chrome.search', 12, 14, 16, 20, 24)
def _(s):
    return {12: '<circle cx="5" cy="5" r="3.5"/><path d="M7.5 7.5 10.5 10.5"/>',
            14: '<circle cx="6" cy="6" r="4.5"/><path d="M9.5 9.5 12.5 12.5"/>',
            16: '<circle cx="7" cy="7" r="4.5"/><path d="M10.5 10.5 14 14"/>',
            20: '<circle cx="8.5" cy="8.5" r="6"/><path d="M13 13 18 18"/>',
            24: '<circle cx="10.5" cy="10.5" r="7"/><path d="M15.5 15.5 21 21"/>'}[s]


def gear(cx, cy, r_ring, r_hub, r_tip, tooth_w, s):
    out = f'<circle cx="{n(cx)}" cy="{n(cy)}" r="{n(r_ring)}"/><circle cx="{n(cx)}" cy="{n(cy)}" r="{n(r_hub)}"/>'
    d = ''
    for k in range(6):
        a = math.radians(k * 60)
        sx, sy = math.sin(a), -math.cos(a)
        r0 = r_ring
        d += f'M{n(cx + sx * r0)} {n(cy + sy * r0)}L{n(cx + sx * r_tip)} {n(cy + sy * r_tip)}'
    return out + f'<path stroke-width="{tooth_w}" stroke-linecap="butt" d="{d}"/>'


@icon('chrome.settings', 16, 24)
def _(s):
    if s == 16:
        return gear(8, 8, 4.5, 1.5, 7, 2, s)
    return gear(11.5, 11.5, 7, 2, 10.5, 3, s)


@icon('chrome.close', 16, 24)
def _(s):
    return {16: '<path d="M4 4 12 12M12 4 4 12"/>', 24: '<path d="M6 6 18 18M18 6 6 18"/>'}[s]


@icon('chrome.chevron', 16, 24)
def _(s):
    return {16: '<path d="M6 3 11 8 6 13"/>', 24: '<path d="M9 4.5 16.5 12 9 19.5"/>'}[s]


@icon('chrome.add', 16, 24)
def _(s):
    return {16: '<path d="M2.5 7.5H12.5M7.5 2.5V12.5"/>', 24: '<path d="M4.5 11.5H18.5M11.5 4.5V18.5"/>'}[s]


@icon('chrome.run', 16, 24)
def _(s):
    return {16: f'<path {A} {F} d="M5 3 14 7.5 5 12Z"/>', 24: f'<path {A} {F} d="M7 5 21 12 7 19Z"/>'}[s]


@icon('chrome.debug', 16, 24)
def _(s):
    if s == 16:
        return ('<path d="M5.5 8.5A2.5 2.5 0 0 1 10.5 8.5V11A2.5 2.5 0 0 1 5.5 11Z"/>'
                '<path d="M6.5 5A1.5 1.5 0 0 1 9.5 5"/>'
                '<path d="M5.5 8.5 3.5 7.5M5.5 10.5H2.5M5.5 12 3.5 13M10.5 8.5 12.5 7.5M10.5 10.5H13.5M10.5 12 12.5 13"/>')
    return ('<path d="M8.5 12.5A3.5 3.5 0 0 1 15.5 12.5V16A3.5 3.5 0 0 1 8.5 16Z"/>'
            '<path d="M9.5 7.5A2.5 2.5 0 0 1 14.5 7.5"/>'
            '<path d="M8.5 12.5 5.5 11M8.5 15.5H4.5M8.5 17.5 5.5 19M15.5 12.5 18.5 11M15.5 15.5H19.5M15.5 17.5 18.5 19"/>')


@icon('chrome.save', 16, 24)
def _(s):
    if s == 16:
        body = ('<path d="M1.5 1.5H11.5L14.5 4.5V14.5H1.5Z"/>' if MODE['corner'] != 'round' else
                '<path d="M2.5 1.5H11.5L14.5 4.5V13.5A1 1 0 0 1 13.5 14.5H2.5A1 1 0 0 1 1.5 13.5V2.5A1 1 0 0 1 2.5 1.5Z"/>')
        return body + '<path d="M4.5 1.5V4.5H10.5V1.5M3.5 14.5V9.5H12.5V14.5"/>'
    return '<path d="M2.5 2.5H16.5L21.5 7.5V21.5H2.5Z"/><path d="M6.5 2.5V7.5H15.5V2.5M5.5 21.5V14.5H18.5V21.5"/>'


@icon('chrome.undo', 16, 24)
def _(s):
    if s == 16:
        return '<path d="M3 5.5H10A3.5 3.5 0 0 1 10 12.5H5.5"/>' + ah(2, 5.5, 'l', s)
    return '<path d="M4 8.5H15A5 5 0 0 1 15 18.5H8.5"/>' + ah(2.5, 8.5, 'l', s)


@icon('chrome.warning', 12, 14, 16, 20, 24)
def _(s):
    return {
        12: f'<path {A} {F} fill-rule="evenodd" d="M6 1 11 11H1ZM5 5H7V8H5ZM5 9H7V10H5Z"/>',
        14: f'<path {A} {F} fill-rule="evenodd" d="M7 1 13 13H1ZM6 5H8V9H6ZM6 10H8V12H6Z"/>',
        16: f'<g {A}><path d="M7.5 1.5 13.5 13.5H1.5Z"/><path {F} d="M7 5H8V10H7ZM7 11H8V12H7Z"/></g>',
        20: f'<g {A}><path d="M9.5 1.5 17.5 17.5H1.5Z"/><path d="M9.5 6.5V11.5"/>'
            f'<circle {F} cx="9.5" cy="14.5" r=".875"/></g>',
        24: f'<g {A}><path d="M12 2.5 21 20.5H3Z"/><path {F} d="M11 8H13V14H11ZM11 16H13V18H11Z"/></g>',
    }[s]


# ---- MCAD --------------------------------------------------------------------------------------------------
@icon('mcad.sketch', 16, 24)
def _(s):
    if s == 16:
        return ('<path d="M2.5 7.5V13.5H12.5"/>'
                '<rect x="1.5" y="6.5" width="2" height="2"/><rect x="1.5" y="12.5" width="2" height="2"/>'
                '<rect x="11.5" y="12.5" width="2" height="2"/>'
                f'<path {F} d="M7.5 6.5 11.5 2.5 13.5 4.5 9.5 8.5Z"/>'
                '<path d="M7.5 6.5 5.5 10.5 9.5 8.5"/>')
    return ('<path d="M3.5 11.5V20.5H18.5"/>'
            '<rect x="1.5" y="9.5" width="4" height="4"/><rect x="1.5" y="18.5" width="4" height="4"/>'
            '<rect x="16.5" y="18.5" width="4" height="4"/>'
            f'<path {F} d="M11.5 9.5 17.5 3.5 20.5 6.5 14.5 12.5Z"/>'
            '<path d="M11.5 9.5 8.5 15.5 14.5 12.5"/>')


@icon('mcad.extrude', 12, 14, 16, 20, 24)
def _(s):
    if s == 12:
        return (f'<path class="subject" {F} d="M0.5 8 5.5 5.5 10.5 8 5.5 10.5Z"/>'
                '<path d="M5.5 4.5V2"/>' + ah(5.5, 1, 'u', s))
    if s == 14:
        return (f'<path {T} d="M1.5 9.5 6.5 7 11.5 9.5 6.5 12Z"/>'
                '<path d="M6.5 9.5V3"/>' + ah(6.5, 1.5, 'u', s))
    if s == 16:
        return (f'<path {T} d="M1.5 9.5 7.5 6.5 13.5 9.5 7.5 12.5Z"/>'
                '<path d="M1.5 9.5V11.5L7.5 14.5 13.5 11.5V9.5M7.5 12.5V14.5"/>'
                '<path d="M7.5 9.5V2"/>' + ah(7.5, 1, 'u', s))
    if s == 20:
        return (f'<path {T} d="M1.5 12.5 9.5 8.5 17.5 12.5 9.5 16.5Z"/>'
                '<path d="M1.5 12.5V14.5L9.5 18.5 17.5 14.5V12.5M9.5 16.5V18.5"/>'
                '<path d="M9.5 12.5V2.5"/>' + ah(9.5, 1.5, 'u', s))
    return (f'<path {T} d="M1.5 14.5 11.5 9.5 21.5 14.5 11.5 19.5Z"/>'
            '<path d="M1.5 14.5V17.5L11.5 22.5 21.5 17.5V14.5M11.5 19.5V22.5"/>'
            '<path d="M11.5 14.5V2.5"/>' + ah(11.5, 1.5, 'u', s))


@icon('mcad.fillet', 16, 24)
def _(s):
    if s == 16:
        return ('<path d="M2.5 14.5V2.5H6.5A7 7 0 0 1 13.5 9.5V14.5Z"/>'
                '<path d="M6.5 9.5 9.5 6.5"/>' + ah_ne(11, 5, s, 3.5))
    return ('<path d="M3.5 21.5V3.5H9.5A11 11 0 0 1 20.5 14.5V21.5Z"/>'
            '<path d="M9.5 14.5 14 10"/>' + ah_ne(17, 7, s, 5))


@icon('mcad.mate', 16, 24)
def _(s):
    if s == 16:
        return (f'<path {T} d="M2.5 1.5H13.5V4.5H10.5V7.5H5.5V4.5H2.5Z"/>'
                '<path d="M1.5 9.5H5.5V12.5H10.5V9.5H14.5V14.5H1.5Z"/>')
    return (f'<path {T} d="M3.5 2.5H20.5V6.5H15.5V11.5H8.5V6.5H3.5Z"/>'
            '<path d="M2.5 14.5H8.5V18.5H15.5V14.5H21.5V21.5H2.5Z"/>')


@icon('mcad.section', 16, 24)
def _(s):
    if s == 16:
        return ('<defs><clipPath id="c"><path d="M4.5 7 10.5 4V10L4.5 13Z"/></clipPath></defs>'
                '<path d="M1.5 5.5 7.5 2.5 10.5 4V10L4.5 13 1.5 11.5Z"/>'
                '<path d="M1.5 5.5 4.5 7V13M4.5 7 10.5 4"/>'
                '<g clip-path="url(#c)"><path d="M3 12 11 4M3 16 12 7M6 17 12 11"/></g>')
    return ('<defs><clipPath id="c"><path d="M7.5 11 16.5 6.5V15.5L7.5 20Z"/></clipPath></defs>'
            '<path d="M2.5 8.5 11.5 4 16.5 6.5V15.5L7.5 20 2.5 17.5Z"/>'
            '<path d="M2.5 8.5 7.5 11V20M7.5 11 16.5 6.5"/>'
            '<g clip-path="url(#c)" stroke-width="1"><path d="M5 16.5 17 4.5M5 21.5 18 8.5M8 23.5 18 13.5"/></g>')


# ---- ECAD --------------------------------------------------------------------------------------------------
@icon('ecad.trace', 16, 24)
def _(s):
    if s == 16:
        return (f'<rect x="1" y="11" width="4" height="4" {F}/><rect x="11" y="1" width="4" height="4" {F}/>'
                '<path d="M3 11V9L9 3H11" stroke-width="2" stroke-linecap="butt" stroke-linejoin="miter"/>')
    return (f'<rect x="2" y="17" width="5" height="5" {F}/><rect x="17" y="2" width="5" height="5" {F}/>'
            '<path d="M4.5 17V13.5L13.5 4.5H17" stroke-width="3" stroke-linecap="butt" stroke-linejoin="miter"/>')


@icon('ecad.via', 16, 24)
def _(s):
    if s == 16:
        return (f'<path {F} fill-rule="evenodd" d="M10 3a5 5 0 1 0 0 10a5 5 0 1 0 0-10Zm0 3a2 2 0 1 1 0 4a2 2 0 1 1 0-4Z"/>'
                '<path d="M1 8H6" stroke-width="2" stroke-linecap="butt"/>')
    return (f'<path {F} fill-rule="evenodd" d="M15.5 5.5a7 7 0 1 0 0 14a7 7 0 1 0 0-14Zm0 4a3 3 0 1 1 0 6a3 3 0 1 1 0-6Z"/>'
            '<path d="M1 12.5H9" stroke-width="3" stroke-linecap="butt"/>')


@icon('ecad.footprint', 12, 14, 16, 20, 24)
def _(s):
    if s == 12:
        return ('<rect x="4.5" y="1.5" width="3" height="9"/>'
                f'<path {F} d="M1 2h2v2h-2zM1 8h2v2h-2zM9 2h2v2h-2zM9 8h2v2h-2z"/>')
    if s == 14:
        return ('<rect x="4.5" y="1.5" width="5" height="11"/>'
                f'<path {F} d="M1 2h2.5v2h-2.5zM1 6h2.5v2h-2.5zM1 10h2.5v2h-2.5zM10.5 2h2.5v2h-2.5zM10.5 6h2.5v2h-2.5zM10.5 10h2.5v2h-2.5z"/>')
    if s == 16:
        return ('<rect x="5.5" y="1.5" width="5" height="13"/>'
                f'<path {F} d="M1 2h3v2h-3zM1 7h3v2h-3zM1 12h3v2h-3zM12 2h3v2h-3zM12 7h3v2h-3zM12 12h3v2h-3z"/>'
                f'<circle cx="7.5" cy="3.5" r="1" {F}/>')
    if s == 20:
        return ('<rect x="7.5" y="1.5" width="5" height="17"/>'
                f'<path {F} d="M1 3h4v2h-4zM1 9h4v2h-4zM1 15h4v2h-4zM15 3h4v2h-4zM15 9h4v2h-4zM15 15h4v2h-4z"/>'
                f'<circle cx="10" cy="4" r="1" {F}/>')
    return ('<rect x="8.5" y="2.5" width="7" height="19"/>'
            f'<path {F} d="M2 3h4v3h-4zM2 10h4v3h-4zM2 17h4v3h-4zM18 3h4v3h-4zM18 10h4v3h-4zM18 17h4v3h-4z"/>'
            f'<circle cx="11" cy="5.5" r="1.25" {F}/>')


@icon('ecad.net', 16, 24)
def _(s):
    if s == 16:
        return ('<path d="M5.5 4.5H9.5M7.5 4.5V10.5"/>'
                f'<circle cx="7.5" cy="4.5" r="1.5" {F}/>'
                '<circle cx="3.5" cy="4.5" r="2"/><circle cx="11.5" cy="4.5" r="2"/><circle cx="7.5" cy="12.5" r="2"/>')
    return ('<path d="M7.5 6.5H15.5M11.5 6.5V16.5"/>'
            f'<circle cx="11.5" cy="6.5" r="2.5" {F}/>'
            '<circle cx="4.5" cy="6.5" r="3"/><circle cx="18.5" cy="6.5" r="3"/><circle cx="11.5" cy="19.5" r="3"/>')


# ---- simulation --------------------------------------------------------------------------------------------
@icon('sim.mesh', 16, 24)
def _(s):
    if s == 16:
        return ('<path d="M1.5 1.5H13.5V13.5H1.5Z"/>'
                '<path d="M1.5 7.5H13.5M7.5 1.5V13.5M1.5 7.5 7.5 1.5M1.5 13.5 13.5 1.5M7.5 13.5 13.5 7.5"/>')
    return ('<path d="M2.5 2.5H20.5V20.5H2.5Z"/>'
            '<path d="M2.5 11.5H20.5M11.5 2.5V20.5M2.5 11.5 11.5 2.5M2.5 20.5 20.5 2.5M11.5 20.5 20.5 11.5"/>')


@icon('sim.load', 12, 14, 16, 20, 24)
def _(s):
    return {
        12: f'<rect class="subject" x="1" y="9" width="10" height="2" {F}/><path d="M5.5 0.5V5"/>' + ah(5.5, 7.5, 'd', 12),
        14: f'<rect x="1.5" y="9.5" width="11" height="3" {T}/><path d="M6.5 1.5V6"/>' + ah(6.5, 8.5, 'd', 14),
        16: f'<rect x="1.5" y="10.5" width="13" height="4" {T}/><path d="M7.5 1.5V8"/>' + ah(7.5, 10, 'd', 16),
        20: f'<rect x="1.5" y="13.5" width="17" height="5" {T}/><path d="M9.5 1.5V10"/>' + ah(9.5, 12.5, 'd', 20),
        24: f'<rect x="2.5" y="15.5" width="19" height="6" {T}/><path d="M11.5 1.5V12"/>' + ah(11.5, 14.5, 'd', 24),
    }[s]


@icon('sim.result', 16, 24)
def _(s):
    if s == 16:
        return ('<defs><clipPath id="c"><rect x="2" y="2" width="12" height="12"/></clipPath></defs>'
                '<g clip-path="url(#c)" stroke="none" fill="currentColor">'
                '<circle cx="2" cy="14" r="12" fill-opacity=".2"/><circle cx="2" cy="14" r="8.5" fill-opacity=".45"/>'
                '<circle cx="2" cy="14" r="5"/></g><rect x="1.5" y="1.5" width="13" height="13"/>')
    return ('<defs><clipPath id="c"><rect x="2.5" y="2.5" width="18" height="18"/></clipPath></defs>'
            '<g clip-path="url(#c)" stroke="none" fill="currentColor">'
            '<circle cx="2.5" cy="20.5" r="18" fill-opacity=".2"/><circle cx="2.5" cy="20.5" r="12.5" fill-opacity=".45"/>'
            '<circle cx="2.5" cy="20.5" r="7"/></g><rect x="2.5" y="2.5" width="18" height="18"/>')


ORDER = ['chrome.file', 'chrome.folder', 'chrome.search', 'chrome.settings', 'chrome.close', 'chrome.chevron',
         'chrome.add', 'chrome.run', 'chrome.debug', 'chrome.save', 'chrome.undo', 'chrome.warning',
         'mcad.sketch', 'mcad.extrude', 'mcad.fillet', 'mcad.mate', 'mcad.section',
         'ecad.trace', 'ecad.via', 'ecad.footprint', 'ecad.net', 'sim.mesh', 'sim.load', 'sim.result']
OPTICAL = ['chrome.file', 'chrome.search', 'chrome.warning', 'mcad.extrude', 'ecad.footprint', 'sim.load']


# ---- badges (composed by the build, never drawn per icon) ------------------------------------------------
# glyph inside the bottom-right cell; knockout = the cell (8x8 at 16, 12x12 at 24) or the glyph dilated 1px
BADGE = {
    16: {'add': '<path d="M9.5 12.5H15.5M12.5 9.5V15.5"/>',
         'remove': '<path d="M9.5 12.5H15.5"/>',
         'warning': f'<path {F} fill-rule="evenodd" d="M12.5 9 16 16H9ZM12 11.5H13V13.5H12ZM12 14.25H13V15.25H12Z"/>',
         'link': '<path d="M11.5 10.5H10.5A2 2 0 0 0 10.5 14.5H11.5M13.5 10.5H14.5A2 2 0 0 1 14.5 14.5H13.5M10.5 12.5H14.5"/>',
         'lock': f'<path d="M10.5 12V10.5A2 2 0 0 1 14.5 10.5V12"/><rect x="9" y="12" width="7" height="4" {F}/>'},
    24: {'add': '<path d="M14.5 18.5H22.5M18.5 14.5V22.5"/>',
         'remove': '<path d="M14.5 18.5H22.5"/>',
         'warning': f'<path {F} fill-rule="evenodd" d="M18.5 13 24 24H13ZM17.5 16.5H19.5V20H17.5ZM17.5 21H19.5V22.5H17.5Z"/>',
         'link': '<path d="M16.5 15.5H15.5A3 3 0 0 0 15.5 21.5H16.5M20.5 15.5H21.5A3 3 0 0 1 21.5 21.5H20.5M15.5 18.5H21.5"/>',
         'lock': f'<path d="M15.5 18V16A3 3 0 0 1 21.5 16V18"/><rect x="13" y="18" width="11" height="6" {F}/>'},
}
BADGE_CELL = {16: 8, 24: 12}


def compose(body, s, badge, knock='cell'):
    g = BADGE[s][badge]
    c = BADGE_CELL[s]
    if knock == 'cell':
        cut = f'<rect x="{s - c}" y="{s - c}" width="{c}" height="{c}" fill="#000"/>'
    else:  # contour: the glyph itself, widened by 1px on every side
        w = SW[s] + 2
        cut = re.sub(r'(fill|stroke)="currentColor"', r'\1="#000"', g)
        cut = f'<g stroke="#000" fill="#000" stroke-width="{w}">{cut}</g>'
    mask = (f'<defs><mask id="k" maskUnits="userSpaceOnUse" x="0" y="0" width="{s}" height="{s}">'
            f'<rect width="{s}" height="{s}" fill="#fff"/>{cut}</mask></defs>')
    return f'{mask}<g mask="url(#k)">{body}</g><g class="badge">{g}</g>'


BADGED = [('chrome.file', 'add'), ('chrome.folder', 'add'), ('mcad.extrude', 'remove'), ('mcad.sketch', 'warning'),
          ('ecad.footprint', 'link'), ('chrome.file', 'lock')]


# ---- self-check (a prototype of the IC1 lint rules in ICON-STYLE section 9) ----------------------------------
NUM = r'-?\d*\.?\d+(?:e-?\d+)?'


def check(iid, s, svg):
    errs = []
    if re.search(r'transform=|<text|<image|style=|Gradient|filter=', svg):
        errs.append('forbidden element or attribute')
    for op in re.findall(r'(?:fill-)?opacity="([^"]+)"', svg):
        if op not in ('.4', '.2', '.45') or (op != '.4' and 'sim.result' not in iid):
            errs.append(f'opacity {op}')
    for col in re.findall(r'(?:fill|stroke)="([^"]+)"', svg):
        if col not in ('none', 'currentColor', '#000', '#fff'):
            errs.append(f'colour {col}')
    for cls in re.findall(r'class="([^"]+)"', svg):
        if cls not in ('subject', 'accent', 'badge'):
            errs.append(f'class {cls}')
    # half-pixel rule: H/V segments of default-width strokes in absolute paths
    base = SW[s]
    for m in re.finditer(r'<path ([^>]*)>', svg):
        attrs = m.group(1)
        d = re.search(r'\bd="([^"]+)"', attrs).group(1)
        if 'stroke="none"' in attrs or 'fill="currentColor"' in attrs and 'class="subject"' not in attrs:
            continue
        w = re.search(r'stroke-width="([^"]+)"', attrs)
        w = float(w.group(1)) if w else base
        want = 0.5 if (w in (1, 1.25, 1.5, 3)) else 0.0
        toks = re.findall(r'[MLHVAZmlhvaz]|' + NUM, d)
        cmd, x, y, i = None, 0.0, 0.0, 0
        while i < len(toks):
            t = toks[i]
            if t.isalpha():
                cmd = t
                i += 1
                if t in 'Zz':
                    continue
                continue
            if cmd in ('M', 'L'):
                x, y = float(toks[i]), float(toks[i + 1]); i += 2
                cmd = 'L' if cmd == 'M' else cmd
                last = ('pt', x, y)
            elif cmd == 'H':
                nx = float(toks[i]); i += 1
                if abs((y % 1) - want) > 1e-6:
                    errs.append(f'H at y={y}')
                x = nx
            elif cmd == 'V':
                ny = float(toks[i]); i += 1
                if abs((x % 1) - want) > 1e-6:
                    errs.append(f'V at x={x}')
                y = ny
            elif cmd == 'A':
                x, y = float(toks[i + 5]), float(toks[i + 6]); i += 7
            else:  # relative commands only appear in fills
                i += 1
    return errs


def build(out):
    count, bad = 0, 0
    for (iid, s), fn in sorted(I.items()):
        svg = head(s) + fn() + '</svg>\n'
        e = check(iid, s, svg)
        if e:
            bad += 1
            print('CHECK', iid, s, sorted(set(e)))
        os.makedirs(os.path.join(out, str(s)), exist_ok=True)
        open(os.path.join(out, str(s), iid + '.svg'), 'w').write(svg)
        count += 1
    for iid, b in BADGED:
        for s in (16, 24):
            svg = head(s) + compose(I[(iid, s)](), s, b) + '</svg>\n'
            open(os.path.join(out, str(s), f'{iid}+{b}.svg'), 'w').write(svg)
    print(count, 'masters written,', bad, 'with check findings')


def variant(iid, s, **mode):
    """SVG text of one icon under an open-choice MODE (used by sheet.py)."""
    old = dict(MODE)
    MODE.update(mode)
    try:
        return head(s) + I[(iid, s)]() + '</svg>\n'
    finally:
        MODE.clear(); MODE.update(old)


if __name__ == '__main__':
    build(sys.argv[1])
