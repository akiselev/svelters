# Which CSS properties / selectors / functions does GTK 4.22's CSS parser accept?
# Uses GtkCssProvider::parsing-error; no display needed for parsing.
import gi, json, sys
gi.require_version('Gtk', '4.0')
from gi.repository import Gtk

def errors(css):
    p = Gtk.CssProvider(); errs = []
    p.connect('parsing-error', lambda _p, sec, err: errs.append(err.message))
    p.load_from_string(css)
    return errs

props = {
 'color':'red','opacity':'0.5','filter':'blur(2px)','font-family':'Sans','font-size':'12px','font-weight':'bold',
 'font-style':'italic','font-variant':'small-caps','font-stretch':'condensed','font-feature-settings':'"tnum"',
 'font-variation-settings':'"wght" 400','font':'12px Sans','letter-spacing':'1px','line-height':'1.5',
 'text-decoration':'underline','text-decoration-color':'red','text-shadow':'1px 1px red','text-transform':'uppercase',
 'text-align':'center','white-space':'nowrap','text-overflow':'ellipsis','caret-color':'red',
 '-gtk-secondary-caret-color':'red','-gtk-icon-source':'none','-gtk-icon-size':'16px','-gtk-icon-style':'symbolic',
 '-gtk-icon-transform':'scale(2)','-gtk-icon-palette':'error red','-gtk-icon-shadow':'none','-gtk-icon-filter':'none',
 '-gtk-icon-weight':'700','-gtk-dpi':'96','-gtk-line-height':'1',
 'transform':'rotate(5deg)','transform-origin':'center','min-width':'10px','min-height':'10px',
 'width':'10px','height':'10px','max-width':'10px','margin':'1px','margin-top':'1px','padding':'1px 2px',
 'padding-left':'1px','border':'1px solid red','border-width':'1px','border-style':'solid','border-color':'red',
 'border-radius':'4px','border-top-left-radius':'4px','border-image':'none','border-spacing':'2px',
 'outline':'1px solid red','outline-offset':'1px','box-shadow':'0 1px 2px red','background':'red',
 'background-color':'red','background-image':'linear-gradient(red, blue)','background-clip':'padding-box',
 'background-size':'cover','background-blend-mode':'multiply','transition':'all 200ms ease',
 'transition-property':'color','animation':'x 1s infinite','animation-name':'x','display':'none','visibility':'hidden',
 'position':'absolute','flex':'1','gap':'4px','justify-content':'center','align-items':'center','z-index':'1',
 'cursor':'pointer','overflow':'hidden','box-sizing':'border-box','text-indent':'1px','word-spacing':'1px',
 'accent-color':'red','direction':'rtl','user-select':'none','pointer-events':'none','tab-size':'4',
 '--my-var':'red',
}
res = {'props_ok': [], 'props_bad': {}}
for k, v in props.items():
    e = errors(f'button {{ {k}: {v}; }}')
    (res['props_ok'].append(k) if not e else res['props_bad'].__setitem__(k, e[0]))

values = {
 'named @color':'@define-color acc red; button { color: @acc; }',
 'var()':'button { --acc: red; color: var(--acc); }',
 'var() fallback':'button { color: var(--nope, blue); }',
 ':root var':':root { --acc: red; } button { color: var(--acc); }',
 'alpha()':'button { color: alpha(red, 0.5); }','mix()':'button { color: mix(red, blue, 0.5); }',
 'shade()':'button { color: shade(red, 0.5); }','color-mix()':'button { color: color-mix(in srgb, red, blue); }',
 'oklch()':'button { color: oklch(0.6 0.1 250); }','rgb()':'button { color: rgb(1 2 3 / 50%); }',
 'hsl()':'button { color: hsl(200 50% 50%); }','relative color':'button { color: rgb(from red r g b / 50%); }',
 'calc()':'button { min-width: calc(10px + 2em); }','em/rem':'button { padding: 1em 1rem; }',
 'percent padding':'button { padding: 10%; }','currentColor':'button { border-color: currentColor; }',
 'accent named':'button { color: @accent_color; }', 'transparent':'button { color: transparent; }',
 '!important':'button { color: red !important; }',
}
selectors = {
 'type':'button {}','class':'.x {}','compound':'button.x.y {}','id (widget name)':'#name {}','*':'* {}',
 'child >':'box > button {}','descendant':'box button {}','adjacent +':'label + button {}','sibling ~':'label ~ button {}',
 ':hover':'button:hover {}',':active':'button:active {}',':focus':'button:focus {}',':focus-visible':'button:focus-visible {}',
 ':focus-within':'box:focus-within {}',':disabled':'button:disabled {}',':checked':'button:checked {}',
 ':backdrop':'button:backdrop {}',':selected':'row:selected {}',':drop(active)':'button:drop(active) {}',
 ':link/:visited':'label:link {}',':first-child':'button:first-child {}',':last-child':'button:last-child {}',
 ':only-child':'button:only-child {}',':nth-child(2n+1)':'button:nth-child(2n+1) {}',':nth-last-child':'button:nth-last-child(1) {}',
 ':not()':'button:not(.x) {}',':not(complex)':'button:not(box > .x) {}',':is()':':is(button, label) {}',':where()':'button:where(.x) {}',
 ':has()':'box:has(button) {}',':root':':root {}',':dir(rtl)':'button:dir(rtl) {}','attribute [x]':'button[x] {}',
 'attribute [x="1"]':'button[x="1"] {}','::selection':'label > selection {}','pseudo-element ::before':'button::before {}',
 'nesting &':'box { & button { color: red; } }','@media dark':'@media (prefers-color-scheme: dark) { button { color: red; } }',
 '@media contrast':'@media (prefers-contrast: more) { button { color: red; } }',
 '@media reduced-motion':'@media (prefers-reduced-motion: reduce) { button { color: red; } }',
 '@media width':'@media (min-width: 400px) { button { color: red; } }',
 '@keyframes':'@keyframes x { from { opacity: 0; } to { opacity: 1; } } button { animation: x 1s; }',
 '@import':'@import url("file:///nonexistent.css");','@property':'@property --x { syntax: "<color>"; inherits: true; initial-value: red; }',
 '@supports':'@supports (color: red) { button { color: red; } }','@layer':'@layer base { button { color: red; } }',
}
for name, group in (('values', values), ('selectors', selectors)):
    res[name] = {k: (errors(css) or ['OK'])[0] for k, css in group.items()}
json.dump(res, sys.stdout, indent=1)
