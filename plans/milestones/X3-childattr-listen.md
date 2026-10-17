# X3: child attributes and listen phases

Lane X3, started 2026-10-17 on master 3070367. Closes `metadata/A1-GAPS.md` G1 and the listen-phase part of G5. Notes: `docs/notes/x3-childattr-listen.md`. ABI text: `plans/architecture/backend-abi.md`, "ABI v2.2 additive (X3)".

## Goals

1. **Child attributes.** Per-child facts that belong to neither the child nor the parent (grid cell, tab label, dock area, stretch) get an additive ABI op, metadata, a JS surface and generated tests on both backends.
2. **Listen phases.** `after` and `capture` listeners through one additive `listen_ex`, with the renderer syntax `on<event>capture` and `{@attach listen(name, fn, {after: true})}`.

## Decisions

- **ABI (additive tail members, all optional):** `set_child_attr`, `get_child_attr`, `listen_ex`; `SVL_ATTR` name kind scoped by the parent kind; listener flags travel back in bits 28..30 of the signal id on `post_event`/`call_sync`.
- **JS shape: prefixed attributes on the child** (`<entry parent:row={1}>`). The alternatives lose:
  - `sv-item` wrapper: another JS-only node in the placement walk, a second source of truth for the child's slot, and an extra component per cell.
  - `{@attach}`: runs after mount, so a Qt grid would place the child twice; reactivity ends up hand-written in the helper.
  - A prefixed attribute is set before the child is inserted, is reactive through the normal attribute path, and the renderer re-applies it whenever the child is inserted into a (new) parent.
- **Order:** attributes are applied right after `insert`, in the same op sequence. The backend clears per-child state on `remove`.
- **Metadata:** `attrs[]` on a slot, extending A1's existing (so far free-form) `attrs` array. Two ways to implement an attr: `via` a related object's property (GTK layout child, notebook page, stack page) or `set`/`get` native ops.
- **Listen phases:** GTK `after` = `connect_closure(after=true)`; `capture` = the controller's propagation phase. Qt `after` is by connection order on signals; `capture` is an application event filter that matches descendants, for the synthetic key signal. The rest is rejected with an error that says why.

## Steps

1. Plan, ABI text, notes (this commit).
2. ABI crate, header, safe wrappers, log backend, host ops, QuickJS binding, Node TestHost, renderer, op-log format, fixtures. Tests on the log backend.
3. GTK: `listen_ex`; `set_child_attr` generated from `attrs`; overrides for Grid, Fixed, Notebook, Overlay, Stack; generated tests; ledger.
4. Qt: same; `grid` element; tab, splitter, box layout, main window attrs.
5. Gallery pages, `.d.ts`, A1-GAPS update, full gates.
