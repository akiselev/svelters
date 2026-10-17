#!/usr/bin/env python3
"""Qt docs frontend: metadata/qt + metadata/docs/qt + overrides -> svelters-docs-ir/v0.

Owns the Qt quirks: qdoc / metaobject facts, overloads and default arguments, Q_PROPERTY, signals,
slots and invokables, enums and QFlags, signal collapsing (clicked() vs clicked(bool)), QSS
sub-controls and pseudo-states, typesystem ownership facts, QObject-tree lifetimes, and the GFDL
text policy (default `brief`: short briefs plus deep links to doc.qt.io; see config.json).

    python3 tools/docs/qt/frontend.py [--out docs/site/.generated/ir/qt.json] [--text-policy brief|links|full]
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "tools/docs/ir"))
sys.path.insert(0, str(ROOT / "tools/metadata/common"))
import irlib  # noqa: E402
import svmeta  # noqa: E402

TK = "qt"
HERE = Path(__file__).resolve().parent
ID_PREFIX = "svelters-api:v1:type:qt:"
UPSTREAM = "https://doc.qt.io/qt-6/"
EVENT_ALIAS = {"clicked": "click", "changed": "change"}
LICENSE = {
    "spdx": "GFDL-1.3-only",
    "name": "GNU Free Documentation License v1.3",
    "url": "https://www.gnu.org/licenses/fdl-1.3.html",
    "attribution": "Documentation excerpts from Qt, (c) The Qt Company Ltd. and other contributors, licensed under the GNU Free Documentation License v1.3.",
    "redistribution": "brief_only",
}
EXTENSION_SCHEMA = {
    "title": "Qt specifics",
    "fields": [
        {"key": "header", "label": "Header", "format": "code", "scope": "entity"},
        {"key": "module", "label": "Qt module", "format": "code", "scope": "entity"},
        {"key": "qobject", "label": "QObject-derived", "format": "text", "scope": "entity"},
        {"key": "qss", "label": "Style sheet (QSS) selector support", "format": "markdown", "scope": "entity"},
        {"key": "ownership", "label": "Ownership facts (PySide typesystem)", "format": "list", "scope": "entity"},
        {"key": "signature", "label": "C++ signature", "format": "code", "scope": "method"},
        {"key": "access", "label": "Access", "format": "text", "scope": "method"},
        {"key": "meta", "label": "Meta kind", "format": "text", "scope": "method"},
        {"key": "signature", "label": "C++ signature", "format": "code", "scope": "event"},
        {"key": "metaobject_signatures", "label": "Meta-method signatures", "format": "list", "scope": "event"},
        {"key": "connect", "label": "Connected overload", "format": "code", "scope": "event"},
        {"key": "collapses", "label": "Collapses overloads", "format": "list", "scope": "event"},
        {"key": "native_type", "label": "C++ type", "format": "code", "scope": "property"},
        {"key": "meta_flags", "label": "Q_PROPERTY flags", "format": "list", "scope": "property"},
    ],
}
TOPLEVEL_BASES = {"QMainWindow", "QDialog", "QDockWidget"}


def nsig(v):
    return (v.get("signal") if isinstance(v, dict) else v) or None


def config() -> dict:
    return json.loads((HERE / "config.json").read_text())


class Frontend:
    def __init__(self, root: Path = ROOT, text_policy: str | None = None):
        self.root = root
        self.policy = text_policy or config()["text_policy"]
        self.meta: dict[str, dict] = {}
        self.docs: dict[str, dict] = {}
        for f in sorted((root / "metadata/qt").glob("*.json")):
            self.meta[f.stem] = svmeta.load_json(f)
            dp = root / "metadata/docs/qt" / f.name
            self.docs[f.stem] = svmeta.load_json(dp) if dp.exists() else {}
        self.names = {m["native"]["name"]: stem for stem, m in self.meta.items()}
        sys.path.insert(0, str(root / "tools/metadata"))
        import ledger  # noqa: E402

        self.verified = ledger.verified_facts("qt")
        self.facts = [e["fact"] for e in ledger.load("qt")["entries"]]
        self.overrides: dict[str, dict] = {}
        for p in sorted((root / "metadata/overrides/qt").glob("*.yaml")):
            self.overrides[p.stem] = {"path": p, "data": svmeta.load_yaml(p)}
        # enum name -> entity id (class scoped); a bare name is resolved through the owner chain, then uniquely.
        self.enum_ids: dict[tuple[str, str], str] = {}
        self.enum_by_name: dict[str, list[str]] = {}
        for stem, m in sorted(self.meta.items()):
            for en in m.get("enums", []):
                i = f"{TK}:{stem}.{en['name']}"
                self.enum_ids[(stem, en["name"])] = i
                self.enum_by_name.setdefault(en["name"], []).append(i)

    # ----- helpers -----
    def chain(self, stem: str) -> list[str]:
        return [stem] + [b for b in self.meta[stem].get("bases", []) if b in self.meta]

    def typeref(self, t: dict, owner: str) -> dict:
        ref = t.get("ref", "unknown")
        head, _, rest = ref.partition("<")
        inner = rest[:-1] if rest.endswith(">") else rest

        def name_of(n: str):
            if head == "object":
                return n, (f"{TK}:{n}" if n in self.names.values() or n in self.meta else None)
            for c in self.chain(owner):
                if (c, n) in self.enum_ids:
                    return n, self.enum_ids[(c, n)]
            cand = self.enum_by_name.get(n, [])
            return n, (cand[0] if len(cand) == 1 else None)

        ts, ent = irlib.neutral_to_ts(ref, name_of)
        out = {"display": ts, "neutral": ref}
        if ent:
            out["ref"] = ent
        if t.get("native"):
            out["native"] = t["native"]
        if t.get("nullable"):
            out["nullable"] = True
        return out

    def text(self, entry: dict | None, full_md: str | None = None) -> dict | None:
        if not entry or not (entry.get("brief") or full_md):
            return None
        url = entry.get("url")
        if self.policy == "full" and full_md:
            return irlib.text(full_md, LICENSE["spdx"], "qdoc-html", upstream_url=url)
        brief = entry.get("brief") or ""
        if not brief:
            return None
        if self.policy == "links":
            brief = entry.get("index_brief") or brief
        return irlib.text(brief, LICENSE["spdx"], "qdoc-html", truncated=True, upstream_url=url)

    def elements_for(self, stem: str) -> list[dict]:
        out = []
        for ov in self.overrides.values():
            for el in ov["data"].get("elements", []) or []:
                if (el.get("construct") or {}).get("class") == self.meta[stem]["native"]["name"]:
                    out.append(el)
        return out

    # ----- members -----
    def find_prop(self, stem: str, native: str) -> tuple[str, dict] | None:
        for c in self.chain(stem):
            for p in self.meta[c].get("properties", []):
                if p["name"] == native:
                    return c, p
        return None

    def properties(self, stem: str, d: dict, elements: list[dict]) -> list[dict]:
        m = self.meta[stem]
        out: dict[str, dict] = {}
        for p in m.get("properties", []):
            mo = p.get("metaobject") or {}
            doc = (d.get("properties") or {}).get(p["name"])
            rec = {
                "name": p["name"],
                "js_name": p["js_name"],
                "origin": p["origin"],
                "origins": sorted({p["origin"], "metaobject"} if mo else {p["origin"]}),
                "type": self.typeref(p["type"], stem),
                "default": irlib.display_value(p.get("default")),
                "range": {k: v for k, v in (p.get("range") or {}).items() if k in ("min", "max")} or None,
                "access": {"read": p["access"]["read"], "write": p["access"]["write"]},
                "notify": nsig(p.get("notify") or mo.get("notify")),
                "bindable": bool(p.get("notify") or mo.get("notify")),
                "role": p.get("role", "value"),
                "accessors": {k: v for k, v in (p.get("accessors") or {}).items() if v and k != "origin"},
                "annotated": False,
                "extensions": {TK: {"native_type": p["type"].get("native", ""), "meta_flags": sorted(k for k in ("constant", "final", "required", "resettable", "stored", "user", "designable") if mo.get(k))}},
            }
            t = self.text(doc)
            if t:
                rec["doc"] = t
            out[p["name"]] = rec
        # annotated element props: the JS name and (for enum props) the value vocabulary come from the override
        for el in elements:
            for ap in el.get("props", []) or []:
                native = ap.get("native") or ap.get("setter", "").removeprefix("set").lower() or ap["js"]
                found = self.find_prop(stem, native)
                rec = out.get(native)
                if rec is None:
                    if found:
                        base = found[1]
                        mo = base.get("metaobject") or {}
                        rec = {
                            "name": native, "js_name": base["js_name"], "origin": base["origin"],
                            "origins": [base["origin"]], "type": self.typeref(base["type"], stem),
                            "default": None, "range": None,
                            "access": {"read": base["access"]["read"], "write": base["access"]["write"]},
                            "notify": nsig(base.get("notify") or mo.get("notify")),
                            "bindable": bool(base.get("notify") or mo.get("notify")),
                            "role": "value", "accessors": {}, "annotated": False,
                            "extensions": {TK: {"native_type": base["type"].get("native", ""), "meta_flags": []}},
                        }
                        t = self.text((self.docs.get(found[0]) or {}).get("properties", {}).get(native))
                        if t:
                            rec["doc"] = t
                    else:
                        rec = {
                            "name": ap["js"], "js_name": ap["js"], "origin": "manual", "origins": ["manual"],
                            "type": {"display": "unknown"}, "default": None, "range": None,
                            "access": {"read": True, "write": True}, "notify": None, "bindable": False,
                            "role": "value", "accessors": {}, "annotated": False,
                            "extensions": {TK: {"native_type": "", "meta_flags": []}},
                        }
                    out[rec["name"]] = rec
                rec["js_name"] = ap["js"]
                rec["annotated"] = True
                if any(f.endswith(f":prop:{ap['js']}") and f in self.verified and (f":{stem}:" in f) for f in self.facts):
                    rec["verified"] = True
                rec["origins"] = sorted(set(rec["origins"]) | {"manual"})
                if ap.get("type") == "enum" and ap.get("values"):
                    rec["type"] = {"display": " | ".join(json.dumps(v) for v in ap["values"]), "neutral": "enum"}
                if ap.get("target") == "layout":
                    rec["accessors"] = {"get": "QBoxLayout::" + ap.get("getter", native), "set": "QBoxLayout::" + ap.get("setter", "set" + native.capitalize())} if ap.get("getter") else {"layout": native}
                    rec["extensions"][TK]["meta_flags"] = sorted(set(rec["extensions"][TK]["meta_flags"]) | {"applied to the layout"})
        return sorted(out.values(), key=lambda r: r["name"])

    def events(self, stem: str, d: dict, elements: list[dict]) -> list[dict]:
        m = self.meta[stem]
        out: dict[str, dict] = {}

        def rec_from(s: dict) -> dict:
            je = s["js_event"]
            doc = (d.get("members") or {}).get(s.get("signature", ""))
            params = [{"name": p["name"], "type": self.typeref(p["type"], stem), **({"default": p["default"]} if p.get("default") else {})} for p in s.get("params", [])]
            rec = {
                "name": s["name"], "js_event": je, "handler_prop": f"on{je}",
                "params": params, "returns": {"display": "void"},
                "handler_result": s.get("handler_result", "none"), "sync_required": bool(s.get("sync_required")),
                "phase": None, "flags": [], "origin": s["origin"],
                "origins": sorted({s["origin"]} | ({"metaobject"} if s.get("metaobject_signatures") else set())),
                "annotated": False, "native_id": s["id"],
                "extensions": {TK: {"signature": s.get("signature", ""), "metaobject_signatures": s.get("metaobject_signatures", [])}},
            }
            if je in EVENT_ALIAS:
                rec["handler_aliases"] = [f"on{EVENT_ALIAS[je]}"]
            t = self.text(doc)
            if t:
                rec["doc"] = t
            return rec

        for s in m.get("signals", []):
            out[s["name"]] = rec_from(s)
        for el in elements:
            for a in el.get("signals", []) or []:
                # Synthetic events (e.g. close-request) have no Qt signal behind them.
                name = a.get("native") or a["synthetic"]
                rec = out.get(name)
                if rec is None:
                    base = None
                    for c in self.chain(stem):
                        base = next((s for s in self.meta[c].get("signals", []) if s["name"] == name), None)
                        if base:
                            owner = c
                            break
                    if base:
                        rec = rec_from(base)
                        t = self.text(((self.docs.get(owner) or {}).get("members") or {}).get(base.get("signature", "")))
                        if t:
                            rec["doc"] = t
                    else:
                        rec = {"name": name, "js_event": a["js"], "handler_prop": f"on{a['js']}", "params": [],
                               "returns": {"display": "void"}, "origin": "manual", "origins": ["manual"], "annotated": False,
                               "extensions": {TK: {"signature": "", "metaobject_signatures": []}}}
                    out[name] = rec
                rec["js_event"] = a["js"]
                rec["handler_prop"] = f"on{a['js']}"
                if a["js"] in EVENT_ALIAS:
                    rec["handler_aliases"] = [f"on{EVENT_ALIAS[a['js']]}"]
                rec["annotated"] = True
                rec["origins"] = sorted(set(rec["origins"]) | {"manual"})
                if a.get("args") is not None and not rec["params"]:
                    rec["params"] = [{"name": f"arg{i}", "type": {"display": x if isinstance(x, str) else "unknown"}} for i, x in enumerate(a["args"])]
                ext = rec["extensions"][TK]
                if a.get("connect"):
                    ext["connect"] = a["connect"]
                if a.get("collapses"):
                    ext["collapses"] = a["collapses"]
                if a.get("synthetic"):
                    ext["synthetic"] = a["synthetic"]
        return sorted(out.values(), key=lambda r: r["name"])

    def methods(self, stem: str, d: dict, ctor: bool, elements: list[dict]) -> list[dict]:
        m = self.meta[stem]
        out = []
        ann = {mm["native"] for el in elements for mm in (el.get("methods") or [])}
        for x in m.get("constructors" if ctor else "methods", []):
            sig = x.get("signature") or x.get("native", "")
            params = [{"name": p["name"], "type": self.typeref(p["type"], stem), **({"default": p["default"]} if p.get("default") else {})} for p in x.get("params", [])]
            ret = x.get("returns") or {}
            rtype = ret if "ref" in ret else {"ref": ret.get("type", "void"), "native": ret.get("native")}
            rt = "void" if rtype["ref"] in ("void", "self") else self.typeref(rtype, stem)["display"]
            rec = {
                "name": x.get("name") or stem, "origin": x["origin"], "signature": sig, "params": params,
                "returns": {"display": rt}, "kind": "constructor" if ctor else {"slot": "slot", "signal": "method"}.get(x.get("meta"), "method"),
                "static": bool(x.get("static")), "virtual": bool(x.get("virtual")), "access": x.get("access", "public"),
                "annotated": (x.get("name") in ann), "native_id": x["id"],
                "extensions": {TK: {"signature": sig, "access": x.get("access", "public"), "meta": x.get("meta", "constructor" if ctor else "plain")}},
            }
            doc = (d.get("members") or {}).get(sig)
            t = self.text(doc)
            if t:
                rec["doc"] = t
            if x.get("deprecated"):
                rec["availability"] = {"deprecated": True}
            out.append(rec)
        return sorted(out, key=lambda r: (r["name"], r["signature"]))

    def children(self, stem: str, elements: list[dict]) -> dict | None:
        for el in elements:
            ch = el.get("children")
            if not ch:
                continue
            out = {"policy": ch.get("policy", "unknown"), "default_slot": ch.get("default_slot"), "slots": []}
            for s in ch.get("slots", []):
                ops, own = {}, {}
                for op, v in sorted((s.get("ops") or {}).items()):
                    if isinstance(v, dict) and v.get("native"):
                        ops[op] = v["native"]
                        eff = v.get("effects")
                        if eff:
                            own[op] = ", ".join(f"{k} {x}" for k, x in sorted(eff.items()))
                out["slots"].append({
                    "name": s["name"], "cardinality": s.get("cardinality", "many"), "accepts": s.get("accepts", []),
                    "default": s["name"] == ch.get("default_slot"), "order": s.get("order"), "ops": ops, "ownership": own,
                    "on_remove": s.get("on_remove"), "attrs": [], "annotated": True,
                    "provenance": [f"{p.get('origin')}: {p.get('ref')}" for p in (s.get("provenance") or []) if isinstance(p, dict)],
                })
            return out
        m = self.meta[stem].get("children")
        if m and m.get("slots"):
            return {"policy": m.get("policy", "unknown"), "default_slot": m.get("default_slot"), "slots": [
                {"name": s["name"], "cardinality": s.get("cardinality", "many"), "accepts": s.get("accepts", []), "ops": {}, "ownership": {}} for s in m["slots"]]}
        return None

    def example(self, tag: str, props: list[dict], events: list[dict], children: dict | None) -> dict:
        ann_p = [p for p in props if p["annotated"] and p["access"]["write"]]
        attrs = []
        for p in ann_p[:3]:
            n = p["type"].get("neutral", "")
            if n == "string":
                attrs.append(f'{p["js_name"]}="Hello"')
            elif n == "enum" and "|" in p["type"]["display"]:
                attrs.append(f'{p["js_name"]}={p["type"]["display"].split(" | ")[-1]}')
            elif n in ("i32", "u32", "f64"):
                attrs.append(f'{p["js_name"]}={{8}}')
        ev = [e for e in events if e["annotated"]][:1]
        head = "<script>\n  let count = $state(0);\n</script>\n\n" if ev else ""
        if ev:
            attrs.append(f'{(ev[0].get("handler_aliases") or [ev[0]["handler_prop"]])[0]}={{() => count++}}')
        a = " ".join(attrs)
        inner = "\n  <label>Child content</label>\n" if children and children["policy"] not in ("none", "unknown") else ""
        body = f"<{tag}{(' ' + a) if a else ''}>{inner}</{tag}>" if inner else f"<{tag}{(' ' + a) if a else ''} />"
        return {"title": f"Using <{tag}>", "lang": "svelte", "code": head + body + "\n", "generated": True, "tested": False}

    # ----- entities -----
    def entity(self, stem: str) -> dict:
        m, d = self.meta[stem], self.docs.get(stem) or {}
        cname = m["native"]["name"]
        elements = self.elements_for(stem)
        tags = sorted({el["tag"] for el in elements})
        annotated = bool(elements)
        doc = d.get("doc")
        url = (doc or {}).get("url") or UPSTREAM + stem.lower() + ".html"
        ext: dict = {"header": m["native"].get("header", ""), "module": m["native"].get("module", ""), "qobject": "yes" if m.get("qobject") else "no"}
        if d.get("qss"):
            q = d["qss"]
            bits = []
            if q.get("box_model"):
                bits.append("Supports the box model.")
            if q.get("sub_controls"):
                bits.append("Sub-controls: " + ", ".join(f"`{s}`" for s in q["sub_controls"]) + ".")
            if q.get("pseudo_states"):
                bits.append("Pseudo-states: " + ", ".join(f"`{s}`" for s in q["pseudo_states"]) + ".")
            bits.append(f"[Qt Style Sheets reference]({q['url']})")
            ext["qss"] = " ".join(bits)
        facts = []
        for o in m.get("ownership", []):
            if o.get("fact") == "parent":
                facts.append(f"`{o['method']}`: argument {o.get('arg')} gets its parent from `{o.get('parent_index')}`")
            elif o.get("fact") == "removed":
                facts.append(f"`{o['method']}` is removed in the PySide typesystem")
            else:
                facts.append(f"`{o.get('method', '?')}`: {o.get('fact')}")
        if facts:
            ext["ownership"] = sorted(set(facts))
        e: dict = {
            "id": f"{TK}:{stem}", "kind": "class", "name": cname, "qualified": cname, "native_name": cname,
            "abstract": bool(m.get("abstract")), "tags": tags,
            "status": {"tier": "annotated" if annotated else "ingested",
                       "tested": any(f.startswith(f"svelters-api:v1:type:qt:QtWidgets:{stem}:") and f in self.verified for f in self.facts),
                       "verified_facts": [sum(1 for f in set(self.facts) if f.startswith(f"svelters-api:v1:type:qt:QtWidgets:{stem}:") and f in self.verified),
                                          sum(1 for f in set(self.facts) if f.startswith(f"svelters-api:v1:type:qt:QtWidgets:{stem}:"))],
                       "metadata_file": f"metadata/qt/{stem}.json",
                       "note": "" if annotated else "Raw: ingested from qdoc and the QMetaObject only. This class has no element tag and no generated backend code yet, so it cannot be used from Svelte until it is annotated."},
            "upstream": {"url": url, "label": f"Qt 6 reference: {cname}"},
            "bases": [f"{TK}:{b}" for b in m.get("bases", [])],
            "interfaces": [], "properties": [], "events": [], "methods": [], "constructors": [], "values": [],
            "serdes": {"status": "unknown"}, "extensions": {TK: ext},
        }
        if annotated:
            files = sorted({f"metadata/overrides/qt/{k}.yaml" for k, ov in self.overrides.items() for el in ov["data"].get("elements", []) or [] if (el.get("construct") or {}).get("class") == cname})
            e["status"]["annotation_files"] = files
            e["status"]["test_note"] = "Annotated facts have generated C++ verification tests (svl_qt_verify.cpp from gen_cpp.py); results are in metadata/verified/qt.json."
        s = self.text(doc)
        if s:
            e["summary"] = s
        if self.policy == "full" and doc and doc.get("md"):
            e["description"] = irlib.text(doc["md"], LICENSE["spdx"], "qdoc-html", upstream_url=url)
        if m.get("availability", {}).get("deprecated"):
            e["availability"] = {"deprecated": True}
        tl = bool(set(m.get("bases", [])) & TOPLEVEL_BASES) or cname in TOPLEVEL_BASES
        if m.get("lifetime"):
            lt = m["lifetime"]
            e["lifetime"] = {"class": {"owned": "owned", "toplevel": "toplevel", "value": "value"}.get(lt["class"], "unknown"),
                             "origin": "manual" if lt.get("origin") == "manual" else "derived", "note": str(lt.get("provenance", ""))}
        elif m.get("qobject"):
            if tl:
                e["lifetime"] = {"class": "toplevel", "origin": "derived", "note": "Top-level window; deleted with deleteLater() by destroy."}
            else:
                e["lifetime"] = {"class": "owned", "origin": "derived", "note": "QObject tree: the parent deletes it while inserted; once removed it is unparented and destroy deletes it."}
        else:
            e["lifetime"] = {"class": "value" if not m.get("abstract") else "unknown", "origin": "derived", "note": "Not a QObject."}
        e["properties"] = self.properties(stem, d, elements)
        e["events"] = self.events(stem, d, elements)
        e["methods"] = self.methods(stem, d, False, elements)
        e["constructors"] = self.methods(stem, d, True, elements)
        ch = self.children(stem, elements)
        if ch:
            e["children"] = ch
        abi = []
        if e["properties"]:
            abi += ["set_prop", "get_prop"]
        if e["events"]:
            abi.append("listen")
        if tags:
            abi += ["create", "destroy"]
        if ch:
            abi += ["insert", "remove"]
        e["abi_symbols"] = sorted(set(abi))
        if tags:
            e["screenshot"] = f"{TK}/{tags[0]}"
            e["examples"] = [self.example(tags[0], e["properties"], e["events"], ch)]
        return e

    def enum_entities(self, stem: str) -> list[dict]:
        m = self.meta[stem]
        out = []
        for en in m.get("enums", []):
            mo = en.get("metaobject") or {}
            kind = "flags" if mo.get("flag") else "enum"
            out.append({
                "id": f"{TK}:{stem}.{en['name']}", "kind": kind, "name": en["name"],
                "qualified": f"{stem}::{en['name']}", "native_name": f"{stem}::{en['name']}", "abstract": False, "tags": [],
                "status": {"tier": "ingested", "tested": False, "metadata_file": f"metadata/qt/{stem}.json"},
                "upstream": {"url": UPSTREAM + stem.lower() + ".html", "label": f"Qt 6 reference: {stem}"},
                "bases": [], "interfaces": [], "properties": [], "events": [], "methods": [], "constructors": [],
                "values": [{"name": v["name"], "value": int(v["value"]) if str(v["value"]).lstrip("-").isdigit() else str(v["value"])} for v in en.get("members", [])],
                "serdes": {"status": "unknown"}, "extensions": {TK: {"module": m["native"].get("module", ""), "header": m["native"].get("header", "")}},
                "see_also": [f"{TK}:{stem}"],
            })
        return out

    def build(self) -> dict:
        classes = [self.entity(s) for s in sorted(self.meta)]
        enums = [x for s in sorted(self.meta) for x in self.enum_entities(s)]
        ents = classes + enums
        src = next(iter(self.meta.values()))["source"]
        by_origin: dict[str, int] = {}
        for e in classes:
            for p in e["properties"]:
                by_origin[p["origin"]] = by_origin.get(p["origin"], 0) + 1
        kinds_by_type = {}
        for e in ents:
            kinds_by_type[e["kind"]] = kinds_by_type.get(e["kind"], 0) + 1
        docs_cov = {
            "kinds_with_summary": sum(1 for e in ents if e.get("summary")),
            "properties_with_doc": sum(1 for e in ents for p in e["properties"] if p.get("doc")),
            "methods_with_doc": sum(1 for e in ents for p in e["methods"] if p.get("doc")),
            "events_with_doc": sum(1 for e in ents for p in e["events"] if p.get("doc")),
        }
        qd = src["qdoc"]
        return {
            "schema": irlib.IR_SCHEMA,
            "toolkit": {
                "id": TK, "name": "Qt 6 (Widgets)", "version": qd["version"], "language": "C++", "status": "working",
                "doc_license": LICENSE, "upstream_docs": UPSTREAM, "text_policy": self.policy,
                "sources": [{"name": f["file"], "version": qd["version"], "sha256": f["sha256"]} for _, f in sorted(qd["files"].items())]
                + [{"name": "PySide6 QMetaObject", "version": (src.get("metaobject") or {}).get("pyside6", "")},
                   {"name": src["typesystem"]["file"], "sha256": src["typesystem"]["sha256"]}],
                "notices": [LICENSE["attribution"] + " Only short briefs are reproduced here (text policy: " + self.policy + "); see doc.qt.io for the full documentation."],
            },
            "entities": ents,
            "extension_schemas": {TK: EXTENSION_SCHEMA},
            "coverage": {
                "kinds": len(ents), "kinds_by_type": dict(sorted(kinds_by_type.items())),
                "elements": sum(1 for e in ents if e["tags"]),
                "annotated_kinds": sum(1 for e in ents if e["status"]["tier"] == "annotated"),
                "tested_kinds": sum(1 for e in ents if e["status"].get("tested")),
                "properties": sum(len(e["properties"]) for e in ents), "events": sum(len(e["events"]) for e in ents),
                "methods": sum(len(e["methods"]) for e in ents),
                "annotated_properties": sum(1 for e in ents for p in e["properties"] if p.get("annotated")),
                "annotated_events": sum(1 for e in ents for p in e["events"] if p.get("annotated")),
                "properties_by_origin": dict(sorted(by_origin.items())), "docs": docs_cov,
                "serdes": {"status": "unknown", "supported_types": None, "total_types": None},
            },
        }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default=str(ROOT / "docs/site/.generated/ir/qt.json"))
    ap.add_argument("--text-policy", choices=("links", "brief", "full"))
    args = ap.parse_args()
    ir = Frontend(text_policy=args.text_policy).build()
    changed = irlib.finish(ir, Path(args.out))
    print(f"qt frontend: {len(ir['entities'])} entities, {ir['coverage']['elements']} elements, policy {ir['toolkit']['text_policy']} ({'written' if changed else 'unchanged'})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
