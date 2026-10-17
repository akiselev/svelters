# P3 - GTK lane

Owns `crates/svelters-gtk`.

## Work
- An `svl_backend` implementation on gtk4-rs (GTK 4.22 installed). Handles map to GObjects through a table with weak refs; destroyed widgets call `host.on_destroyed`.
- Hand-mapped kinds:

| kind | widget | notes |
| --- | --- | --- |
| `window` | GtkApplicationWindow | `child` slot; `title` |
| `box` | GtkBox | `orientation`, `spacing`; ordered children with `insert_child_after` |
| `label` | GtkLabel | `label` |
| `button` | GtkButton | `label`; `clicked` |
| `entry` | GtkEntry | `text`; `changed` (and `notify::text` for M4 binding) |
| `scrolled` | GtkScrolledWindow | `child` |
| `headerbar` | GtkHeaderBar | slots `start`/`end`/`title` (named-slot smoke test) |

- Properties go through `ObjectExt::set_property_from_value` by name, with conversion from `svl_value` to `glib::Value` using the param spec type. Signals use `connect_closure` by name, with arguments converted to `svl_value`.
- Loop: `GtkApplication` + `run`. `wake` is a `glib::idle_add_once` via the main context, which is thread-safe. A checkpoint runs from that idle source.

## Acceptance
- A Rust test drives the backend directly with a scripted op sequence under `xvfb-run` and verifies the widget tree (types, order, props) through GTK getters, plus signal delivery via `emit_clicked`.
- A window with the counter tree appears on screen, built from a hand-written op sequence. Full JS integration is P5.
