//! EM1 (plans/research/ARCH-EMBED.md §10): many Svelte roots in one runtime, embedded in a host's own
//! egui ui. The bundle is `fixtures/embed-egui/dist/embed.js` (`svelteConfig({ embed })`, `pnpm build`).
//! Headless kittest: no display, no GPU.

use egui_kittest::kittest::{NodeT, Queryable};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use svelters_abi::Value;
use svelters_egui::embed::{
    Busy, Embed, EmbedConfig, EmbedEvent, EntryId, FaultCause, MountSpec, Props, RootId, RootState, SlotId, SlotOptions,
};
use svelters_egui::harness::EmbedHarness;
use svelters_host::HostConfig;

fn bundle() -> PathBuf {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/embed-egui/dist/embed.js");
    assert!(p.exists(), "{} is missing: run `pnpm build` (needs `cargo build -p svelters-compiler` first)", p.display());
    p
}

fn cfg(salt: &'static str) -> EmbedConfig {
    EmbedConfig {
        id_salt: salt,
        host: HostConfig { echo_console: false, echo_errors: false, quit_on_last_window: false, ..HostConfig::default() },
        ..EmbedConfig::default()
    }
}

fn rig() -> (EmbedHarness, EntryId) {
    let eh = EmbedHarness::new(egui::vec2(800.0, 600.0), cfg("svelters")).unwrap();
    let entry = eh.embed.load(bundle()).unwrap();
    (eh, entry)
}

fn has(eh: &EmbedHarness, label: &str) -> bool {
    eh.harness.query_by_label(label).is_some()
}

fn mount(e: &Embed, entry: EntryId, export: &'static str, slot: SlotId, key: &str, props: Props) -> RootId {
    e.mount(MountSpec::new(entry, export, slot, key).props(props)).unwrap()
}

/// Draws `slots` side by side, as panels of the host app.
fn columns(eh: &mut EmbedHarness, slots: Vec<SlotId>) {
    eh.set_ui(move |e, ui| {
        let n = slots.len();
        ui.columns(n, |cols| {
            for (i, s) in slots.iter().enumerate() {
                e.show_slot(&mut cols[i], *s);
            }
        });
    });
}

fn faults(events: &[EmbedEvent]) -> Vec<(RootId, FaultCause, String)> {
    events
        .iter()
        .filter_map(|e| match e {
            EmbedEvent::Faulted { root, error } => Some((*root, error.cause, error.error.message.clone())),
            _ => None,
        })
        .collect()
}

#[test]
fn two_roots_in_one_runtime_update_reactively_and_unmount_cleanly() {
    let (mut eh, entry) = rig();
    let e = eh.embed.clone();
    let left = e.create_slot("left", SlotOptions::fill().label("Left pane"));
    let right = e.create_slot("right", SlotOptions::fill());
    columns(&mut eh, vec![left, right]);
    eh.run();
    let arena0 = e.backend().with_scene(|s| s.len());
    let callbacks0 = e.host().live_callbacks();
    assert_eq!(arena0, 2, "two empty slots");

    let a = mount(&e, entry, "Panel", left, "panel.left", Props::new().with("title", "Left").with("n", 1));
    let b = mount(&e, entry, "Panel", right, "panel.right", Props::new().with("title", "Right").with("n", 2));
    eh.run();
    assert!(has(&eh, "Left: 1") && has(&eh, "Right: 2"), "both roots render in their panels");
    let ev = e.take_events();
    assert!(ev.iter().any(|x| matches!(x, EmbedEvent::Mounted(r) if *r == a)));
    assert!(ev.iter().any(|x| matches!(x, EmbedEvent::Mounted(r) if *r == b)));
    assert!(faults(&ev).is_empty(), "{ev:?}");
    // one panel left, one right
    let (l, r) = (eh.harness.get_by_label("Left: 1").rect(), eh.harness.get_by_label("Right: 2").rect());
    assert!(l.max.x <= r.min.x, "left {l:?} right {r:?}");

    // local state is per root
    eh.harness.get_by_label("Inc Left").click();
    eh.run();
    assert!(has(&eh, "Left clicks: 1") && has(&eh, "Right clicks: 0"));
    let ev = e.take_events();
    assert!(
        ev.iter().any(|x| matches!(x, EmbedEvent::Emitted { root, name, value: Value::Map(m) }
            if *root == a && name == "clicked" && m.iter().any(|(k, v)| k == "clicks" && matches!(v, Value::I64(1))))),
        "{ev:?}"
    );
    assert!(ev.iter().any(|x| matches!(x, EmbedEvent::InfoChanged(r) if *r == a)));
    assert_eq!(e.root_info(a).unwrap().title, "Left (1)", "root().title reaches the host");
    assert_eq!(e.root_info(b).unwrap().title, "Right (0)");

    // props are reactive (a shallow merge)
    e.set_root_props(a, Props::new().with("n", 5)).unwrap();
    eh.run();
    assert!(has(&eh, "Left: 5") && has(&eh, "Left clicks: 1"), "a props patch keeps local state");
    assert!(has(&eh, "Right: 2"));

    // a close request with no onCloseRequest handler is allowed
    e.request_close(b);
    eh.run();
    assert!(e.take_events().iter().any(|x| matches!(x, EmbedEvent::CloseDecided { root, allow: true } if *root == b)));

    // unmount frees every handle and callback
    e.unmount(a);
    eh.run();
    assert!(!has(&eh, "Left: 5") && has(&eh, "Right: 2"));
    e.unmount(b);
    eh.run();
    e.host().collect_garbage();
    eh.run();
    assert_eq!(e.backend().with_scene(|s| s.len()), arena0, "only the slots are left");
    assert_eq!(e.host().live_callbacks(), callbacks0);
    assert!(e.roots().is_empty());

    // the slot takes a new root
    let c = mount(&e, entry, "Panel", left, "panel.left", Props::new().with("title", "Again"));
    eh.run();
    assert!(has(&eh, "Again: 0"));
    assert!(matches!(e.root_info(c).unwrap().state, RootState::Live));
}

#[test]
fn ids_are_stable_across_a_keyed_reorder_a_second_root_and_a_second_embed() {
    let (mut eh, entry) = rig();
    let e = eh.embed.clone();
    let s1 = e.create_slot("one", SlotOptions::fill());
    let s2 = e.create_slot("two", SlotOptions::fill());
    // a second embed with another salt in the same Context
    let e2 = Embed::new(&eh.harness.ctx, cfg("other")).unwrap();
    let entry2 = e2.load(bundle()).unwrap();
    let s3 = e2.create_slot("one", SlotOptions::fill());
    let shown = std::rc::Rc::new(std::cell::Cell::new(false));
    {
        let (e2, shown) = (e2.clone(), shown.clone());
        eh.set_ui(move |e, ui| {
            e2.logic(ui.ctx());
            ui.columns(3, |cols| {
                e.show_slot(&mut cols[0], s1);
                e.show_slot(&mut cols[1], s2);
                if shown.get() {
                    e2.show_slot(&mut cols[2], s3);
                }
            });
        });
    }
    let r1 = mount(&e, entry, "Panel", s1, "panel.one", Props::new().with("title", "A").with("items", vec!["x", "y", "z"]));
    eh.run();
    let node = |eh: &EmbedHarness, l: &str| eh.harness.get_by_label(l).accesskit_node().id();
    let ids = |eh: &EmbedHarness| ["A: 0", "A item y", "Inc A", "A clicks: 0"].map(|l| node(eh, l));
    let before = ids(&eh);
    let anchor_y = e.anchor_id(r1, "item-y").unwrap();
    assert_ne!(e.anchor_id(r1, "item-x").unwrap(), anchor_y);

    // keyed reorder: the same nodes move
    e.set_root_props(r1, Props::new().with("items", vec!["z", "y", "x"])).unwrap();
    eh.run();
    assert_eq!(ids(&eh), before, "a keyed reorder keeps every id");
    assert_eq!(e.anchor_id(r1, "item-y"), Some(anchor_y));

    // another root mounting
    let _r2 = mount(&e, entry, "Panel", s2, "panel.two", Props::new().with("title", "B").with("items", vec!["y"]));
    eh.run();
    assert!(has(&eh, "B item y"));
    assert_eq!(ids(&eh), before, "a second root keeps the first root's ids");
    let b_y = node(&eh, "B item y");
    assert_ne!(b_y, before[1], "same anchor, another root: another id");

    // a second embed with another salt, same root key and anchors
    let r3 = mount(&e2, entry2, "Panel", s3, "panel.one", Props::new().with("title", "C").with("items", vec!["y"]));
    shown.set(true);
    eh.run();
    assert!(has(&eh, "C item y"), "{:?}", e2.take_events());
    assert_eq!(ids(&eh), before, "a second embed keeps the first embed's ids");
    assert_ne!(e2.anchor_id(r3, "item-y"), Some(anchor_y), "the salt separates the embeds");
    assert!(![b_y, before[1]].contains(&node(&eh, "C item y")));

    // anchored ids are the root base + anchor: the same in a fresh embed (a new run)
    let (mut eh2, entry3) = rig();
    let f = eh2.embed.clone();
    let s = f.create_slot("one", SlotOptions::fill());
    columns(&mut eh2, vec![s]);
    // other handles first, so handle numbers differ from the first run
    let _pad = f.create_slot("pad", SlotOptions::fill());
    let r = mount(&f, entry3, "Panel", s, "panel.one", Props::new().with("title", "A").with("items", vec!["y"]));
    eh2.run();
    assert_eq!(f.anchor_id(r, "item-y"), Some(anchor_y), "anchored ids are stable across runs");
}

#[test]
fn a_throwing_component_faults_only_its_root() {
    let (mut eh, entry) = rig();
    let e = eh.embed.clone();
    let good = e.create_slot("good", SlotOptions::fill());
    let bad = e.create_slot("bad", SlotOptions::fill());
    let fragile = e.create_slot("fragile", SlotOptions::fill());
    columns(&mut eh, vec![good, bad, fragile]);
    let g = mount(&e, entry, "Panel", good, "good", Props::new().with("title", "Good"));
    let b = mount(&e, entry, "Broken", bad, "bad", Props::new());
    let f = mount(&e, entry, "Fragile", fragile, "fragile", Props::new());
    eh.run();
    let ev = e.take_events();
    let fs = faults(&ev);
    assert_eq!(fs.len(), 1, "{ev:?}");
    assert_eq!((fs[0].0, fs[0].1), (b, FaultCause::Render));
    assert!(fs[0].2.contains("Broken: boom"), "{fs:?}");
    assert!(has(&eh, "bad failed: Broken: boom"), "the slot draws the fault UI");
    assert!(has(&eh, "Good: 0") && has(&eh, "Fragile ok"));
    assert!(e.root_info(b).unwrap().state.is_faulted());

    // the fault UI's Reload mounts it again (with its last props): Broken throws again
    eh.harness.get_by_label("Reload").click();
    eh.run();
    let fs = faults(&e.take_events());
    assert_eq!(fs.len(), 1, "{fs:?}");
    assert_eq!(fs[0].0, b);
    assert_eq!(e.stats().mounts, 4);

    // an error after mount (a template effect) reaches the root's boundary
    eh.harness.get_by_label("Break").click();
    eh.run();
    let ev = e.take_events();
    let fs = faults(&ev);
    assert_eq!(fs.len(), 1, "{ev:?}");
    assert_eq!((fs[0].0, fs[0].1), (f, FaultCause::Render));
    assert!(fs[0].2.contains("template exploded"), "{fs:?}");
    assert!(!has(&eh, "Fragile ok"));
    assert_eq!(e.backend().with_scene(|s| s.n(fragile.handle()).children.len()), 0, "the faulted root's nodes are gone");

    // the other roots are unaffected
    eh.harness.get_by_label("Inc Good").click();
    eh.run();
    assert!(has(&eh, "Good clicks: 1"));
    assert!(matches!(e.root_info(g).unwrap().state, RootState::Live));

    // restart: mounted again
    e.restart_root(f);
    eh.run();
    assert!(has(&eh, "Fragile ok"), "restarted");
    assert!(matches!(e.root_info(f).unwrap().state, RootState::Live));
    assert!(e.root_info(b).unwrap().state.is_faulted());
}

#[test]
fn an_infinite_handler_loop_is_interrupted_within_100ms_and_faults_only_its_root() {
    let (mut eh, entry) = rig();
    let e = eh.embed.clone();
    let spin = e.create_slot("spin", SlotOptions::fill());
    let other = e.create_slot("other", SlotOptions::fill());
    columns(&mut eh, vec![spin, other]);
    let s = mount(&e, entry, "Spinner", spin, "spinner", Props::new());
    let o = mount(&e, entry, "Panel", other, "other", Props::new().with("title", "Other"));
    eh.run();
    assert!(has(&eh, "Spinner ready") && has(&eh, "Other: 0"));
    e.take_events();

    eh.harness.get_by_label("Spin").click();
    let mut fault = None;
    for _ in 0..10 {
        let t = Instant::now();
        eh.step();
        let took = t.elapsed();
        if let Some(f) = faults(&e.take_events()).into_iter().next() {
            fault = Some((f, took, e.stats().last_logic));
            break;
        }
    }
    let ((root, cause, _), step, logic) = fault.expect("the spinning root faulted");
    assert_eq!((root, cause), (s, FaultCause::Interrupt));
    eprintln!("interrupt: logic {logic:?}, frame {step:?}");
    assert!(logic <= Duration::from_millis(100), "interrupted in {logic:?}");
    assert!(logic >= Duration::from_millis(45), "the budget ran ({logic:?})");
    eh.run();
    assert!(has(&eh, "spinner failed: interrupted"));

    // the other root still runs
    eh.harness.get_by_label("Inc Other").click();
    eh.run();
    assert!(has(&eh, "Other clicks: 1"));
    assert!(matches!(e.root_info(o).unwrap().state, RootState::Live));
}

#[test]
fn run_until_settled_converges_and_reports_an_injected_cycle() {
    let (mut eh, entry) = rig();
    let e = eh.embed.clone();
    let a = e.create_slot("a", SlotOptions::fill());
    let t = e.create_slot("t", SlotOptions::fill());
    columns(&mut eh, vec![a, t]);
    eh.run();
    let probe = e.busy_probe();

    mount(&e, entry, "Panel", a, "a", Props::new().with("title", "A").with("items", vec!["1", "2"]));
    assert!(e.settled().busy.contains(Busy::COMMANDS | Busy::MOUNTING));
    let n = eh.run_until_settled(10).expect("settles");
    assert!(n <= 3, "settled in {n} iterations");
    assert!(has(&eh, "A: 0"));
    assert_eq!(probe.count(), 0);

    // a finite rAF chain settles too
    let tk = mount(&e, entry, "Ticker", t, "ticker", Props::new());
    let n = eh.run_until_settled(10).expect("one frame then settled");
    assert!(n <= 4, "{n}");
    assert!(has(&eh, "Ticks started"));

    // an endless animation-frame loop never settles, and the chain says why
    e.unmount(tk);
    eh.run();
    mount(&e, entry, "Ticker", t, "ticker", Props::new().with("cycle", true));
    let err = eh.run_until_settled(10).expect_err("a cycle");
    assert_eq!(err.chain.len(), 10);
    assert!(err.chain[2..].iter().all(|b| b.contains(Busy::FRAME)), "{:?}", err.chain);
    assert!(probe.busy().contains(Busy::FRAME));
}

#[test]
fn idle_is_zero_frames_with_five_slots() {
    let (mut eh, entry) = rig();
    let e = eh.embed.clone();
    let slots: Vec<SlotId> = (0..5).map(|i| e.create_slot(format!("s{i}"), SlotOptions::fill())).collect();
    columns(&mut eh, slots.clone());
    for (i, &s) in slots.iter().enumerate() {
        mount(&e, entry, "Panel", s, &format!("root{i}"), Props::new().with("title", format!("P{i}")).with("items", vec!["a", "b"]));
    }
    eh.run();
    for i in 0..5 {
        assert!(has(&eh, &format!("P{i}: 0")));
    }
    assert!(e.settled().is_settled(), "{:?}", e.settled());
    let delay = eh.harness.output().viewport_output[&egui::ViewportId::ROOT].repaint_delay;
    assert!(delay >= Duration::from_secs(2), "the idle embed asks for a repaint in {delay:?}");

    // a frame the OS forces (a resize, an expose) runs no JS and leaves it idle
    let checkpoints = e.backend().stats().checkpoints;
    let logics = e.stats().logics;
    eh.step();
    assert_eq!(e.stats().logics, logics + 1);
    assert_eq!(e.backend().stats().checkpoints, checkpoints, "no checkpoint while idle");
    let delay = eh.harness.output().viewport_output[&egui::ViewportId::ROOT].repaint_delay;
    assert!(delay >= Duration::from_secs(2), "{delay:?}");

    // hidden slots: not drawn, still mounted
    e.set_visible(slots[0], false);
    eh.run();
    assert!(!has(&eh, "P0: 0") && has(&eh, "P1: 0"));
    e.set_visible(slots[0], true);
    eh.run();
    assert!(has(&eh, "P0 visible: true"));
}
