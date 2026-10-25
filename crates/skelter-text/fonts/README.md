# Test fonts

Subsets of Noto fonts for skelter-text's deterministic tests (`skelter_text::test_fonts`). License:
SIL Open Font License 1.1 (`OFL.txt`); the fonts keep their Reserved Font Name-free Noto names
(Noto has no Reserved Font Names).

| File | Source | Subset |
|---|---|---|
| `NotoSans-Regular.subset.ttf`, `NotoSans-Bold.subset.ttf` | notofonts.github.io `fonts/NotoSans/hinted/ttf/NotoSans-{Regular,Bold}.ttf` | `U+0020-007E,U+00A0-00FF,U+2010-2027,U+2030-203A,U+20AC,U+2122,U+FB01-FB02` |
| `NotoSansHebrew-Regular.subset.ttf` | notofonts.github.io `fonts/NotoSansHebrew/hinted/ttf/NotoSansHebrew-Regular.ttf` | all code points |
| `NotoColorEmoji-COLRv1.subset.ttf` | google/fonts `ofl/notocoloremoji/NotoColorEmoji-Regular.ttf` (COLRv1) | `U+1F600,U+1F44D,U+2764,U+FE0F`, `SVG ` table dropped |

Command (fonttools 4.55.0): `pyftsubset <font> --unicodes=<set> --layout-features='*' --name-IDs='*' --notdef-outline [--drop-tables+=SVG]`.
Hinting instructions (`fpgm`, `prep`, `cvt `, `gasp`) are kept.
