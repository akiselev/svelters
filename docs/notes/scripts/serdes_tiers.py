import sys, json, glob, collections
sys.path.insert(0, __import__('os').path.dirname(__import__('os').path.abspath(__file__)))
import serdes_qt_inventory as q, serdes_gir_inventory as g
T0 = {"void","bool","i32","i64","f64","string","enum","flags","object"}
T1 = T0 | {"value_struct","strv","value_struct(out)","i32(out)","f64(out)","string(out)","bool(out)","enum(out)"}
T2 = T1 | {"boxed_ref","image_value","image_object","rich_value","bytes","list","list_model","nonqobject_pointer","model_index","iter_struct","iter_struct(out)","date","variant","gtype"}
T3 = T2 | {"error(out)","error","callback","async_callback","cancellable"}
def tiers(rows):
    mem = collections.defaultdict(set)
    for site, owner, m, c, _, mods in rows:
        site = "method" if site in ("param","return") else ("signal" if site.startswith("signal") else site)
        mem[(site, owner, m)].add(c + ("(out)" if ("out" in mods or "inout" in mods) else ""))
    out = {}
    for site in ("property","method","signal"):
        ms = [v for k,v in mem.items() if k[0]==site]
        out[site] = [len(ms)] + [round(100*sum(1 for v in ms if v <= T)/max(1,len(ms))) for T in (T0,T1,T2,T3)]
    return out
kinds = g.load('/usr/share/gir-1.0')
P = {"Window","ApplicationWindow","Box","Button","Entry","HeaderBar","Label","ScrolledWindow","Widget","Editable","Orientable"}
print("GTK P", tiers(g.walk(kinds, classes=P)))
print("GTK all", tiers(g.walk(kinds)))
e,f,o = q.load_qdoc('/usr/share/doc/qt6')
docs = [json.load(open(x)) for x in sorted(glob.glob('/projects/svelters/metadata/qt/*.json'))]
QP = {"QWidget","QLabel","QLineEdit","QMainWindow","QMenuBar","QPushButton","QScrollArea","QStatusBar","QAbstractButton","QFrame","QAbstractScrollArea"}
print("Qt P", tiers([r for d in docs if d['native']['name'] in QP for r in q.rows_for(d,e,f,o)]))
print("Qt all", tiers([r for d in docs for r in q.rows_for(d,e,f,o)]))
