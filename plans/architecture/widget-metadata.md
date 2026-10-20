# Widget metadata, annotations and codegen

Status: direction set. The M3 implementation starts after P5. Research is in `docs/notes/widget-metadata-ingestion.md` and `docs/notes/signals-and-slots-semantics.md`.

## Sources

| Toolkit | Machine-readable sources | Gives |
| --- | --- | --- |
| GTK4 | `.gir` XML (stdlib Python parser) | types, nullability, transfer, array lengths, callbacks, defaults (as C literals) |
| GTK4 | PyGObject live pass | typed defaults, ranges, interface props, signal flags; cross-checks GIR against the installed library |
| Qt 6 | qdoc `.index` XML | the full public API: signatures, default args, virtuals, property accessors |
| Qt 6 | QMetaObject dump (PySide6) | properties, signals, slots, invokables, enums |
| Qt 6 | PySide typesystem XML | ownership rules (parent ownership, old child released by `setCentralWidget` etc.) |
| Qt 6 | uic behaviour on `.ui` samples | canonical child placement for QMainWindow, QTabWidget, layouts |

Ingestion scripts start from `docs/notes/scripts/` and move to `tools/metadata/` in M3.

## What no upstream source encodes

These are hand-annotated per element kind:

- Child policy and named slots: cardinality, insert/remove ops, per-child attributes, whether a slot replaces a default sub-widget, and whether replacing destroys the old one.
- Ownership transfer where the typesystem is silent or wrong.
- Signal behaviour: sync-required/return value, run-first vs after, echo suppression for two-way binding, overload collapsing (Qt `clicked()`/`clicked(bool)`).
- Which widget-typed properties are references (`default-widget`) rather than slots.

## Annotation loop

Agents draft the annotations, and generated tests verify them.

1. An agent drafts annotations for a batch of kinds from docs and source, with a provenance field on every fact.
2. The generator emits a verification test per annotated fact and runs it headless (Xvfb):
   - **Slots:** insert into the slot, read it back through the native getter, remove it, check the default is restored or the slot is empty.
   - **Ownership:** create, parent, destroy the parent, check the child's liveness through a weak ref or `QPointer`. Replace a slot child and check whether the old one survives.
   - **Signals:** emit or trigger natively, check delivery, the return-value effect (e.g. a `close-request` veto keeps the window open) and echo suppression.
3. A failing test sends the annotation back to the agent with the failure output. Merged annotations always have passing tests.
4. The GIR/PyGObject cross-check runs again on a toolkit upgrade. Changed facts re-run their tests.

## Generator language

The ingestion scripts and generators are Python through development. Near release (M6), once the generators have stopped changing, the generators move to Rust so that a project's `build.rs` can run them in-process, with no Python at build time. The ingestion scripts stay Python: maintainers run them offline and check in their output. Until then they must stay deterministic: inputs are only the metadata JSON, the overrides and the build config, and they write byte-identical output on rerun. The port can then be checked by diffing outputs.

Layout: `tools/metadata/{common,gtk,qt}/` holds the scripts, `metadata/{gtk,qt}/*.json` holds the generated, checked-in element kinds, and `metadata/overrides/{gtk,qt}/*.yaml` holds the annotations.

## Schema

`svelters-element-kind/v0`, frozen for the prototype. It is promoted from `docs/notes/widget-metadata-ingestion.md` §4.

Change rules during P:

- Lanes may add optional fields.
- Renaming or removing a field, or changing a field's meaning, goes through the coordinator and updates this section.
- `children.slots[].ops` is provisional. Its final shape will be the agent-fillable op-descriptor format from `docs/notes/signals-and-slots-semantics.md`.

Shared helpers are in `tools/metadata/common/svmeta.py`: load, override merge, validation and a deterministic writer.

```yaml
schema: svelters-element-kind/v0
toolkit: gtk                     # gtk | qt
id: "svelters-api:v1:type:gtk:Gtk:Button"
native: { name: GtkButton, gtype: GtkButton, c_type: GtkButton }   # qt: { name: QPushButton, header: qpushbutton.h, module: QtWidgets }
kind: object                     # object | interface | enum | flags | boxed | value
abstract: false
bases: [Gtk.Widget, GObject.InitiallyUnowned, GObject.Object]     # full chain, nearest first
interfaces: [Gtk.Accessible, Gtk.Actionable, Gtk.Buildable, Gtk.ConstraintTarget]
availability: { since: "4.0", deprecated: null }
constructors:
  - id: ...ctor:new_with_label(utf8)
    native: gtk_button_new_with_label
    params: [{ name: label, type: string, nullable: false, ownership: borrowed }]
    returns: { type: self, ownership: floating_sink }   # qt: parent_owned via parent arg
    invocation: { dev: introspected, release: generated_thunk }
  - id: ...ctor:generic             # g_object_new_with_properties / qt: none
    invocation: { dev: reflected }
properties:
  - name: label
    js_name: label
    type: { ref: string, nullable: true }
    access: { read: true, write: true, construct: false, construct_only: false }
    default: null                # typed value (from GParamSpec / Q_PROPERTY reset) not C literal
    range: null                  # {min, max} for numeric
    notify: { signal: "notify::label", explicit: true }    # qt: labelChanged()
    accessors: { get: gtk_button_get_label, set: gtk_button_set_label }
    invocation: reflected
    role: value                  # value | child_slot | reference (widget refs like default-widget)
signals:
  - name: clicked
    js_event: click
    params: []
    returns: { type: void }
    handler_result: none         # none | bool_stop | value
    sync_required: false
    phase: first                 # gtk when; qt: n/a
    flags: [action]              # action, detailed, no_recurse, cleanup
    connection: { disconnect_on: [handle_drop, native_destroy] }
methods:
  - name: set_child               # usually hidden in favour of slots
    params: [...]
    invocation: { dev: introspected, release: generated_thunk }
    unsupported_reason: null      # varargs | non_introspectable | callback_scope | ...
enums: []                         # ids of enum/flags kinds referenced; enum kinds list members {name, nick, value}
children:
  policy: single_child            # none | single_child | ordered | named_slots | layout_managed | pages | model_driven | custom
  default_slot: child
  slots:
    - name: child
      cardinality: one            # one | many
      accepts: [Gtk.Widget]
      ops: { set: {property: child}, clear: {property: child, value: null} }
      ownership: parent_owned
      attached_props: []
  # e.g. HeaderBar:
  #   - {name: start, cardinality: many, ops: {insert: pack_start, remove: remove}, order: append_only}
  #   - {name: end,   cardinality: many, ops: {insert: pack_end,   remove: remove}}
  #   - {name: title, cardinality: one,  ops: {set: {property: title-widget}}}
  # QMainWindow:
  #   - {name: central, cardinality: one, ops: {set: setCentralWidget, take: takeCentralWidget}, replace: releases_old}
  #   - {name: menubar, cardinality: one, accepts: [QMenuBar], ops: {set: setMenuBar}}
  #   - {name: toolbars, cardinality: many, accepts: [QToolBar], ops: {insert: "addToolBar(area,w)", remove: removeToolBar},
  #      attached_props: [{name: area, type: Qt.ToolBarArea, default: TopToolBarArea}, {name: break_before, type: bool}]}
  #   - {name: docks, cardinality: many, accepts: [QDockWidget], ops: {insert: "addDockWidget(area,w)", remove: removeDockWidget},
  #      attached_props: [{name: area, type: Qt.DockWidgetArea}]}
  #   - {name: statusbar, cardinality: one, accepts: [QStatusBar], ops: {set: setStatusBar}}
semantics: { role: button, actions: [activate] }   # link to semantic-graph defaults
source:                                            # provenance per fact
  gir: { file: Gtk-4.0.gir, sha256: ..., version: "4.22.4" }
  runtime: { pygobject: "3.56.2" }
  overrides: overrides/gtk/Button.yaml             # hand-annotated facts are marked origin: manual
```

Type references use a small neutral vocabulary (`bool, i32, u32, i64, u64, f32, f64,
string, enum<Id>, flags<Id>, object<Id>, boxed<Id>, value<Id>, list<T>, variant, callback<Id>`)
plus `native` (raw GIR/C++ spelling) for diagnostics. Every fact carries `origin:
gir | runtime | qdoc | metaobject | typesystem | apiextractor | manual`.

### Observability facts (milestone R0)

Whether a native change of a property can be observed is its own fact per platform:
`svelters-api:v1:type:<tk>:<Ns>:<Type>:prop:<prop>:observe` (plans/research/REACTIVITY.md section 4,
`plans/milestones/R0-observability-facts.md`). The record and its validation live in
`tools/metadata/common/observe.py`:

```yaml
prop: text
class: exact                     # constant | set_only | exact | partial | none
via:                             # ordered mechanisms, primary first: notify (GTK notify::, Qt NOTIFY, KVO, DP callback),
  - {mechanism: notify}          #   bindable, signal, event, notification, target_action, via {object, prop}
missed: [user_edit]              # partial only: user_edit | programmatic | cross_prop | parent | layout | wm | async
fires: on_change                 # on_change | on_set | coarse
derived_from: [label]            # props whose set also announces this one
value_cost: cheap                # cheap | proportional
timing: sync                     # sync | deferred
provenance: [{origin: probe, ref: "..."}]
verify: {on: Gtk:Entry, check: change}   # how the generated test checks it (not semantics)
```

The facts stack like every other annotation (ingested < inferred < manual) but are merged apart from the
kind records, so they never change a property fact's fingerprint:

- **ingested**: the raw inputs of the rules, already in each toolkit's facts: GTK `runtime.flags`
  (`EXPLICIT_NOTIFY`, `CONSTRUCT_ONLY`) and `notify`, plus the flags of every non-GUI type from the observe probe
  (GIR has none); Qt `metaobject.notify`/`constant` and moc's BINDABLE (`metadata/qt/moc/properties.json`,
  `tools/metadata/qt/ingest_moc.py`); WinUI `dependency_property`; AppKit the probe's KVO column.
- **inferred**: `metadata/inferred/<tk>/observe/<Ns>.<Type>.yaml` (rules O1-O10 in each toolkit's `infer.py`).
- **manual**: `metadata/overrides/<tk>/observe/<Ns>.<Type>.yaml`, same format; a record replaces the inferred one.
- **merged**: `metadata/observe/<tk>.json` (`tools/metadata/common/observe.py`, `--check` in `pnpm test:meta`),
  read by `gen_semantics.py` (`observe` on every spec property: `ObserveSpec` in
  `crates/svelters-compiler/src/semantics/spec.rs`), the GTK and Qt verification generators, the ledger and
  `coverage.py`.

## Outputs

- Element-kind tables (`metadata/<toolkit>/*.json`, checked in and versioned).
- `.d.ts` typings for native elements, wrapper components and `bind:this` targets.
- Qt: C++ dispatch/subclass/connector code via `svelters-qt-build` (see `backend-abi.md`).
- GTK: dev uses dynamic GObject property/signal access plus GIRepository/libffi method invocation. Release uses generated thin `-sys` calls for the tree-shaken roots (M6).
