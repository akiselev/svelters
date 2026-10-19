---
title: "Coverage dashboard"
description: "Kinds ingested vs annotated vs tested, members per toolkit, docs and serdes coverage"
tableOfContents: false
sidebar:
  label: Coverage
---

Everything on this page is computed at build time from the generated IR. Serdes support is read from metadata when present; until the serdes lane adds it, it shows **unknown**.

## Demo Toolkit

| Measure | Value |
| --- | --- |
| Kinds ingested | 3 (2 class, 1 enum) |
| Element tags | <span class="svl-bar" title="1 of 3"><span style="width:33%"></span></span> 1 / 3 |
| Annotated kinds | <span class="svl-bar" title="1 of 3"><span style="width:33%"></span></span> 1 / 3 |
| Tested kinds | <span class="svl-bar" title="1 of 3"><span style="width:33%"></span></span> 1 / 3 |
| Properties | 2 (1 annotated) |
| Events | 1 (1 annotated) |
| Methods | 1 |
| Dangerous members | 1 properties, 0 events, 1 methods (warned on their pages) |
| Properties by origin | <span class="svl-badge svl-origin-gir" title="GObject-Introspection repository">gir</span> 2 |
| Docs: kinds with description | 1 |
| Serdes (per-type support) | **unknown** (no serdes fields in metadata yet) |
| Doc text policy | `full` (MIT) |

### Annotated and tested kinds (1)

[\<button>](/api/demo/button/)
