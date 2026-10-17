---
title: How these docs are made
description: "The metadata-to-site pipeline, with per-toolkit frontends, the toolkit-neutral docs IR, and the shared output generator."
---

The API reference, the support matrix and the coverage dashboard are generated. Hand-written guides sit next to them. The pipeline has three stages, and the boundaries between them are strict.

```text
GIR / qdoc / QMetaObject            tools/metadata/{gtk,qt}/ingest.py      metadata/{gtk,qt}/*.json
                                                                           metadata/overrides/**/*.yaml
GIR <doc>, qdoc HTML                tools/metadata/{gtk,qt}/ingest_docs.py metadata/docs/{gtk,qt}/*.json
                                              |
              per-toolkit frontends:  tools/docs/gtk/frontend.py   tools/docs/qt/frontend.py
                                              |    (svelters-docs-ir/v0, validated against tools/docs/ir/schema.json)
                                    docs/site/.generated/ir/{gtk,qt}.json
                                              |
              shared output generator: docs/site/scripts/gen-pages.mjs (reads ONLY the IR)
                                              |
                        Astro Starlight build, Pagefind index, link checker
```

## Why two frontends

GTK and Qt are special cases. A GTK frontend deals with gi-docgen markdown, GObject properties and `notify::` signals, CSS nodes and floating references. A Qt frontend deals with overloads and default arguments, `Q_PROPERTY`, signal collapsing (`clicked()` against `clicked(bool)`), QFlags, QSS sub-controls, typesystem ownership facts and the GFDL. Merging those into one code base would put toolkit conditionals in the output. Each frontend owns its toolkit's quirks and emits the same neutral shape. Future toolkits (WinUI, AppKit and UIKit, egui) add `tools/docs/<toolkit>/frontend.py` and nothing else changes.

## The IR

`svelters-docs-ir/v0` is one JSON document per toolkit:

- `toolkit`: id, name, version, language, documentation licence, text policy, sources with hashes, attribution notices.
- `entities[]`: classes, interfaces, enums, flags and boxed types. Each has tags (the Svelte element names), a status tier (`annotated` or `ingested`), summary and description text blocks, upstream link, inheritance chain, lifetime class, properties, events, methods, constructors, enum values, child slots, examples, a screenshot key, a serdes status and the ABI symbols it touches.
- Every **text block** carries its own SPDX licence, origin and a `truncated` flag.
- Cross-references are Markdown links with an `xref:<entity id>#<kind>:<name>` target. The output resolves them to site URLs, or drops the link when the target is not documented.
- Toolkit specifics go in `extensions.<toolkit>`. The IR declares the fields in `extension_schemas`, and the output renders them as a generic "specifics" panel. GTK uses it for the GType and the CSS-node, accessibility and shortcut sections of a class description; Qt for the header, module, QSS support and ownership facts.
- `coverage`: counts of kinds, annotated and tested kinds, members by origin, documentation coverage and serdes support (`unknown` until the serdes lane writes metadata fields).

The schema lives in `tools/docs/ir/schema.json`. A frontend that emits invalid IR fails its build.

## What is generated, and what is marked

| Output | From |
| --- | --- |
| One page per element and per type, per toolkit | IR entities |
| Support matrix with portable elements cross-linked | element tags across IRs |
| Coverage dashboard | IR coverage blocks |
| Origin badges (`gir`, `qdoc`, `metaobject`, `manual`, ...) | `origin` on every member |
| "Annotated and tested" against "Raw ingested" | `status.tier` |
| "Edit this annotation" link | `status.annotation_files`, or a create-file link for raw kinds |
| Screenshots | `tools/docs/screenshots/`, generated headless under Xvfb into the build output |

Usage examples on the API pages are generated from the metadata and are labelled *generated, illustrative*. They are not compiled.

## Commands

| Command | What it does |
| --- | --- |
| `pnpm docs:ir` | run every frontend and write the IR |
| `pnpm docs:build` | IR, then the static site, then the link checker |
| `pnpm docs:dev` | IR, then the Astro dev server |
| `pnpm docs:screenshots` | render the widget screenshots (needs Xvfb, PyGObject, PySide6) |
| `pnpm docs:test` | IR schema tests, the golden output test, the smoke test |

Builds are limited to four CPU cores.
