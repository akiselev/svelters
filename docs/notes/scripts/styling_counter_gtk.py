# Counter styled through the GTK path: host token provider + translated component CSS + inline-style
# class. Renders states to PNG via WidgetPaintable (no X screenshot tool needed).
import gi, json, sys, warnings; warnings.filterwarnings('ignore')
gi.require_version('Gtk', '4.0')
from gi.repository import Gtk, Gdk, GLib
S = sys.argv[1]; T = json.load(open(f'{S}/counter_translated.json')); H = T['hash']
Gtk.init(); disp = Gdk.Display.get_default(); APP = Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION
# (c) host token sheet: portable tokens -> GTK theme named colours (plain GTK, no libadwaita)
TOKENS = """
:root { --sv-accent: @accent_color; --sv-accent-hover: shade(@accent_color, 1.15); --sv-accent-fg: white;
        --sv-surface: @theme_base_color; --sv-border: @borders; --sv-fg: @theme_fg_color; }
@media (prefers-color-scheme: dark) {
  :root { --sv-surface: #2a2a2e; --sv-border: #444; --sv-fg: #eeeeec; }
  window { background: #1e1e20; }
}"""
providers = []
def provider(css, prio):
    p = Gtk.CssProvider(); errs = []
    p.connect('parsing-error', lambda _p, sec, e: errs.append(f'{sec.to_string()}: {e.message}'))
    p.load_from_string(css); Gtk.StyleContext.add_provider_for_display(disp, p, prio); providers.append(p)
    if errs: print('CSS errors:', errs)
    return p
provider(TOKENS, APP - 10)
provider(T['gtk']['css'], APP)                     # (a) component sheet, one provider per component
inline = provider('', APP + 10)                    # inline styles: per-widget class, rules in one provider

win = Gtk.Window(title='Counter (GTK)'); win.set_default_size(260, 150)
box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8); box.set_css_classes(['card', H])
box.set_margin_top(12); box.set_margin_bottom(12); box.set_margin_start(12); box.set_margin_end(12)
lbl = Gtk.Label(label='Count: 0'); lbl.set_css_classes(['count', H, 'sv-i1'])
btn = Gtk.Button(label='Increment'); btn.set_css_classes(['primary', H])
box.append(lbl); box.append(btn); win.set_child(box); win.present()
def set_inline(style): inline.load_from_string(f'.sv-i1 {{ {style} }}')
set_inline('font-weight: bold;')

def spin(n=30):
    for _ in range(n): GLib.MainContext.default().iteration(False)
def shot(name):
    spin(); w, h = win.get_width(), win.get_height()
    snap = Gtk.Snapshot(); Gtk.WidgetPaintable.new(win).snapshot(snap, w, h)
    tex = win.get_native().get_renderer().render_texture(snap.to_node(), None); tex.save_to_png(f'{S}/shots/gtk_{name}.png')
    c = btn.get_color(); print(f'{name:10s} button fg={c.to_string()} label fg={lbl.get_color().to_string()}')
GLib.timeout_add(300, lambda: None); spin(200)
shot('normal')
btn.set_state_flags(Gtk.StateFlags.PRELIGHT, False); spin(); GLib.usleep(200_000); spin(); shot('hover')  # after 150ms transition
btn.unset_state_flags(Gtk.StateFlags.PRELIGHT); spin(); GLib.usleep(250_000); spin()
lbl.set_label('Count: 3'); set_inline('font-weight: bold; color: #c01c28;'); shot('inline')
for p in providers: p.props.prefers_color_scheme = Gtk.InterfaceColorScheme.DARK
Gtk.Settings.get_default().props.gtk_interface_color_scheme = Gtk.InterfaceColorScheme.DARK
shot('dark')
