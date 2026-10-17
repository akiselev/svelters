# A1 gtk-text notes

- Entry.yaml carried the `bind:text` binding (Editable.yaml has none); rewriting Entry.yaml dropped it until restored. PasswordEntry, SearchEntry and Text are not subclasses of Entry, so each got its own `bind:text` binding (same block) on its own file.
- EditableLabel `bind:editing` needs a declared `notify::editing` signal (derived_from) and `echo_suppress: ["notify::editing"]` on the property.
- Dropped: Label `ellipsize`, `wrap-mode` (Pango enums are not in the metadata: "enum Pango:EllipsizeMode not in metadata"). Same would apply to Inscription `wrap-mode`.
- Label `attributes` (Pango AttrList): not tried beyond the ingested boxed type; no string form, left out.
- TextView text: `get_buffer` returns a handle/boxed value (verified), but GtkTextBuffer has no generated kind, so buffer methods (`set_text`, `insert`) cannot be called on it: still a gap (borrowed handles of kind 0 have no method plan keyed on the kind; see x1-borrowed-gi.md follow-ups).
- SearchEntry `search-changed` fires after a delay on programmatic text sets too (gallery shows 2 events after the app's own write); it is not a change-echo binding signal, so the page keeps it off an `echo` label.
- Gallery: event names with `::` go through attribute spread (`{...{'onnotify::editing': fn}}`).
