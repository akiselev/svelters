---
title: Metadata and annotations
description: How element metadata is ingested, how contributors annotate kinds, and how generated tests verify every annotated fact.
---

Svelters does not hand-write bindings per widget. Upstream toolkit metadata is ingested into JSON, hand and agent annotations add what no upstream source encodes, and generators turn both into backend code, typings and tests. This page is the contributor guide.

## Pipeline at a glance

```text
upstream sources --> ingest.py --> metadata/<toolkit>/<Kind>.json   (checked in)
metadata/overrides/<toolkit>/<Kind>.yaml  (annotations) --merged by ingest / read by generator
metadata/<toolkit>/*.json + overrides + templates --> gen_backend.py / gen_cpp.py --> backend code + verify tests + .d.ts
upstream doc text --> ingest_docs.py --> metadata/docs/<toolkit>/<Kind>.json   (separate pass)
```

The generators are Python during the prototype. They move to Rust in M3 so a project's `build.rs` can run them in-process. The port is checked by diffing outputs, which is why determinism is a hard rule (below).

## Sources per toolkit

| Toolkit | Source | Gives |
| --- | --- | --- |
| GTK4 | `.gir` XML (stdlib Python parser, `tools/metadata/gtk/gir.py`) | types, nullability, transfer, array lengths, callbacks, defaults as C literals |
| GTK4 | PyGObject live pass | typed defaults, ranges, interface props, signal flags; cross-checks GIR against the installed library |
| Qt 6 | qdoc `.index` XML | the full public API: signatures, default args, virtuals, property accessors |
| Qt 6 | QMetaObject dump (PySide6) | properties, signals, slots, invokables, enums |
| Qt 6 | PySide typesystem XML | ownership rules (parent ownership, `setCentralWidget` releasing the old child) |
| Qt 6 | uic behaviour on `.ui` samples | canonical child placement for QMainWindow, QTabWidget, layouts |

Current counts (from `plans/STATUS.md`): GTK 443 kinds ingested, 298 kinds, 1304 properties and 349 signals in the generated tables. Qt 192 classes ingested, 8 elements annotated, 41 generated tests passing.

What no upstream source encodes, and is therefore annotated by hand:

- child policy and named slots: cardinality, insert and remove operations, per-child attributes, whether a slot replaces a default sub-widget, whether replacing destroys the old one;
- ownership transfer where the typesystem is silent or wrong;
- signal behaviour: sync-required or return value, run-first versus after, echo suppression for two-way binding, overload collapsing (Qt `clicked()` and `clicked(bool)`);
- which widget-typed properties are references (`default-widget`) rather than slots.

## Schema `svelters-element-kind/v0`

The schema is frozen for the prototype. During P, lanes may add optional fields; renaming, removing or changing the meaning of a field goes through the coordinator. A kind file carries `schema`, `toolkit`, `id`, `native`, `kind`, `bases`, `interfaces`, `constructors`, `properties`, `signals`, `methods`, `children` and `source`. Abbreviated:

```yaml
schema: svelters-element-kind/v0
toolkit: gtk                     # gtk | qt
id: "svelters-api:v1:type:gtk:Gtk:Button"
native: { name: GtkButton, gtype: GtkButton, c_type: GtkButton }
kind: object                     # object | interface | enum | flags | boxed | value
properties:
  - name: label
    js_name: label
    type: { ref: string, nullable: true }
    access: { read: true, write: true, construct: false, construct_only: false }
    notify: { signal: "notify::label", explicit: true }
signals:
  - name: clicked
    js_event: click
    handler_result: none         # none | bool_stop | value
    sync_required: false
children:
  policy: single_child           # none | single_child | ordered | named_slots | layout_managed | pages | model_driven | custom
  default_slot: child
```

Type references use a neutral vocabulary (`bool, i32, u32, i64, u64, f32, f64, string, enum<Id>, flags<Id>, object<Id>, boxed<Id>, value<Id>, list<T>, variant, callback<Id>`). The shared helpers are in `tools/metadata/common/svmeta.py`: load, override merge, validation and a deterministic writer.

## Override file format

Annotations live in `metadata/overrides/<toolkit>/<Kind>.yaml`, keyed by the native kind (`Button.yaml` for `GtkButton`, `QPushButton.yaml` for `QPushButton`). The smallest GTK annotation (`metadata/overrides/gtk/Button.yaml`):

```yaml
properties:
  - name: label
    js_name: label
    provenance: "GtkButton:label (Gtk 4 docs); P prop"
signals:
  - name: clicked
    js_event: clicked
    handler_result: none
    sync_required: false
    provenance: "GtkButton::clicked is an action signal with void return (Gtk 4 docs)"
```

Records in `properties`, `signals`, `methods`, `slots` are merged over the ingested facts by `name` (constructors by `id`). Every annotated fact gets a generated test.

Qt overrides use an `elements` list. One element is a (class, tag) pairing, so `box` lives in `QWidget.yaml`. `metadata/overrides/qt/QPushButton.yaml`:

```yaml
elements:
  - tag: button
    construct: { class: QPushButton }
    props:
      - { js: label, native: text }
    methods:
      - { js: click, native: click }
    signals:
      - js: clicked
        native: clicked
        # clicked() and clicked(bool) are separate metamethods; connecting both would deliver twice.
        collapses: ["clicked()", "clicked(bool)"]
        connect: "clicked(bool)"
        args: []
        trigger: "$w->click()"
```

A layout-managed container (`metadata/overrides/qt/QWidget.yaml`, abbreviated) shows props, composite construction and a slot with native operations:

```yaml
elements:
  - tag: box
    construct:
      class: QWidget
      composite:
        layout: { class: QBoxLayout, args: ["QBoxLayout::TopToBottom"] }
    props:
      - js: orientation
        target: layout
        type: enum
        getter: direction
        setter: setDirection
        values:
          horizontal: QBoxLayout::LeftToRight
          vertical: QBoxLayout::TopToBottom
      - { js: spacing, target: layout, native: spacing }
    children:
      policy: layout_managed
      default_slot: children
      slots:
        - name: children
          content: widget
          accepts: [QWidget]
          cardinality: many
          order: native_index
          default: { kind: none }
          on_remove: empty
          ops:
            insert:
              native: "QBoxLayout::addWidget(QWidget*, int, Qt::Alignment)"
              args: [child]
              variants:
                - when: "before != null"
                  native: "QBoxLayout::insertWidget(int, QWidget*, int, Qt::Alignment)"
                  args: [index, child]
              effects: { child: parented }
            remove:                              # removeWidget leaves the widget parented and visible
              native: "QLayout::removeWidget(QWidget*)"
              args: [child]
              then:
                - { native: "QWidget::setParent(QWidget*)", receiver: child, args: [null] }
              effects: { child: unparented }
          provenance: [{ origin: uic, ref: "layout/item" }, { origin: typesystem, ref: addownership-1 }, { origin: probe, ref: qt_slot_ownership_probe.py }]
```

GTK slot annotations follow the same descriptor shape (`metadata/overrides/gtk/Window.yaml`): `ops.get`, `ops.set`, `ops.remove` with `native: "GtkWindow::set_child(GtkWidget*)"`, `args`, and `effects`. Interface-owned facts (`text`, `changed`, `orientation`) are annotated on the interface kind (`Editable`, `Orientable`) with `verify_subject: {tag: entry}` naming the element the tests use.

### Slot descriptor vocabulary

| Field | Values |
| --- | --- |
| `children.policy` | `none`, `single_child`, `ordered`, `named_slots`, `layout_managed`, `pages`, `model_driven` |
| `slots[].cardinality` | `one`, `many` |
| `slots[].order` | `none`, `native_index`, `before_sibling`, `append_only`, `per_key`, `unordered` |
| `slots[].default.kind` | `none`, `builtin`, `lazy`, `convenience` |
| `slots[].on_remove` | `empty`, `restore_default`, `destroys_child` |
| `ops` | `set`, `insert`, `remove`, `replace`, `get`, `list`, `clear`, `restore`, `move`, `attr_get`, `attr_set`, plus `index_of` and `then` |
| op argument expressions | `self`, `child`, `before`, `index`, `attr.<name>`, `item.<name>`, `const:<literal>`, `null` |
| `effects.child` | `parented`, `unparented`, `deleted`, `wrapped:<Kind>`, `referenced`, `unchanged` |
| `effects.old` | `deleted`, `kept_child`, `unparented`, `unref`, `referenced_only` |

The `native:` string must parse, and the method must exist with that arity on the named class; the generator validates this when the class has metadata. The full format with worked examples (QMainWindow, QComboBox, grids, GtkNotebook) is in `docs/notes/signals-and-slots-semantics.md` section 2.5.

Not implemented in the generators: the `verify.steps` test DSL, `attrs`, `requires` (parsed, not enforced) and `restore`.

## Provenance and origin

Every fact carries a source. In override files that is `provenance`, a string or a list of `{origin, ref}`. `origin` is one of:

| Origin | Meaning |
| --- | --- |
| `gir` | GObject-Introspection repository |
| `runtime` | PyGObject live probe |
| `qdoc` | qdoc index |
| `metaobject` | Qt `QMetaObject` |
| `typesystem` | PySide typesystem XML |
| `apiextractor` | Shiboken API extractor |
| `uic` | uic behaviour on `.ui` samples |
| `manual` | hand or agent annotation |
| `probe` | used in slot provenance for probe scripts (`qt_slot_ownership_probe.py`) |

The ingester writes a per-record `annotated: {origin, provenance, fields}` for every overridden or new fact. Generated tests exist per `annotated` record, and the API reference shows the origin badge on every property and signal.

## The agent loop

1. An agent drafts annotations for a batch of kinds from the upstream docs and source, with provenance on every fact.
2. The generator emits one verification test per annotated fact and runs it headless under Xvfb:
   - **Slots:** insert into the slot, read it back through the native getter, remove it, check the default is restored or the slot is empty.
   - **Ownership:** create, parent, destroy the parent, check the child's liveness through a weak ref or `QPointer`. Replace a slot child and check whether the old one survives.
   - **Signals:** emit or trigger natively, check delivery, the return-value effect, and echo suppression.
3. A failing test goes back to the agent with the failure output. Merged annotations always have passing tests.
4. The GIR and PyGObject cross-check reruns on a toolkit upgrade. Changed facts rerun their tests. The Qt tests assume Qt 6.11 behaviour and are the regression net for the ownership facts.

## Running the tools

Ingesters need Python 3 with PyYAML. The GTK pass needs PyGObject and the GIR files. The Qt pass needs PySide6, the qdoc indexes and the PySide typesystem files.

GTK (`docs/notes/p3-gtk-lane.md`):

```sh
python3 tools/metadata/gtk/ingest.py [--girdir /usr/share/gir-1.0]   # GIR + PyGObject + overrides -> metadata/gtk/<Kind>.json
python3 tools/metadata/gtk/gen_backend.py                            # -> crates/svelters-gtk/src/generated/*.rs and packages/renderer/types/gtk.d.ts
xvfb-run -a cargo nextest run -p svelters-gtk                        # generated verification tests
```

Qt (`docs/notes/p4-qt-lane.md`):

```sh
python3 tools/metadata/qt/ingest.py [--docdir /usr/share/doc/qt6] [--typesystem <xml>] [--out metadata/qt]
python3 tools/metadata/qt/gen_cpp.py --config tools/metadata/qt/gen.dev.toml --out <dir>   # manual generator run
```

In normal builds the Qt generator runs from the leaf crate's `build.rs` (`crates/svelters-qt-build`), which compiles the output with `cc`. The generated tests run as `counter-qt::generated verify_all` under `cargo nextest` (see [Getting started](/guides/getting-started/)). The generator output is `svl_qt_backend.cpp`, `svl_qt_verify.cpp` (one test per annotated fact) and `qt.d.ts`.

## The documentation-text pass

Description text is a separate pass from structural ingestion, so docs can be regenerated without touching element facts. It writes `metadata/docs/<toolkit>/<Kind>.json` in schema `svelters-docs/v0`.

```sh
python3 tools/metadata/gtk/ingest_docs.py [--girdir /usr/share/gir-1.0] [--out metadata/docs/gtk] [--check]
python3 tools/metadata/qt/ingest_docs.py  [--docdir /usr/share/doc/qt6] [--out metadata/docs/qt] [--mode brief|links|full]
```

- GTK: reads the GIR `<doc>`, `<doc-deprecated>`, `version` and `deprecated-version`, and converts gi-docgen links (`[class@Gtk.Button]`) into IR cross-references. GTK documentation is LGPL-2.1-or-later. `--check` exits 1 if the output would change.
- Qt: reads qdoc `.index` files and, when present, the qdoc HTML. Qt documentation is GFDL-1.3, so the default `--mode brief` stores only short briefs plus deep links to doc.qt.io; `links` stores only index briefs and links; `full` stores whole descriptions and requires the site to carry the GFDL notice.

The API pages are generated from `metadata/`, `metadata/docs/` and `metadata/overrides/` by `tools/docs/build_ir.py` (`pnpm docs:ir`) and the site generator (`pnpm docs:build`); see [Docs generator](/concepts/docs-generator/).

## Determinism rule

Ingesters and generators take only the metadata JSON, the overrides, templates and build config. They use no clocks, environment, network or absolute paths, sort keys and records, and write byte-identical output on rerun. Rerun, a different config path and the `build.rs` `OUT_DIR` all give identical output. Each generated Qt file starts with an `inputs-digest` line, a sha256 over the metadata, overrides, templates and script actually read. Commit generated output together with the annotation that caused it, and rerun to confirm there is no diff.

## The "Edit this annotation" link

Each element page in the [API reference](/api/) ends its status box with a link.

- **Annotated kinds** (tier "Annotated and tested") get **Edit this annotation**. It opens `metadata/overrides/<toolkit>/<Kind>.yaml` in the GitHub web editor (the `edit/<branch>/` URL). The file is the first annotation file recorded for the kind.
- **Unannotated (raw) kinds** show a "Raw ingested kind" caution with a **create the override file** link: the GitHub `new/<branch>/metadata/overrides/<toolkit>` URL with `?filename=<Kind>.yaml` preselected.

The page's front matter carries the same URL as Starlight's `editUrl`.

## What "annotated and tested" means

| Tier | Meaning |
| --- | --- |
| Annotated and tested | The kind has a hand or agent annotation, and every annotated fact has a generated verification test that passes headless. |
| Raw | Facts straight from GIR, qdoc and the QMetaObject. No annotation, no behavioural test. Properties and signals are listed, but child slots, ownership and signal semantics are unknown. |

Only the prototype kinds are annotated: `window`, `box`, `label`, `button`, `entry`, `scrolledwindow` on both toolkits, plus `statusbar` and `menubar` on Qt. The other 185 ingested Qt classes are raw, and so are most GTK kinds. To promote a kind, add an override file following the examples above, run the generator, and make its tests pass.

Sources: `plans/architecture/widget-metadata.md`, `docs/notes/widget-metadata-ingestion.md`, `docs/notes/signals-and-slots-semantics.md` (section 2.5), `docs/notes/p3-gtk-lane.md`, `docs/notes/p4-qt-lane.md`, `metadata/overrides/gtk/`, `metadata/overrides/qt/`, `tools/metadata/`.
