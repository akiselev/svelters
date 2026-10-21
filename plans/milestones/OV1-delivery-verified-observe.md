# OV1: delivery-verified observation (plans/research/REACTIVITY.md §4.5, decision §13 #8)

Lane: the observe facts and their rules (`metadata/rules/<tk>.yaml` `observe:`), the GTK observe probe inputs and
generated observe rows (`tools/metadata/gtk/{observe_rules,gen_backend}.py`, `crates/svelters-gtk/src/testing_observe.rs`),
the Qt observe triggers, `tools/metadata/common/observe.py`, `tools/metadata/semantics/{gen_semantics,gen_observe_outputs}.py`,
the compiler's `plan_bind` and diagnostics, the R2 goldens. Branch `ci/ov1` (worktree branch
`worktree-agent-a8ec847313838ab49`). The ABI is unchanged.

## Problem

Decision §13 #8 lowers every `bind:` on an `exact` prop onto native observation (R2). That is safe only if the prop
really announces its native changes. R0 left 238 GTK `exact` facts whose generated row only installed the mechanism
(`connect`) and 58 with no row at all (the brief's "309 weak" counts the 71 `refused` constants too, which are not
`exact`); Qt had 14 `connect`-only and 45 row-less `exact` facts, WinUI 8 and 69, AppKit 1. If such a prop is silent,
`bind:` compiles and the bound value goes stale: R2's runtime fallback to the signal plan triggers only when the host
*refuses* the observation, never when it is silent.

## The rule

`exact` in §13 #8 means **`exact` and delivery-verified**:

| Fact on the platform | `bind:` lowering |
|---|---|
| `exact`, `verified: delivery` | `$n.bind_prop` / `$n.bind_readonly` (as R2) |
| `exact`, `verified: weak` or `none`, with a declared signal plan | `$n.bind(node, prop, signal, arg, get, set)`: the signal plan stays, no warning |
| `exact`, `verified: weak` or `none`, no signal plan | `$n.bind_prop` / `$n.bind_readonly` + **`bind_observation_unverified`** warning (error under `--strict-native`) naming the fact |
| anything else | unchanged (R2 table) |

Verification strength (`tools/metadata/common/observe.py` `verification`, `VERIFIED`):

- `delivery`: every ledger entry of the fact passes and its row made a native change (a native set not through our
  `set_prop`, a native trigger, or the forwarded object's set) and saw it announced through the real ABI
  (`observe` -> mechanism -> `host.prop_changed`), with the read value changed; for a `silent` row (a missed cause of
  a `partial`/`none` fact) the change was made and *not* announced.
- `weak`: the passing row only installed and removed the mechanism (`connect`) or checked the backend's answer
  (`refused`, `unsupported`).
- `none`: no passing row.

The strength comes from the toolkit's committed facts manifest (GTK, AppKit, WinUI: their generators decide the
check) or, for Qt, from the fact's `verify` block as `qt/gen_cpp.py` maps it; the ledger decides pass. It reaches
the compiler as `ObserveSpec.verified` (`delivery` | `weak` | `none`; absent reads `none`) next to `fact`
(`<Ns>:<Type>:<prop>`, for messages) in `metadata/semantics/<tk>.json`.

Unaffected on purpose: the GTK `SVL_M_OBSERVABLE` descriptor flag and the reactive getters (R1). They answer "can
this prop be observed" (class `exact`/`partial` with a mechanism), not "may `bind:` drop its signal plan"; a weakly
verified prop still has a working mechanism (its row installs it), and a getter that observes a silent prop is no
worse than the plain read it replaced. Only `bind:` lowering changes.

### Diagnostic `bind_observation_unverified`

Warning (error under `--strict-native`), in `codes::STRICT_NATIVE`:

```text
warning[bind_observation_unverified] (gtk): bind:is-active on <window>: `is-active` is exact on gtk (fact Gtk:Window:is-active),
but no native change has been seen delivered (its test only checks that the observation installs), so the bound value could go stale
  help: read it explicitly where it matters (node.getProp), or opt out with `|nonreactive`; verifying the fact (metadata/observe) removes this warning
```

Golden `crates/svelters-compiler/tests/r2/unverified-gtk.{svelte,expected}`; unit test
`r2_observe.rs::an_unverified_exact_fact_does_not_replace_the_signal_plan`. The typings
(`packages/native-runtime/types/observe.d.ts`) remark `delivery not verified (weak|none)` and how the compiler binds it.

## Earning the evidence

### GTK (generated rows, `testing_observe.rs`)

- **Generic native values** (`next_value`): flags (toggle a single-bit member), `GStrv` (append), `GVariant` (`s`, `*`,
  `b`, `i`, `u`), plus enum props of the non-GUI types (their `ref` is `enum`, which the rules' scalar test missed).
- **Sample values** for object and boxed props: `observe.values` in `metadata/rules/gtk.yaml`, by value GType (50
  types: widgets, models, factories, adjustments, files, icons, paintables, textures, RGBA, font descriptions, attr
  lists, tab arrays, transforms, ...). The rules copy them into the fact (`verify.values`), the generator emits
  `Check::Values`; each call sets a fresh value. Object-like values are compared by identity natively (the ABI may
  not read them: an expression reads Null); the row passes on a normalized read-back (GtkScrolledWindow wraps a
  child in a viewport) but requires the read value to change.
- **Triggers** (`observe.triggers`) for props whose plain set is ignored on a bare instance (no model, children,
  columns, pages) or that are read-only: Stack visible child, ComboBox/DropDown/SingleSelection/Notebook selection,
  TreeView expander column, Window focus widget, PopoverMenu submenu, TextBuffer can-redo, Image resource, every
  wrapping list model's `n-items`, DirectoryList loading, Gio.Application id/resource path, PrintOperation n-pages,
  BoolFilter expression, MessageDialog secondary markup.
- **Instances** (`observe.instances`) where the probe made none: bare Gio/Gtk Application, PrintOperation,
  MountOperation, and a tree view's selection (`verify.make: borrow:Gtk.TreeView.get_selection`; `On::Borrow`, also
  `On::Static`).

### Facts that turned out not exact (through the rules: `observe.missed`)

| Fact | Now | Why (verified by a `silent` row) |
|---|---|---|
| `Gtk:Image:file` | partial, missed `programmatic` | `gtk_image_set_from_file` with a loadable file changes `file` without `notify::file` (a property set notifies, through GObject) |
| `Gtk:TreeView:headers-clickable` | partial, missed `cross_prop` | it reads whether every column is clickable; appending a non-clickable column flips it without notify |

### Qt

Native triggers (`metadata/rules/qt.yaml` `observe.triggers`) for the 12 `connect`-only facts that have a reachable
change: animation `currentLoop`/`currentTime`/`direction`/`loopCount`/`state`, `QSequentialAnimationGroup:currentAnimation`,
`QVariantAnimation` `currentValue`/`easingCurve`, `QAbstractProxyModel:sourceModel`,
`QSortFilterProxyModel:filterRegularExpression`, `QItemSelectionModel:selectedIndexes`,
`QRegularExpressionValidator:regularExpression`.

@@RESULTS@@
