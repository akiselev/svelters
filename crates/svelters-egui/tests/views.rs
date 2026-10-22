//! WG4a (plans/milestones/WG4a-tier1-views.md): the Tier 1 views under kittest, built from Rust in an
//! embed slot (no JS bundle): `tab-view`, `split-view`, `table` (with a 10k-row scroll and its frame
//! time) and `toast-host`. Each: renders, takes input, and is found by AccessKit role and label.
//! The Svelte side (bind:, events, methods from JS) is `tests/views_svelte.rs`.

use egui::Role;
use egui_kittest::kittest::{NodeT, Queryable};
use std::time::Instant;
use svelters_abi::{Backend, Value};
use svelters_egui::embed::{EmbedConfig, SlotId, SlotOptions};
use svelters_egui::harness::EmbedHarness;
use svelters_egui::kinds;
use svelters_egui::scene::{NodeId, ProviderId, SceneHandle, W};
use svelters_host::HostConfig;

fn cfg() -> EmbedConfig {
    EmbedConfig { host: HostConfig { echo_console: false, echo_errors: false, quit_on_last_window: false, ..HostConfig::default() }, ..EmbedConfig::default() }
}

struct Rig {
    h: EmbedHarness,
    scene: SceneHandle,
    slot: NodeId,
}

const APP: &str = "wg4a.test";

impl Rig {
    fn new(w: f32, h: f32) -> Rig {
        let mut eh = EmbedHarness::new(egui::vec2(w, h), cfg()).unwrap();
        let e = eh.embed.clone();
        let slot: SlotId = e.create_slot("views", SlotOptions::fill());
        eh.set_ui(move |e, ui| {
            e.show_slot(ui, slot);
        });
        let scene = e.backend().scene();
        Rig { h: eh, scene, slot: NodeId::from_raw(slot.handle()).unwrap() }
    }

    /// Creates `tag` under `parent` with `props`.
    fn add(&self, parent: NodeId, tag: &str, props: &[(&str, Value)]) -> NodeId {
        let props: Vec<(String, Value)> = props.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
        let tag = tag.to_string();
        self.scene
            .write(ProviderId::named(APP), move |m| {
                let n = m.create_tag(&tag).unwrap();
                for (k, v) in &props {
                    m.node(n).unwrap().set_by_name(k, v).unwrap_or_else(|e| panic!("{tag}.{k}: {e:?}"));
                }
                m.insert(parent, n, None).unwrap();
                n
            })
            .unwrap()
    }

    fn set(&self, n: NodeId, name: &str, v: Value) {
        let name = name.to_string();
        self.scene.write(ProviderId::named(APP), move |m| m.node(n).unwrap().set_by_name(&name, &v).unwrap()).unwrap();
    }

    fn get(&self, n: NodeId, name: &str) -> Value {
        self.scene.read(|s| s.get(n).unwrap().value_of(name).unwrap())
    }

    /// Calls method `name` of node `n` through the backend, as JS does.
    fn call(&self, n: NodeId, name: &str, args: &[Value]) -> Value {
        let b = self.h.embed.backend();
        let kind = self.scene.read(|s| kinds::tag_index(s.get(n).unwrap().tag()).unwrap());
        let (m, _) = kinds::find_method(kind, name).unwrap();
        b.call(n.to_raw(), m, args).unwrap()
    }

    fn rect(&self, n: NodeId) -> egui::Rect {
        self.scene.read(|s| s.get(n).unwrap().rect())
    }

    fn has(&self, label: &str) -> bool {
        self.h.harness.query_by_label(label).is_some()
    }

    fn run(&mut self) {
        self.h.run();
    }
}

fn s(x: &str) -> Value {
    Value::Str(x.into())
}

#[test]
fn tab_view_shows_the_selected_page_and_switches_on_click() {
    let mut r = Rig::new(400.0, 300.0);
    let tv = r.add(r.slot, "tab-view", &[("flex-grow", Value::F64(1.0))]);
    let t1 = r.add(tv, "tab", &[("title", s("General"))]);
    let t2 = r.add(tv, "tab", &[("title", s("Advanced")), ("closable", Value::Bool(true))]);
    r.add(t1, "label", &[("text", s("Page one"))]);
    r.add(t2, "label", &[("text", s("Page two"))]);
    r.run();
    // the bar: one Tab per page, found by role and label, the first selected
    let general = r.h.harness.get_by_role_and_label(Role::Tab, "General");
    assert!(general.accesskit_node().is_selected().unwrap_or(false), "General is the selected tab");
    assert!(r.h.harness.query_by_role_and_label(Role::Tab, "Advanced").is_some());
    assert!(r.has("Close Advanced"), "a closable tab has a close button");
    assert!(r.has("Page one") && !r.has("Page two"), "only the selected page is drawn");
    assert_eq!(r.rect(t2).size(), egui::Vec2::ZERO, "the hidden page has the zero layout");
    let page = r.rect(t1);
    assert!(page.min.y > r.rect(tv).min.y + 10.0, "the page is under the bar: {page:?}");

    // a click on a tab selects it (a widget write: bind:selected)
    r.h.harness.get_by_role_and_label(Role::Tab, "Advanced").click();
    r.run();
    assert_eq!(r.get(tv, "selected"), Value::I64(1));
    assert!(r.has("Page two") && !r.has("Page one"));
    assert!(r.rect(t2).height() > 100.0, "the second page is laid out now: {:?}", r.rect(t2));

    // a host write switches back
    r.set(tv, "selected", Value::I64(0));
    r.run();
    assert!(r.has("Page one") && !r.has("Page two"));
}

#[test]
fn split_view_places_two_panes_and_the_divider_moves_them() {
    let mut r = Rig::new(400.0, 200.0);
    let sv = r.add(r.slot, "split-view", &[("flex-grow", Value::F64(1.0)), ("accessible-name", s("Panes"))]);
    let a = r.add(sv, "box", &[]);
    let b = r.add(sv, "box", &[]);
    r.add(a, "label", &[("text", s("Left pane"))]);
    r.add(b, "label", &[("text", s("Right pane"))]);
    r.run();
    let (ra, rb, rs) = (r.rect(a), r.rect(b), r.rect(sv));
    // null position: half each, the divider between
    assert!((ra.width() - rb.width()).abs() <= 1.0, "{ra:?} {rb:?}");
    assert!((rb.min.x - ra.max.x - 6.0).abs() < 0.5, "a 6pt divider: {ra:?} {rb:?}");
    assert!((ra.height() - rs.height()).abs() < 0.5);
    assert!(r.h.harness.get_by_label("Left pane").rect().max.x <= r.h.harness.get_by_label("Right pane").rect().min.x);

    // a host write: the first pane is 100pt
    r.set(sv, "divider-position", Value::F64(100.0));
    r.run();
    assert_eq!(r.rect(a).width(), 100.0);

    // the divider, by role and name: drag it 50pt right
    let div = r.h.harness.get_by_role_and_label(Role::Splitter, "Panes").rect();
    let c = div.center();
    r.h.harness.hover_at(c);
    r.h.harness.step();
    r.h.harness.drag_at(c);
    r.h.harness.step();
    r.h.harness.hover_at(c + egui::vec2(25.0, 0.0));
    r.h.harness.step();
    r.h.harness.hover_at(c + egui::vec2(50.0, 0.0));
    r.h.harness.step();
    r.h.harness.drop_at(c + egui::vec2(50.0, 0.0));
    r.run();
    assert_eq!(r.get(sv, "divider-position"), Value::F64(150.0), "the drag wrote divider-position");
    assert_eq!(r.rect(a).width(), 150.0, "and the panes followed");

    // keys on the focused divider step it by 10pt; min-size clamps
    r.h.harness.get_by_role_and_label(Role::Splitter, "Panes").focus();
    r.h.harness.step();
    r.h.harness.key_press(egui::Key::ArrowLeft);
    r.run();
    assert_eq!(r.get(sv, "divider-position"), Value::F64(140.0));
    r.set(sv, "divider-position", Value::F64(5.0));
    r.run();
    assert_eq!(r.rect(a).width(), 32.0, "min-size keeps the pane at 32pt");

    // vertical: stacked
    r.set(sv, "orientation", s("vertical"));
    r.set(sv, "divider-position", Value::F64(60.0));
    r.run();
    let (ra, rb) = (r.rect(a), r.rect(b));
    assert_eq!(ra.height(), 60.0);
    assert!(rb.min.y >= ra.max.y + 5.5 && (ra.width() - rb.width()).abs() < 0.5, "{ra:?} {rb:?}");
}

fn rows(first: usize, n: usize, cols: usize) -> Value {
    Value::List((first..first + n).map(|r| Value::List((0..cols).map(|c| Value::Str(format!("r{r}c{c}"))).collect())).collect())
}

fn asked(r: &Rig, t: NodeId) -> Option<(u64, u64)> {
    r.scene.read(|s| match s.get(t).unwrap().builtin() {
        W::Table(t) => t.asked,
        _ => unreachable!(),
    })
}

fn table_rig(rows_n: i64) -> (Rig, NodeId) {
    let r = Rig::new(500.0, 400.0);
    let t = r.add(r.slot, "table", &[("flex-grow", Value::F64(1.0)), ("row-count", Value::I64(rows_n)), ("accessible-name", s("Parts"))]);
    for (title, w) in [("Name", 120.0), ("Value", 100.0), ("Footprint", 160.0)] {
        r.add(t, "table-column", &[("title", s(title)), ("width", Value::F64(w))]);
    }
    (r, t)
}

#[test]
fn table_asks_for_its_window_draws_it_and_selects_rows() {
    let (mut r, t) = table_rig(1000);
    r.run();
    // header cells by role and label; the table by role and name
    assert!(r.h.harness.query_by_role_and_label(Role::ColumnHeader, "Footprint").is_some());
    assert!(r.h.harness.query_by_role_and_label(Role::Table, "Parts").is_some());
    // no data yet: it asked for the visible rows plus 20 of overscan
    let (first, last) = asked(&r, t).expect("a range");
    assert_eq!(first, 0);
    assert!((30..60).contains(&last), "visible (~18 rows) + 20: {last}");
    assert!(!r.has("r0c0"));
    // the answer: the window
    r.call(t, "set-rows", &[Value::I64(0), rows(0, last as usize + 1, 3)]);
    r.run();
    assert!(r.has("r0c0") && r.has("r5c2"));
    assert!(!r.has("r100c0"), "virtual: rows below the fold are not drawn");

    // a click on a row selects it
    r.h.harness.get_by_label("r3c1").click();
    r.run();
    assert_eq!(r.get(t, "selected"), Value::I64(3));

    // scroll-to-row: a new range, outside the window rows draw empty until set-rows
    r.call(t, "scroll-to-row", &[Value::I64(500)]);
    r.run();
    let (f2, l2) = asked(&r, t).unwrap();
    assert!(f2 <= 500 && 500 <= l2 && f2 > 400, "{f2}..={l2}");
    assert!(!r.has("r500c0"));
    r.call(t, "set-rows", &[Value::I64(f2 as i64), rows(f2 as usize, (l2 - f2 + 1) as usize, 3)]);
    r.run();
    assert!(r.has("r500c0") && !r.has("r0c0"));

    // errors name the method and the argument
    let b = r.h.embed.backend();
    let (m, _) = kinds::find_method(kinds::tag_index("table").unwrap(), "set-rows").unwrap();
    let e = b.call(t.to_raw(), m, &[Value::I64(-1), Value::List(vec![])]).unwrap_err();
    assert!(e.contains("set-rows first"), "{e}");
}

/// 10k rows, all sent once; scrolled through in jumps and AccessKit scroll steps. Prints the
/// frame time (debug build, headless kittest: no rendering) for the milestone record.
#[test]
fn table_with_10k_rows_scrolls_and_frame_time() {
    let (mut r, t) = table_rig(10_000);
    r.run();
    let t0 = Instant::now();
    r.call(t, "set-rows", &[Value::I64(0), rows(0, 10_000, 3)]);
    let set_ms = t0.elapsed().as_secs_f64() * 1e3;
    r.run();
    assert!(r.has("r0c0"));
    // a frame with nothing changing (the floor: egui + kittest's AccessKit tree, debug build)
    let t0 = Instant::now();
    for _ in 0..20 {
        r.h.step();
    }
    let idle_ms = t0.elapsed().as_secs_f64() * 1e3 / 20.0;
    let mut frames = 0u64;
    let t0 = Instant::now();
    for k in 1..=40u64 {
        r.call(t, "scroll-to-row", &[Value::I64((k * 245) as i64)]);
        // the jump and egui's scroll animation, to rest
        frames += r.h.run();
    }
    let jump_ms = t0.elapsed().as_secs_f64() * 1e3 / frames as f64;
    assert!(r.has("r9800c0"), "the last jump shows row 9800 (asked {:?})", asked(&r, t));
    // small steps upward, one frame each (5 rows a frame, as a fast wheel scroll)
    let t0 = Instant::now();
    for k in 1..=20i64 {
        r.call(t, "scroll-to-row", &[Value::I64(9800 - 5 * k)]);
        r.h.step();
    }
    let step_ms = t0.elapsed().as_secs_f64() * 1e3 / 20.0;
    r.run();
    assert!(r.has("r9700c1") && !r.has("r9800c1"), "scrolled up to row 9700");
    let (f, l) = asked(&r, t).unwrap();
    assert!(l < 9800 && l - f < 80, "the range follows the scroll: {f}..={l}");
    eprintln!(
        "WG4a 10k table: set-rows(0, 10k x 3) {set_ms:.1} ms; still frame {idle_ms:.2} ms; 40 jumps in {frames} frames, {jump_ms:.2} ms per frame; scroll-step frame {step_ms:.2} ms"
    );
}

#[test]
fn toast_host_shows_toasts_from_js_methods_and_rust() {
    let mut r = Rig::new(400.0, 300.0);
    let host = r.add(r.slot, "toast-host", &[]);
    r.add(r.slot, "label", &[("text", s("Body"))]);
    r.run();
    assert_eq!(r.rect(host).height(), 0.0, "the host takes no space");

    let id = r.call(host, "show", &[s("Saved"), s("success"), Value::F64(0.0)]);
    assert_eq!(id, Value::I64(1));
    r.run();
    assert!(r.h.harness.query_by_role_and_label(Role::Alert, "Success: Saved").is_some(), "a toast is an Alert");
    let toast = r.h.harness.get_by_role_and_label(Role::Alert, "Success: Saved").rect();
    assert!(toast.max.x > 300.0 && toast.max.y > 200.0, "bottom-right: {toast:?}");

    // its close button
    r.h.harness.get_by_label("Dismiss").click();
    r.run();
    assert!(!r.has("Saved"));

    // dismiss / clear from JS
    let a = r.call(host, "show", &[s("One")]);
    r.call(host, "show", &[s("Two"), Value::Null, Value::F64(0.0)]);
    r.run();
    assert!(r.has("One") && r.has("Two"));
    assert_eq!(r.call(host, "dismiss", &[a]), Value::Bool(true));
    r.run();
    assert!(!r.has("One") && r.has("Two"));
    r.call(host, "clear", &[]);
    r.run();
    assert!(!r.has("Two"));

    // the Rust API, with a short duration: gone after it
    // (no pointer over the corner: hovering holds a toast open)
    r.h.harness.remove_cursor();
    let ctx = r.h.harness.ctx.clone();
    svelters_egui::toast::show(&ctx, "From Rust", svelters_egui::toast::Level::Warning, Some(0.2));
    assert_eq!(svelters_egui::toast::queued(&ctx), 1);
    r.h.step();
    r.h.step();
    assert!(r.has("From Rust"));
    assert_eq!(svelters_egui::toast::queued(&ctx), 0, "the host took the queue");
    for _ in 0..30 {
        r.h.step();
    }
    r.run();
    assert!(!r.has("From Rust"), "timed out");

    // props
    r.set(host, "corner", s("top-left"));
    r.call(host, "show", &[s("Corner")]);
    r.run();
    let c = r.h.harness.get_by_label("Corner").rect();
    assert!(c.min.x < 100.0 && c.min.y < 100.0, "top-left: {c:?}");
    let (m, _) = kinds::find_method(kinds::tag_index("toast-host").unwrap(), "show").unwrap();
    let e = r.h.embed.backend().call(host.to_raw(), m, &[s("x"), s("loud")]).unwrap_err();
    assert!(e.contains("show level"), "{e}");
}
