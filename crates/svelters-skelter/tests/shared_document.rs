//! SV82: several backends over one document (`SkelterBackend::shared`, one host each). Whichever
//! backend dispatches or pumps, each host gets the events and changes of its own nodes.

use skelter_base::event::{InputEnvelope, InputEvent, Modifiers, MouseButton, PointerEvent, PointerPhase, PointerType};
use skelter_base::geom::{Point, Scale, Size, Vec2};
use skelter_base::ids::{InputSeq, NodeId, WindowId};
use skelter_base::time::Time;
use skelter_core::{Document, KindId};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use svelters_abi::*;
use svelters_skelter::SkelterBackend;
use svelters_skelter::embed::{Embed, EmbedConfig, MountSpec};

const COUNTER: &str = r#"<script>
  let count = $state(0);
</script>
<box>
  <button text="Go" width={60} height={30} onclicked={() => count++} />
  <label text={`n=${count}`} />
</box>
"#;

/// A scratch app exporting `Counter`, for skelter.
fn scratch(name: &str) -> PathBuf {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/embed-egui").canonicalize().unwrap();
    assert!(fixture.join("node_modules").exists(), "run `pnpm install` first");
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/svelters-skelter-shared").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("package.json"), r#"{ "name": "shared-doc", "private": true, "type": "module", "svelters": { "toolkit": "skelter", "embed": { "Counter": "src/Counter.svelte" } } }"#).unwrap();
    std::fs::write(dir.join("src/Counter.svelte"), COUNTER).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(fixture.join("node_modules"), dir.join("node_modules")).unwrap();
    dir.canonicalize().unwrap()
}

struct Side {
    backend: SkelterBackend,
    embed: Embed,
    pane: NodeId,
}

impl Side {
    fn new(doc: &Rc<RefCell<Document>>, pane: NodeId, name: &str) -> Side {
        let backend = SkelterBackend::shared(svelters_skelter::builtin_kinds(), doc.clone()).unwrap();
        let host = svelters_host::HostConfig { echo_console: false, echo_errors: true, quit_on_last_window: false, ..svelters_host::HostConfig::default() };
        let embed = Embed::with_backend(backend.clone(), EmbedConfig { host, ..EmbedConfig::default() }).unwrap();
        let mut cfg = svelters_host::modules::SourceConfig::for_path(&scratch(name)).unwrap();
        cfg.backend = "skelter".into();
        let entry = embed.load_from(Rc::new(svelters_host::modules::SourceProvider::new(cfg).unwrap())).unwrap();
        let slot = embed.adopt_slot("panel", pane).unwrap();
        embed.mount(MountSpec::new(entry, "Counter", slot, "panel")).unwrap();
        embed.logic();
        Side { backend, embed, pane }
    }

    /// (button, label) of the mounted counter.
    fn nodes(&self) -> (NodeId, NodeId) {
        let doc = self.backend.document();
        let d = doc.borrow();
        let bx = d.children(self.pane)[0];
        (d.children(bx)[0], d.children(bx)[1])
    }

    fn label(&self) -> Value {
        let (_, label) = self.nodes();
        let h = self.backend.handle(label).unwrap();
        let p = self.backend.resolve(NameKind::Prop, self.backend.handle_info(h).unwrap().kind, "text");
        self.backend.get_prop(h, p).unwrap()
    }
}

/// A window with two panes side by side, each the slot of its own backend and host.
fn two_sides(tag: &str) -> (Rc<RefCell<Document>>, Side, Side) {
    let reg = svelters_skelter::builtin_kinds().registry().clone();
    let mut doc = Document::new(reg, Box::new(svelters_skelter::backend::NoText));
    let root = doc.create_window(WindowId(1), KindId::of("box"), Size::new(400.0, 200.0), Scale(1.0));
    doc.set_style(root, skelter_core::StyleInput { inline: Some("flex-direction: row; width: 400px; height: 200px".into()), ..Default::default() });
    let mut panes = Vec::new();
    for _ in 0..2 {
        let p = doc.create(KindId::of("box"));
        doc.insert(root, p, None, None).unwrap();
        doc.set_style(p, skelter_core::StyleInput { inline: Some("width: 200px; height: 200px".into()), ..Default::default() });
        panes.push(p);
    }
    let doc = Rc::new(RefCell::new(doc));
    let a = Side::new(&doc, panes[0], &format!("{tag}-a"));
    let b = Side::new(&doc, panes[1], &format!("{tag}-b"));
    doc.borrow_mut().frame(WindowId(1), Time(0));
    (doc, a, b)
}

fn pointer(phase: PointerPhase, seq: u64, pos: Point) -> InputEnvelope {
    let p = PointerEvent {
        phase,
        pointer: 0,
        kind: PointerType::Mouse,
        button: matches!(phase, PointerPhase::Down | PointerPhase::Up).then_some(MouseButton::Primary),
        buttons: if phase == PointerPhase::Down { 1 } else { 0 },
        pos,
        modifiers: Modifiers::default(),
        pressure: 0.5,
        tilt: Vec2::ZERO,
        history: Default::default(),
    };
    let t = Time(seq * 100_000_000);
    InputEnvelope { seq: InputSeq(seq), window: WindowId(1), os_time: t, recv_time: t, event: InputEvent::Pointer(p) }
}

/// Clicks the middle of `node` through `via`'s dispatch.
fn click(via: &SkelterBackend, doc: &Rc<RefCell<Document>>, node: NodeId, seq: &mut u64) {
    let r = doc.borrow().world_rect(node).expect("laid out");
    let at = Point::new((r.min.x + r.max.x) / 2.0, (r.min.y + r.max.y) / 2.0);
    for phase in [PointerPhase::Down, PointerPhase::Up] {
        *seq += 1;
        via.dispatch(&pointer(phase, *seq, at));
        via.pump();
    }
}

#[test]
fn sv82_each_backend_gets_its_own_events_whoever_dispatches() {
    let (doc, a, b) = two_sides("dispatch");
    assert_eq!(a.label(), Value::Str("n=0".into()));
    assert_eq!(b.label(), Value::Str("n=0".into()));
    let mut seq = 0;
    // every input goes through `a`'s dispatch, as one shell loop would: `b`'s click is routed to it
    click(&a.backend, &doc, a.nodes().0, &mut seq);
    click(&a.backend, &doc, b.nodes().0, &mut seq);
    click(&a.backend, &doc, b.nodes().0, &mut seq);
    for s in [&a, &b] {
        s.embed.logic();
    }
    assert_eq!(a.label(), Value::Str("n=1".into()), "a's click reached a's host");
    assert_eq!(b.label(), Value::Str("n=2".into()), "b's clicks reached b's host");
}

#[test]
fn sv82_queued_output_reaches_the_owner_whoever_pumps() {
    let (_doc, a, b) = two_sides("queue");
    let ea = a.backend.target_event(a.nodes().0, "clicked", vec![]).expect("a listens");
    let eb = b.backend.target_event(b.nodes().0, "clicked", vec![]).expect("b listens");
    // one event of each, queued on `b`: only `b` pumps before the hosts' turns
    b.backend.queue(svelters_skelter::Output { events: vec![ea, eb], ..Default::default() });
    b.backend.pump();
    for s in [&b, &a] {
        s.embed.logic();
    }
    assert_eq!(a.label(), Value::Str("n=1".into()), "a's event was handed to a");
    assert_eq!(b.label(), Value::Str("n=1".into()));
}
