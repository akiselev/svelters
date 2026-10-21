//! N3 on Qt, end to end: native item models feeding `@svelters/lists` (`tests/qt-types-models`, built by
//! `pnpm build`) in the QuickJS host on the pooled `QListView`. The twin of
//! `crates/svelters-gtk/tests/lists_models.rs`, through the same model-mode contract: a `QStringListModel`
//! mutated from JS (one native row signal per mutation, rows keep their item and index, a dataChanged
//! re-renders the row, a sort is one layout change), a `QSortFilterProxyModel` chain, 10k/100k-row models
//! (pooled rows, scrolling) and the JS-array path at the same sizes. Timings are printed, never asserted.
//! Needs a display (xvfb-run) and the main thread, so no libtest harness.

use counter_qt::hooks;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};
use svelters_abi::BackendRef;
use svelters_host::{Host, HostConfig};

fn main() {
    counter_qt::run_as_test("native_models_on_qt", body);
}

fn status() -> String {
    hooks::label_starting("mode=")
}

fn has_row(text: &str) -> bool {
    hooks::vlist_rows().iter().any(|r| r == text)
}

fn has_index(i: i64) -> bool {
    let p = format!("{i} ");
    hooks::vlist_rows().iter().any(|r| r.starts_with(&p) && r.len() > p.len())
}

struct Step {
    name: String,
    run: Box<dyn Fn()>,
    until: Box<dyn Fn() -> bool>,
}

fn step(name: &str, run: impl Fn() + 'static, until: impl Fn() -> bool + 'static) -> Step {
    Step { name: name.to_string(), run: Box::new(run), until: Box::new(until) }
}

fn click(label: &'static str) -> impl Fn() {
    move || assert!(hooks::click_button(label), "no button `{label}`")
}

fn click_until(name: &str, button: &'static str, until: impl Fn() -> bool + 'static) -> Step {
    step(name, click(button), until)
}

#[derive(Default)]
struct Poll {
    queue: VecDeque<Step>,
    current: Option<(Step, Instant)>,
    marks: Vec<(String, f64)>,
    fails: Vec<String>,
    pools: Vec<(String, i32)>,
    done: bool,
}

fn poll(st: Rc<RefCell<Poll>>, quit: BackendRef<'static>) {
    let cur = st.borrow_mut().current.take();
    match cur {
        None => {
            let next = st.borrow_mut().queue.pop_front();
            match next {
                Some(step) => {
                    (step.run)();
                    st.borrow_mut().current = Some((step, Instant::now()));
                }
                None => {
                    if !st.borrow().done {
                        st.borrow_mut().done = true;
                        hooks::run_later(100, Box::new(move || quit.quit()));
                    }
                    return;
                }
            }
        }
        Some((step, t0)) => {
            if (step.until)() {
                st.borrow_mut().marks.push((step.name.clone(), t0.elapsed().as_secs_f64() * 1000.0));
            } else if t0.elapsed() > Duration::from_secs(60) {
                let msg = format!("step `{}` timed out; status `{}`, model {}, rows {:?}", step.name, status(), hooks::vlist_count(), hooks::vlist_rows().iter().take(4).collect::<Vec<_>>());
                let mut s = st.borrow_mut();
                s.fails.push(msg);
                s.queue.clear();
            } else {
                st.borrow_mut().current = Some((step, t0));
            }
        }
    }
    let st2 = st.clone();
    hooks::run_later(15, Box::new(move || poll(st2, quit)));
}

fn body() {
    let bundle = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/qt-types-models/dist/app.js");
    assert!(bundle.is_file(), "build the fixture first: pnpm --filter @svelters-fixtures/qt-types-models build");
    let raw: BackendRef<'static> = unsafe { BackendRef::from_raw(counter_qt::backend()) }.unwrap();
    let cfg = HostConfig { echo_console: false, echo_errors: false, ..HostConfig::default() };
    let host = Host::new(raw, cfg).unwrap();
    host.load_bundle(&bundle).unwrap();

    let st = Rc::new(RefCell::new(Poll::default()));
    // The shown model's notifications since the click must be exactly `want`.
    let mutation = |st: &Rc<RefCell<Poll>>, button: &'static str, want: &'static str, until: fn() -> bool| {
        let st = st.clone();
        step(&format!("store {button}"), move || {
            hooks::vlist_watch();
            hooks::vlist_changes(true);
            assert!(hooks::click_button(button));
        }, move || {
            let done = until();
            if done {
                let got = hooks::vlist_changes(false);
                if got != want {
                    st.borrow_mut().fails.push(format!("{button}: expected `{want}`, got `{got}`"));
                }
            }
            done
        })
    };
    let mut steps: Vec<Step> = Vec::new();
    steps.push(step("start", || {}, || status().starts_with("mode=store ") && hooks::vlist_present()));
    steps.push(mutation(&st, "fill", "reset;", || hooks::vlist_count() == 10 && has_row("0 S0") && has_row("2 S2")));
    steps.push(mutation(&st, "insert", "I 2 1;C 2 1;", || hooks::vlist_count() == 11 && has_row("2 ins") && has_row("3 S2")));
    steps.push(mutation(&st, "remove", "R 0 1;", || hooks::vlist_count() == 10 && has_row("0 S1") && has_row("1 ins") && has_row("2 S2")));
    steps.push(mutation(&st, "rename", "C 0 1;", || has_row("0 renamed")));
    steps.push(mutation(&st, "sort", "layout;", || has_row("0 renamed") && has_row("1 ins") && has_row("2 S9") && has_row("9 S2")));
    steps.push(step("activate", || assert!(hooks::vlist_activate(1)), || status().contains("activated=1:ins")));
    steps.push(step("select natively", || assert!(hooks::vlist_select(2)), || status().contains("selected=2 ")));
    steps.push(click_until("chain", "chain", || status().starts_with("mode=chain ") && hooks::vlist_count() == 3 && has_row("0 e") && has_row("1 c") && has_row("2 a")));
    steps.push(click_until("chain add", "chainAdd", || hooks::vlist_count() == 4 && has_row("0 g") && has_row("1 e") && has_row("3 a")));
    steps.push(click_until("chain refilter", "refilter", || hooks::vlist_count() == 3 && has_row("0 f") && has_row("1 d") && has_row("2 b")));
    for (what, button, n) in [("strings 10k", "str10k", 10_000i64), ("strings 100k", "str100k", 100_000), ("array 10k", "arr10k", 10_000), ("array 100k", "arr100k", 100_000)] {
        steps.push(click_until(&format!("{what} fill"), button, move || hooks::vlist_count() == n && has_row("0 Item 0")));
        for (label, b, target) in [("scrollEnd", "scrollEnd", n - 1), ("scrollMid", "scrollMid", n / 2), ("scrollStart", "scrollStart", 0)] {
            let st = st.clone();
            steps.push(step(&format!("{what} {label}"), click(b), move || {
                let ok = has_index(target);
                if ok {
                    let pool = hooks::vlist_pool();
                    st.borrow_mut().pools.push((format!("{what} {label}"), pool));
                    if !(1..=60).contains(&pool) {
                        st.borrow_mut().fails.push(format!("{what}: {pool} row slots for {n} items"));
                    }
                }
                ok
            }));
        }
    }
    steps.push(click_until("back to store", "store", || status().starts_with("mode=store ") && has_row("0 renamed")));

    let total = steps.len();
    st.borrow_mut().queue = steps.into_iter().collect();
    let st2 = st.clone();
    hooks::run_later(15, Box::new(move || poll(st2, raw)));
    hooks::run_later(600_000, Box::new(move || raw.quit())); // safety net
    host.run().unwrap();

    let errs: Vec<String> = host.take_errors().into_iter().map(|e| format!("{}\n{}", e.message, e.stack.unwrap_or_default())).collect();
    let s = st.borrow();
    eprintln!("--- timings (ms, {total} steps; action to condition, includes polling at 15 ms) ---");
    for (name, ms) in &s.marks {
        eprintln!("{name:28} {ms:9.1}");
    }
    eprintln!("--- row slots: {:?}", s.pools);
    assert!(errs.is_empty(), "{errs:?}");
    assert!(s.fails.is_empty(), "{:#?}", s.fails);
    assert_eq!(s.marks.len(), total, "every step finished");
    println!("lists_models ok");
}
