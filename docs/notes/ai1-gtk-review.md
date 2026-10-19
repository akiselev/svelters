# AI1 GTK review: the 135 middle/low-band inferred facts

Review of every fact `infer.py` left below `promote_at` (0.9) in `metadata/inferred/gtk/_review.json` after AI1
(45 middle band, 90 low band, GTK 4.22.4 probe). Reviewer: Claude Opus 5.5 agent, 2026-10-19.

Result: **38 accepted, 76 rejected, 21 overridden** (135). 80 of the 135 are now settled by rule changes, so a
re-probe gets them right without review; the other 55 are per-fact decisions in `metadata/inferred/gtk/_reviewed.yaml`.
The review queue is empty (`_review.json`: middle 0, low 0).

## Mechanism

`metadata/inferred/gtk/_reviewed.yaml` (schema `svelters-inferred-reviewed/v1`) holds one decision per fact:
`fact: Kind.section.name`, `pattern: band:pattern` (as in the review row), `decision`, a one-line `reason`, `reviewer`.
`infer.py` reads it:

- **accept**: the candidate record is written to `metadata/inferred/gtk/<Kind>.yaml` with the review appended to its
  provenance and a `review` entry in `<Kind>.evidence.json`; it is then tested and ledgered like any inferred fact
  (and parked if its test fails). Only facts with a candidate record can be accepted; a `hold` (no record) cannot.
- **reject**: the fact leaves the review queue; nothing is written.
- **override**: a manual record in `metadata/overrides/gtk/<Kind>.yaml` covers the fact; `infer.py` checks the record exists.

`_review.json` counts the decided facts under `reviewed` (by decision and pattern). Errors, in both normal and
`--check` mode: a malformed or duplicate decision, an `accept` without a candidate, an `override` without a manual
record, a decision whose pattern no longer matches its row (a re-probe changed the evidence: review it again), and a
decision that matches no row (stale, or a rule promoted the fact). `infer.py` never writes or deletes `_reviewed.yaml`
or `_parked.json`, so reruns stay byte-identical. Tests: `tools/metadata/gtk/test_infer.py` (`pnpm test:meta` runs it and
`infer.py --check`).

## Decisions by pattern

| Pattern (band) | Facts | Accepted | Rejected | Overridden | How |
|---|---:|---:|---:|---:|---|
| prop:round-trip (middle) | 37 | 25 | 2 | 10 | review |
| prop:scalar-identity (middle) | 5 | 5 | | | rule: float sample from the GIR range |
| method:action (middle) | 3 | 1 | | 2 | review |
| method:accessor_set-no_sample (low) | 70 | 5 | 65 | | rule: 5 float samples, 29 denied re-parenting setters, 36 non-scalar skipped |
| method:accessor_set-differs (low) | 3 | | 1 | 2 | 1 rule (non-scalar), 2 review |
| method:accessor_get-differs (low) | 2 | | 2 | | 1 rule (non-scalar), 1 review |
| method:glib-warning (low) | 4 | | 3 | 1 | 1 rule (`set_startup_id` denied), 3 review |
| prop:error (low) | 2 | | | 2 | review |
| prop:ignored (low) | 4 | | 3 | 1 | review |
| prop:needs-fixture (low) | 1 | | | 1 | review (manual fact exists) |
| prop:normalized (low) | 1 | | | 1 | review (manual fact exists) |
| prop:recipe-populate (low) | 1 | | | 1 | review (manual fact exists) |
| signal:trigger-default-unsampled (low) | 2 | 2 | | | rule: delivery-only fallback |
| **Total** | **135** | **38** | **76** | **21** | 55 review, 80 rule |

Overridden: 14 were already manual facts (the manual record wins, nothing to add), 7 are new manual records.

### prop:round-trip (37): non-scalar properties the probe could only write back unchanged

The probe had no sample for these types, so it wrote back the value it read (`identity`, 0.8). The generated test does
the same (read, write, compare shape), and the value converter (`crates/svelters-gtk/src/convert.rs`) is what it checks.

- **Accepted (25)**: string lists `AboutDialog.{artists,authors,documenters}`; paintables and icons with a string form
  (`AboutDialog.logo`, `Entry.{primary,secondary}-icon-{gicon,paintable}`); `Actionable.action-target` (variant codec,
  null clears it); flags `{Entry,SearchEntry,Text,TextView}.input-hints`, `PopoverMenu.flags`; Pango boxed values with
  no string form yet, carried as opaque handles (`{Entry,Text,Label,Inscription}.attributes`, `{Entry,Text,TextView,Label}.tabs`,
  `FontDialogButton.language`); media streams (`MediaControls`, `Video`).
- **Rejected (2)**: `Window.application` (the backend sets every toplevel's application at creation; an app clearing it
  detaches the window and can quit the app), `Window.display` (window-system state, not app state).
- **Overridden (10)**: 9 already manual (`Calendar.date`, `ColorDialogButton.rgba`, `FontDialogButton.font-desc`,
  `Image.{gicon,paintable}`, `Picture.paintable`, `ScaleButton.icons`, `Widget.{css-classes,cursor}`).
  `Popover.pointing-to` was accepted first and its generated test failed: GTK raises a 0x0 rectangle to 1x1, so the
  unset value does not round trip. The new manual record sets 1,2 3x4 in `setup` first.

### prop:scalar-identity (5) and the float setters (5): the probe does not sample floats

`{AspectFrame,Label}.{xalign,yalign}`, `Inscription.yalign` and their `set_*` methods. Rule: a float property with a GIR
range gets the first of `float_samples` (0.25, 0.75, 2.0) inside the range, not the default and exact as an f32. The
generated test is the check (0.9, auto-promoted). The four `AspectFrame`/`Label` properties were already manual facts;
the inferred record merges under them.

### method:action (3)

`Notebook.popup_enable` accepted (sets `enable-popup`, nothing else, no window needed). `EditableLabel.start_editing` and
`Spinner.start` are already manual.

### method:accessor_set-no_sample (70)

- **29 denied by a new pattern**: `set_child`, `set_{start,end}_child`, `set_{center,start,end,label,title}_widget`,
  `set_titlebar`, `Widget.set_parent`, `Window.set_application`, `Window.set_display`. Children go through the slot
  model (`children` slots); a method call that re-parents behind the renderer corrupts its tree, and the backend owns
  a toplevel's application and display.
- **36 skipped as non-scalar** (string lists, boxed, flags, object/widget references such as `set_mnemonic_widget`,
  `set_stack`, `set_transient_for`): the generated test has no literal for the argument, so these could never become
  facts. An exact non-scalar setter was already skipped this way; now the outcome does not matter. The properties
  behind them are the API (most were accepted above).
- **5 promoted by the float rule** (above).

### The low-band singles

| Fact | Decision | Reason |
|---|---|---|
| `Widget.get_name` | reject | not an accessor: falls back to the type name (`GtkLabel`) when `name` is unset |
| `FontDialogButton.get_font_desc` | reject (rule) | non-scalar getter |
| `PasswordEntry.set_extra_menu` | reject (rule) | non-scalar setter |
| `Stack.set_visible_child_name` | override (new) | needs an existing page: `setup` adds pages `a`, `b`; sets `b` |
| `Widget.set_opacity` | override (new) | GTK keeps 8 bits (`255 * opacity`, truncated): 0.5 reads 0.50196; sample 0.2 = 51/255 |
| `CheckButton.set_group`, `ToggleButton.set_group` | reject | `group` is write-only and takes a peer button; a radio group needs a two-button manual fact |
| `Inscription.set_markup` | override (new) | `markup` is write-only; the test checks the parsed `text` |
| `Window.set_startup_id` | reject (rule) | startup notification is window-system state: added to the window-system deny pattern |
| `Entry.invisible-char`, `Text.invisible-char` | override (new) | real gunichar property; the probe crashed on `ParamSpecUnichar`; sample U+25CF |
| `Image.resource` | reject | needs a registered GResource; ignoring the probe's unknown path is right; apps use `file`/`icon-name` |
| `PasswordEntry.extra-menu` | reject | GTK bug: the setter never stores the model while it is NULL, so it always reads NULL (the menu is still applied) |
| `PopoverMenu.visible-submenu` | reject | names a submenu of the model; needs a menu fixture with a submenu |
| `Stack.visible-child-name` | override (new) | as `set_visible_child_name` |
| `Image.file`, `Widget.opacity`, `Notebook.page` | override | already manual |
| `{Text,TextView}.preedit-changed` | accept (rule) | setting `im-module` fires it only as a side effect (the IM context resets its preedit), and its default (NULL) cannot be written back; now a delivery-only synthetic-emission fact |

## Rule changes (implemented)

In `metadata/rules/gtk.yaml` and `tools/metadata/gtk/infer.py`:

1. `deny_methods`: a re-parenting/toplevel pattern (29 facts) and `set_startup_id` in the window-system pattern.
2. `float_samples`: unsampled ranged floats get a sample from the GIR range (properties and `set_*`, 10 facts).
3. A non-scalar accessor is skipped whatever the probe outcome, not held for review (38 facts).
4. A signal fired only by a property whose default cannot be sampled falls back to a synthetic emission
   (delivery-only), instead of being held (2 facts).

## Recommended, not implemented

- **Probe**: handle `ParamSpecUnichar` (no `minimum`); sample flags (one non-default bit) and string lists (`["a", "b"]`),
  so `input-hints`, `artists` and the like get an exact round trip and a generic `bind:`. Probe changes need a maintainer
  re-probe (`probe.py`), which this review did not run.
- **Converter**: string forms for `PangoAttrList` and `PangoTabArray` (`pango_attr_list_from_string`,
  `pango_tab_array_from_string`), so JS can write them rather than only pass handles back.
- **Exposure**: `deny_methods` only steers inference; GIRepository still reaches `set_child`, `set_parent` and
  `Window.set_application` at run time, and the `application`/`display` properties stay writable. A runtime deny list
  for re-parenting calls belongs to the exposure policy (X1), not to this review.
- **Generator**: a widget-handle argument literal in the generated tests would let reference setters
  (`set_mnemonic_widget`, `set_group`) be tested; a `Stack:visible-child-name` binding (notify fires once a page
  exists) needs the same `setup` in the binding echo test.

## Environment note

The machine now has GTK 4.24.1, and PyGObject is not installed for its Python 3.14. The checked-in metadata and probe
are GTK 4.22.4. `ingest.py` was rerun against GTK 4.22.4 extracted from the Arch archive (`gtk4-1:4.22.4-1`, the GIR
sha256 matches `metadata/gtk/*.json`, plus the `gobject-introspection` 1.86 GIRs) with PyGObject 3.56.2 in a private
venv, under `tools/dev/headless`. The rerun reproduced the old ingest exactly; the only diffs were the new facts and their
`inherited` lists. The generated tests ran on the system GTK 4.24.1.
