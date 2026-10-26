//! AX1 (cadatomic G-shell-7): the content of a `scroll-area`, the realised rows of a `ListView`
//! (`virtual-list`) and the items of an open context menu are in the AccessKit tree with their
//! bounds, visible to `Session::query`, and actionable by automation, in process (kittest) and over
//! the AU2 transport. Clipped rows are present only while realised; `scroll_into_view` brings a
//! node in a scroll-area, or a list row, into view; anchors inside list rows keep SV53's keyed ids.

use egui::accesskit::Role;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;
use svelters_automation::client::App;
use svelters_automation::server::Endpoint;
use svelters_automation::{ActOptions, Session, Tier, WaitState};
use svelters_egui::embed::{EmbedConfig, MountSpec, Props, SlotOptions};
use svelters_egui::harness::{EmbedHarness, TestApp};
use svelters_host::HostConfig;
use svelters_host::modules::{SourceConfig, SourceProvider};

/// cadatomic's Jobs panel (rows with Cancel buttons in a scroll-area), Output (a 1,000-row
/// `ListView` with an anchor per row) and Problems (rows with a context menu: waive, unwaive).
const PANELS: &str = r#"<script>
  import { ListView } from '@svelters/lists';
  let log = $state('');
  const jobs = Array.from({ length: 12 }, (_, i) => ({ id: `j${i}`, name: `Job ${i}` }));
  const lines = Array.from({ length: 1000 }, (_, i) => ({ id: i, text: `Line ${i}` }));
  const problems = [{ id: 'p1', text: 'Clearance' }, { id: 'p2', text: 'Overlap' }];
</script>

<box gap={4}>
  <label>Log {log}</label>
  <scroll-area anchor="jobs" height={120}>
    <box gap={2}>
      {#each jobs as j (j.id)}
        <box flex-direction="row" gap={4}>
          <label>{j.name}</label>
          <button anchor="cancel" onclick={() => (log = `cancel ${j.id}`)}>Cancel {j.name}</button>
        </box>
      {/each}
    </box>
  </scroll-area>
  <ListView items={lines} key={(it) => it.id} row-height={20} height={160} anchor="output">
    {#snippet row(item)}
      <label anchor="line">{item.text}</label>
    {/snippet}
  </ListView>
  {#each problems as p (p.id)}
    <box anchor="problem">
      <label>{p.text}</label>
      <context-menu>
        <action label="Waive" onactivate={() => (log = `waive ${p.id}`)} />
        <action label="Unwaive" onactivate={() => (log = `unwaive ${p.id}`)} />
      </context-menu>
    </box>
  {/each}
</box>
"#;

/// A scratch app: `Panels.svelte` as an embed component, and `App.svelte` showing it in a window
/// (for `TestApp` and `svelters-run`).
fn scratch(name: &str) -> PathBuf {
    scratch_with(name, PANELS)
}

fn scratch_with(name: &str, panels: &str) -> PathBuf {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/lists-egui").canonicalize().unwrap();
    assert!(fixture.join("node_modules/@svelters/lists").exists(), "run `pnpm install` first");
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/ax1-au").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    // an app (`App.svelte`, for `TestApp` and `svelters-run`) or, for a host's slot, an embed entry
    let svelters = if name.starts_with("app") { r#"{ "toolkit": "egui" }"# } else { r#"{ "toolkit": "egui", "embed": { "Panels": "src/Panels.svelte" } }"# };
    std::fs::write(dir.join("package.json"), format!(r#"{{ "name": "ax1au", "private": true, "type": "module", "svelters": {svelters} }}"#)).unwrap();
    std::fs::write(dir.join("src/Panels.svelte"), panels).unwrap();
    std::fs::write(
        dir.join("src/App.svelte"),
        "<script>\n  import Panels from './Panels.svelte';\n</script>\n\n<window title=\"AX1\" default-width={400} default-height={600}>\n  <Panels />\n</window>\n",
    )
    .unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(fixture.join("node_modules"), dir.join("node_modules")).unwrap();
    dir.canonicalize().unwrap()
}

/// The panels mounted into an embed slot (a host's dock panel).
fn embedded(name: &str) -> Session<EmbedHarness> {
    embedded_dir(scratch(name))
}

fn embedded_dir(dir: PathBuf) -> Session<EmbedHarness> {
    let ecfg = EmbedConfig {
        id_salt: "ax1",
        host: HostConfig { echo_console: false, echo_errors: false, quit_on_last_window: false, ..HostConfig::default() },
        ..EmbedConfig::default()
    };
    let mut eh = EmbedHarness::new(egui::vec2(400.0, 600.0), ecfg).unwrap();
    let e = eh.embed.clone();
    let entry = e.load_from(Rc::new(SourceProvider::new(SourceConfig::for_path(&dir).unwrap()).unwrap())).unwrap();
    let slot = e.create_slot("panels", SlotOptions::fill());
    eh.set_ui(move |e, ui| {
        e.show_slot(ui, slot);
    });
    e.mount(MountSpec::new(entry, "Panels", slot, "panels").props(Props::new())).unwrap();
    let mut s = Session::for_test(eh);
    s.wait_settled(10_000.0).unwrap();
    s
}

fn log(s: &Session<EmbedHarness>) -> String {
    s.query("text=Log", None).unwrap().first().map(|n| n.text.clone()).unwrap_or_default()
}

fn one(s: &Session<EmbedHarness>, sel: &str) -> svelters_automation::NodeInfo {
    let r = s.locate(sel).unwrap_or_else(|e| panic!("{sel}: {e:?}"));
    s.node(r).unwrap()
}

fn inside(a: svelters_automation::Rect, b: svelters_automation::Rect) -> bool {
    b.expand(0.5).contains_rect(a)
}

#[test]
fn scroll_area_content_is_reachable_and_scrolled_into_view() {
    let mut s = embedded("scroll");
    let area = one(&s, "anchor=jobs").rect.expect("the area is drawn");
    let rows = s.query("anchor=cancel", None).unwrap();
    assert_eq!(rows.len(), 12, "every row is in the scene");
    // the first rows are drawn, inside the area; the last are scrolled out: attached, visible (as
    // a browser's are), not drawn
    let first = &rows[0];
    assert!(inside(first.rect.unwrap(), area), "{first:?} in {area:?}");
    assert_eq!(first.visible_rect, first.rect);
    let last = &rows[11];
    assert_eq!(last.rect, None, "scrolled out: not drawn this pass");
    assert!(last.states.is_empty(), "{:?}", last.states);
    s.wait_for(r#"text="Cancel Job 11""#, WaitState::Visible, None, 100.0).expect("scrolled out is still visible");
    // a row cut by the area's bottom edge: its part in view is smaller than its rect
    let cut = rows.iter().find(|n| n.rect.is_some_and(|r| r.max.y > area.max.y + 1.0)).expect("a row cut by the edge");
    assert!(cut.visible_rect.unwrap().height() < cut.rect.unwrap().height(), "{cut:?}");
    // AccessKit: the drawn buttons are in the tree with their bounds and author ids; the scrolled
    // out ones are not
    let ak = s.a11y_tree().unwrap();
    let ak_first = ak.iter().find(|n| n.label.as_deref() == Some("Cancel Job 0")).expect("in the AccessKit tree");
    assert_eq!((ak_first.role, ak_first.author_id.as_deref()), (Role::Button, Some("cancel")));
    assert_eq!(ak_first.rect, first.rect.unwrap());
    assert!(ak.iter().all(|n| n.label.as_deref() != Some("Cancel Job 11")));

    // a click on a scrolled-out button scrolls it into view, then clicks it
    s.click(r#"text="Cancel Job 11""#, ActOptions::default()).unwrap();
    assert_eq!(log(&s), "Log cancel j11");
    let last = one(&s, r#"text="Cancel Job 11""#);
    assert!(inside(last.visible_rect.unwrap(), area) && last.visible_rect == last.rect, "{last:?} in {area:?}");
    assert!(s.a11y_tree().unwrap().iter().any(|n| n.label.as_deref() == Some("Cancel Job 11")));
    assert_eq!(one(&s, r#"text="Cancel Job 0""#).rect, None, "now the first rows are out");
    // and a hover on a scrolled-out one (any act scrolls first)
    s.hover(r#"text="Cancel Job 1""#, ActOptions::default()).unwrap();
    assert!(inside(one(&s, r#"text="Cancel Job 1""#).rect.expect("scrolled in"), area));
    // scroll_into_view alone: centred
    s.scroll_into_view(r#"text="Cancel Job 6""#, true, ActOptions::default()).unwrap();
    let six = one(&s, r#"text="Cancel Job 6""#);
    let r = six.rect.unwrap();
    assert!(inside(r, area), "{r:?} in {area:?}");
    assert!((r.center().y - area.center().y).abs() < 20.0, "centred: {r:?} in {area:?}");
}

#[test]
fn list_rows_have_keyed_anchors_and_scroll_to_any_row() {
    let mut s = embedded("list");
    let list = one(&s, "anchor=output");
    assert_eq!(list.role, "list");
    let view = list.rect.expect("the list is drawn");
    // only realised rows are in the scene: a few more than fit, not 1,000
    let lines = s.query("anchor=line", None).unwrap();
    assert!((8..40).contains(&lines.len()), "{} realised rows", lines.len());
    let drawn: Vec<_> = lines.iter().filter(|n| n.rect.is_some()).collect();
    assert!(drawn.len() >= 8, "{drawn:?}");
    for n in &drawn {
        assert!(view.intersects(n.rect.unwrap()) && inside(n.visible_rect.unwrap(), view), "{n:?} in {view:?}");
    }
    // SV53: each row's anchor is scoped by its item's key: one egui id, so one AccessKit node, per
    // row, and `key=` finds a row's content
    let ak = s.a11y_tree().unwrap();
    let ak_lines = ak.iter().filter(|n| n.author_id.as_deref() == Some("line")).count();
    assert_eq!(ak_lines, drawn.len(), "one AccessKit node per drawn row");
    let options: Vec<_> = ak.iter().filter(|n| n.role == Role::ListBoxOption).collect();
    assert_eq!(options.len(), drawn.len());
    for (k, o) in options.iter().enumerate() {
        assert_eq!(o.label.as_deref(), Some(format!("Line {k}").as_str()), "the row is named by its content");
    }
    assert_eq!(one(&s, "key=3 >> anchor=line").text, "Line 3");
    assert!(s.query("key=500 >> anchor=line", None).unwrap().is_empty(), "row 500 is not realised");

    // scroll the list to row 500: its content is realised, drawn in the list, found by its key
    s.scroll_to_row("anchor=output", 500, ActOptions::default()).unwrap();
    let row = one(&s, "key=500 >> anchor=line");
    assert_eq!(row.text, "Line 500");
    assert!(inside(row.rect.expect("drawn"), view), "{row:?} in {view:?}");
    assert!(s.query("key=3 >> anchor=line", None).unwrap().is_empty(), "row 3 is no longer realised");
    assert!(s.a11y_tree().unwrap().iter().any(|n| n.role == Role::ListBoxOption && n.label.as_deref() == Some("Line 500")));
    // a realised row out of view is scrolled in by an act on it (here: a click selects it)
    let below = s.query("anchor=line", None).unwrap().into_iter().filter(|n| n.rect.is_none()).max_by_key(|n| n.text.len() * 10_000 + n.text.parse::<usize>().unwrap_or(0)).expect("a realised row out of view");
    let text = below.text.clone();
    s.click(below.r#ref, ActOptions::default()).unwrap();
    let now = one(&s, &format!("text=\"{text}\""));
    assert!(inside(now.rect.expect("scrolled in"), view), "{now:?} in {view:?}");
}

#[test]
fn context_menu_items_show_while_open_and_are_actionable() {
    let mut s = embedded("menu");
    let waive = r#"key=p1 >> role=menu[name="Waive"]"#;
    // closed: the entries are in the scene, not visible, not drawn, not in the AccessKit tree
    assert_eq!(one(&s, waive).rect, None);
    s.wait_for(waive, WaitState::Hidden, None, 100.0).expect("a closed menu's entries are hidden");
    assert!(s.a11y_tree().unwrap().iter().all(|n| n.role != Role::MenuItem));
    // a right click on the row opens its menu: the entries are visible, drawn, in the tree
    s.context_click("key=p1", ActOptions::default()).unwrap();
    s.wait_for(waive, WaitState::Visible, None, 1_000.0).unwrap();
    let item = one(&s, waive);
    let ak = s.a11y_tree().unwrap();
    let ak_item = ak.iter().find(|n| n.role == Role::MenuItem && n.label.as_deref() == Some("Waive")).expect("in the AccessKit tree");
    assert_eq!(Some(ak_item.rect), item.rect);
    assert!(ak.iter().any(|n| n.role == Role::MenuItem && n.label.as_deref() == Some("Unwaive")));
    // a click on an entry picks it; the menu closes and its entries are hidden again
    s.click(waive, ActOptions::default()).unwrap();
    assert_eq!(log(&s), "Log waive p1");
    s.wait_for(waive, WaitState::Hidden, None, 1_000.0).unwrap();
    assert_eq!(one(&s, waive).rect, None);
    // the other row's menu, picked through AccessKit
    s.context_click("key=p2", ActOptions::default()).unwrap();
    s.click(r#"key=p2 >> role=menu[name="Unwaive"]"#, ActOptions { tier: Tier::Semantic, ..ActOptions::default() }).unwrap();
    assert_eq!(log(&s), "Log unwaive p2");
    s.wait_for(r#"key=p2 >> role=menu[name="Unwaive"]"#, WaitState::Hidden, None, 1_000.0).unwrap();
}

/// The JSON body, run in process (`Session::call`) and out of process (`Client::call`, over AU2's
/// socket to `svelters-run`): the three cases end to end. Returns the log after each step.
fn body(call: &mut dyn FnMut(&str, Value) -> Result<Value, svelters_automation::Error>) -> Vec<String> {
    let mut out = Vec::new();
    let mut logged = |call: &mut dyn FnMut(&str, Value) -> Result<Value, svelters_automation::Error>| {
        let q = call("scene.query", json!({"selector": "text=Log"})).unwrap();
        out.push(q[0]["text"].as_str().unwrap().to_string());
    };
    // scroll-area: a scrolled-out row is attached and visible, not drawn; a click scrolls it in
    let q = call("scene.query", json!({"selector": r#"text="Cancel Job 11""#})).unwrap();
    assert_eq!(q[0]["rect"], Value::Null, "{q}");
    call("wait.for", json!({"selector": r#"text="Cancel Job 11""#, "state": "visible", "timeout": 100})).unwrap();
    call("input.click", json!({"selector": r#"text="Cancel Job 11""#})).unwrap();
    logged(call);
    let q = call("scene.query", json!({"selector": r#"text="Cancel Job 11""#})).unwrap();
    assert_eq!(q[0]["rect"], q[0]["visible_rect"], "wholly in view: {q}");
    // list: scroll to a row, find its content by key
    call("input.scroll_into_view", json!({"selector": "anchor=output", "row": 640})).unwrap();
    let q = call("scene.query", json!({"selector": "key=640 >> anchor=line"})).unwrap();
    assert_eq!(q[0]["text"], "Line 640", "{q}");
    assert!(q[0]["rect"].is_array(), "{q}");
    let tree = call("a11y.tree", json!({})).unwrap();
    assert!(tree.as_array().unwrap().iter().any(|n| n["label"] == "Line 640" && n["role"] == "ListBoxOption"), "{tree}");
    // context menu: open with a right click, pick an entry
    call("wait.for", json!({"selector": r#"key=p2 >> role=menu[name="Waive"]"#, "state": "hidden", "timeout": 100})).unwrap();
    call("input.click", json!({"selector": "key=p2", "button": "secondary"})).unwrap();
    call("wait.for", json!({"selector": r#"key=p2 >> role=menu[name="Waive"]"#, "state": "visible", "timeout": 1000})).unwrap();
    let tree = call("a11y.tree", json!({})).unwrap();
    assert!(tree.as_array().unwrap().iter().any(|n| n["label"] == "Waive" && n["role"] == "MenuItem"), "{tree}");
    call("input.click", json!({"selector": r#"key=p2 >> role=menu[name="Waive"]"#})).unwrap();
    logged(call);
    out
}

#[test]
fn the_three_cases_in_and_out_of_process() {
    let dir = scratch("app-au2");
    let mut s = Session::for_test(TestApp::from_source_stepped(dir.clone(), 0.016).unwrap());
    s.wait_settled(10_000.0).unwrap();
    let inproc = body(&mut |m, p| s.call(m, &p));
    assert_eq!(inproc, ["Log cancel j11", "Log waive p2"]);

    #[cfg(unix)]
    let ep = {
        let d = std::env::temp_dir().join(format!("sax1-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        Endpoint::Unix(d.join("a.sock"))
    };
    #[cfg(not(unix))]
    let ep = Endpoint::Tcp(0);
    let bin = Path::new(env!("CARGO_BIN_EXE_svelters-run"));
    let mut app = App::spawn(bin, &[dir.to_str().unwrap(), "--headless", "--clock", "virtual", "--seed", "3"], &ep, Duration::from_secs(120)).unwrap();
    let outproc = body(&mut |m, p| app.client.call(m, &p));
    assert_eq!(outproc, inproc, "the same body gives the same results over the socket");
}
