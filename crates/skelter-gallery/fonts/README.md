# The gallery's fonts

The fonts every gallery cell draws with (`skelter_gallery::fonts`), on top of skelter-text's test fonts
(`skelter_text::test_fonts`: Noto Sans Latin subsets, Noto Sans Hebrew, three Noto Color Emoji). System fonts are
off, so a shot is the same on every machine (`--system-fonts` adds the OS's fonts behind these, for looking only).

The theme's own families come first: `--sv-font-family` is `Inter, system-ui, sans-serif` and `--sv-font-mono` is
`"JetBrains Mono", monospace` (`skelter-widgets/src/theme/data/shape.json`), so the gallery bundles those two, as an
app embedding skelter does. The rest are fallbacks: by script (Arabic, Han, kana, Hangul; skelter-text registers each
bundled font for the scripts it covers) and, for Common-script symbols, the gallery's symbol chain
(`Noto Sans Symbols 2`, `Noto Sans Symbols`, `Noto Color Emoji`, `Noto Sans Math`, set with
`FontStore::set_symbol_chain`).

All are under the SIL Open Font License 1.1, which allows bundling subsets (modified versions) with software. None
uses a Reserved Font Name (Noto CJK's is "Source").

| File | Source | Subset | Licence |
|---|---|---|---|
| `Inter-{Light,Regular,SemiBold,Bold}.subset.ttf` | google/fonts `ofl/inter/Inter[opsz,wght].ttf`, static instances at opsz 14, wght 300/400/600/700 | Latin (ASCII, Latin-1, Latin Extended-A), basic Greek and Cyrillic, punctuation, € ™, four arrows, − | `OFL-Inter.txt` |
| `JetBrainsMono-Regular.subset.ttf` | JetBrains/JetBrainsMono `fonts/ttf/JetBrainsMono-Regular.ttf` | as Inter, plus box drawing and blocks | `OFL-JetBrainsMono.txt` |
| `NotoSansArabic-Regular.subset.ttf` | notofonts.github.io `fonts/NotoSansArabic/hinted/ttf/` | Arabic, Arabic Supplement and Extended-A, presentation forms, ZWNJ/ZWJ and the marks | `OFL-Noto.txt` |
| `NotoSansSymbols2-Regular.subset.ttf`, `NotoSansSymbols-Regular.subset.ttf` | notofonts.github.io `fonts/NotoSansSymbols{2,}/hinted/ttf/` | arrows, technical (⌘ ⌫ ⏎), enclosed, box drawing, shapes (▶ ◀), misc symbols (★ ⚙), dingbats (✂ ✎ ✓), misc symbols and arrows | `OFL-Noto.txt` |
| `NotoSansMath-Regular.subset.ttf` | notofonts.github.io `fonts/NotoSansMath/hinted/ttf/` | mathematical operators (⋯) and supplements; `MATH` table dropped | `OFL-Noto.txt` |
| `NotoSansSC-Regular.subset.ttf` | google/fonts `ofl/notosanssc/NotoSansSC[wght].ttf`, wght 400 | CJK punctuation, all kana, fullwidth forms, and the Han of `cjk-sample.txt` | `OFL-NotoSansCJK.txt` |
| `NotoSansKR-Regular.subset.ttf` | google/fonts `ofl/notosanskr/NotoSansKR[wght].ttf`, wght 400 | Hangul compatibility jamo and the Hangul and Han of `cjk-sample.txt` | `OFL-NotoSansCJK.txt` |

CJK is a sample, not the full set (the full fonts are 10–18 MB): Chinese, Japanese and Korean UI words. A CJK
character outside `cjk-sample.txt` draws as a box; add the words to the file and rebuild.

Rebuild (fonttools 4.55.0, `pip install fonttools==4.55.0`), with the source files above downloaded into one
directory under their upstream names (the two variable CJK fonts saved as `NotoSansSC.ttf` and `NotoSansKR.ttf`,
Inter's as `Inter.ttf`):

```sh
python3 -I crates/skelter-gallery/fonts/subset.py <source dir>
```

Total: about 1.5 MB.
