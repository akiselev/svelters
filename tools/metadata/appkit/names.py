"""Native name -> JS name rules for AppKit. Mirrors crates/svelters-apple/src/names.rs; both are
tested against the same pairs (test_names.py, `names::tests::PAIRS`)."""
from __future__ import annotations


def kebab(ident: str) -> str:
    """camelCase / PascalCase -> kebab-case. A run of capitals is one word, except that its last
    capital starts the next word when a lowercase letter follows (URLString -> url-string)."""
    out: list[str] = []
    for i, c in enumerate(ident):
        if c in "_:-":
            if out and out[-1] != "-":
                out.append("-")
            continue
        if c.isascii() and c.isupper() and i > 0 and out and out[-1] != "-":
            prev = ident[i - 1]
            nxt = ident[i + 1] if i + 1 < len(ident) else ""
            if (prev.isascii() and (prev.islower() or prev.isdigit())) or (prev.isupper() and nxt.isascii() and nxt.islower()):
                out.append("-")
        out.append(c.lower() if c.isascii() else c)
    return "".join(out).rstrip("-")


def class_tag(cls: str, prefix: str = "NS") -> str:
    return kebab(cls[len(prefix):] if cls.startswith(prefix) else cls)


def selector_name(sel: str) -> str:
    parts = [p for p in sel.split(":") if p]
    s = parts[0] + "".join(p[:1].upper() + p[1:] for p in parts[1:]) if parts else ""
    return kebab(s)


def selector_arity(sel: str) -> int:
    return sel.count(":")


def notification_signal(name: str, prefix: str = "NS") -> str:
    n = name[len(prefix):] if name.startswith(prefix) else name
    return kebab(n.removesuffix("Notification"))
