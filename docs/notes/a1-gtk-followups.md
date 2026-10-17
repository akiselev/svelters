# A1 gtk-followups notes

- Strict merge: an override record whose name is not ingested (Picture `resource`) leaves a typeless record and ingest crashes with `KeyError: 'type'` instead of a clear error.
- Image `file` only round-trips for a file GTK can load; sample uses a repo-relative path from the crate dir. `resource` needs a registered GResource: dropped.
