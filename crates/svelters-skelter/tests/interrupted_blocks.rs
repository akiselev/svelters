//! SV84 follow-up: `{#if}`, `{#key}` and `{#await}` blocks whose branch switch the host interrupts
//! (the turn budget, in the flush after a click: no root faults) are rebuilt by the passes after it,
//! so the document holds exactly the current branch's nodes. One component per block kind; each
//! branch has three labels that each take longer than a third of the interrupt budget to render.

use skelter_base::geom::{Scale, Size};
use skelter_base::ids::{NodeId, WindowId};
use skelter_base::time::Time;
use skelter_core::{Document, KindId};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use svelters_abi::*;
use svelters_skelter::SkelterBackend;
use svelters_skelter::embed::{Budgets, Embed, EmbedConfig, EmbedEvent, MountSpec, Props, RootId};

const SCRIPT: &str = r#"<script>
  import { untrack } from 'svelte';
  let { slow = 0 } = $props();
  let step = $state(0);
  function cost(x) {
    const ms = untrack(() => slow);
    if (ms > 0) {
      const t = Date.now();
      while (Date.now() - t < ms) {}
    }
    return x;
  }
  // settles after a timer: each step shows `pending`, then builds `then` again
  const later = (v) => new Promise((resolve) => setTimeout(() => resolve(v), 0));
  let p = $state(later(0));
</script>
<button text="next" onclicked={() => { step++; p = later(step); }} />
"#;

const IF: &str = r#"<box>
  {#if step % 2 === 0}
    <label text={cost(`even ${step}`)} /><label text={cost('even b')} /><label text={cost('even c')} />
  {:else}
    <label text={cost(`odd ${step}`)} /><label text={cost('odd b')} /><label text={cost('odd c')} />
  {/if}
</box>"#;

const KEY: &str = r#"<box>
  {#key step}
    <label text={cost(`key ${step}`)} /><label text={cost('key b')} /><label text={cost('key c')} />
  {/key}
</box>"#;

const AWAIT: &str = r#"<box>
  {#await p}
    <label text="pending" />
  {:then v}
    <label text={cost(`then ${v}`)} /><label text={cost('then b')} /><label text={cost('then c')} />
  {/await}
</box>"#;

fn scratch(name: &str, markup: &str) -> PathBuf {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/embed-egui").canonicalize().unwrap();
    assert!(fixture.join("node_modules").exists(), "run `pnpm install` first");
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/svelters-skelter-interrupted").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("package.json"), r#"{ "name": "interrupted", "private": true, "type": "module", "svelters": { "toolkit": "skelter", "embed": { "Block": "src/Block.svelte" } } }"#).unwrap();
    std::fs::write(dir.join("src/Block.svelte"), format!("{SCRIPT}{markup}\n")).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(fixture.join("node_modules"), dir.join("node_modules")).unwrap();
    dir.canonicalize().unwrap()
}

struct Rig {
    doc: Rc<RefCell<Document>>,
    backend: SkelterBackend,
    embed: Embed,
    root: RootId,
    pane: NodeId,
    t: u64,
}

impl Rig {
    fn new(name: &str, markup: &str) -> Rig {
        let reg = svelters_skelter::builtin_kinds().registry().clone();
        let mut doc = Document::new(reg, Box::new(svelters_skelter::backend::NoText));
        let win = doc.create_window(WindowId(1), KindId::of("box"), Size::new(300.0, 300.0), Scale(1.0));
        let pane = doc.create(KindId::of("box"));
        doc.insert(win, pane, None, None).unwrap();
        let doc = Rc::new(RefCell::new(doc));
        let backend = SkelterBackend::shared(svelters_skelter::builtin_kinds(), doc.clone()).unwrap();
        let host = svelters_host::HostConfig { echo_console: false, echo_errors: false, quit_on_last_window: false, ..svelters_host::HostConfig::default() };
        let long = std::time::Duration::from_secs(60);
        let budgets = Budgets { warn: long, interrupt: std::time::Duration::from_millis(150), mount: long };
        let embed = Embed::with_backend(backend.clone(), EmbedConfig { host, budgets, ..EmbedConfig::default() }).unwrap();
        let mut cfg = svelters_host::modules::SourceConfig::for_path(&scratch(name, markup)).unwrap();
        cfg.backend = "skelter".into();
        let entry = embed.load_from(Rc::new(svelters_host::modules::SourceProvider::new(cfg).unwrap())).unwrap();
        let slot = embed.adopt_slot("panel", pane).unwrap();
        let root = embed.mount(MountSpec::new(entry, "Block", slot, "panel")).unwrap();
        let mut rig = Rig { doc, backend, embed, root, pane, t: 0 };
        rig.settle();
        rig
    }

    fn pass(&mut self) {
        self.embed.logic();
        self.t += 16_000_000;
        let _ = self.doc.borrow_mut().frame(WindowId(1), Time(self.t));
    }

    fn settle(&mut self) {
        for _ in 0..3 {
            self.pass();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    /// The labels under the slot, in tree order.
    fn labels(&self) -> Vec<String> {
        let doc = self.doc.borrow();
        let label = self.backend.resolve(NameKind::Kind, 0, "label");
        let mut out = Vec::new();
        let mut stack: Vec<NodeId> = doc.children(self.pane).iter().rev().copied().collect();
        while let Some(n) = stack.pop() {
            if let Some(h) = self.backend.handle(n) {
                let info = self.backend.handle_info(h).unwrap();
                if info.kind == label
                    && let Ok(Value::Str(s)) = self.backend.get_prop(h, self.backend.resolve(NameKind::Prop, info.kind, "text"))
                {
                    out.push(s.to_string());
                }
            }
            stack.extend(doc.children(n).iter().rev().copied());
        }
        out
    }

    /// Clicks "next" with every label slow (interrupted), then settles at full speed; returns
    /// whether the host reported an interrupt.
    fn slow_step(&mut self, ms: f64) -> bool {
        self.embed.set_root_props(self.root, Props::new().with("slow", ms)).unwrap();
        self.pass();
        let button = self.doc.borrow().children(self.pane)[0];
        let click = self.backend.target_event(button, "clicked", vec![]).expect("the button listens");
        self.backend.queue(svelters_skelter::Output { events: vec![click], ..Default::default() });
        // the click, then (for `{#await}`) the timer that settles the promise
        for _ in 0..3 {
            self.pass();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let mut interrupted = false;
        for e in self.embed.take_events() {
            match e {
                EmbedEvent::Error { root: None, error } if error.kind == svelters_host::ErrorKind::Interrupted => interrupted = true,
                EmbedEvent::Faulted { .. } => panic!("the root faulted: {e:?}"),
                _ => {}
            }
        }
        self.embed.set_root_props(self.root, Props::new().with("slow", 0.0)).unwrap();
        self.settle();
        interrupted
    }
}

fn run(name: &str, markup: &str, expected: impl Fn(u32) -> Vec<String>) {
    let mut rig = Rig::new(name, markup);
    assert_eq!(rig.labels(), expected(0), "mounted");
    let mut interrupted = 0;
    for step in 1..=4 {
        interrupted += rig.slow_step(60.0) as u32;
        assert_eq!(rig.labels(), expected(step), "after interrupted step {step}");
    }
    assert!(interrupted > 0, "some switch was interrupted");
}

fn three(a: String, b: &str, c: &str) -> Vec<String> {
    vec![a, b.to_string(), c.to_string()]
}

#[test]
fn sv84_an_interrupted_if_block_is_rebuilt() {
    run("if", IF, |s| if s % 2 == 0 { three(format!("even {s}"), "even b", "even c") } else { three(format!("odd {s}"), "odd b", "odd c") });
}

#[test]
fn sv84_an_interrupted_key_block_is_rebuilt() {
    run("key", KEY, |s| three(format!("key {s}"), "key b", "key c"));
}

#[test]
fn sv84_an_interrupted_await_block_is_rebuilt() {
    run("await", AWAIT, |s| three(format!("then {s}"), "then b", "then c"));
}

/// The `{#await}` component settles each step when nothing is interrupted (the harness itself).
#[test]
fn sv84_await_settles_without_interrupts() {
    let mut rig = Rig::new("await-plain", AWAIT);
    for step in 1..=2 {
        rig.slow_step(0.0);
        assert_eq!(rig.labels(), three(format!("then {step}"), "then b", "then c"));
    }
}
