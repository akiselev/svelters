#!/usr/bin/env python3
"""Probe the Android SDK platform package for backend metadata (research aid, mobile notes).

Usage: python3 -I android_api_probe.py <platform-dir> [Class ...]

<platform-dir> is an unpacked `platform-XX_rYY.zip` from dl.google.com/android/repository
(it holds android.jar, android-stubs-src.jar and data/{api-versions.xml,widgets.txt,
annotations.zip,res/values/attrs.xml}). Reads the Java stub sources (no JDK needed), not
android.jar. Prints, for android.view + android.widget and for the named classes:

- getter/setter pairs (property candidates), listener setters/adders (signal candidates),
- listener interfaces and how many of their methods return a value (call_sync candidates),
- @IntDef/@ColorInt-style annotations from annotations.zip (enum/flag/colour typing),
- since-API levels from api-versions.xml,
- declare-styleable attrs that match a setter by name (XML attr -> setter heuristic).
"""
import re
import sys
import zipfile
import xml.etree.ElementTree as ET
from collections import Counter
from pathlib import Path

SLICE = ["android.view.View", "android.widget.LinearLayout", "android.widget.TextView",
         "android.widget.Button", "android.widget.Switch", "android.widget.CompoundButton",
         "android.widget.SeekBar", "android.widget.EditText", "android.widget.ProgressBar"]

METHOD = re.compile(r"^public (?:static )?(?:final )?(?:abstract )?(?:synchronized )?(?:default )?(?:<[^>]+> )?"
                    r"([\w.<>\[\]?, ]+?) (\w+)\((.*)\)")
TYPE_DECL = re.compile(r"^public (?:static )?(?:abstract )?(?:final )?(class|interface|enum|@interface) (\w+)")


def strip_ann(s):
    return re.sub(r"@[\w.]+(\([^)]*\))?\s*", "", s)


def parse_stub(src, pkg_cls):
    """Return {qualified_type: {"kind", "methods": [(ret, name, params)]}} for one stub file."""
    out, stack = {}, []
    for raw in src.splitlines():
        line = raw.strip()
        m = TYPE_DECL.match(strip_ann(line))
        if m and line.endswith("{"):
            name = pkg_cls if not stack else stack[-1] + "$" + m.group(2)
            out[name] = {"kind": m.group(1), "methods": []}
            stack.append(name)
            continue
        if line == "}" and stack:
            stack.pop()
            continue
        if stack:
            mm = METHOD.match(strip_ann(line))
            if mm:
                ret, name, params = mm.groups()
                if name == stack[-1].split("$")[-1].split(".")[-1]:
                    continue  # constructor
                params = [p for p in strip_ann(params).split(",") if p.strip()]
                out[stack[-1]]["methods"].append((ret.strip(), name, len(params)))
    return out


def load(platform):
    types = {}
    with zipfile.ZipFile(platform / "android-stubs-src.jar") as z:
        for n in z.namelist():
            if not n.endswith(".java"):
                continue
            pkg = n.rsplit("/", 1)[0].replace("/", ".")
            if pkg not in ("android.view", "android.widget", "android.text"):
                continue
            q = n[:-5].replace("/", ".")
            types.update(parse_stub(z.read(n).decode(), q))
    return types


def classify(methods):
    names = {n: (r, k) for r, n, k in methods}
    props, listeners, adders = [], [], []
    for r, n, k in methods:
        if re.match(r"set[A-Z]", n) and k == 1:
            base = n[3:]
            if base.endswith("Listener"):
                listeners.append(n)
            elif ("get" + base) in names or ("is" + base) in names or ("has" + base) in names:
                props.append(base[0].lower() + base[1:])
        elif re.match(r"(add|remove)[A-Z]\w*(Listener|Callback|Watcher)$", n):
            adders.append(n)
    return sorted(set(props)), sorted(set(listeners)), sorted(set(adders))


def since_map(platform):
    tree = ET.parse(platform / "data" / "api-versions.xml")
    out = {}
    for c in tree.getroot().iter("class"):
        out[c.get("name").replace("/", ".")] = c.get("since", "1")
    return out


def styleables(platform):
    root = ET.parse(platform / "data" / "res" / "values" / "attrs.xml").getroot()
    return {s.get("name"): [a.get("name") for a in s.findall("attr")] for s in root.iter("declare-styleable")}


def annotations(platform):
    c = Counter()
    with zipfile.ZipFile(platform / "data" / "annotations.zip") as z:
        for n in z.namelist():
            if n.startswith(("android/widget/", "android/view/annotations")):
                for a in ET.fromstring(z.read(n)).iter("annotation"):
                    c[a.get("name").rsplit(".", 1)[-1]] += 1
    return c


def main():
    platform = Path(sys.argv[1])
    want = sys.argv[2:] or SLICE
    types = load(platform)
    since = since_map(platform)
    sty = styleables(platform)

    ifaces = {q: t for q, t in types.items() if t["kind"] == "interface" and re.search(r"(Listener|Watcher|Callback)$", q)}
    returning = {q: sum(1 for r, _, _ in t["methods"] if r != "void") for q, t in ifaces.items()}
    print(f"view/widget/text types parsed: {len(types)}; listener-like interfaces: {len(ifaces)}; "
          f"their methods: {sum(len(t['methods']) for t in ifaces.values())}, non-void: {sum(returning.values())}")
    print("annotations (android.widget + android.view):", dict(annotations(platform).most_common(8)))
    print(f"declare-styleable blocks: {len(sty)}, attrs in them: {sum(map(len, sty.values()))}")
    print()
    for q in want:
        t = types.get(q)
        if not t:
            print(q, "not found")
            continue
        props, listeners, adders = classify(t["methods"])
        simple = q.rsplit(".", 1)[-1]
        attrs = sty.get(simple, [])
        attr_hit = [a for a in attrs if a in props]
        print(f"{q} (since API {since.get(q, '?')}): {len(t['methods'])} methods, {len(props)} get/set props, "
              f"listener setters {listeners}, add/remove {adders}")
        print(f"   styleable {simple}: {len(attrs)} attrs, {len(attr_hit)} named like a setter: {attr_hit[:12]}")
    print()
    for q in sorted(ifaces):
        if q.startswith(("android.view.View$", "android.widget.", "android.text.TextWatcher", "android.view.ViewTreeObserver$")):
            ms = ifaces[q]["methods"]
            nv = [f"{n}->{r}" for r, n, _ in ms if r != "void"]
            print(f"  {q}: {len(ms)} methods; returns: {nv}")


if __name__ == "__main__":
    main()
