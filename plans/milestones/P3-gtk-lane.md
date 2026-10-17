# P3 - GTK lane (metadata-generated)

Owns `tools/metadata/gtk/`, `metadata/gtk/`, `metadata/overrides/gtk/` and `crates/svelters-gtk`.

The backend is generated from metadata from the start. The generators are Python for now and will be ported to Rust (M3) so they can run inside self-contained `build.rs` scripts. Keep them deterministic, with no network access and no state beyond their inputs, so the port is mechanical.

## Work
1. **Ingest:**
   - `tools/metadata/gtk/ingest.py` reads `Gtk-4.0.gir` (plus its GObject/Gio includes) and does a PyGObject live pass. It emits `metadata/gtk/<Kind>.json` in the `svelters-element-kind/v0` schema (`plans/architecture/widget-metadata.md#schema`).
   - Every fact carries an `origin`.
   - Start from `docs/notes/scripts/gir_dump.py`, `gobject_live_dump.py` and `gir_child_slots.py`.
   - Generate the full Gtk namespace. The P kinds below are the only ones that must be correct and tested.
2. **Annotate:** `metadata/overrides/gtk/*.yaml` holds hand/agent annotations for child policy and slots, signal `handler_result`/`sync_required`, JS names and element tag names. They are merged over the ingested facts with `origin: manual`.
3. **Generate:** `tools/metadata/gtk/gen_backend.py` emits Rust tables into `crates/svelters-gtk/src/generated/`, checked in so the crate builds without Python:
   - kind → GType name;
   - prop ID → (name, value type, flags);
   - signal ID → (name, arg types, result kind);
   - slot ID → op (set-property / named method / ordered insert).
   It also emits `.d.ts` for the elements into `packages/dom/types/gtk.d.ts`.
4. **Runtime:** `svelters-gtk` implements `svl_backend` over these tables:
   - properties use `set_property_from_value` by name with `svl_value` ↔ `glib::Value` conversion driven by the table type;
   - signals use `connect_closure` by name;
   - slot ops are dispatched by table entry. The handful of child methods (e.g. `gtk_box_insert_child_after`, `gtk_header_bar_pack_start`) are generated as direct gtk4-rs calls, since slots need them before dynamic GI invocation exists (M3).
   - Handles map to GObjects with weak refs; destroyed widgets call `host.on_destroyed`.
   - Loop: GtkApplication; `wake` is an idle source on the main context; the checkpoint runs from it.
5. **Verify:** the generator also emits a Rust test per annotated slot/signal fact. Each test inserts, reads back through the native getter, removes, emits, and checks delivery. It runs under `xvfb-run`.

P kinds that must be correct: `window` (ApplicationWindow, `child`), `box` (ordered), `label`, `button` (`clicked`), `entry` (`changed`, `notify::text`), `scrolledwindow` (`child`), `headerbar` (`start`/`end`/`title`).

## Acceptance
- `python tools/metadata/gtk/ingest.py && python tools/metadata/gtk/gen_backend.py` is reproducible, i.e. a no-op diff on rerun.
- The generated verification tests pass under `xvfb-run cargo nextest run -p svelters-gtk`.
- The counter tree appears on screen from a hand-written op sequence. JS integration is P5.
