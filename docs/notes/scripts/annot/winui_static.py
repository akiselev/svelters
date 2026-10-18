#!/usr/bin/env python3
"""Static rule coverage over a WinUI winmd dump (winmd-tool dump of the Controls classes).

  target/winmd-tool/release/winmd-tool dump Microsoft.UI.Xaml.Controls.<C>... -- @winmds.txt > dump.json
  python3 docs/notes/scripts/annot/winui_static.py dump.json
Rules: P (prop with get+set), D (prop backed by a DependencyProperty -> notify through
RegisterPropertyChangedCallback, so two-way bindable), E (event; args with settable
Handled/Cancel -> sync result), C (ContentProperty attribute -> default slot), K (Panel.Children /
ItemsControl.Items -> many slot), A (static GetX/SetX(DependencyObject|UIElement) pair -> attached
child attribute).
"""
import collections
import json
import re
import sys

SCALAR = re.compile(r"^(bool|i32|u32|i64|u64|f32|f64|string|Windows\.Foundation\.TimeSpan)$")


def main() -> int:
    types = json.load(open(sys.argv[1]))["types"]
    c = collections.Counter()
    ctrls = {k: v for k, v in types.items() if v["category"] == "class" and k.startswith("Microsoft.UI.Xaml.Controls.")
             and not k.endswith(("EventArgs", "Factory", "Statics"))}
    for name, t in ctrls.items():
        c["classes"] += 1
        dps = {d[:-len("Property")] for d in t.get("dependency_properties", [])}
        for p in t.get("props", []):
            c["prop"] += 1
            if p.get("get") and p.get("set"):
                c["prop:rw"] += 1
                if p["name"] in dps:
                    c["prop:rw:dp(bindable)"] += 1
                if SCALAR.match(p["type"]) or types.get(p["type"], {}).get("category") == "enum":
                    c["prop:rw:scalar_or_enum"] += 1
        for e in t.get("events", []):
            c["event"] += 1
            blob = json.dumps(e) + json.dumps(types.get(e.get("delegate", ""), {}))
            args = re.findall(r"Microsoft\.UI\.Xaml[\w.]*EventArgs", blob)
            settable = False
            for a in args:
                for ap in types.get(a, {}).get("props", []):
                    if ap["name"] in ("Handled", "Cancel") and ap.get("set"):
                        settable = True
            c["event:sync_result(Handled/Cancel)" if settable else "event:plain"] += 1
        attrs = t.get("attributes", {})
        if "ContentProperty" in attrs:
            c["content_property"] += 1
        sm = {m["name"] if isinstance(m, dict) else m for m in t.get("static_methods", [])}
        for m in sm:
            mm = re.match(r"Set(\w+)$", m)
            if mm and f"Get{mm.group(1)}" in sm:
                c["attached_pair"] += 1
        c["methods"] += len(t.get("methods", []))
    for k, v in sorted(c.items()):
        print(f"  {k:40} {v}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
