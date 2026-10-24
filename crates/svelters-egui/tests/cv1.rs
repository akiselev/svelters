//! CV1 (plans/milestones/CV1-canvas-seams.md): the seams cadatomic's ECAD canvases need. SV49: a
//! pane the host paints answers hit tests with its own parts; SV50: `pointermove` for Svelte roots;
//! SV53: repeated `anchor`s in a keyed `{#each}` get one egui id per row. The app is written to a
//! scratch dir and loaded from source (`svelters-modules`), so it needs `pnpm install` but no
//! built bundle.

use egui_kittest::kittest::{NodeT, Queryable};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use svelters_egui::embed::{Embed, EmbedConfig, EntryId, MountSpec, Props, RootId, SlotOptions};
use svelters_egui::events::{EventKind, FilterScope, Flow, SceneEvent};
use svelters_egui::scene::{Hit, HitCx, PaneHit, SceneRef, Target, VNodeId};
use svelters_egui::harness::EmbedHarness;
use svelters_host::HostConfig;
use svelters_host::modules::{SourceConfig, SourceProvider};

/// A scratch app with the given components (`(file name, source)`), exported by file stem.
fn scratch(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/embed-egui").canonicalize().unwrap();
    assert!(fixture.join("node_modules").exists(), "run `pnpm install` first");
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/cv1").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    let embed: Vec<String> = files.iter().map(|(f, _)| format!("\"{}\": \"src/{f}\"", f.trim_end_matches(".svelte"))).collect();
    std::fs::write(
        dir.join("package.json"),
        format!(r#"{{ "name": "cv1", "private": true, "type": "module", "svelters": {{ "toolkit": "egui", "embed": {{ {} }} }} }}"#, embed.join(", ")),
    )
    .unwrap();
    for (f, src) in files {
        std::fs::write(dir.join("src").join(f), src).unwrap();
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(fixture.join("node_modules"), dir.join("node_modules")).unwrap();
    dir.canonicalize().unwrap()
}

fn rig(dir: &Path, salt: &'static str) -> (EmbedHarness, EntryId) {
    let provider = Rc::new(SourceProvider::new(SourceConfig::for_path(dir).unwrap()).unwrap());
    let ecfg = EmbedConfig {
        id_salt: salt,
        host: HostConfig { echo_console: false, echo_errors: false, quit_on_last_window: false, ..HostConfig::default() },
        // 1,000 rows in a debug build: more than a handler's budget
        budgets: svelters_egui::embed::Budgets { interrupt: std::time::Duration::from_secs(30), mount: std::time::Duration::from_secs(30), ..Default::default() },
        ..EmbedConfig::default()
    };
    let eh = EmbedHarness::new(egui::vec2(800.0, 600.0), ecfg).unwrap();
    let entry = eh.embed.load_from(provider).unwrap();
    (eh, entry)
}

/// Mounts `export` into a slot filling the host's ui.
fn mount_one(eh: &mut EmbedHarness, entry: EntryId, export: &'static str, key: &str, props: Props) -> RootId {
    let e = eh.embed.clone();
    let slot = e.create_slot("main", SlotOptions::fill());
    eh.set_ui(move |e, ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            e.show_slot(ui, slot);
        });
    });
    let r = e.mount(MountSpec::new(entry, export, slot, key).props(props)).unwrap();
    eh.run();
    r
}

/// The egui ids of the nodes with `anchor` in the root keyed `key`, with their item keys.
fn anchored(e: &Embed, key: &str, anchor: &str) -> Vec<(egui::Id, Option<String>)> {
    e.backend().scene().read(|s| {
        let r = s.root_by_key(key).expect("root");
        s.all_by_anchor(r, anchor).map(|n| (n.egui_id(), n.item_key().map(str::to_string))).collect()
    })
}

const ROWS: &str = r#"<script>
  let { rows = [], groups = [] } = $props();
</script>

<box>
  {#each rows as r (r)}
    <box flex-direction="row"><button anchor="row">Row {r}</button></box>
  {/each}
  {#each groups as g (g.id)}
    <box anchor="group">
      {#each g.cells as c (c)}
        <button anchor="cell">Cell {g.id}.{c}</button>
      {/each}
    </box>
  {/each}
  <button anchor="single">Single</button>
</box>
"#;

#[test]
fn sv53_repeated_anchors_in_a_keyed_each_get_one_id_per_row() {
    let dir = scratch("sv53", &[("Rows.svelte", ROWS)]);
    let (mut eh, entry) = rig(&dir, "cv1");
    let e = eh.embed.clone();
    let keys: Vec<String> = (0..1000).map(|i| format!("k{i}")).collect();
    let root = mount_one(&mut eh, entry, "Rows", "rows", Props::new().with("rows", keys.clone()));
    let errs = e.host().take_errors();
    assert!(errs.is_empty(), "{errs:?} {:?}", e.take_events());
    let n = anchored(&e, "rows", "row").len();
    assert!(eh.harness.query_by_label("Row k0").is_some(), "the rows are drawn ({n} rows; {:?})", e.take_events());

    // 1,000 rows with the same anchor: 1,000 ids (kittest would have panicked on a clash already)
    let rows = anchored(&e, "rows", "row");
    assert_eq!(rows.len(), 1000);
    assert_eq!(rows.iter().map(|r| r.0).collect::<HashSet<_>>().len(), 1000, "one id per row");
    // selectors address a row by anchor plus key
    let k42 = rows.iter().find(|r| r.1.as_deref() == Some("k42")).expect("the row of key k42").0;
    // an anchor outside every keyed each keeps the plain derivation (stable across runs)
    let single = e.anchor_id(root, "single").unwrap();

    // a keyed reorder keeps each row's id (the key follows the item)
    let mut rev = keys.clone();
    rev.reverse();
    e.set_root_props(root, Props::new().with("rows", rev)).unwrap();
    eh.run();
    let after = anchored(&e, "rows", "row");
    assert_eq!(after.iter().find(|r| r.1.as_deref() == Some("k42")).unwrap().0, k42, "a keyed reorder keeps the id");

    // nested keyed eaches: a repeated anchor inside a repeated anchor is scoped by both keys
    let groups = svelters_egui::embed::json_value(serde_json::json!([
        { "id": "a", "cells": [1, 2] },
        { "id": "b", "cells": [1, 2] },
    ]));
    e.set_root_props(root, Props::new().with("rows", Vec::<String>::new()).with("groups", svelters_egui::embed::PropValue(groups))).unwrap();
    eh.run();
    assert!(e.host().take_errors().is_empty());
    let cells = anchored(&e, "rows", "cell");
    assert_eq!(cells.len(), 4);
    assert_eq!(cells.iter().map(|r| r.0).collect::<HashSet<_>>().len(), 4, "cell 1 of group a is not cell 1 of group b");
    assert_eq!(e.anchor_id(root, "single"), Some(single));

    // the same app in a fresh embed (a new run): the same ids
    let (mut eh2, entry2) = rig(&dir, "cv1");
    let keys2: Vec<String> = (40..50).map(|i| format!("k{i}")).collect();
    let _ = mount_one(&mut eh2, entry2, "Rows", "rows", Props::new().with("rows", keys2));
    let again = anchored(&eh2.embed, "rows", "row");
    assert_eq!(again.iter().find(|r| r.1.as_deref() == Some("k42")).unwrap().0, k42, "scoped ids are stable across runs");
}

/// Two pads on a canvas: part 1 at (10, 10)-(60, 60), part 2 at (100, 10)-(150, 60), pane-local.
#[derive(Default)]
struct Pads {
    asked: usize,
}

impl Pads {
    const PADS: [(u64, egui::Rect); 2] = [
        (1, egui::Rect { min: egui::pos2(10.0, 10.0), max: egui::pos2(60.0, 60.0) }),
        (2, egui::Rect { min: egui::pos2(100.0, 10.0), max: egui::pos2(150.0, 60.0) }),
    ];
}

impl PaneHit for Pads {
    fn hit_test(&self, _cx: &HitCx, local: egui::Pos2) -> Hit {
        Pads::PADS.iter().find(|(_, r)| r.contains(local)).map_or(Hit::Miss, |(k, _)| Hit::Part(*k))
    }
    fn part_rect(&self, key: u64) -> Option<egui::Rect> {
        Pads::PADS.iter().find(|(k, _)| *k == key).map(|(_, r)| *r)
    }
    fn part_anchor(&self, key: u64) -> Option<String> {
        Some(format!("pad-{key}"))
    }
}

#[test]
fn sv49_a_host_painted_pane_answers_hit_tests_with_its_own_parts() {
    let dir = scratch("sv49", &[("Rows.svelte", ROWS)]);
    let (mut eh, _entry) = rig(&dir, "cv1");
    let e = eh.embed.clone();
    let pane = e.create_pane("board", Pads::default());
    let id = pane.id();
    let origin = egui::pos2(40.0, 40.0);
    eh.set_ui(move |e, ui| {
        // the host paints its canvas, then records it for hit tests
        let rect = egui::Rect::from_min_size(origin, egui::vec2(300.0, 200.0));
        ui.painter().rect_filled(rect, 0.0, egui::Color32::DARK_GRAY);
        e.show_pane(ui, id, rect);
    });
    eh.run();
    let vp = egui::ViewportId::ROOT;
    let on_pad1 = origin + egui::vec2(20.0, 20.0);
    let part = |key| Target::Virtual(VNodeId { host: pane.node(), key });
    e.backend().scene().read(|s| {
        assert_eq!(s.hit_test(vp, on_pad1), vec![part(1)]);
        assert_eq!(s.hit_test(vp, origin + egui::vec2(120.0, 30.0)), vec![part(2)]);
        assert_eq!(s.hit_test(vp, origin + egui::vec2(80.0, 30.0)), vec![], "between the pads: a miss");
        assert_eq!(s.hit_test(vp, egui::pos2(10.0, 10.0)), vec![], "outside the pane");
        assert_eq!(s.event_target(vp, on_pad1), Some(part(1)));
        // overlays and automation see the part like a provider's
        assert_eq!(s.target_rect(part(2)), Some((vp, egui::Rect::from_min_size(origin + egui::vec2(100.0, 10.0), egui::vec2(50.0, 50.0)))));
        assert_eq!(s.target_anchor(part(1)).as_deref(), Some("board/pad-1"));
    });

    // event filters on the pane see the part as the target, with the point in the part's rect
    let seen: Rc<std::cell::RefCell<Vec<(EventKind, Target, Option<egui::Pos2>)>>> = Rc::default();
    {
        let seen = seen.clone();
        let _ = e.register_filter(FilterScope::Subtree(pane.node()), move |ev: &SceneEvent, _s: SceneRef<'_>| {
            seen.borrow_mut().push((ev.kind, ev.target, ev.local));
            Flow::Pass
        });
    }
    eh.harness.hover_at(on_pad1);
    for pressed in [true, false] {
        eh.harness.event(egui::Event::PointerButton { pos: on_pad1, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
    }
    eh.run();
    let got = seen.borrow().clone();
    assert!(got.contains(&(EventKind::PointerDown, part(1), Some(egui::pos2(10.0, 10.0)))), "{got:?}");
    assert!(got.iter().any(|g| g.0 == EventKind::Click && g.1 == part(1)), "{got:?}");

    // the tester's state is the host's: a moved pad answers at once
    pane.with_mut(|p| p.asked += 1);
    e.destroy_pane(pane.id());
    eh.run();
    e.backend().scene().read(|s| assert_eq!(s.hit_test(vp, on_pad1), vec![]));
}

const MOVES: &str = r#"<script>
  let n = $state(0);
  let last = $state('');
  function mv(e) {
    n++;
    last = `${Math.round(e.offsetX)},${Math.round(e.offsetY)} b${e.button} s${e.buttons} p${e.eventPhase}`;
  }
</script>

<box anchor="pad" padding={30} onpointermove={mv}>
  <label>Moves {n}: {last}</label>
</box>
"#;

#[test]
fn sv50_pointermove_reaches_svelte_once_per_frame_with_local_coordinates() {
    let dir = scratch("sv50", &[("Moves.svelte", MOVES)]);
    let (mut eh, entry) = rig(&dir, "cv1");
    let e = eh.embed.clone();
    let root = mount_one(&mut eh, entry, "Moves", "moves", Props::new());
    let (_, pad) = e.anchor_rect(root, "pad").expect("the box is drawn");
    // three moves in one frame: one pointermove, at the last point (the box's padding: the box is
    // the target, so the offset is from its top left)
    for d in [2.0, 4.0, 6.0] {
        eh.harness.input_mut().events.push(egui::Event::PointerMoved(pad.min + egui::vec2(d, d)));
    }
    eh.run();
    let errs = e.host().take_errors();
    assert!(errs.is_empty(), "{errs:?}");
    assert!(eh.harness.query_by_label("Moves 1: 6,6 b-1 s0 p2").is_some(), "one move per frame, local to the target");
    // with the primary button held: `buttons` 1
    eh.harness.input_mut().events.push(egui::Event::PointerButton { pos: pad.min + egui::vec2(6.0, 6.0), button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
    eh.harness.input_mut().events.push(egui::Event::PointerMoved(pad.min + egui::vec2(8.0, 9.0)));
    eh.run();
    assert!(eh.harness.query_by_label("Moves 2: 8,9 b-1 s1 p2").is_some());
    // over the label it bubbles to the box (phase 3), the offset from the label's rect
    let label = eh.harness.get_by_label_contains("Moves 2").rect();
    eh.harness.input_mut().events.push(egui::Event::PointerButton { pos: pad.min + egui::vec2(8.0, 9.0), button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
    eh.harness.input_mut().events.push(egui::Event::PointerMoved(label.min + egui::vec2(3.0, 2.0)));
    eh.run();
    assert!(eh.harness.query_by_label("Moves 3: 3,2 b-1 s0 p3").is_some(), "{:?}", eh.harness.get_by_label_contains("Moves").accesskit_node().label());
}
