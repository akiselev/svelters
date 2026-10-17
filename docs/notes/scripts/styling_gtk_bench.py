# 1000 GtkButtons: cost of styling strategies. Times: API calls ("apply") and the next
# frame (style recompute + layout + paint, measured via the frame clock's after-paint).
import gi, sys, time, json, warnings
gi.require_version('Gtk', '4.0')
from gi.repository import Gtk, Gdk, GLib
warnings.filterwarnings('ignore')
N = int(sys.argv[2]) if len(sys.argv) > 2 else 1000
MODE = sys.argv[1]
app = Gtk.Application(application_id='dev.svelters.bench')
res = {'mode': MODE, 'n': N}

def frame(win):
    """Queue a redraw and spin until one full frame has been painted; return seconds."""
    t0 = time.perf_counter(); done = []
    clock = win.get_frame_clock()
    hid = clock.connect('after-paint', lambda *_: done.append(1))
    win.queue_draw()
    ctx = GLib.MainContext.default()
    while not done: ctx.iteration(True)
    clock.disconnect(hid)
    return time.perf_counter() - t0

def run(app):
    win = Gtk.ApplicationWindow(application=app, default_width=1200, default_height=900)
    sw = Gtk.ScrolledWindow(); flow = Gtk.FlowBox(max_children_per_line=40, selection_mode=Gtk.SelectionMode.NONE)
    sw.set_child(flow); win.set_child(sw)
    btns = [Gtk.Button(label=f'{i}') for i in range(N)]
    for b in btns: flow.append(b)
    win.present()
    disp = Gdk.Display.get_default()
    P = Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION
    res['baseline_frame'] = frame(win); frame(win)
    res['steady_frame'] = frame(win)
    t0 = time.perf_counter()
    if MODE == 'class':            # one provider, one rule, one shared class on every button
        p = Gtk.CssProvider(); p.load_from_string('.sv-a { color: white; background: #3584e4; padding: 4px 10px; }')
        Gtk.StyleContext.add_provider_for_display(disp, p, P)
        for b in btns: b.add_css_class('sv-a')
    elif MODE == 'inline_one_provider':   # per-widget class + 1000 rules in ONE provider, loaded once
        css = ''.join(f'.sv-i{i} {{ color: rgb({i%256},0,0); padding: {i%7}px; }}\n' for i in range(N))
        p = Gtk.CssProvider(); p.load_from_string(css)
        Gtk.StyleContext.add_provider_for_display(disp, p, P)
        for i, b in enumerate(btns): b.add_css_class(f'sv-i{i}')
    elif MODE == 'inline_n_providers':    # per-widget class + one display provider per widget
        provs = []
        for i, b in enumerate(btns):
            p = Gtk.CssProvider(); p.load_from_string(f'.sv-i{i} {{ color: rgb({i%256},0,0); padding: {i%7}px; }}')
            Gtk.StyleContext.add_provider_for_display(disp, p, P); provs.append(p)
            b.add_css_class(f'sv-i{i}')
    elif MODE == 'inline_widget_ctx':     # deprecated per-widget GtkStyleContext provider
        provs = []
        for i, b in enumerate(btns):
            p = Gtk.CssProvider(); p.load_from_string(f'* {{ color: rgb({i%256},0,0); padding: {i%7}px; }}')
            b.get_style_context().add_provider(p, P); provs.append(p)
    elif MODE == 'dedupe10':   # inline styles deduped by content into shared classes: 10 distinct styles
        css = ''.join(f'.sv-s{k} {{ color: rgb({k*20},0,0); padding: {k%7}px; }}\n' for k in range(10))
        p = Gtk.CssProvider(); p.load_from_string(css)
        Gtk.StyleContext.add_provider_for_display(disp, p, P)
        for i, b in enumerate(btns): b.add_css_class(f'sv-s{i%10}')
    res['apply'] = time.perf_counter() - t0
    res['first_frame'] = frame(win)
    # incremental update: change ONE widget's inline style
    t0 = time.perf_counter()
    if MODE == 'class':
        btns[0].remove_css_class('sv-a'); btns[0].add_css_class('sv-a')  # toggle class off/on (class: directive)
        upd_apply = time.perf_counter() - t0
        upd_frame = frame(win)
        # toggling the class on ALL widgets
        t0 = time.perf_counter()
        for b in btns: b.remove_css_class('sv-a')
        res['toggle_all_apply'] = time.perf_counter() - t0; res['toggle_all_frame'] = frame(win)
    elif MODE == 'inline_one_provider':    # rewrite the shared provider with one rule changed
        css = css.replace('.sv-i0 { color: rgb(0,0,0)', '.sv-i0 { color: rgb(0,200,0)')
        p.load_from_string(css); upd_apply = time.perf_counter() - t0; upd_frame = frame(win)
    elif MODE == 'inline_n_providers':
        provs[0].load_from_string('.sv-i0 { color: green; padding: 0px; }'); upd_apply = time.perf_counter() - t0; upd_frame = frame(win)
    elif MODE == 'inline_widget_ctx':
        provs[0].load_from_string('* { color: green; padding: 0px; }'); upd_apply = time.perf_counter() - t0; upd_frame = frame(win)
    elif MODE == 'dedupe10':   # one widget switches to an existing style class
        btns[0].remove_css_class('sv-s0'); btns[0].add_css_class('sv-s7'); upd_apply = time.perf_counter() - t0; upd_frame = frame(win)
    res['update_one_apply'] = upd_apply; res['update_one_frame'] = upd_frame
    # second, warm update on another widget (colour only, size-neutral)
    t0 = time.perf_counter()
    if MODE == 'class': btns[1].add_css_class('sv-warm')
    elif MODE == 'dedupe10': btns[1].remove_css_class('sv-s1'); btns[1].add_css_class('sv-s8')
    elif MODE == 'inline_widget_ctx': provs[1].load_from_string('* { color: blue; padding: 1px; }')
    elif MODE == 'inline_one_provider': css = css.replace('.sv-i1 { color: rgb(1,0,0)', '.sv-i1 { color: rgb(0,0,200)'); p.load_from_string(css)
    elif MODE == 'inline_n_providers': provs[1].load_from_string('.sv-i1 { color: blue; padding: 1px; }')
    res['update_two_apply'] = time.perf_counter() - t0; res['update_two_frame'] = frame(win)
    # HMR-style: swap a display provider holding 50 rules
    hp = Gtk.CssProvider(); hp.load_from_string('button { margin: 1px; }')
    Gtk.StyleContext.add_provider_for_display(disp, hp, P); frame(win)
    t0 = time.perf_counter(); hp.load_from_string('button { margin: 2px; }' + ''.join(f'.hmr{i} {{ color: red; }}' for i in range(50)))
    res['hmr_reload_apply'] = time.perf_counter() - t0; res['hmr_reload_frame'] = frame(win)
    print(json.dumps({k: (round(v * 1000, 2) if isinstance(v, float) else v) for k, v in res.items()}))
    app.quit()
app.connect('activate', run); app.run([])
