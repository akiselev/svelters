//! SV87: a block rebuild that overruns the interrupt budget again makes progress. SV84 rebuilds a
//! block the host interrupted mid-update from nothing; when that rebuild itself costs more than the
//! budget, each retry used to be interrupted again, forever. Now each retry of the rebuild gets twice
//! the budget of the one before (up to `HostConfig::repair_budget`), the host reports the interrupt
//! once, and a rebuild that overruns even the largest budget leaves its block empty (no stale nodes)
//! until the block's inputs change. One component per block kind: `{#each}`, `{#if}`, `{#key}` and
//! `{#await}`, each rebuilding labels whose total cost is a few times the budget.

use skelter_base::geom::{Scale, Size};
use skelter_base::ids::{NodeId, WindowId};
use skelter_base::time::Time;
use skelter_core::{Document, KindId};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;
use svelters_abi::*;
use svelters_skelter::SkelterBackend;
use svelters_skelter::embed::{Budgets, Busy, Embed, EmbedConfig, EmbedEvent, MountSpec, Props, RootId};

/// The interrupt budget of these tests.
const BUDGET: Duration = Duration::from_millis(100);
/// Labels each block builds per step.
const ROWS: u32 = 8;

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
  const ROWS = 8;
  const rows = (s) => Array.from({ length: ROWS }, (_, i) => `${s}.${i}`);
  // settles after a timer: each step shows `pending`, then builds `then` again
  const later = (v) => new Promise((resolve) => setTimeout(() => resolve(v), 0));
  let p = $state(later(0));
</script>
<button text="next" onclicked={() => { step++; p = later(step); }} />
"#;

const EACH: &str = r#"<box>
  {#each rows(step) as r (r)}<label text={cost(`each ${r}`)} />{/each}
</box>"#;

const IF: &str = r#"<box>
  {#if step % 2 === 0}
    {#each rows(step) as r}<label text={cost(`even ${r}`)} />{/each}
  {:else}
    {#each rows(step) as r}<label text={cost(`odd ${r}`)} />{/each}
  {/if}
</box>"#;

const KEY: &str = r#"<box>
  {#key step}
    {#each rows(step) as r}<label text={cost(`key ${r}`)} />{/each}
  {/key}
</box>"#;

const AWAIT: &str = r#"<box>
  {#await p}
    <label text="pending" />
  {:then v}
    {#each rows(v) as r}<label text={cost(`then ${r}`)} />{/each}
  {/await}
</box>"#;

fn scratch(name: &str, markup: &str) -> PathBuf {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/embed-egui").canonicalize().unwrap();
    assert!(fixture.join("node_modules").exists(), "run `pnpm install` first");
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/svelters-skelter-overrun").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("package.json"), r#"{ "name": "overrun", "private": true, "type": "module", "svelters": { "toolkit": "skelter", "embed": { "Block": "src/Block.svelte" } } }"#).unwrap();
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
    fn new(name: &str, markup: &str, repair_budget: Duration) -> Rig {
        let reg = svelters_skelter::builtin_kinds().registry().clone();
        let mut doc = Document::new(reg, Box::new(svelters_skelter::backend::NoText));
        let win = doc.create_window(WindowId(1), KindId::of("box"), Size::new(300.0, 300.0), Scale(1.0));
        let pane = doc.create(KindId::of("box"));
        doc.insert(win, pane, None, None).unwrap();
        let doc = Rc::new(RefCell::new(doc));
        let backend = SkelterBackend::shared(svelters_skelter::builtin_kinds(), doc.clone()).unwrap();
        let host = svelters_host::HostConfig {
            echo_console: false,
            echo_errors: false,
            quit_on_last_window: false,
            repair_budget,
            ..svelters_host::HostConfig::default()
        };
        let long = Duration::from_secs(60);
        let budgets = Budgets { warn: long, interrupt: BUDGET, mount: long };
        let embed = Embed::with_backend(backend.clone(), EmbedConfig { host, budgets, ..EmbedConfig::default() }).unwrap();
        let mut cfg = svelters_host::modules::SourceConfig::for_path(&scratch(name, markup)).unwrap();
        cfg.backend = "skelter".into();
        let entry = embed.load_from(Rc::new(svelters_host::modules::SourceProvider::new(cfg).unwrap())).unwrap();
        let slot = embed.adopt_slot("panel", pane).unwrap();
        let root = embed.mount(MountSpec::new(entry, "Block", slot, "panel")).unwrap();
        let mut rig = Rig { doc, backend, embed, root, pane, t: 0 };
        for _ in 0..3 {
            rig.pass();
        }
        rig.embed.take_events();
        rig
    }

    fn pass(&mut self) {
        self.embed.logic();
        self.t += 16_000_000;
        let _ = self.doc.borrow_mut().frame(WindowId(1), Time(self.t));
        std::thread::sleep(Duration::from_millis(20));
    }

    fn set_slow(&mut self, ms: f64) {
        self.embed.set_root_props(self.root, Props::new().with("slow", ms)).unwrap();
        self.pass();
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

    /// Nothing left to run in JS (the test slot never gets a size, so LAYOUT stays set).
    fn idle(&self) -> bool {
        Busy::from_bits(self.embed.settled().busy.bits() & !Busy::LAYOUT.bits()).is_empty()
    }

    /// Clicks "next", then runs passes until the labels are `expected` and the embed is idle, at
    /// most `max` passes (panics past that); returns the interrupts the host reported.
    fn step(&mut self, expected: &[String], max: u32) -> u32 {
        let button = self.doc.borrow().children(self.pane)[0];
        let click = self.backend.target_event(button, "clicked", vec![]).expect("the button listens");
        self.backend.queue(svelters_skelter::Output { events: vec![click], ..Default::default() });
        let mut interrupts = 0;
        let mut passes = 0;
        loop {
            self.pass();
            passes += 1;
            for e in self.embed.take_events() {
                match e {
                    EmbedEvent::Error { root: None, error } if error.kind == svelters_host::ErrorKind::Interrupted => interrupts += 1,
                    EmbedEvent::Faulted { .. } => panic!("the root faulted: {e:?}"),
                    _ => {}
                }
            }
            // at least a few passes: the `{#await}` timer, and a retry that would still come
            if passes >= 4 && self.labels() == expected && self.idle() {
                return interrupts;
            }
            assert!(passes < max, "not settled after {passes} passes ({interrupts} interrupts reported); labels {:?}, busy {:?}", self.labels(), self.embed.settled());
        }
    }
}

fn rows(prefix: &str, s: u32) -> Vec<String> {
    (0..ROWS).map(|i| format!("{prefix} {s}.{i}")).collect()
}

/// Each step's rebuild costs about 2.5 budgets: it is interrupted, and a retry with a grown budget
/// finishes it. The host reports one interrupt per step, and the block holds exactly its nodes.
fn progress(name: &str, markup: &str, expected: impl Fn(u32) -> Vec<String>) {
    let mut rig = Rig::new(name, markup, Duration::from_secs(2));
    assert_eq!(rig.labels(), expected(0), "mounted");
    // ROWS labels at 32 ms: ~256 ms a rebuild, against a 100 ms budget
    rig.set_slow(32.0);
    for step in 1..=2 {
        let interrupts = rig.step(&expected(step), 12);
        assert_eq!(interrupts, 1, "step {step}: the repeated interrupt is reported once");
    }
}

/// A rebuild that costs more than the largest grown budget gives up: the block is left empty (no
/// stale nodes), the host settles, and the block builds again when its inputs change.
fn gives_up(name: &str, markup: &str, expected: impl Fn(u32) -> Vec<String>) {
    let mut rig = Rig::new(name, markup, Duration::from_millis(200));
    // ~480 ms a rebuild, against a budget that grows from 100 ms to at most 200 ms
    rig.set_slow(60.0);
    let interrupts = rig.step(&[], 12);
    assert!((1..=2).contains(&interrupts), "the interrupt, then the give-up ({interrupts} reported)");
    rig.set_slow(0.0);
    rig.step(&expected(2), 8);
}

#[test]
fn sv87_an_overrunning_each_rebuild_settles() {
    progress("each", EACH, |s| rows("each", s));
}

#[test]
fn sv87_an_overrunning_if_rebuild_settles() {
    progress("if", IF, |s| rows(if s % 2 == 0 { "even" } else { "odd" }, s));
}

#[test]
fn sv87_an_overrunning_key_rebuild_settles() {
    progress("key", KEY, |s| rows("key", s));
}

#[test]
fn sv87_an_overrunning_await_rebuild_settles() {
    progress("await", AWAIT, |s| rows("then", s));
}

#[test]
fn sv87_a_rebuild_past_the_largest_budget_leaves_the_each_block_empty() {
    gives_up("each-give-up", EACH, |s| rows("each", s));
}

#[test]
fn sv87_a_rebuild_past_the_largest_budget_leaves_the_if_block_empty() {
    gives_up("if-give-up", IF, |s| rows(if s % 2 == 0 { "even" } else { "odd" }, s));
}
