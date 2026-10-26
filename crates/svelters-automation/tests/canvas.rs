//! CV1 (SV51): agents drive canvases end to end: `input.drag` (press, moves, release, with steps),
//! `input.pointer_move` and `input.wheel` at points in a node's local coordinates, with modifiers,
//! on a Svelte box listening to `pointer*`/`wheel` (SV50) and on a pane the host paints (SV49).
//! Deterministic: two fresh runs of the same acts give the same frames and the same log. SV53:
//! `key=` selects a keyed each item's rows.

use serde_json::json;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use svelters_automation::{ActOptions, At, Drag, Session, Wheel};
use svelters_egui::embed::{EmbedConfig, MountSpec, PaneId, Props, SlotOptions};
use svelters_egui::events::{EventKind, FilterScope, Flow, SceneEvent};
use svelters_egui::harness::EmbedHarness;
use svelters_egui::scene::{Hit, HitCx, PaneHit, SceneRef, Target};
use svelters_host::HostConfig;
use svelters_host::modules::{SourceConfig, SourceProvider};

const PAD: &str = r#"<script>
  let log = $state([]);
  let at = $state('');
  const add = (s) => (log = [...log, s].slice(-6));
  const p = (e) => `${Math.round(e.offsetX)},${Math.round(e.offsetY)}`;
</script>

<box anchor="pad" width={300} height={200}
  onpointerdown={(e) => add(`d${p(e)}${e.shiftKey ? 'S' : ''}`)}
  onpointermove={(e) => { at = p(e); if (e.buttons) add(`m${p(e)}`); }}
  onpointerup={(e) => add(`u${p(e)}`)}
  onwheel={(e) => add(`w${e.deltaY > 0 ? '+' : '-'}${e.ctrlKey ? 'C' : ''}`)}>
  <label>Log {log.join(' ')}</label>
  <label>At {at}</label>
</box>
{#each ['a', 'b'] as k (k)}
  <button anchor="row" onclick={() => add(`row ${k}`)}>Row {k}</button>
{/each}
"#;

fn scratch(name: &str) -> PathBuf {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/embed-egui").canonicalize().unwrap();
    assert!(fixture.join("node_modules").exists(), "run `pnpm install` first");
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/cv1-au").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("package.json"),
        r#"{ "name": "cv1au", "private": true, "type": "module", "svelters": { "toolkit": "egui", "embed": { "Pad": "src/Pad.svelte" } } }"#,
    )
    .unwrap();
    std::fs::write(dir.join("src/Pad.svelte"), PAD).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(fixture.join("node_modules"), dir.join("node_modules")).unwrap();
    dir.canonicalize().unwrap()
}

/// A canvas pane under the Svelte root: one part (key 7) at (20, 20)-(80, 80), pane-local.
struct Board;

impl PaneHit for Board {
    fn hit_test(&self, _cx: &HitCx, local: egui::Pos2) -> Hit {
        if egui::Rect::from_min_max(egui::pos2(20.0, 20.0), egui::pos2(80.0, 80.0)).contains(local) { Hit::Part(7) } else { Hit::Node }
    }
}

type Seen = Rc<RefCell<Vec<(EventKind, Target, Option<egui::Pos2>, bool)>>>;

/// The pad mounted in the top half, a host-painted board pane in the bottom half, and a filter
/// recording the board's events.
fn app(name: &str) -> (Session<EmbedHarness>, PaneId, Seen) {
    let dir = scratch(name);
    let ecfg = EmbedConfig {
        id_salt: "cv1",
        host: HostConfig { echo_console: false, echo_errors: false, quit_on_last_window: false, ..HostConfig::default() },
        ..EmbedConfig::default()
    };
    let mut eh = EmbedHarness::new(egui::vec2(400.0, 600.0), ecfg).unwrap();
    let e = eh.embed.clone();
    let entry = e.load_from(Rc::new(SourceProvider::new(SourceConfig::for_path(&dir).unwrap()).unwrap())).unwrap();
    let slot = e.create_slot("pad", SlotOptions::fill());
    let pane = e.create_pane("board", Board).id();
    eh.set_ui(move |e, ui| {
        let full = ui.available_rect_before_wrap();
        let (top, bottom) = full.split_top_bottom_at_y(full.top() + 300.0);
        let mut t = ui.new_child(egui::UiBuilder::new().max_rect(top));
        e.show_slot(&mut t, slot);
        ui.painter().rect_filled(bottom, 0.0, egui::Color32::DARK_GRAY);
        e.show_pane(ui, pane, bottom);
    });
    e.mount(MountSpec::new(entry, "Pad", slot, "pad").props(Props::new())).unwrap();
    let seen: Seen = Rc::default();
    {
        let seen = seen.clone();
        let _ = e.register_filter(FilterScope::Subtree(pane.node()), move |ev: &SceneEvent, _s: SceneRef<'_>| {
            seen.borrow_mut().push((ev.kind, ev.target, ev.local, ev.modifiers.shift));
            Flow::Pass
        });
    }
    let mut s = Session::for_test(eh);
    s.wait_settled(10_000.0).unwrap();
    (s, pane, seen)
}

fn label(s: &Session<EmbedHarness>, prefix: &str) -> String {
    let found = s.query(&format!("text={prefix}"), None).unwrap();
    found.first().map(|n| n.text.clone()).unwrap_or_default()
}

/// The acts of one run; returns the frames each ended at and the pad's log.
fn run(name: &str) -> (Vec<u64>, String, Vec<(EventKind, Target, Option<egui::Pos2>, bool)>) {
    let (mut s, pane, seen) = app(name);
    let o = ActOptions::default();
    let mut frames = Vec::new();
    // a drag in the pad's local points, with Shift: press, 4 moves, release
    let r = s.drag("anchor=pad", Drag::to(egui::pos2(150.0, 140.0)).from(At::Local(svelters_automation::pos2(50.0, 100.0))).steps(4).modifiers(egui::Modifiers::SHIFT), o).unwrap();
    frames.push(r.frame);
    let log = label(&s, "Log");
    assert_eq!(log, "Log d50,100S m75,110 m100,120 m125,130 m150,140 u150,140", "press, one move per step, release");
    // a move without buttons, through the JSON path
    let r = s.call("input.pointer_move", &json!({"selector": "anchor=pad", "at": [12, 150]})).unwrap();
    frames.push(r["frame"].as_u64().unwrap());
    assert_eq!(label(&s, "At"), "At 12,150");
    // two wheel notches up with Ctrl (DOM: deltaY < 0), then pixels down
    let r = s.wheel("anchor=pad", Wheel::lines(egui::vec2(0.0, 2.0)).at(At::Local(svelters_automation::pos2(60.0, 150.0))).modifiers(egui::Modifiers::CTRL), o).unwrap();
    frames.push(r.frame);
    let r = s.call("input.wheel", &json!({"selector": "anchor=pad", "dy": -30, "unit": "pixel", "at": [60, 150]})).unwrap();
    frames.push(r["frame"].as_u64().unwrap());
    let log = label(&s, "Log");
    assert!(log.ends_with("w-C w+"), "{log}");
    // the board pane: a drag onto its part, addressed by the pane's key
    let r = s.call("input.drag", &json!({"selector": "anchor=board", "from": [10, 10], "to": [40, 50], "steps": 2})).unwrap();
    frames.push(r["frame"].as_u64().unwrap());
    let seen = seen.borrow().clone();
    let _ = pane;
    // SV53: rows by anchor plus key
    let rows = s.query("anchor=row", None).unwrap();
    assert_eq!(rows.len(), 2);
    let b = s.query("key=b >> anchor=row", None).unwrap();
    assert_eq!(b.iter().map(|n| n.text.as_str()).collect::<Vec<_>>(), ["Row b"]);
    s.click("key=b >> anchor=row", o).unwrap();
    assert!(label(&s, "Log").ends_with("row b"));
    (frames, log, seen)
}

#[test]
fn sv51_drag_pointer_move_and_wheel_drive_a_canvas_deterministically() {
    let (frames, log, seen) = run("a");
    // the board: pressed on the pane itself, released on part 7 (pane-local (40, 50) is in it)
    let downs: Vec<_> = seen.iter().filter(|e| e.0 == EventKind::PointerDown).collect();
    assert_eq!(downs.len(), 1, "{seen:?}");
    assert!(matches!(downs[0].1, Target::Node(_)), "{seen:?}");
    let up = seen.iter().find(|e| e.0 == EventKind::PointerUp).expect("a release on the board");
    assert!(matches!(up.1, Target::Virtual(v) if v.key == 7), "{seen:?}");
    // `Board` names no part rects: the offset is from the pane's top left
    assert_eq!(up.2, Some(egui::pos2(40.0, 50.0)));
    // a fresh run of the same acts: the same frames, the same log
    let (frames2, log2, _) = run("b");
    assert_eq!((frames, log), (frames2, log2), "deterministic under the virtual clock");
}
