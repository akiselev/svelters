# X1: borrowed handles and dynamic GI calls (working notes)

Plan: `plans/milestones/X1-borrowed-gi.md`. ABI behaviour: `plans/architecture/backend-abi.md` ("Borrowed handles and dynamic methods").

## GTK identity and borrowed handles

- Identity lives on the object: GObject qdata `svelters-handle` holds `(backend id, handle)`. The backend id keeps two backends in one process (tests) apart, and `handle_of` also checks that the handle is still in the table, so a stale tag can never resolve. The old pointer-keyed `by_obj` map is gone.
- A borrowed entry holds only a `glib::WeakRef` and a raw `g_object_weak_ref` notify (`WeakNote`). The notify runs inside the object's dispose, so it only removes the entry and calls `on_destroyed(h, OWNER)`; it takes nothing else from the object. The entry owns the notify data: `release_entry` unrefs the notify and frees the box when the app forgets the handle first, and the notify frees it when it fires.
- `ref_count` of the object is unchanged by minting (test `minting_does_not_keep_objects_alive`).
- Cascade on a native window close: kids are slot children plus borrowed handles whose `owner` (the handle whose getter minted them) is already in the cascade. They report `PARENT`, not `OWNER`, and their weak notify is removed before the host hears about them, so each handle is reported once.
- Kind of a borrowed handle: the nearest ancestor GType with a generated kind (`GtkText` for an entry's inner text; `PangoLayout` has none, so kind 0 and only the generated-table lookups fail for it).
- Windows we did not create are never held strongly (the G5 zombie trap), so a transfer-full window return mints a borrowed handle.

## GTK dynamic calls (`gi.rs`)

- `gi_function_info_invoke` is the libffi layer (girepository drives libffi itself), so there is no direct libffi dependency. Linking is `#[link(name = "girepository-2.0")]`; GLib >= 2.80 ships it. No new crates.
- The generated `METHODS` table has no signatures (just `present`), so it cannot drive calls. Dynamic ids are `METHODS.len() + 1 + index`; the plan cache is per `(kind id, name as written)` and the plan is built once from the typelib. A plan with an unsupported feature is cached with its error, so `call` explains what is unsupported.
- Receiver check at call time: the receiver must be an instance of the type that declares the method (a method found through a kind may be called on a handle of a sibling kind with the same cached id).
- Gotcha found by test: `bool::then_some(Owned(p))` evaluates eagerly and unrefs a NULL info. Use `then(|| ...)`.
- GTK quantises opacity to 8 bits, so `set_opacity(0.25)` reads back 0.251.

### Supported type matrix (GTK)

| GI type | In | Out |
| --- | --- | --- |
| boolean | bool, integer | bool |
| int8/16/32/64, uint8/16/32/64 | integer (range-checked; integral floats accepted) | integer |
| float, double | number | number |
| utf8, filename | string; `null` if nullable; `transfer full` is `g_strdup`ed | string or null; `transfer full` freed |
| enum | nick or integer (validated against the enum class) | nick |
| flags | integer, one nick, or `a\|b` nicks | integer |
| object / interface | handle (type-checked), `null` if nullable; `transfer full` gets its own ref | existing handle, else BORROWED (`transfer none`) or OWNED (`transfer full`) |
| void | | null |

Rejected with a message that names the feature (recorded as follow-ups): callbacks (and their user_data/destroy), out/inout parameters, arrays, GList/GSList/GHashTable, GError values, GType, gunichar, structs/unions/boxed (ABI v3, lane S1), untyped `void*` parameters, static functions and constructors. Varargs functions are not in the typelib at all, so they resolve as unknown methods. A `throws` method raises its GError as an exception.

### Follow-ups

- Out parameters (needs an ABI shape for multiple returns; likely a list or record in v3).
- Arrays of strings/ints (`set_css_classes`, `GtkWidget.measure`-style out args).
- Callbacks (`add_tick_callback`, `GtkExpression` closures): needs the host `invoke_callback` of ABI v3.
- Static functions as `call` on the class (`Gtk.Widget.get_default_direction`).
- Methods declared on a type more derived than the handle's kind (a borrowed `GtkBuiltinIcon` is kind 0 or its nearest ancestor): the typelib lookup keys on the kind. A per-object resolve would need `handle_info` to carry the GType.
- Release builds: generated thunks (M6) can reuse `Plan`/`Slot` as their description.
