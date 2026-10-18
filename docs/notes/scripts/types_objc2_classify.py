#!/usr/bin/env python3
"""Prototype auto-classifier for Foundation/AppKit, over objc2's header-translator output.

objc2 (https://github.com/madsmtm/objc2) translates the SDK headers with libclang and publishes the
result as plain Rust source (crates objc2-foundation, objc2-app-kit, ...), so on Linux it is a
licence-clean, machine-readable mirror of the ObjC headers with extra facts the headers do not
state directly:
  #[unsafe(super(A, B, ...))]       full superclass chain
  #[thread_kind = MainThreadOnly]   main-thread affinity
  unsafe impl Send/Sync for X       thread-safe classes
  unsafe impl NSCopying for X       copyable (value-like) classes
  pub fn  vs  pub unsafe fn         objc2's reviewed "safe to call" set (translation-config.toml)
  block2::DynBlock params           completion handlers
  #[repr(C)] pub struct X {pub ..}  plain C structs (records)

Rules (plans/research/NON-GUI-TYPES.md section 3.1):
  NSResponder/NSCell/NSViewController/NSWindow descendants -> widget (elements, out of scope)
  NSMutableX                                               -> object (SHARED)
  X with an NSMutableX counterpart                         -> value
  NSCopying, no property setters, not main-thread               -> value
  other NSObject subclasses                                 -> object (SHARED)
  plain struct with public fields                           -> record
  method with a DynBlock param named *handler/*completion*  -> async candidate

Usage: python3 -I types_objc2_classify.py [--registry ~/.cargo/registry/src] [--show CATEGORY]
"""
import argparse
import collections
import glob
import os
import re

CRATES = ["objc2-foundation-0.3.2", "objc2-app-kit-0.3.2"]
WIDGET_ROOTS = {"NSResponder", "NSCell", "NSViewController", "NSWindow", "NSView"}
CLASS_RE = re.compile(r"extern_class!\(\s*(?:///[^\n]*\n\s*)*#\[unsafe\(super\(([^)]*)\)\)\]\s*((?:#\[[^\n]*\]\s*)*)pub struct (\w+)", re.M)
STRUCT_RE = re.compile(r"#\[repr\(C\)\]\s*(?:#\[[^\n]*\]\s*)*pub struct (\w+) \{([^}]*)\}", re.M)
METHOD_RE = re.compile(r"#\[unsafe\(method\(([^)]*)\)\)\]\s*(?:#\[[^\n]*\]\s*)*pub (unsafe )?fn (\w+)\(([^)]*(?:\([^)]*\)[^)]*)*)\)", re.M)
IMPL_RE = re.compile(r"^(?:\s*)impl(?:<[^>]*>)? (\w+)(?:<[^>]*>)? \{", re.M)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--registry", default=os.path.expanduser("~/.cargo/registry/src"))
    ap.add_argument("--show")
    a = ap.parse_args()
    classes, structs, send, copying, main_only = {}, {}, set(), set(), set()
    methods = collections.defaultdict(lambda: collections.Counter())
    for crate in CRATES:
        roots = glob.glob(f"{a.registry}/*/{crate}/src/generated")
        if not roots:
            raise SystemExit(f"{crate} not found under {a.registry}")
        fw = "Foundation" if "foundation" in crate else "AppKit"
        for f in glob.glob(f"{roots[0]}/*.rs"):
            src = open(f).read()
            for m in CLASS_RE.finditer(src):
                supers = [s.strip() for s in m.group(1).split(",")]
                classes[m.group(3)] = (fw, supers)
                if "MainThreadOnly" in m.group(2):
                    main_only.add(m.group(3))
            for m in STRUCT_RE.finditer(src):
                if "pub " in m.group(2):
                    structs[m.group(1)] = fw
            send |= set(re.findall(r"unsafe impl Send for (\w+)", src))
            copying |= set(re.findall(r"unsafe impl NSCopying for (\w+)", src))
            # attribute each method to the nearest preceding `impl X {`
            impls = [(m.start(), m.group(1)) for m in IMPL_RE.finditer(src)]
            for m in METHOD_RE.finditer(src):
                owner = None
                for pos, name in impls:
                    if pos < m.start():
                        owner = name
                    else:
                        break
                if owner is None:
                    continue
                c = methods[owner]
                c["unsafe" if m.group(2) else "safe"] += 1
                params = m.group(4)
                if "DynBlock" in params and re.search(r"(handler|completion)\w*\s*:", params, re.I):
                    c["async"] += 1
                if "DynBlock" in params:
                    c["block"] += 1
                # property setters carry a `/// Setter for [`prop`]` doc line in objc2's output
                if "Setter for" in src[max(0, m.start() - 300):m.start()].split("#[unsafe(method(")[-1]:
                    c["setters"] += 1

    def cat(n):
        fw, sup = classes[n]
        if n in WIDGET_ROOTS or any(s in WIDGET_ROOTS for s in sup):
            return "widget"
        if n.startswith("NSMutable"):
            return "object"
        # an immutable class of a mutable/immutable pair, or a copyable class with no property setters that
        # is not main-thread bound (NSCopying alone is too weak: NSMenu, NSFormatter copy too)
        if f"NSMutable{n[2:]}" in classes:
            return "value"
        if n in copying and methods[n]["setters"] == 0 and n not in main_only:
            return "value"
        return "object"

    cats = {n: cat(n) for n in classes}
    per = collections.defaultdict(collections.Counter)
    for n, c in cats.items():
        per[classes[n][0]][c] += 1
    for n, fw in structs.items():
        per[fw]["record"] += 1
    print("== classes and structs by category (objc2-foundation 0.3.2 + objc2-app-kit 0.3.2)")
    for fw in ("Foundation", "AppKit"):
        print(f"  {fw:10} " + ", ".join(f"{k} {v}" for k, v in sorted(per[fw].items())))
    nonw = [n for n, c in cats.items() if c != "widget"]
    print(f"  non-widget classes: {len(nonw)}; MainThreadOnly among them: {sum(1 for n in nonw if n in main_only)}; "
          f"Send (thread-safe): {sum(1 for n in nonw if n in send)}")

    tot = collections.Counter()
    for n in nonw:
        tot.update(methods[n])
    k = tot["safe"] + tot["unsafe"]
    print(f"\n== methods of non-widget classes: {k}; objc2-safe {tot['safe']} ({100 * tot['safe'] // max(k, 1)}%), "
          f"unsafe {tot['unsafe']}; with block params {tot['block']} (completion-handler shaped {tot['async']})")
    for fw in ("Foundation", "AppKit"):
        t = collections.Counter()
        for n in nonw:
            if classes[n][0] == fw:
                t.update(methods[n])
        kk = t["safe"] + t["unsafe"]
        if kk:
            print(f"  {fw:10} methods {kk:5}  safe {t['safe']:5} ({100 * t['safe'] // kk}%)")
    if a.show:
        print(f"\n== {a.show}:")
        names = sorted(structs) if a.show == "record" else sorted(n for n, c in cats.items() if c == a.show)
        print("  " + " ".join(names))


if __name__ == "__main__":
    main()
