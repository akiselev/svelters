"""The generated C++ of the Qt non-GUI types (plans/research/NON-GUI-TYPES.md section 2.3, N2).

Called by gen_cpp.py. Input: the classified facts (`metadata/qt/types/*.json`, types_qt_classify.py). Output:
two backend sections that call only the hand-written core (templates/types_core.hpp.in), the converters
and Qt's own API (the boundary rule of section 2.4, checked by test_types_cpp.py):

  TYPES_DECLS  (before the element enums) TypeKindOf / ValueCast / Conv specializations, EnumInfo for the
               enums the types mention, Tracked<T> item subclasses
  TYPES        kinds, member tables, one thunk per (class, JS name) with the runtime overload dispatch of
               decision Q4, statics and constructors, descriptors, the metatype converter table
"""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import types_qt_classify as tqc  # noqa: E402

KIND_BASE = 0x20000
MEMBER_BASE = 0x200000
CAT = {"object": "svl::C_OBJECT", "model": "svl::C_MODEL", "value": "svl::C_VALUE", "unique": "svl::C_UNIQUE",
       "item": "svl::C_ITEM", "statics": "svl::C_STATICS"}


def cid(s: str) -> str:
    return re.sub(r"[^A-Za-z0-9]", "_", s)


def ins_of(o: dict) -> list[dict]:
    """The parameters JS passes (in and in-out), in order."""
    return [p for p in o["params"] if p["type"].get("out") is not True]


def required(o: dict) -> int:
    return sum(1 for p in ins_of(o) if not p.get("default"))


class TypesGen:
    def __init__(self, facts: dict, element_types: dict[str, str]):
        self.types = facts["types"]
        self.enums = facts["enums"]
        self.names = sorted(self.types)
        self.kind = {n: KIND_BASE + i for i, n in enumerate(self.names)}
        self.element_types = element_types  # element tag -> bound type name
        # Member ids: one per (class, section, JS name), and one per prop and signal.
        self.members: list[dict] = []
        for n in self.names:
            t = self.types[n]
            for p in t["properties"]:
                self._add(n, "prop", p["name"], p)
            for s in t["signals"]:
                if not any(m["what"] == "signal" and m["cls"] == n and m["js"] == s["name"] for m in self.members):
                    self._add(n, "signal", s["name"], s)
            for js, ovs in sorted(t["methods"].items()):
                self._add(n, "method", js, ovs)
            for js, ovs in sorted(t["statics"].items()):
                self._add(n, "static", js, ovs)
            if t["constructors"]:
                self._add(n, "ctor", "new", t["constructors"])

    def _add(self, cls: str, what: str, js: str, data) -> None:
        self.members.append({"id": MEMBER_BASE + len(self.members), "cls": cls, "what": what, "js": js, "data": data})

    # ---- what the element sections must know ---------------------------------------------------------
    def value_classes(self) -> list[str]:
        return [n for n in self.names if self.types[n]["category"] == "value"]

    def ptr_classes(self) -> list[str]:
        return [n for n in self.names if self.types[n]["category"] in ("unique", "item")]

    def enum_tables(self) -> dict[str, list[dict]]:
        return {full: e["members"] for full, e in self.enums.items()}

    def headers(self) -> set[str]:
        out = set()
        for n in self.names:
            t = self.types[n]
            if t.get("header"):
                out.add(f"{t['module']}/{t['header']}")
        return out

    # ---- TYPES_DECLS ------------------------------------------------------------------------------------
    def decls(self, boxed_done: set[str], enums_done: set[str], enum_names) -> str:
        out = ["// ---- generated: non-GUI type declarations (gen_types_cpp.py) ----"]
        for n in self.names:
            out.append(f"constexpr svl_id TK_{cid(n)} = 0x{self.kind[n]:x};")
        for n in self.value_classes():
            if n != "QModelIndex":  # held as a QPersistentModelIndex: its kind comes from the core
                out.append(f"template <> struct TypeKindOf<{n}> {{ static constexpr svl_id id = TK_{cid(n)}; }};")
        # A derived value class where its base is expected.
        for n in self.value_classes():
            derived = [d for d in self.value_classes() if n in self.types[d]["bases"] and n != "QModelIndex"]
            if not derived:
                continue
            out.append(f"template <> struct ValueCast<{n}> {{\n  static const {n} *ptr(const QVariant &v) {{")
            for d in derived:
                out.append(f"    if (v.metaType() == QMetaType::fromType<{d}>()) return static_cast<const {n} *>(static_cast<const {d} *>(v.constData()));")
            out.append("    return nullptr;\n  }\n};")
        for full in sorted(self.enums):
            if full in enums_done:
                continue
            mem = ", ".join(f'{{"{js}", static_cast<int64_t>({cpp})}}' for js, cpp in sorted(enum_names(self.enums[full]["members"]).items()))
            out.append(f"template <>\nstruct EnumInfo<{full}> {{\n  static const char *name() {{ return \"{full}\"; }}\n"
                       f"  static const std::vector<std::pair<std::string, int64_t>> &members() {{\n"
                       f"    static const std::vector<std::pair<std::string, int64_t>> v = {{{mem}}};\n    return v;\n  }}\n}};")
        for n in self.value_classes():
            if n in boxed_done or n == "QModelIndex":
                continue
            out.append(f"template <> struct Conv<{n}> : BoxedConv<{n}> {{}};")
        for n in self.ptr_classes():
            out.append(f"template <> struct Conv<{n} *> : svl::PtrConv<{n}> {{}};")
            out.append(f"template <> inline svl_id svl::PtrConv<{n}>::kind() {{ return TK_{cid(n)}; }}")
        for n in self.ptr_classes():
            if self.types[n]["category"] != "item":
                continue
            # Made by JS (or cloned by a model whose prototype is one): the core's subclass reports native death.
            clone = any(o["cpp"] == "clone" and not ins_of(o) for o in self.types[n]["methods"].get("clone", []))
            out.append(f"using Tracked_{cid(n)} = svl::{'TrackedClone' if clone else 'Tracked'}<{n}>;")
        out.append("// ---- end generated: non-GUI type declarations ----")
        return "\n".join(out)

    # ---- TYPES ------------------------------------------------------------------------------------------
    def body(self) -> str:
        out = ["// ---- generated: non-GUI types (gen_types_cpp.py) ----", "namespace svl {"]
        out += self.emit_kinds()
        out += self.emit_member_tables()
        out += self.emit_ptr_helpers()
        out += self.emit_thunks()
        out += self.emit_descs()
        out += self.emit_metaconv()
        out.append("}  // namespace svl")
        out.append("// ---- end generated: non-GUI types ----")
        return "\n".join(out)

    def emit_kinds(self) -> list[str]:
        out = ["const TypeRow kTypeRows[] = {"]
        for n in self.names:
            t = self.types[n]
            par = f"TK_{cid(t['parent'])}" if t["parent"] else "0"
            mo = f"&{n}::staticMetaObject" if t["qobject"] else "nullptr"
            out.append(f'  {{"{n}", TK_{cid(n)}, {par}, {CAT[t["category"]]}, {mo}}},')
        out.append("};")
        out.append(f"constexpr size_t kTypeCount = {len(self.names)};")
        out.append("const TypeRow *gen_type(svl_id kind) {")
        out.append(f"  if (kind < 0x{KIND_BASE:x} || kind >= 0x{KIND_BASE:x} + kTypeCount) return nullptr;")
        out.append(f"  return &kTypeRows[kind - 0x{KIND_BASE:x}];")
        out.append("}")
        out.append("svl_id gen_type_by_name(std::string_view name) {")
        out.append("  // kTypeRows is sorted by name")
        out.append("  size_t lo = 0, hi = kTypeCount;")
        out.append("  while (lo < hi) {")
        out.append("    const size_t mid = (lo + hi) / 2;")
        out.append("    const int c = name.compare(kTypeRows[mid].name);")
        out.append("    if (c == 0) return kTypeRows[mid].kind;")
        out.append("    if (c < 0) hi = mid; else lo = mid + 1;")
        out.append("  }")
        out.append("  return 0;")
        out.append("}")
        out.append("svl_id gen_element_type(svl_id element_kind) {")
        out.append("  switch (element_kind) {")
        for tag, ty in sorted(self.element_types.items()):
            out.append(f"    case K_{cid(tag)}: return TK_{cid(ty)};")
        out.append("    default: return 0;\n  }\n}")
        # QObject default construction (`construct`): models get a tracked item prototype.
        out.append("QObject *gen_default_new(svl_id kind) {")
        out.append("  switch (kind) {")
        for n in self.names:
            t = self.types[n]
            if not t["constructible"]:
                continue
            out.append(f"    case TK_{cid(n)}: {{\n      auto *o = new {n}();")
            out += self.after_new(n, "o")
            out.append("      return o;\n    }")
        out.append("    default: return nullptr;\n  }\n}")
        return out

    def after_new(self, n: str, var: str) -> list[str]:
        """A model that clones items from a prototype gets a tracked one, so every item it makes is watched."""
        out = []
        for o in self.types[n]["methods"].get("setItemPrototype", []):
            ps = o["params"]
            if len(ps) == 1 and ps[0]["type"]["k"] == "item":
                it = ps[0]["type"]["t"]
                out.append(f"      {var}->setItemPrototype(svl::tracked_prototype<Tracked_{cid(it)}>());")
        return out

    def emit_member_tables(self) -> list[str]:
        what = {"prop": "SVL_PROP", "signal": "SVL_SIGNAL", "method": "SVL_METHOD", "static": "SVL_METHOD", "ctor": "SVL_METHOD"}
        rows = sorted(((self.kind[m["cls"]], what[m["what"]], m["js"], m["id"]) for m in self.members),
                      key=lambda r: (r[0], r[1], r[2]))
        # Statics and instance methods may share a JS name (resolved to the instance one; statics by `call_static`).
        seen, uniq = set(), []
        for r in rows:
            if (r[0], r[1], r[2]) in seen:
                continue
            seen.add((r[0], r[1], r[2]))
            uniq.append(r)
        out = ["struct MemberRow { svl_id kind; uint32_t what; std::string_view name; svl_id id; };",
               "const MemberRow kMemberRows[] = {"]
        out += [f'  {{0x{k:x}, {w}, "{n}", 0x{i:x}}},' for k, w, n, i in uniq]
        out.append("};")
        statics = sorted(((self.kind[m["cls"]], m["js"], m["id"]) for m in self.members if m["what"] in ("static", "ctor")))
        out.append("const MemberRow kStaticRows[] = {")
        out += [f'  {{0x{k:x}, SVL_METHOD, "{n}", 0x{i:x}}},' for k, n, i in statics]
        out.append("};")
        out.append("""bool row_less(const MemberRow &r, svl_id kind, uint32_t what, std::string_view name) {
  if (r.kind != kind) return r.kind < kind;
  if (r.what != what) return r.what < what;
  return r.name < name;
}
svl_id find_row(const MemberRow *b, const MemberRow *e, svl_id kind, uint32_t what, std::string_view name) {
  const MemberRow *it = std::lower_bound(b, e, 0, [&](const MemberRow &r, int) { return row_less(r, kind, what, name); });
  return it != e && it->kind == kind && it->what == what && it->name == name ? it->id : 0;
}
svl_id gen_member(uint32_t what, svl_id kind, std::string_view name) {
  return find_row(std::begin(kMemberRows), std::end(kMemberRows), kind, what, name);
}
// call_static resolves statics first: an instance method of the same name is not a static.
svl_id gen_static_member(svl_id kind, std::string_view name) {
  return find_row(std::begin(kStaticRows), std::end(kStaticRows), kind, SVL_METHOD, name);
}""")
        out.append("const char *gen_member_native(svl_id id) {")
        out.append("  switch (id) {")
        for m in self.members:
            if m["what"] in ("prop", "signal"):
                out.append(f'    case 0x{m["id"]:x}: return "{m["js"]}";')
        out.append("    default: return nullptr;\n  }\n}")
        return out

    def emit_ptr_helpers(self) -> list[str]:
        out = ["void *gen_ptr_as(svl_id have, svl_id want, void *p) {", "  if (have == want) return p;", "  switch (have) {"]
        for n in self.ptr_classes():
            bases = [b for b in self.types[n]["bases"] if self.types.get(b, {}).get("category") in ("unique", "item")]
            if not bases:
                continue
            out.append(f"    case TK_{cid(n)}:")
            for b in bases:
                out.append(f"      if (want == TK_{cid(b)}) return static_cast<{b} *>(static_cast<{n} *>(p));")
            out.append("      return nullptr;")
        out.append("    default: return nullptr;\n  }\n}")
        return out

    # ---- thunks -----------------------------------------------------------------------------------------
    def receiver(self, n: str, js: str) -> list[str]:
        cat = self.types[n]["category"]
        what = f'"{n}.{js}"'
        if cat in ("object", "model"):
            return [f"  auto *self = svl::object<{n}>(r);", f'  if (!self) return type_fail({what}, "the receiver is not a {n}");']
        if cat == "value":
            return [f"  ValueRef<{n}> self(r.h);", f'  if (!self) return type_fail({what}, "the receiver is not a {n}");']
        return [f"  auto *self = static_cast<{n} *>(gen_ptr_as(r.kind, TK_{cid(n)}, r.ptr));",
                f'  if (!self) return type_fail({what}, "the receiver is not a {n}");']

    @staticmethod
    def local_type(t: dict) -> str:
        return t["cpp"]

    def call_expr(self, n: str, m: dict, o: dict, k: int) -> str:
        """The C++ call with the first `k` JS arguments (C++ supplies the defaults of the rest)."""
        args = []
        ji = 0
        for i, p in enumerate(o["params"]):
            t = p["type"]
            if t.get("out") is True:
                args.append(f"&a{i}")
                continue
            if ji >= k:
                break
            ji += 1
            if t.get("deref"):
                args.append(f"*a{i}")
            else:
                args.append(f"a{i}{t.get('pass', '')}")
        a = ", ".join(args)
        cat = self.types[n]["category"]
        if m["what"] == "ctor":
            if cat in ("object", "model"):
                return f"new {n}({a})"
            if cat == "value":
                return f"{n}({a})"
            if cat == "item":
                return f"item_new<{n}, Tracked_{cid(n)}>(TK_{cid(n)}{', ' if a else ''}{a})"
            return f"new {n}({a})"
        if m["what"] == "static":
            return f"{n}::{o['cpp']}({a})"
        return f"self->{o['cpp']}({a})"

    @staticmethod
    def ret_cast(t: dict, expr: str) -> str:
        # Object and item results may be `const T *`: the handle does not carry constness.
        return f"const_cast<{t['cpp']}>({expr})" if t["k"] in ("object", "item") else f"static_cast<{t['cpp']}>({expr})"

    def emit_result(self, n: str, m: dict, o: dict, call: str, ind: str) -> list[str]:
        cat = self.types[n]["category"]
        outs = [(i, p) for i, p in enumerate(o["params"]) if p["type"].get("out")]
        ret = o["ret"]
        lines = []
        if m["what"] == "ctor":
            if cat in ("object", "model"):
                lines.append(f"{ind}auto *o = {call};")
                lines += [ln.replace("      ", ind) for ln in self.after_new(n, "o")]
                lines.append(f"{ind}*out = mkv(SVL_HANDLE);\n{ind}out->u.h = handle_for(o, TK_{cid(n)});")
            elif cat == "value":
                lines.append(f"{ind}Conv<{n}>::out({call}, out);")
            elif cat == "item":
                lines.append(f"{ind}*out = mkv(SVL_HANDLE);\n{ind}out->u.h = {call};")
            else:
                lines.append(f"{ind}*out = mkv(SVL_HANDLE);\n{ind}out->u.h = unique({call}, TK_{cid(n)});")
            return lines
        void = ret["k"] == "void"
        if ret["k"] == "async":
            lines.append(f"{ind}async_from_future({call}, cb);")
            return lines
        if void:
            lines.append(f"{ind}{call};")
        else:
            lines.append(f"{ind}auto res = {call};")
        own = o.get("ownership", [])
        if outs:
            lines.append(f"{ind}svl_value *vec = out_list(out, {len(outs) + 1});")
            if not void:
                lines.append(f"{ind}Conv<{ret['cpp']}>::out({self.ret_cast(ret, 'res')}, &vec[0]);")
            for j, (i, p) in enumerate(outs):
                lines.append(f"{ind}Conv<{p['type']['cpp']}>::out(a{i}, &vec[{j + 1}]);")
        elif not void:
            lines.append(f"{ind}Conv<{ret['cpp']}>::out({self.ret_cast(ret, 'res')}, out);")
        # Ownership facts (PySide typesystem): arguments moved into the receiver, results given back.
        ins = ins_of(o)
        for f in own:
            if f.get("fact") == "parent" and f.get("index") == "this" and f.get("action") == "add" and f["arg"].isdigit():
                pi = int(f["arg"]) - 1
                if 0 <= pi < len(o["params"]) and o["params"][pi] in ins:
                    j = ins.index(o["params"][pi])
                    lines.append(f"{ind}if (n > {j}) moved_into_container(&a[{j}]);")
            if f["arg"] == "return" and ((f.get("fact") == "parent" and f.get("action") == "remove") or
                                         (f.get("fact") == "ownership" and f.get("owner") == "target")):
                lines.append(f"{ind}given_back(out);")
        return lines

    def emit_group(self, m: dict) -> list[str]:
        n, js = m["cls"], m["js"]
        ovs = m["data"]
        fn = f"t_{m['id']:x}"
        out = [f"// {n}.{js} ({m['what']}, {len(ovs)} overload(s))",
               f"int {fn}(Recv &r, const svl_value *a, size_t n, svl_value *out) {{", "  (void)r; (void)a; (void)n; (void)out;"]
        if m["what"] == "method":
            out += self.receiver(n, js)
        out.append("  std::string er;")
        multi = len(ovs) > 1
        for o in ovs:
            ins = ins_of(o)
            req, mx = required(o), len(ins)
            is_async = o["ret"]["k"] == "async"
            extra = 1 if is_async else 0
            cond = [f"n >= {req + extra}", f"n <= {mx + extra}"] if not (req == mx) else [f"n == {mx + extra}"]
            if multi:
                for j, p in enumerate(ins):
                    acc = f"Accept<{self.local_type(p['type'])}>::ok(&a[{j}])"
                    cond.append(acc if j < req else f"(n <= {j} || {acc})")
            out.append(f"  if ({' && '.join(cond)}) {{  // {o['key']}")
            j = 0
            for i, p in enumerate(o["params"]):
                t = p["type"]
                lt = self.local_type(t)
                if t.get("deref"):
                    lt = f"{t['t']} *"
                out.append(f"    {lt} a{i}{{}};")
                if t.get("out") is True:
                    continue
                label = f'"{n}.{js}({p["name"]})"'
                if j < req:
                    out.append(f"    if (!conv_in(&a[{j}], a{i}, {label}, er)) return fail(er);")
                else:
                    out.append(f"    if (n > {j} && !conv_in(&a[{j}], a{i}, {label}, er)) return fail(er);")
                if t.get("deref"):
                    out.append(f"    if (n > {j} && !a{i}) return type_fail({label}, \"null\");")
                j += 1
            if is_async:
                out.append(f"    if (a[n - 1].tag != SVL_CALLBACK) return type_fail(\"{n}.{js}\", \"expected the completion callback\");")
                out.append("    const uint64_t cb = a[n - 1].u.u;")
            if req == mx:
                out += self.emit_result(n, m, o, self.call_expr(n, m, o, mx), "    ")
                out.append("    return 0;")
            else:
                out.append(f"    switch (n - {extra}) {{")
                for k in range(req, mx + 1):
                    out.append(f"      case {k}: {{")
                    out += self.emit_result(n, m, o, self.call_expr(n, m, o, k), "        ")
                    out.append("        return 0;\n      }")
                out.append("      default: break;\n    }")
            out.append("  }")
        if multi:
            sigs = " | ".join(o["key"].split(".", 1)[1] for o in ovs).replace('"', "'")
            out.append(f'  return type_fail("{n}.{js}", "no overload takes these arguments (have: {sigs})");')
        else:
            out.append(f'  return type_fail("{n}.{js}", "expected " + std::string("{required(ovs[0])}") + (std::string("{len(ins_of(ovs[0]))}") != "{required(ovs[0])}" ? " to {len(ins_of(ovs[0]))}" : "") + " argument(s), got " + std::to_string(n));')
        out.append("}")
        return out

    def emit_thunks(self) -> list[str]:
        out = []
        for m in self.members:
            if m["what"] in ("method", "static", "ctor"):
                out += self.emit_group(m)
        out.append("int gen_method(svl_id id, Recv &r, const svl_value *a, size_t n, svl_value *out) {")
        out.append("  switch (id) {")
        for m in self.members:
            if m["what"] == "method":
                out.append(f"    case 0x{m['id']:x}: return t_{m['id']:x}(r, a, n, out);")
        out.append("    default: return fail(\"call: unknown method id \" + std::to_string(id) + \" for this kind\");\n  }\n}")
        out.append("int gen_static(svl_id id, const svl_value *a, size_t n, svl_value *out) {")
        out.append("  Recv r;")
        out.append("  switch (id) {")
        for m in self.members:
            if m["what"] in ("static", "ctor"):
                out.append(f"    case 0x{m['id']:x}: return t_{m['id']:x}(r, a, n, out);")
        out.append("    default: return fail(\"call_static: not a static method id \" + std::to_string(id));\n  }\n}")
        return out

    # ---- descriptors --------------------------------------------------------------------------------
    def slot_expr(self, t: dict, name: str) -> str:
        return f'Conv<{t["cpp"]}>::slot("{name}")'

    def emit_descs(self) -> list[str]:
        out = ["const svl_member_desc *gen_member_desc(uint32_t what, svl_id id) {", "  (void)what;", "  switch (id) {"]
        for m in self.members:
            i = f"0x{m['id']:x}"
            if m["what"] == "prop":
                p = m["data"]
                out.append(f"    case {i}: {{")
                out.append(f"      static svl_slot_desc ps[1] = {{{self.slot_expr(p['type'], p['name'])}}};")
                out.append(f'      static svl_member_desc d = {{"{p["name"]}", ps, {1 if p["writable"] else 0}, ps[0], 0}};')
                out.append("      return &d;\n    }")
            elif m["what"] in ("method", "static", "ctor"):
                ovs = m["data"]
                flags = {"method": "0", "static": "SVL_M_STATIC", "ctor": "SVL_M_STATIC | SVL_M_CTOR"}[m["what"]]
                if any(o["ret"]["k"] == "async" for o in ovs):
                    continue  # async members: the completion slot needs a callback type (none bound in N2)
                out.append(f"    case {i}: {{")
                if len(ovs) == 1:
                    o = ovs[0]
                    parts = []
                    for p in o["params"]:
                        t = p["type"]
                        s = self.slot_expr({**t, "cpp": f"{t['t']} *" if t.get("deref") else t["cpp"]}, p["name"])
                        if t.get("out") is True:
                            s = f"with_flags({s}, SVL_D_OUT)"
                        elif t.get("out") == "inout":
                            s = f"with_flags({s}, SVL_D_INOUT)"
                        elif p.get("default"):
                            s = f"with_flags({s}, SVL_D_OPTIONAL)"
                        parts.append(s)
                    np = len(parts)
                    if o["ret"]["k"] in ("void", "self"):
                        ret = "slot_desc(nullptr, SVL_NULL)" if o["ret"]["k"] == "void" else "slot_desc(nullptr, SVL_HANDLE)"
                    else:
                        ret = f"Conv<{o['ret']['cpp']}>::slot(nullptr)"
                else:
                    # Runtime dispatch: dynamic slots (a variant slot lets plain objects through as maps).
                    mx = max(len(ins_of(o)) for o in ovs)
                    req = min(required(o) for o in ovs)
                    names = [next((ins_of(o)[k]["name"] for o in ovs if len(ins_of(o)) > k), f"arg{k}") for k in range(mx)]
                    parts = [f'slot_desc("{names[k]}", SVL_NULL, {"SVL_D_NULLABLE" if k < req else "SVL_D_NULLABLE | SVL_D_OPTIONAL"}, SVL_T_VARIANT)'
                             for k in range(mx)]
                    np = mx
                    ret = "slot_desc(nullptr, SVL_NULL, SVL_D_NULLABLE)"
                slots = ", ".join(parts) or "slot_desc(nullptr, SVL_NULL)"
                out.append(f"      static svl_slot_desc ps[{max(np, 1)}] = {{{slots}}};")
                out.append(f'      static svl_member_desc d = {{"{m["js"]}", ps, {np}, {ret}, {flags}}};')
                out.append("      return &d;\n    }")
        out.append("    default: return nullptr;\n  }\n}")
        return out

    def emit_metaconv(self) -> list[str]:
        """metatype id -> converter, for the property and signal types (the core's metaobject paths)."""
        cpps: dict[str, dict] = {}

        def add(t: dict):
            if t["k"] in ("void", "variant", "async", "self") or t.get("out") or t.get("deref"):
                return
            cpps.setdefault(t["cpp"], t)

        for n in self.names:
            t = self.types[n]
            for p in t["properties"]:
                add(p["type"])
            for s in t["signals"]:
                for a in s["args"]:
                    add(a["type"])
        out = ["bool gen_metaconv(int id, const void *p, svl_value *o) {",
               "  using Fn = void (*)(const void *, svl_value *);",
               "  static const std::unordered_map<int, Fn> table = {"]
        for cpp in sorted(cpps):
            out.append(f"      {{QMetaType::fromType<{cpp}>().id(), [](const void *x, svl_value *v) {{ Conv<{cpp}>::out(*static_cast<{cpp} const *>(x), v); }}}},")
        out.append("  };")
        out.append("  auto it = table.find(id);\n  if (it == table.end()) return false;\n  it->second(p, o);\n  return true;\n}")
        out.append("bool gen_metaconv_in(QMetaType mt, const svl_value *v, QVariant &q, std::string &e) {")
        out.append("  using Fn = bool (*)(const svl_value *, QVariant &, std::string &);")
        out.append("  static const std::unordered_map<int, Fn> table = {")
        for cpp, t in sorted(cpps.items()):
            if t["k"] == "value" and not self.default_ctor(t["t"]):
                continue
            out.append(f"      {{QMetaType::fromType<{cpp}>().id(), [](const svl_value *x, QVariant &r, std::string &er) {{ {cpp} y{{}}; "
                       f"if (!Conv<{cpp}>::in(x, y, er)) return false; r = QVariant::fromValue(y); return true; }}}},")
        out.append("  };")
        out.append("  auto it = table.find(mt.id());\n  if (it == table.end()) return false;\n  return it->second(v, q, e);\n}")
        return out

    def default_ctor(self, n: str) -> bool:
        t = self.types.get(n)
        return bool(t) and (not t["constructors"] or any(required(o) == 0 for o in t["constructors"]))


def load(meta_dir: Path) -> dict | None:
    d = meta_dir / "types"
    if not (d / "QtCore.json").exists():
        return None
    types, enums = {}, {}
    for mod in tqc.MODULES:
        x = json.loads((d / f"{mod}.json").read_text())
        types.update(x["types"])
        enums.update(x["enums"])
    return {"types": types, "enums": enums, "files": [d / f"{mod}.json" for mod in tqc.MODULES]}
