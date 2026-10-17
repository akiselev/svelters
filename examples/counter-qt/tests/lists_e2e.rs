//! M5 on Qt, end to end: the real Svelte `ListView` (fixtures/lists-stress-qt, built by `pnpm build`)
//! running in the QuickJS host on the pooled `QListView`. The twin of
//! `crates/svelters-gtk/tests/lists_e2e.rs`: it clicks the fixture's buttons by label and watches
//! the native model's notifications, the row pool, the visible row text, selection and activation,
//! handle accounting and timings (printed, never asserted). Needs a display (xvfb-run) and the
//! main thread, so no libtest harness.

use counter_qt::hooks;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};
use svelters_abi::BackendRef;
use svelters_host::{Host, HostConfig};

fn main() {
    counter_qt::run_as_test("svelte_list_view_on_qt", body);
}

fn status() -> String {
    hooks::label_starting("items=")
}

fn has_row(text: &str) -> bool {
    hooks::vlist_rows().iter().any(|r| r == text)
}

fn has_index(i: usize) -> bool {
    let p = format!("{i} ");
    hooks::vlist_rows().iter().any(|r| r.starts_with(&p) && r.len() > p.len())
}

struct Step {
    name: &'static str,
    run: Box<dyn Fn()>,
    until: Box<dyn Fn() -> bool>,
}

fn step(name: &'static str, run: impl Fn() + 'static, until: impl Fn() -> bool + 'static) -> Step {
    Step { name, run: Box::new(run), until: Box::new(until) }
}

fn click(label: &'static str) -> impl Fn() {
    move || assert!(hooks::click_button(label), "no button `{label}`")
}

fn click_until(name: &'static str, button: &'static str, until: impl Fn() -> bool + 'static) -> Step {
    step(name, click(button), until)
}

struct Poll {
    queue: VecDeque<Step>,
    current: Option<(Step, Instant)>,
    marks: Vec<(String, f64)>,
    fails: Vec<String>,
    handles: Vec<(&'static str, usize)>,
    done: bool,
}

fn poll(st: Rc<RefCell<Poll>>, quit: BackendRef<'static>) {
    // Steps borrow `st` themselves, so none of them runs while it is borrowed here.
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
                st.borrow_mut().marks.push((step.name.to_string(), t0.elapsed().as_secs_f64() * 1000.0));
            } else if t0.elapsed() > Duration::from_secs(40) {
                let msg = format!("step `{}` timed out; status `{}`, model {}, pool {}", step.name, status(), hooks::vlist_count(), hooks::vlist_pool());
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
    let bundle = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/lists-stress-qt/dist/app.js");
    assert!(bundle.is_file(), "build the fixture first: pnpm --filter @svelters-fixtures/lists-stress-qt build");
    let raw: BackendRef<'static> = unsafe { BackendRef::from_raw(counter_qt::backend()) }.unwrap();
    let cfg = HostConfig { echo_console: false, echo_errors: false, ..HostConfig::default() };
    let host = Host::new(raw, cfg).unwrap();
    host.load_bundle(&bundle).unwrap();

    let st = Rc::new(RefCell::new(Poll { queue: VecDeque::new(), current: None, marks: Vec::new(), fails: Vec::new(), handles: Vec::new(), done: false }));
    let mut steps: Vec<Step> = Vec::new();
    let note = |st: &Rc<RefCell<Poll>>, name: &'static str| {
        let st = st.clone();
        step(name, || {}, move || {
            st.borrow_mut().handles.push((name, hooks::live_handles()));
            true
        })
    };
    // Notifications since the step started must match exactly.
    let expect = |st: &Rc<RefCell<Poll>>, name: &'static str, button: &'static str, want: &'static str, status_prefix: &'static str| {
        let st = st.clone();
        step(name, move || {
            hooks::vlist_changes(true);
            assert!(hooks::click_button(button));
        }, move || {
            let done = status().starts_with(status_prefix) && hooks::vlist_count() >= 0;
            if done {
                let got = hooks::vlist_changes(false);
                if got != want {
                    st.borrow_mut().fails.push(format!("{button}: expected `{want}`, got `{got}`"));
                }
            }
            done
        })
    };
    let at_most = |st: &Rc<RefCell<Poll>>, name: &'static str, button: &'static str, max: usize, settled: fn() -> bool| {
        let st = st.clone();
        step(name, move || {
            hooks::vlist_changes(true);
            assert!(hooks::click_button(button));
        }, move || {
            let done = settled();
            if done {
                let got = hooks::vlist_changes(false);
                let k = got.matches(';').count();
                if k == 0 || k > max {
                    st.borrow_mut().fails.push(format!("{button}: {k} notifications (`{got}`), want 1..={max}"));
                }
            }
            done
        })
    };

    steps.push(note(&st, "start"));
    // `start` runs before the app is up: replace it by waiting for the empty list first.
    steps.clear();
    {
        let st = st.clone();
        steps.push(step("start", || {}, move || {
            let ok = status() == "items=0 selected=-1 activated=-" && hooks::vlist_present();
            if ok && st.borrow().handles.is_empty() {
                st.borrow_mut().handles.push(("start", hooks::live_handles()));
            }
            ok
        }));
    }
    steps.push(click_until("fill10k", "fill10k", || status().starts_with("items=10000 ") && hooks::vlist_count() == 10000 && has_row("0 Item 0")));
    {
        let st = st.clone();
        steps.push(step("pool after fill10k", || {}, move || {
            let pool = hooks::vlist_pool();
            if !(1..=60).contains(&pool) {
                st.borrow_mut().fails.push(format!("{pool} row slots for 10000 items"));
            }
            hooks::vlist_watch();
            true
        }));
    }
    {
        let st = st.clone();
        steps.push(step("rename: no notifications", click("rename"), move || {
            let ok = has_row("0 Item 0!");
            if ok && !hooks::vlist_changes(false).is_empty() {
                st.borrow_mut().fails.push(format!("rename raised `{}`", hooks::vlist_changes(false)));
            }
            ok
        }));
    }
    steps.push(expect(&st, "append", "append", "I 10000 1;", "items=10001 "));
    steps.push(expect(&st, "prepend", "prepend", "I 0 1;", "items=10002 "));
    steps.push(expect(&st, "removeFirst", "removeFirst", "R 0 1;", "items=10001 "));
    steps.push(expect(&st, "removeMiddle", "removeMiddle", "R 5 3;", "items=9998 "));
    steps.push(expect(&st, "moveFirst", "moveFirst", "R 0 1;I 9997 1;", "items=9998 "));
    steps.push(at_most(&st, "reverse", "reverse", 4, || hooks::vlist_count() == 9998 && hooks::vlist_changes(false).matches(';').count() >= 1));
    steps.push(at_most(&st, "sortDesc", "sortDesc", 4, || hooks::vlist_count() == 9998 && hooks::vlist_changes(false).matches(';').count() >= 1));
    steps.push(at_most(&st, "odd filter", "odd", 2, || hooks::vlist_count() > 0 && hooks::vlist_count() < 9998 && status().starts_with("items=")));
    steps.push(step("scrollEnd", click("scrollEnd"), || has_index(hooks::vlist_count() as usize - 1)));
    steps.push(step("scrollStart", click("scrollStart"), || has_index(0)));
    steps.push(step("select", || assert!(hooks::vlist_select(5)), || status().contains("selected=5 ")));
    steps.push(step("activate", || assert!(hooks::vlist_activate(3)), || status().contains("activated=3:")));
    steps.push(click_until("fill100k", "fill100k", || status().starts_with("items=100000 ") && hooks::vlist_count() == 100000 && has_index(0)));
    steps.push(step("scrollEnd 100k", click("scrollEnd"), || has_index(99999)));
    steps.push(step("scrollMid 100k", click("scrollMid"), || has_index(50000)));
    steps.push(click_until("clear", "clear", || status().starts_with("items=0 ") && hooks::vlist_count() == 0));
    steps.push(note(&st, "after first clear"));
    for i in 0..3 {
        steps.push(click_until(["cycle fill 1", "cycle fill 2", "cycle fill 3"][i], "fill10k", || hooks::vlist_count() == 10000 && has_index(0)));
        steps.push(step(["cycle end 1", "cycle end 2", "cycle end 3"][i], click("scrollEnd"), || has_index(9999)));
        steps.push(click_until(["cycle clear 1", "cycle clear 2", "cycle clear 3"][i], "clear", || hooks::vlist_count() == 0));
    }
    steps.push(note(&st, "after cycles"));
    steps.push(click_until("hide list", "toggle", || !hooks::vlist_present()));
    steps.push(note(&st, "hidden"));
    steps.push(click_until("show list", "toggle", hooks::vlist_present));
    steps.push(note(&st, "shown again"));

    let total = steps.len();
    st.borrow_mut().queue = steps.into_iter().collect();
    let st2 = st.clone();
    hooks::run_later(15, Box::new(move || poll(st2, raw)));
    hooks::run_later(300_000, Box::new(move || raw.quit())); // safety net
    host.run().unwrap();

    let errs: Vec<String> = host.take_errors().into_iter().map(|e| format!("{}\n{}", e.message, e.stack.unwrap_or_default())).collect();
    let s = st.borrow();
    eprintln!("--- timings (ms, {total} steps; click to condition, includes polling at 15 ms) ---");
    for (name, ms) in &s.marks {
        eprintln!("{name:28} {ms:9.1}");
    }
    eprintln!("--- handles: {:?}", s.handles);
    assert!(errs.is_empty(), "{errs:?}");
    assert!(s.fails.is_empty(), "{:#?}", s.fails);
    assert_eq!(s.marks.len(), total, "every step finished");
    let get = |k: &str| s.handles.iter().find(|(n, _)| *n == k).unwrap().1;
    assert_eq!(get("hidden") + 1, get("start"), "hiding the list released the view and every row handle");
    assert_eq!(get("shown again"), get("start"), "showing it again restores exactly the handles of an empty list");
    assert!(get("after cycles") <= get("after first clear") + 40, "handles grew across fill/scroll/clear cycles: {:?}", s.handles);
    println!("lists_e2e ok");
}
