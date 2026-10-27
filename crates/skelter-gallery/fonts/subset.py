#!/usr/bin/env python3
"""Rebuilds the gallery's bundled font subsets (see README.md). Needs fonttools 4.55.0.

    python3 -I subset.py <dir with the source fonts> [<out dir, default: this dir>]

The source files and where they come from are in README.md. Static instances are cut from the
variable sources (Inter at opsz 14; Noto Sans SC/KR) before subsetting.
"""
import pathlib, sys
from fontTools.ttLib import TTFont
from fontTools.varLib import instancer
from fontTools import subset

src = pathlib.Path(sys.argv[1])
here = pathlib.Path(__file__).resolve().parent
out = pathlib.Path(sys.argv[2]) if len(sys.argv) > 2 else here

LATIN = "U+0020-007E,U+00A0-017F,U+0391-03C9,U+0410-044F,U+2010-2027,U+2030-203A,U+20AC,U+2122,U+2190-2193,U+2212,U+FB01-FB02"
SYMBOLS = "U+2190-21FF,U+2300-23FF,U+2460-24FF,U+2500-25FF,U+2600-26FF,U+2700-27BF,U+2B00-2BFF"
CJK_COMMON = "U+3000-303F,U+3040-30FF,U+3130-318F,U+FF00-FFEF"

def sample():
    chars = set()
    for line in (here / "cjk-sample.txt").read_text(encoding="utf-8").splitlines():
        if not line.startswith("#"):
            chars.update(ch for ch in line if ord(ch) > 0x2E7F)
    return ",".join(f"U+{ord(c):04X}" for c in sorted(chars))

def cut(font, unicodes, name):
    opts = subset.Options()
    opts.layout_features = ["*"]
    opts.name_IDs = ["*"]
    opts.notdef_outline = True
    opts.drop_tables += ["SVG ", "STAT", "MATH"]
    s = subset.Subsetter(opts)
    s.populate(unicodes=subset.parse_unicodes(unicodes))
    s.subset(font)
    font.save(out / name)
    print(name, (out / name).stat().st_size)

def static(path, **axes):
    return instancer.instantiateVariableFont(TTFont(src / path), axes, updateFontNames=True)

for w, n in [(300, "Light"), (400, "Regular"), (600, "SemiBold"), (700, "Bold")]:
    cut(static("Inter.ttf", opsz=14, wght=w), LATIN, f"Inter-{n}.subset.ttf")
cut(TTFont(src / "JetBrainsMono-Regular.ttf"), LATIN + ",U+2500-259F", "JetBrainsMono-Regular.subset.ttf")
cut(TTFont(src / "NotoSansArabic-Regular.ttf"), "U+0600-06FF,U+0750-077F,U+08A0-08FF,U+FB50-FDFF,U+FE70-FEFF,U+200C-200F", "NotoSansArabic-Regular.subset.ttf")
cut(TTFont(src / "NotoSansSymbols2-Regular.ttf"), SYMBOLS, "NotoSansSymbols2-Regular.subset.ttf")
cut(TTFont(src / "NotoSansSymbols-Regular.ttf"), SYMBOLS, "NotoSansSymbols-Regular.subset.ttf")
cut(TTFont(src / "NotoSansMath-Regular.ttf"), "U+2200-22FF,U+27C0-27EF,U+2980-2AFF", "NotoSansMath-Regular.subset.ttf")
cjk = CJK_COMMON + "," + sample()
cut(static("NotoSansSC.ttf", wght=400), cjk, "NotoSansSC-Regular.subset.ttf")
cut(static("NotoSansKR.ttf", wght=400), "U+3130-318F," + sample(), "NotoSansKR-Regular.subset.ttf")
