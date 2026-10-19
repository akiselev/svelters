#!/usr/bin/env python3
"""GTK docs frontend: metadata/gtk + metadata/docs/gtk + overrides -> svelters-docs-ir/v0.

Owns the GTK quirks: GIR / gi-docgen text (already converted to xref links by the docs pass),
GObject properties/signals, `notify::` bindability, CSS-node / accessibility / shortcut sections
of class docs, since/deprecated versions, LGPL notices, and the
floating-reference lifetime model. Emits only the neutral IR (plus an `extensions.gtk` block).

    python3 tools/docs/gtk/frontend.py [--out docs/site/.generated/ir/gtk.json]
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "tools/docs/ir"))
sys.path.insert(0, str(ROOT / "tools/metadata/common"))
import irlib  # noqa: E402
import svmeta
LIFE = {"owned": "owned", "toplevel": "toplevel", "value": "value", "borrowed_only": "unknown"}  # noqa: E402

TK = "gtk"
LICENSE = {
    "spdx": "LGPL-2.1-or-later",
    "name": "GNU Lesser General Public License v2.1 or later",
    "url": "https://www.gnu.org/licenses/old-licenses/lgpl-2.1.html",
    "attribution": "Documentation text from the GTK project (GIR documentation comments), (c) the GTK authors. "
                   "Cross-reference markup has been converted for this site.",
    "redistribution": "full",
}
UPSTREAM = "https://docs.gtk.org/gtk4/"
ID_PREFIX = "svelters-api:v1:type:gtk:"
KIND_MAP = {"object": "class", "interface": "interface", "enum": "enum", "flags": "flags", "boxed": "boxed"}
UPSTREAM_PAGE = {"class": "class", "interface": "iface", "enum": "enum", "flags": "flags", "boxed": "struct"}
SECTION_KEYS = {
    "CSS nodes": "css_nodes",
    "Accessibility": "accessibility",
    "Shortcuts and Gestures": "shortcuts",
    "Actions": "actions",
    "GtkBuilder UI Definition": "builder_ui",
    "CSS Nodes": "css_nodes",
    "Style classes": "style_classes",
}
EXTENSION_SCHEMA = {
    "title": "GTK specifics",
    "fields": [
        {"key": "gtype", "label": "GType", "format": "code", "scope": "entity"},
        {"key": "get_type", "label": "get_type()", "format": "code", "scope": "entity"},
        {"key": "css_nodes", "label": "CSS nodes", "format": "markdown", "scope": "entity"},
        {"key": "style_classes", "label": "Style classes", "format": "markdown", "scope": "entity"},
        {"key": "accessibility", "label": "Accessibility", "format": "markdown", "scope": "entity"},
        {"key": "shortcuts", "label": "Shortcuts and gestures", "format": "markdown", "scope": "entity"},
        {"key": "actions", "label": "Actions", "format": "markdown", "scope": "entity"},
        {"key": "builder_ui", "label": "GtkBuilder UI definition", "format": "markdown", "scope": "entity"},
        {"key": "source", "label": "Documented in", "format": "code", "scope": "entity"},
        {"key": "param_flags", "label": "GParamFlags", "format": "list", "scope": "property"},
        {"key": "gtype", "label": "GType", "format": "code", "scope": "property"},
        {"key": "when", "label": "Emission phase", "format": "code", "scope": "event"},
        {"key": "signal_flags", "label": "Signal flags", "format": "list", "scope": "event"},
        {"key": "c_symbol", "label": "C symbol", "format": "code", "scope": "method"},
        {"key": "invocation", "label": "Invocation (dev / release)", "format": "code", "scope": "method"},
    ],
}


def eid(ns: str, name: str) -> str:
    return f"{TK}:{ns}.{name}"


def split_type_id(tid: str) -> tuple[str, str]:
    ns, name = tid[len(ID_PREFIX):].split(":")
    return ns, name


def split_sections(md: str) -> tuple[str, dict[str, str]]:
    """Split a class description at top-level `# Heading` lines (ignoring code fences)."""
    pre: list[str] = []
    sections: dict[str, list[str]] = {}
    cur = None
    fence = False
    for line in md.split("\n"):
        if line.startswith("```"):
            fence = not fence
        m = None if fence else re.match(r"^# (.+)$", line)
        if m:
            cur = m.group(1).strip()
            sections[cur] = []
        elif cur is None:
            pre.append(line)
        else:
            sections[cur].append(line)
    return "\n".join(pre).strip(), {k: "\n".join(v).strip() for k, v in sections.items()}


def demote_unknown(sections: dict[str, str]) -> tuple[dict[str, str], str]:
    ext, rest = {}, []
    for title, body in sections.items():
        key = SECTION_KEYS.get(title)
        if key and body:
            ext[key] = body
        elif body:
            rest.append(f"#### {title}\n\n{body}")
    return ext, "\n\n".join(rest)


class Frontend:
    def __init__(self, root: Path = ROOT):
        self.root = root
        self.meta: dict[str, dict] = {}
        self.docs: dict[str, dict] = {}
        for f in sorted((root / "metadata/gtk").glob("*.json")):
            self.meta[f.stem] = svmeta.load_json(f)
            dp = root / "metadata/docs/gtk" / f.name
            self.docs[f.stem] = svmeta.load_json(dp) if dp.exists() else {}
        self.by_id = {m["id"]: stem for stem, m in self.meta.items()}
        self.overrides = {p.stem: p for p in sorted((root / "metadata/overrides/gtk").glob("*.yaml"))}
        # The verified ledger (A1): a fact is tested when every generated test of it passed.
        sys.path.insert(0, str(root / "tools/metadata"))
        import ledger  # noqa: E402

        self.verified = ledger.verified_facts("gtk")
        facts = root / "crates/svelters-gtk/src/generated/facts.json"
        self.facts = [f["fact"] for f in json.loads(facts.read_text())["facts"]] if facts.exists() else []
        # Dangerous members (metadata/dangerous/<tk>.yaml, override `dangerous:` blocks): docs warn on them.
        import dangerous  # noqa: E402  (tools/metadata/common)

        self.dangerous = dangerous.Dangerous(TK, root)

    # ----- types -----
    def name_of(self, tid: str) -> tuple[str, str | None]:
        full = tid if tid.startswith(ID_PREFIX) else ID_PREFIX + tid
        ns, name = split_type_id(full)
        disp = name if ns == "Gtk" else f"{ns}.{name}"
        return disp, (eid(ns, name) if full in self.by_id else None)

    def typeref(self, t: dict) -> dict:
        ts, ref = irlib.neutral_to_ts(t["ref"], self.name_of)
        out = {"display": ts, "neutral": t["ref"]}
        if ref:
            out["ref"] = ref
        if t.get("native"):
            out["native"] = t["native"]
        if t.get("nullable"):
            out["nullable"] = True
        return out

    # ----- text -----
    def text(self, block: dict | None, md: str | None = None, url: str | None = None) -> dict | None:
        if not block and not md:
            return None
        return irlib.text(md if md is not None else block["md"], LICENSE["spdx"], "gir", upstream_url=url)

    def avail(self, doc: dict | None, m: dict | None = None) -> dict | None:
        since = (doc or {}).get("since") or (m or {}).get("since")
        dep = (doc or {}).get("deprecated")
        out: dict = {}
        if since:
            out["since"] = since
        if dep:
            out["deprecated"] = dep
            if (doc or {}).get("deprecated_md"):
                out["deprecated_note"] = doc["deprecated_md"]
        return out or None

    # ----- members -----
    def properties(self, m: dict, d: dict, page_url: str) -> list[dict]:
        out = []
        for p in m.get("properties", []):
            doc = d.get("properties", {}).get(p["name"])
            notify = (p.get("notify") or {}).get("signal")
            acc = p["access"]
            rec = {
                "name": p["name"],
                "js_name": p["js_name"],
                "origin": p["origin"],
                "type": self.typeref(p["type"]),
                "default": irlib.display_value(p.get("default")),
                "range": p.get("range"),
                "access": {"read": acc["read"], "write": acc["write"], "construct_only": acc.get("construct_only", False)},
                "notify": notify,
                "bindable": bool(notify) and acc["write"],
                "role": p.get("role", "value"),
                "accessors": {k: v for k, v in (p.get("accessors") or {}).items() if v},
                "annotated": "annotated" in p,
                **({"verified": True} if p.get("id") in self.verified else {}),
                "native_id": p["id"],
                "extensions": {TK: {"gtype": (p.get("runtime") or {}).get("gtype", ""), "param_flags": (p.get("runtime") or {}).get("flags", [])}},
            }
            origins = sorted({p["origin"]} | ({"runtime"} if p.get("runtime") else set()) | ({p["annotated"].get("origin", "manual")} if "annotated" in p else set()))
            rec["origins"] = origins
            av = self.avail(doc, p)
            if av:
                rec["availability"] = av
            t = self.text(doc, url=f"{page_url}")
            if t:
                rec["doc"] = t
            out.append(rec)
        return sorted(out, key=lambda r: r["name"])

    def events(self, m: dict, d: dict) -> list[dict]:
        out = []
        for s in m.get("signals", []):
            doc = d.get("signals", {}).get(s["name"])
            je = s["js_event"]
            attr = f"on{je}"
            rec = {
                "name": s["name"],
                "js_event": je,
                "handler_prop": attr,
                "params": [{"name": p.get("name", f"arg{i}"), "type": self.typeref(p["type"] if "type" in p and isinstance(p["type"], dict) else {"ref": p.get("type", "unknown")})} for i, p in enumerate(s.get("params", []))],
                "returns": {"display": "void" if s["returns"]["type"] == "void" else irlib.neutral_to_ts(s["returns"]["type"], self.name_of)[0]},
                "handler_result": s.get("handler_result", "none"),
                "sync_required": bool(s.get("sync_required", False)),
                "phase": s.get("phase"),
                "flags": s.get("flags", []),
                "origin": s["origin"],
                "origins": sorted({s["origin"], "runtime"} | ({s["annotated"].get("origin", "manual")} if "annotated" in s else set())),
                "annotated": "annotated" in s,
                **({"verified": True} if s.get("id") in self.verified else {}),
                "native_id": s.get("id", m["id"] + ":signal:" + s["name"]),
                "extensions": {TK: {"when": s.get("phase") or "", "signal_flags": s.get("flags", [])}},
            }
            av = self.avail(doc)
            if av:
                rec["availability"] = av
            t = self.text(doc)
            if t:
                rec["doc"] = t
            out.append(rec)
        return sorted(out, key=lambda r: r["name"])

    def methods(self, m: dict, d: dict, ctor: bool) -> list[dict]:
        out = []
        for x in m.get("constructors" if ctor else "methods", []):
            if x["name"] == "generic" and x.get("origin") == "runtime":
                continue
            doc = d.get("constructors" if ctor else "methods", {}).get(x["name"])
            params = []
            for p in x.get("params", []):
                params.append({"name": p["name"], "type": self.typeref({"ref": p["type"], "native": p.get("native"), "nullable": p.get("nullable")})})
            ret = x.get("returns") or {}
            rt = irlib.neutral_to_ts(ret.get("type", "void"), self.name_of)[0] if ret.get("type") not in (None, "self") else ("self" if ret.get("type") == "self" else "void")
            sig = f"{x['name']}({', '.join(p['name'] + ': ' + p['type']['display'] for p in params)}): {rt}"
            inv = x.get("invocation") or {}
            rec = {
                "name": x["name"],
                "origin": x["origin"],
                "signature": sig,
                "params": params,
                "returns": {"display": rt},
                "kind": "constructor" if ctor else "method",
                "unsupported_reason": x.get("unsupported_reason"),
                "native_id": x["id"],
                "extensions": {TK: {"c_symbol": x.get("native", ""), "invocation": " / ".join(f"{k}: {v}" for k, v in sorted(inv.items()))}},
            }
            av = self.avail(doc)
            if av:
                rec["availability"] = av
            t = self.text(doc)
            if t:
                rec["doc"] = t
            out.append(rec)
        return sorted(out, key=lambda r: r["name"])

    def children(self, m: dict) -> dict | None:
        ch = m.get("children")
        if not ch:
            return None
        out: dict = {"policy": ch.get("policy", "unknown"), "default_slot": ch.get("default_slot"), "slots": []}
        cand = ch.get("candidates") or {}
        out["candidates"] = sorted([f"property:{p}" for p in cand.get("widget_props", [])] + [f"method:{o}" for o in cand.get("child_ops", [])])
        for s in ch.get("slots", []):
            ops = {}
            own = {}
            for op, v in sorted((s.get("ops") or {}).items()):
                if not isinstance(v, dict):
                    continue
                if "native" in v:
                    ops[op] = v["native"]
                elif "contains" in v:
                    ops[op] = f"contains: {v['contains']}"
                elif "iterate" in v:
                    ops[op] = "iterate: " + ", ".join(f"{k}={x}" for k, x in sorted(v["iterate"].items()) if k != "origin")
                eff = v.get("effects")
                if eff:
                    own[op] = ", ".join(f"{k} {x}" for k, x in sorted(eff.items()) if k != "origin")
            ann = s.get("annotated") or {}
            out["slots"].append({
                "name": s["name"],
                "cardinality": s.get("cardinality", "many"),
                "accepts": s.get("accepts", []),
                "default": s["name"] == ch.get("default_slot"),
                "order": s.get("order"),
                "ops": ops,
                "ownership": own,
                "on_remove": s.get("on_remove"),
                "attrs": [a if isinstance(a, str) else a.get("name", "?") for a in s.get("attrs", [])],
                "annotated": bool(ann),
                "provenance": [x if isinstance(x, str) else f"{x.get('origin')}: {x.get('ref')}" for x in (ann.get("provenance") if isinstance(ann.get("provenance"), list) else [ann.get("provenance")] if ann.get("provenance") else [])],
            })
        return out

    # ----- examples -----
    def example(self, tag: str, m: dict, props: list[dict], events: list[dict], children: dict | None) -> dict:
        def sample(p):
            t = p["type"]["neutral"]
            if t == "bool":
                return "{true}" if p["default"] != "true" else "{false}"
            if t in ("i32", "u32", "i64", "u64", "f32", "f64"):
                r = p.get("range") or {}
                v = 8
                if r.get("min") is not None and r["min"] > v:
                    v = r["min"]
                return "{%s}" % v
            if t == "string":
                return '"Hello"' if p["name"] in ("label", "title", "text", "placeholder-text", "description") else None
            if t.startswith("enum"):
                dv = (p["default"] or '""').strip('"')
                return json_str(dv)
            return None
        cand = [p for p in props if p["access"]["write"] and not p["access"]["construct_only"] and p["role"] == "value" and not p.get("dangerous") and sample(p)]
        cand.sort(key=lambda p: (not p["annotated"], p["type"]["neutral"].split("<")[0] not in ("string",), p["name"]))
        attrs = [f'{p["js_name"]}={sample(p)}' for p in cand[:3]]
        ev = sorted(events, key=lambda e: (not e["annotated"], e["name"]))[:1]
        script = []
        handlers = []
        if ev:
            script.append("  let count = $state(0);")
            handlers.append(f'{(ev[0].get("handler_aliases") or [ev[0]["handler_prop"]])[0]}={{() => count++}}')
        head = f"<script>\n{chr(10).join(script)}\n</script>\n\n" if script else ""
        pol = (children or {}).get("policy", "none")
        inner = ""
        if pol in ("single_child", "ordered", "named_slots", "layout_managed", "pages"):
            inner = "\n  <label>Child content</label>\n"
        a = " ".join(attrs + handlers)
        body = f"<{tag}{(' ' + a) if a else ''}>{inner}</{tag}>" if inner else f"<{tag}{(' ' + a) if a else ''} />"
        return {"title": f"Using <{tag}>", "lang": "svelte", "code": head + body + "\n", "generated": True, "tested": False}

    # ----- entity -----
    def entity(self, stem: str) -> dict:
        m, d = self.meta[stem], self.docs.get(stem, {})
        ns, name = split_type_id(m["id"])
        kind = KIND_MAP[m["kind"]]
        page = f"{UPSTREAM}{UPSTREAM_PAGE[kind]}.{name}.html"
        doc = d.get("doc")
        tag = (m.get("element") or {}).get("tag")
        ov = self.overrides.get(stem)
        annotated = ov is not None
        ext: dict = {"gtype": m["native"].get("gtype", ""), "get_type": m["native"].get("get_type", "")}
        description = None
        if doc:
            pre, sections = split_sections(doc["md"])
            sec_ext, rest = demote_unknown(sections)
            ext.update(sec_ext)
            md = pre + (("\n\n" + rest) if rest else "")
            description = self.text(doc, md=md, url=page)
            if doc.get("file"):
                ext["source"] = f"{doc['file']}:{doc.get('line', 0)}"
        e: dict = {
            "id": eid(ns, name),
            "kind": kind,
            "name": name,
            "qualified": f"{ns}.{name}",
            "native_name": m["native"].get("name") or name,
            "abstract": bool(m.get("abstract")),
            "tags": [tag] if tag else [],
            "status": {
                "tier": "annotated" if annotated else "ingested",
                "tested": any(f.startswith(m["id"] + ":") and f in self.verified for f in self.facts),
                "verified_facts": [sum(1 for f in self.facts if f.startswith(m["id"] + ":") and f in self.verified),
                                   sum(1 for f in self.facts if f.startswith(m["id"] + ":"))],
                "metadata_file": f"metadata/gtk/{stem}.json",
                "note": "" if annotated else "Raw: GTK properties and signals of this kind are reachable through the generated tables (dynamic GObject access), but its child slots, ownership and signal semantics are neither annotated nor tested.",
            },
            "upstream": {"url": page, "label": f"GTK 4 reference: GTK {name}"},
            "bases": [f"{TK}:{b}" for b in m.get("bases", [])],
            "interfaces": [f"{TK}:{b}" for b in m.get("interfaces", [])],
            "properties": [],
            "events": [],
            "methods": [],
            "constructors": [],
            "values": [],
            "serdes": {"status": "unknown"},
            "extensions": {TK: ext},
        }
        if annotated:
            e["status"]["annotation_files"] = [f"metadata/overrides/gtk/{stem}.yaml"]
            e["status"]["test_note"] = "Annotated facts have generated verification tests (crates/svelters-gtk); results are in metadata/verified/gtk.json."
        if doc and doc.get("brief"):
            e["summary"] = self.text(doc, md=doc["brief"], url=page)
        if description and description["md"]:
            e["description"] = description
        av = self.avail(doc)
        if av:
            e["availability"] = av
        if m.get("lifetime"):
            lt = m["lifetime"]
            prov = lt.get("provenance")
            note = prov if isinstance(prov, str) else "; ".join(f"{x['origin']}: {x['ref']}" for x in prov or [])
            e["lifetime"] = {"class": LIFE.get(lt["class"], "unknown"), "origin": "manual" if lt.get("origin") == "manual" else "derived", "note": note}
        elif m["kind"] == "object":
            top = "Gtk.Window" in m.get("bases", []) or name == "Window"
            if m.get("abstract") or not m["native"].get("get_type"):
                e["lifetime"] = {"class": "unknown", "origin": "derived", "note": "Abstract or not instantiable from JS."}
            elif top:
                e["lifetime"] = {"class": "toplevel", "origin": "derived", "note": "Toplevel window: kept alive by GTK's toplevel list until closed."}
            else:
                e["lifetime"] = {"class": "owned", "origin": "derived", "note": "GObject (floating reference sunk by the handle table); the parent widget also holds a reference while inserted."}
        e["properties"] = self.properties(m, d, page)
        e["events"] = self.events(m, d)
        e["methods"] = self.methods(m, d, False)
        e["constructors"] = self.methods(m, d, True)
        if m["kind"] in ("enum", "flags"):
            for mem in m.get("members", []):
                v = {"name": mem["name"], "nick": mem.get("nick", mem["name"]), "value": mem["value"], "native": mem.get("native", "")}
                dd = d.get("members", {}).get(mem["name"]) or d.get("members", {}).get(mem.get("nick", ""))
                t = self.text(dd)
                if t:
                    v["doc"] = t
                e["values"].append(v)
        ch = self.children(m) if m["kind"] in ("object", "interface") else None
        if ch:
            e["children"] = ch
        abi = []
        if e["properties"]:
            abi += ["set_prop", "get_prop"]
        if e["events"]:
            abi += ["listen"]
        if tag:
            abi += ["create", "destroy"]
        if ch and ch["policy"] not in ("none", "unknown"):
            abi += ["insert", "remove"]
        irlib.annotate_dangerous(e, stem, self.dangerous)  # before the example, which skips dangerous members
        e["abi_symbols"] = sorted(set(abi))
        if tag:
            e["screenshot"] = f"{TK}/{tag}"
            e["examples"] = [self.example(tag, m, e["properties"], e["events"], ch)]
        return e

    def build(self) -> dict:
        ents = [self.entity(s) for s in sorted(self.meta)]
        any_meta = next(iter(self.meta.values()))
        src = any_meta["source"]
        props = sum(len(e["properties"]) for e in ents)
        by_origin: dict[str, int] = {}
        for e in ents:
            for p in e["properties"]:
                by_origin[p["origin"]] = by_origin.get(p["origin"], 0) + 1
        kinds_by_type: dict[str, int] = {}
        for e in ents:
            kinds_by_type[e["kind"]] = kinds_by_type.get(e["kind"], 0) + 1
        docs_cov = {
            "kinds_with_description": sum(1 for e in ents if e.get("description")),
            "properties_with_doc": sum(1 for e in ents for p in e["properties"] if p.get("doc")),
            "events_with_doc": sum(1 for e in ents for p in e["events"] if p.get("doc")),
            "methods_with_doc": sum(1 for e in ents for p in e["methods"] if p.get("doc")),
        }
        ir = {
            "schema": irlib.IR_SCHEMA,
            "toolkit": {
                "id": TK,
                "name": "GTK 4",
                "version": src["gir"]["version"],
                "language": "C / GObject",
                "status": "working",
                "doc_license": LICENSE,
                "upstream_docs": UPSTREAM,
                "text_policy": "full",
                "sources": [
                    {"name": src["gir"]["file"], "version": src["gir"]["version"], "sha256": src["gir"]["sha256"]},
                    {"name": "PyGObject", "version": (src.get("runtime") or {}).get("pygobject", "")},
                ],
                "notices": [LICENSE["attribution"] + " Licensed under LGPL-2.1-or-later."],
            },
            "entities": ents,
            "extension_schemas": {TK: EXTENSION_SCHEMA},
            "coverage": {
                "kinds": len(ents),
                "kinds_by_type": dict(sorted(kinds_by_type.items())),
                "elements": sum(1 for e in ents if e["tags"]),
                "annotated_kinds": sum(1 for e in ents if e["status"]["tier"] == "annotated"),
                "tested_kinds": sum(1 for e in ents if e["status"].get("tested")),
                "properties": props,
                "events": sum(len(e["events"]) for e in ents),
                "methods": sum(len(e["methods"]) for e in ents),
                "annotated_properties": sum(1 for e in ents for p in e["properties"] if p.get("annotated")),
                "annotated_events": sum(1 for e in ents for p in e["events"] if p.get("annotated")),
                "properties_by_origin": dict(sorted(by_origin.items())),
                "docs": docs_cov,
                "serdes": {"status": "unknown", "supported_types": None, "total_types": None},
                "dangerous": irlib.dangerous_coverage(ents),
            },
        }
        return ir


def json_str(s: str) -> str:
    return '"' + s.replace('"', '\\"') + '"'


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default=str(ROOT / "docs/site/.generated/ir/gtk.json"))
    args = ap.parse_args()
    ir = Frontend().build()
    changed = irlib.finish(ir, Path(args.out))
    print(f"gtk frontend: {len(ir['entities'])} entities, {ir['coverage']['elements']} elements ({'written' if changed else 'unchanged'})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
