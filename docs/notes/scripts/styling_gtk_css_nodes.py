# Dump CSS name, default style classes and the internal widget (CSS node) tree of every
# instantiable GtkWidget subclass in GTK 4.22. Run under Xvfb. Output JSON on stdout.
import gi, json, sys, warnings
gi.require_version('Gtk', '4.0')
from gi.repository import Gtk, GObject
warnings.filterwarnings('ignore')
Gtk.init()

def tree(w, depth=0):
    node = {'css': w.get_css_name(), 'classes': list(w.get_css_classes())}
    kids = []; c = w.get_first_child()
    while c is not None and depth < 6:
        kids.append(tree(c, depth + 1)); c = c.get_next_sibling()
    if kids: node['children'] = kids
    return node

def all_subtypes(t):
    for c in GObject.type_children(t):
        yield c; yield from all_subtypes(c)

out = {}
types = []
for a in dir(Gtk):
    c = getattr(Gtk, a, None)
    if isinstance(c, type) and issubclass(c, Gtk.Widget): types.append(c.__gtype__)
for gt in sorted(set(types), key=lambda t: t.name):
    name = gt.name
    if GObject.type_is_a(gt, GObject.type_from_name('GtkDialog')) or 'Chooser' in name and 'Dialog' in name: continue
    if gt.is_abstract(): continue
    try:
        w = GObject.new(gt)
    except Exception as e:
        out[name] = {'error': str(e)}; continue
    if name in ('GtkButton', 'GtkToggleButton'): w.set_label('x')
    out[name] = tree(w)
json.dump(out, sys.stdout, indent=1, sort_keys=True)
