---
title: A Svelte panel in a dock pane
description: Mount a Svelte component as the body of a berth pane. Lazy mount, tab title and dirty mark, the close guard, moving between windows, and faults.
status: available
verified_at: 424ebcf4
api:
  - svelters_egui::Embed::{create_slot, set_visible, set_viewport, mount, request_close, destroy_slot, slot, show_slot, take_events, root_info}
  - svelters_egui::embed::{with_current, show_viewport_passes, EmbedEvent, MountSpec, RootKey, RootOwner}
cases:
  - docs/egui/cases/recipe-dock-pane.svelte
snippets:
  - examples/egui-docs/src/dock_pane.rs
---

<!-- SAMPLE (plans/research/DOCS-PLAN.md §8). Final path: docs/egui/recipes/panel-in-dock-pane.md.
     The Rust code follows crates/svelters-egui/src/runner/dock.rs (SvelteDock, the EM2 adapter tested
     by tests/viewports.rs). DOC4 moves it into the compiled examples/egui-docs crate that the snippet
     markers point at. -->

# A Svelte panel in a dock pane

**Goal:** berth draws the pane chrome (tab, close button, splitter), and a Svelte component draws the
body. The panel keeps its state while it's hidden behind another tab or dragged to a second window.
It shows a dirty mark in its tab and can refuse to close.

**You need:** an `Embed` you created at startup, and a built embed bundle that exports the component.
See [svelte/project-setup](../svelte/project-setup.md#embed-bundles).

## How it fits together

| berth says | You call | Effect |
|---|---|---|
| (your view registry opens a Svelte pane) | `create_slot(pane)`, `set_visible(slot, false)` | The slot exists. Nothing is mounted yet. |
| `Shown { pane, viewport }` | `set_viewport`, `set_visible(true)`; `mount` the first time | The component mounts on first show and runs at the next `logic`. |
| `Hidden { pane }` | `set_visible(slot, false)` | The root stays mounted. Its animation frames pause. |
| `MovedToViewport { to, .. }` | `set_viewport(slot, to)` | Nothing remounts. Focus follows the slot. |
| the user clicks the tab's ✕ | `request_close(root)` | JS `onCloseRequest` handlers answer with `EmbedEvent::CloseDecided`. |
| `Closed { pane, .. }` | `destroy_slot(slot)` | The root unmounts, and its slot and ids are freed. |
| pane body draw callback | `show_slot(ui, slot)` via `with_current` | Draws the slot. Never runs JS. |
| end of each window's ui | `show_viewport_passes(ui)` | Runs the scene passes (overlays) for that window. |

The pane id is the slot key **and** the root key. Keys salt every egui id and AccessKit id inside
the panel, and they key the persisted UI state, so they must be stable across runs. Use berth's
`PaneId::as_str()`, never an index.

## Rust: the adapter

<!-- snippet: examples/egui-docs/src/dock_pane.rs#adapter -->
```rust
use std::collections::HashMap;
use svelters_egui::Embed;
use svelters_egui::embed::{EmbedError, EntryId, MountSpec, Props, RootId, RootKey, RootOwner, SlotOptions, with_current};

pub struct SveltePanes {
    pub embed: Embed,
    roots: HashMap<String, RootId>,
    pending: HashMap<String, (EntryId, String, Props)>,
}

impl SveltePanes {
    /// Your view registry decided this pane shows `export` from `entry`.
    pub fn open(&mut self, pane: &str, entry: EntryId, export: &str, props: Props) {
        let slot = self.embed.create_slot(pane, SlotOptions::fill().label(pane));
        self.embed.set_visible(slot, false);
        self.pending.insert(pane.to_owned(), (entry, export.to_owned(), props));
    }

    /// Each berth event, in `App::logic`, after `dock.take_events()` and before `embed.logic(ctx)`.
    pub fn on_event(&mut self, ev: &berth::PaneEvent) -> Result<(), EmbedError> {
        let Some(slot) = self.embed.slot(ev.pane().as_str()) else { return Ok(()) }; // not ours
        match ev {
            berth::PaneEvent::Shown { pane, viewport } => {
                self.embed.set_viewport(slot, *viewport);
                self.embed.set_visible(slot, true);
                if let Some((entry, export, props)) = self.pending.remove(pane.as_str()) {
                    let spec = MountSpec { entry, export: export.into(), slot, key: RootKey::from(pane.as_str()), props, owner: RootOwner::InProcess };
                    self.roots.insert(pane.as_str().to_owned(), self.embed.mount(spec)?);
                }
            }
            berth::PaneEvent::Hidden { .. } => self.embed.set_visible(slot, false),
            berth::PaneEvent::MovedToViewport { to, .. } => self.embed.set_viewport(slot, *to),
            berth::PaneEvent::Closed { pane, .. } => {
                self.pending.remove(pane.as_str());
                self.roots.remove(pane.as_str());
                self.embed.destroy_slot(slot);
            }
            _ => {} // berth's PaneEvent is #[non_exhaustive]
        }
        Ok(())
    }

    /// berth's draw callback for a pane body. It takes only the pane id, so a deferred
    /// viewport's `Send + Sync` closure can call it.
    pub fn pane_ui(ui: &mut egui::Ui, pane: &str) -> bool {
        with_current(|e| e.slot(pane).map(|s| { e.show_slot(ui, s); }).is_some()).unwrap_or(false)
    }
}
```

svelters ships this same adapter as `svelters_egui::harness::dock::SvelteDock`, but only behind the
`kittest` feature, because it is the shape the tests drive. Copy it as above. (Moving it out of the test
feature is DOCS-PLAN open question Q7.)

## Rust: the frame

<!-- snippet: examples/egui-docs/src/dock_pane.rs#frame -->
```rust
fn logic(&mut self, ctx: &egui::Context) {
    for ev in self.dock.take_events() { self.panes.on_event(&ev)?; }
    self.panes.embed.logic(ctx);                     // all JS for every panel runs here
    for ev in self.panes.embed.take_events() {
        match ev {
            EmbedEvent::InfoChanged(root) => {        // JS wrote root().title or root().dirty
                let info = self.panes.embed.root_info(root).unwrap();
                self.dock.set_tab(info.key.as_str(), &info.title, info.dirty);   // berth side: illustrative
            }
            EmbedEvent::CloseDecided { root, allow: true } => {
                let info = self.panes.embed.root_info(root).unwrap();
                self.dock.close(info.key.as_str());   // berth's DockCommand::Close; it answers with Closed
            }
            EmbedEvent::Faulted { root, error } => tracing::warn!(?root, %error, "panel faulted"),
            _ => {}
        }
    }
}
// berth's PaneHost::viewport_done(ui, window): svelters_egui::embed::show_viewport_passes(ui);
```

Build the `Embed` with `EmbedConfig { prepass_plugin: false, .. }` when your app calls `logic` itself
(cadatomic does). Input in a deferred window then costs one root frame, and `logic` routes the
repaint back to that window.

## Svelte: the panel

<!-- snippet: docs/egui/cases/recipe-dock-pane.svelte -->
```svelte
<script lang="ts">
  import { root } from 'svelters:embed';

  let { partName }: { partName: string } = $props();
  let notes = $state('');
  let saved = $state('');
  const r = root();

  $effect(() => {
    r.title = partName;
    r.dirty = notes !== saved;
  });
  r.onCloseRequest(() => notes === saved);   // false keeps the pane open
</script>

<box gap={4} padding={4} flex-grow={1}>
  <text-edit multiline bind:text={notes} hint-text="Notes" flex-grow={1} />
  <button default onclicked={() => (saved = notes)}>Save</button>
</box>
```

- Call `root()` during component initialisation, not in a handler.
- `r.visible` turns false while the pane is hidden. Animation frames already pause on their own.
  Read it only to skip work of your own, such as polling.
- A closed pane loses its Svelte state. For state that must outlive the pane, bind to a Rust store or
  to `uiState()` from `svelters:settings`.

## Faults

A render error, an effect error, a handler that runs longer than the 50 ms interrupt budget, or 5 handler
exceptions within 10 s fault **that root only**. Its slot draws the default fault UI, with the key, the
message and a Reload button, and the other panes keep running. To draw your own, pass
`SlotOptions { fault_ui: FaultUi::Custom(..), .. }`. To restart from Rust, call
`embed.restart_root(root)`. Change the budgets and the policy in `EmbedConfig::{budgets, faults}`.

## Check it

- `docs/egui/cases/recipe-dock-pane.svelte` runs in CI. It types into the notes and expects `dirty`
  in the root info. Saving clears `dirty`, and a close with unsaved notes is refused.
- svelters' own test of this shape is `crates/svelters-egui/tests/viewports.rs`, which covers a pane moved
  to a deferred window, focus restore and the per-window repaint.

## Mistakes we have seen

- **Mounting at open time.** Every pane in a restored layout mounts at startup, even panes that are
  never shown. Mount on the first `Shown`.
- **Keys from indexes** (`pane-3`). Reordering panes swaps their persisted UI state and their AccessKit ids.
- **Not forwarding `Hidden`/`MovedToViewport`.** `embed.take_warnings()` reports each case once:
  - `SlotWarning::NotDrawn` means a visible slot went undrawn for 60 passes;
  - `SlotWarning::WrongViewport` means it was drawn in a window other than the one set, so its repaints go
    to the wrong window;
  - `SlotWarning::DrawnWhileHidden` means `show_slot` was called on a hidden slot.

  Log `take_warnings()` in debug builds.
- **Capturing `&Embed` in a deferred viewport's closure.** That closure must be `Send + Sync`. Reach the
  embed through `with_current` (as `pane_ui` does) or a `SlotPainter` from `embed.painter(slot)`.
