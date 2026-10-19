"""The boundary of the Qt non-GUI type layer (plans/research/NON-GUI-TYPES.md section 2.4, N2).

Generated C++ calls only the hand-written core (templates/types_core.hpp.in), the converters and Qt's own
API: it never deletes, refs, keeps a map of objects or touches the core's tables. Constructor thunks may
say `new <BoundClass>(` and nothing else may allocate. The core itself stays small.
"""
from __future__ import annotations

import re
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]

# Text that would mean the generated layer manages lifetimes or state itself.
FORBIDDEN = [r"\bdelete\b", r"\bdeleteLater\b", r"\bref\(", r"\bunref\(", r"\bRelease\(", r"\bretain\b", r"\bQPointer\b",
             r"\bg_object_", r"\bCFRetain\b", r"\bg\.table\b", r"\bg\.boxed\b", r"\bg\.by_obj\b", r"\bg_ptrs\b",
             r"\bg_ptr_by_addr\b", r"\bg_tracked\b", r"\bg_boxed_kind\b", r"\bg_out\b", r"\bg_relays\b", r"\bsetParent\(",
             r"\bQVariant::fromValue\b.*\bboxed\b"]
CORE_BUDGET = 1100  # lines of templates/types_core.hpp.in (section 2.2: the Qt core is ~2.2k with serdes.hpp.in)


def generated_sections() -> list[tuple[str, str]]:
    with tempfile.TemporaryDirectory() as d:
        subprocess.run([sys.executable, str(HERE / "gen_cpp.py"), "--config", str(HERE / "gen.dev.toml"), "--out", d], check=True)
        text = (Path(d) / "svl_qt_backend.cpp").read_text()
    out = []
    for name in ("non-GUI type declarations", "non-GUI types"):
        start = text.index(f"// ---- generated: {name}")
        end = text.index(f"// ---- end generated: {name} ----")
        out.append((name, text[start:end]))
    return out


def bound_classes() -> set[str]:
    sys.path.insert(0, str(HERE))
    import types_qt_classify as tqc
    return set(tqc.load_facts()["types"])


def test_generated_code_stays_inside_the_boundary():
    bound = bound_classes()
    bad = []
    for name, sec in generated_sections():
        for n, line in enumerate(sec.splitlines(), 1):
            code = re.sub(r'"(?:[^"\\]|\\.)*"', '""', line).split("//", 1)[0]  # literals (enum member names) do not count
            for pat in FORBIDDEN:
                if re.search(pat, code):
                    bad.append(f"{name}:{n}: {pat}: {line.strip()[:160]}")
            for m in re.finditer(r"\bnew\s+([A-Za-z_][\w:]*)", code):
                if m.group(1) not in bound:
                    bad.append(f"{name}:{n}: allocation outside a constructor form: {line.strip()[:160]}")
    assert not bad, "\n".join(bad[:40])


def test_the_core_stays_small():
    n = len((HERE / "templates/types_core.hpp.in").read_text().splitlines())
    assert n <= CORE_BUDGET, f"types_core.hpp.in has {n} lines (budget {CORE_BUDGET}): move per-type code into the generator"


def test_facts_are_current():
    r = subprocess.run([sys.executable, str(HERE / "types_qt_classify.py"), "--check"], capture_output=True, text=True)
    assert r.returncode == 0, r.stderr
