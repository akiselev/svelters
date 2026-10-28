//! SV84: a keyed `{#each}` (groups of rows, both keyed, the rows filtered by a root prop) driven
//! through all → few → all → few with the updates falling across embed `logic` passes and document
//! commits in every order: a pass per update, two updates per pass, an update between `logic` and
//! the commit, several passes per commit. After every commit the document under the slot and the
//! accessibility tree a platform adapter folds from the commits hold exactly the current rows.

use skelter_base::geom::{Scale, Size};
use skelter_base::ids::{NodeId, WindowId};
use skelter_base::time::Time;
use skelter_core::{Document, KindId};
use skelter_base::accesskit::{NodeId as AxId, Role, TreeId};
use skelter_test::{A11yTree, Consumer};
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use svelters_abi::*;
use svelters_skelter::SkelterBackend;
use svelters_skelter::embed::{Embed, EmbedConfig, MountSpec, Props, RootId};

const LIST: &str = r#"<script>
  import { untrack } from 'svelte';
  let { filter: given = '', slow = 0, steps = [] } = $props();
  // the filter from a click (an event handler's update, flushed in the job after it) or the prop
  let step = $state(-1);
  const filter = $derived(step < 0 ? given : steps[step % steps.length]);
  // a row that costs `slow` ms to render (a loaded machine's debug build)
  function cost(r) {
    const ms = untrack(() => slow);
    if (ms > 0) {
      const t = Date.now();
      while (Date.now() - t < ms) {}
    }
    return r;
  }
  const all = [
    { id: 'g1', rows: ['alpha one', 'alpha two', 'beta one', 'gamma'] },
    { id: 'g2', rows: ['beta two', 'delta', 'alpha three'] },
    { id: 'g3', rows: ['epsilon', 'beta three', 'zeta', 'eta', 'theta one'] },
    { id: 'g4', rows: ['theta two', 'iota'] },
  ];
  const shown = $derived(all.map((g) => ({ id: g.id, rows: g.rows.filter((r) => r.includes(filter)) })).filter((g) => g.rows.length));
</script>
<button text="next" onclicked={() => step++} />
<box>
  {#each shown as g (g.id)}
    <box>
      <label text={`[${g.id}]`} />
      {#each g.rows as r (r)}
        <box>
          <label text={cost(r)} />
          {#if r.includes('one')}<label text={`${r}: first`} />{/if}
        </box>
      {/each}
    </box>
  {/each}
</box>
"#;

const ALL: [(&str, &[&str]); 4] = [
    ("g1", &["alpha one", "alpha two", "beta one", "gamma"]),
    ("g2", &["beta two", "delta", "alpha three"]),
    ("g3", &["epsilon", "beta three", "zeta", "eta", "theta one"]),
    ("g4", &["theta two", "iota"]),
];

/// The labels the list shows for `filter`, in order.
fn expected(filter: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (g, rows) in ALL {
        let rows: Vec<&str> = rows.iter().copied().filter(|r| r.contains(filter)).collect();
        if rows.is_empty() {
            continue;
        }
        out.push(format!("[{g}]"));
        for r in rows {
            out.push(r.to_string());
            if r.contains("one") {
                out.push(format!("{r}: first"));
            }
        }
    }
    out
}

fn scratch(name: &str) -> PathBuf {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/embed-egui").canonicalize().unwrap();
    assert!(fixture.join("node_modules").exists(), "run `pnpm install` first");
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/svelters-skelter-keyed-each").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("package.json"), r#"{ "name": "keyed-each", "private": true, "type": "module", "svelters": { "toolkit": "skelter", "embed": { "List": "src/List.svelte" } } }"#).unwrap();
    std::fs::write(dir.join("src/List.svelte"), LIST).unwrap();
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
    consumer: Consumer,
    tree: A11yTree,
    time: Cell<u64>,
}

impl Rig {
    fn new(name: &str) -> Rig {
        // no interrupts here, however slow the machine
        let long = std::time::Duration::from_secs(60);
        Rig::with_budgets(name, svelters_skelter::embed::Budgets { warn: long, interrupt: long, mount: long })
    }

    fn with_budgets(name: &str, budgets: svelters_skelter::embed::Budgets) -> Rig {
        let reg = svelters_skelter::builtin_kinds().registry().clone();
        let mut doc = Document::new(reg, Box::new(svelters_skelter::backend::NoText));
        let win = doc.create_window(WindowId(1), KindId::of("box"), Size::new(400.0, 800.0), Scale(1.0));
        doc.set_style(win, skelter_core::StyleInput { inline: Some("width: 400px; height: 800px".into()), ..Default::default() });
        let pane = doc.create(KindId::of("box"));
        doc.insert(win, pane, None, None).unwrap();
        let doc = Rc::new(RefCell::new(doc));
        let backend = SkelterBackend::shared(svelters_skelter::builtin_kinds(), doc.clone()).unwrap();
        let host = svelters_host::HostConfig { echo_console: false, echo_errors: true, quit_on_last_window: false, ..svelters_host::HostConfig::default() };
        let embed = Embed::with_backend(backend.clone(), EmbedConfig { host, budgets, ..EmbedConfig::default() }).unwrap();
        let mut cfg = svelters_host::modules::SourceConfig::for_path(&scratch(name)).unwrap();
        cfg.backend = "skelter".into();
        let entry = embed.load_from(Rc::new(svelters_host::modules::SourceProvider::new(cfg).unwrap())).unwrap();
        let slot = embed.adopt_slot("panel", pane).unwrap();
        let root = embed.mount(MountSpec::new(entry, "List", slot, "panel")).unwrap();
        let mut rig = Rig { doc, backend, embed, root, pane, consumer: Consumer::tree_only(), tree: A11yTree::default(), time: Cell::new(0) };
        rig.embed.logic();
        rig.commit();
        rig
    }

    fn set(&self, filter: &str) {
        self.embed.set_root_props(self.root, Props::new().with("filter", filter)).unwrap();
    }

    /// A document frame (the commit), its accessibility update folded into the consumer.
    fn commit(&mut self) {
        let t = self.time.get() + 16_000_000;
        self.time.set(t);
        let c = self.doc.borrow_mut().frame(WindowId(1), Time(t));
        if let Some(u) = c.and_then(|c| c.a11y) {
            self.consumer.apply(&u);
            self.tree.apply(&u);
        }
    }

    /// The labels in the document under the slot, in tree order.
    fn doc_labels(&self) -> Vec<String> {
        let doc = self.doc.borrow();
        let mut out = Vec::new();
        let mut stack: Vec<NodeId> = doc.children(self.pane).iter().rev().copied().collect();
        while let Some(n) = stack.pop() {
            if let Some(h) = self.backend.handle(n) {
                let info = self.backend.handle_info(h).unwrap();
                if info.kind == self.backend.resolve(NameKind::Kind, 0, "label") {
                    let p = self.backend.resolve(NameKind::Prop, info.kind, "text");
                    if let Ok(Value::Str(s)) = self.backend.get_prop(h, p) {
                        out.push(s.to_string());
                    }
                }
            }
            stack.extend(doc.children(n).iter().rev().copied());
        }
        out
    }

    /// The labels the accessibility tree has under the slot, in tree order.
    fn a11y_labels(&self) -> Vec<String> {
        let mut out = Vec::new();
        let mut stack = vec![AxId(self.pane.0)];
        let mut first = true;
        while let Some(id) = stack.pop() {
            let Some(n) = self.tree.node((TreeId::ROOT, id)) else { continue };
            if !first
                && n.role() == Role::Label
                && let Some(l) = n.value().or(n.label())
            {
                out.push(l.to_string());
            }
            first = false;
            stack.extend(n.children().iter().rev().copied());
        }
        out
    }

    #[track_caller]
    fn check(&self, filter: &str, step: &str) {
        let want = expected(filter);
        assert_eq!(self.doc_labels(), want, "{step}: the document under the slot");
        assert_eq!(self.a11y_labels(), want, "{step}: the accessibility tree");
    }
}

/// all → few → all → few, as typing a filter, clearing it and typing another would.
const SEQ: [&str; 12] = ["", "a", "al", "alpha", "", "t", "th", "the", "theta", "", "be", ""];

#[test]
fn sv84_a_pass_and_a_commit_per_update() {
    let mut rig = Rig::new("pass-per-update");
    rig.check("", "mounted");
    for f in SEQ {
        rig.set(f);
        rig.embed.logic();
        rig.commit();
        rig.check(f, &format!("filter {f:?}"));
    }
}

#[test]
fn sv84_two_updates_per_pass() {
    let mut rig = Rig::new("two-per-pass");
    for pair in SEQ.chunks(2) {
        for f in pair {
            rig.set(f);
        }
        rig.embed.logic();
        rig.commit();
        rig.check(pair[pair.len() - 1], &format!("after {pair:?}"));
    }
}

#[test]
fn sv84_an_update_between_logic_and_commit() {
    let mut rig = Rig::new("between");
    let mut last = "";
    for f in SEQ {
        rig.embed.logic();
        rig.set(f);
        rig.commit();
        // the prop reaches JS at the next pass: the commit shows the filter before it
        rig.check(last, &format!("commit before {f:?} reached JS"));
        last = f;
    }
    rig.embed.logic();
    rig.commit();
    rig.check(last, "settled");
}

#[test]
fn sv84_several_passes_per_commit() {
    let mut rig = Rig::new("passes-per-commit");
    for chunk in SEQ.chunks(3) {
        for f in chunk {
            rig.set(f);
            rig.embed.logic();
        }
        rig.commit();
        rig.check(chunk[chunk.len() - 1], &format!("after {chunk:?}"));
    }
}

#[test]
fn sv84_irregular_interleaving() {
    // a fixed pseudo-random schedule: each step sets the next filter, runs logic or commits
    let mut rig = Rig::new("irregular");
    let mut seed: u64 = 0x5eed_0084;
    let mut i = 0;
    let mut current = "";
    let mut set_pending = false;
    for _ in 0..400 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        match (seed >> 33) % 3 {
            0 => {
                current = SEQ[i % SEQ.len()];
                i += 1;
                rig.set(current);
                set_pending = true;
            }
            1 => {
                rig.embed.logic();
                set_pending = false;
            }
            _ => {
                rig.commit();
                if !set_pending {
                    rig.check(current, &format!("step {i}"));
                }
            }
        }
    }
    rig.embed.logic();
    rig.commit();
    rig.check(current, "settled");
}

#[test]
fn sv84_an_interrupted_update_leaves_no_rows_behind() {
    // the flush after a click whose rows take longer than the interrupt budget is stopped
    // part-way through the keyed reconcile (no node's handler is running then, so no root faults);
    // the passes after it still end with exactly the current rows
    let budgets = svelters_skelter::embed::Budgets { warn: std::time::Duration::from_secs(60), interrupt: std::time::Duration::from_millis(150), mount: std::time::Duration::from_secs(60) };
    let mut rig = Rig::with_budgets("interrupted", budgets);
    let steps = svelters_abi::Value::List(SEQ.iter().map(|s| svelters_abi::Value::Str((*s).into())).collect());
    rig.embed.set_root_props(rig.root, Props::new().with("steps", steps)).unwrap();
    rig.embed.logic();
    rig.commit();
    rig.check("", "mounted");
    let button = rig.doc.borrow().children(rig.pane)[0];
    let mut interrupted = 0;
    for (i, f) in SEQ.iter().enumerate() {
        // every other update renders slowly enough to be interrupted
        let slow = if i % 2 == 0 { 60.0 } else { 0.0 };
        rig.embed.set_root_props(rig.root, Props::new().with("slow", slow)).unwrap();
        rig.embed.logic();
        let click = rig.backend.target_event(button, "clicked", vec![]).expect("the button listens");
        rig.backend.queue(svelters_skelter::Output { events: vec![click], ..Default::default() });
        rig.embed.logic();
        rig.commit();
        for e in rig.embed.take_events() {
            match e {
                svelters_skelter::embed::EmbedEvent::Error { root: None, error } if error.kind == svelters_host::ErrorKind::Interrupted => interrupted += 1,
                svelters_skelter::embed::EmbedEvent::Faulted { .. } => panic!("the root faulted: {e:?}"),
                _ => {}
            }
        }
        rig.embed.set_root_props(rig.root, Props::new().with("slow", 0.0)).unwrap();
        for _ in 0..2 {
            rig.embed.logic();
            rig.commit();
        }
        rig.check(f, &format!("filter {f:?} (step {i}) after a slow pass"));
    }
    assert!(interrupted > 0, "some pass was interrupted");
}
