//! N3 on GTK, end to end: native list models feeding `@svelters/lists` (`tests/gtk-types-models`, built by
//! `pnpm -r build`) in the QuickJS host on GtkListView. The test clicks the fixture's buttons by label and
//! watches the native view: the model it shows, the `items-changed` of the app's model, the row slots and
//! their text, selection, activation and timings. Covers a `Gio.ListStore` mutated from JS (one native
//! change per mutation, rows keep their index), a filter/sort chain with JS functions, reactive getters on
//! items, an app-owned `GtkSingleSelection`, a 10k-file `Gtk.DirectoryList` (pooled rows, scrolling),
//! `Gtk.StringList`s of 10k/100k and the JS-array path at the same sizes for comparison.
//! Needs a display (xvfb-run); timings are printed, never asserted (the machine is shared).

use gtk::glib;
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};
use svelters_abi::BackendRef;
use svelters_gtk::testing::on_gtk;
use svelters_gtk::GtkBackend;
use svelters_host::{Host, HostConfig};

const DIR_FILES: usize = 10_000;

fn walk(w: &gtk::Widget, f: &mut dyn FnMut(&gtk::Widget)) {
    f(w);
    let mut c = w.first_child();
    while let Some(x) = c {
        walk(&x, f);
        c = x.next_sibling();
    }
}

type Changes = Rc<RefCell<Vec<(u32, u32, u32)>>>;

struct Ctx {
    changes: Changes,
    /// The model the counter is connected to, and the handler.
    counted: RefCell<Option<(gtk::gio::ListModel, glib::SignalHandlerId)>>,
}

impl Ctx {
    fn win(&self) -> gtk::Widget {
        // The app constructs its models before the window, so the window is not handle 1.
        gtk::Window::list_toplevels().into_iter().find(|w| w.downcast_ref::<gtk::Window>().and_then(|w| w.title()).as_deref() == Some("Native models")).expect("the fixture's window")
    }

    fn find<T: IsA<gtk::Widget>>(&self, pred: impl Fn(&T) -> bool) -> Option<T> {
        let mut found = None;
        walk(&self.win(), &mut |w| {
            if found.is_none() {
                if let Some(x) = w.downcast_ref::<T>() {
                    if pred(x) {
                        found = Some(x.clone());
                    }
                }
            }
        });
        found
    }

    fn click(&self, label: &str) {
        self.find::<gtk::Button>(|b| b.label().as_deref() == Some(label)).unwrap_or_else(|| panic!("no button `{label}`")).emit_clicked();
    }

    fn status(&self) -> String {
        self.find::<gtk::Label>(|l| l.text().starts_with("mode=")).map(|l| l.text().to_string()).unwrap_or_default()
    }

    fn view(&self) -> Option<gtk::ListView> {
        self.find::<gtk::ListView>(|_| true)
    }

    /// The selection model the view shows.
    fn selection(&self) -> Option<gtk::SelectionModel> {
        self.view()?.model()
    }

    /// The app's model behind the view: the model our `GtkSingleSelection` wraps, or the app's own
    /// selection model when the view shows that one directly.
    fn user_model(&self) -> Option<gtk::gio::ListModel> {
        let sel = self.selection()?;
        if self.status().starts_with("mode=sel ") {
            return Some(sel.upcast());
        }
        sel.downcast_ref::<gtk::SingleSelection>().and_then(|s| s.model())
    }

    fn n(&self) -> u32 {
        self.user_model().map(|m| m.n_items()).unwrap_or(0)
    }

    /// Texts of the labels in every row slot, one entry per slot (`"index text"`).
    fn slots(&self) -> Vec<String> {
        let Some(v) = self.view() else { return vec![] };
        let mut out = Vec::new();
        walk(v.upcast_ref(), &mut |w| {
            if w.type_().name() == "GtkListItemWidget" {
                let mut parts = Vec::new();
                walk(w, &mut |x| {
                    if let Some(l) = x.downcast_ref::<gtk::Label>() {
                        parts.push(l.text().to_string());
                    }
                });
                out.push(parts.join(" "));
            }
        });
        out
    }

    fn has_row(&self, text: &str) -> bool {
        self.slots().iter().any(|s| s == text)
    }

    fn has_index(&self, index: u32) -> bool {
        let p = format!("{index} ");
        self.slots().iter().any(|s| s.starts_with(&p) && s.len() > p.len())
    }

    /// Counts the `items-changed` of the app's model from now on.
    fn count_changes(&self) {
        if let Some((m, id)) = self.counted.borrow_mut().take() {
            m.disconnect(id);
        }
        self.changes.borrow_mut().clear();
        if let Some(m) = self.user_model() {
            let c = self.changes.clone();
            let id = m.connect_items_changed(move |_, p, r, a| c.borrow_mut().push((p, r, a)));
            *self.counted.borrow_mut() = Some((m, id));
        }
    }
}

struct Step {
    name: String,
    run: Box<dyn Fn(&Ctx)>,
    until: Box<dyn Fn(&Ctx) -> bool>,
}

fn step(name: &str, run: impl Fn(&Ctx) + 'static, until: impl Fn(&Ctx) -> bool + 'static) -> Step {
    Step { name: name.to_string(), run: Box::new(run), until: Box::new(until) }
}

fn click_until(name: &str, button: &'static str, until: impl Fn(&Ctx) -> bool + 'static) -> Step {
    step(name, move |c| c.click(button), until)
}

type Fails = Rc<RefCell<Vec<String>>>;

/// Click `button` and wait for `until`; then the app's model must have raised exactly `expect`.
fn mutation(name: &'static str, button: &'static str, expect: Vec<(u32, u32, u32)>, until: impl Fn(&Ctx) -> bool + 'static, fails: Fails) -> Step {
    step(name, move |c| {
        c.count_changes();
        c.click(button)
    }, move |c| {
        let done = until(c);
        if done && *c.changes.borrow() != expect {
            fails.borrow_mut().push(format!("{button}: expected items-changed {expect:?}, got {:?}", c.changes.borrow()));
        }
        done
    })
}

fn check(name: &str, fails: Fails, f: impl Fn(&Ctx) -> Option<String> + 'static) -> Step {
    step(name, |_| {}, move |c| {
        if let Some(e) = f(c) {
            fails.borrow_mut().push(e);
        }
        true
    })
}

/// The scroll and size steps of one big list (`what`), recording the row-slot count after each.
fn big_list(steps: &mut Vec<Step>, what: &'static str, open: Step, n: u32, fails: Fails, pool: Rc<RefCell<Vec<(String, usize)>>>) {
    steps.push(open);
    for (label, button, target) in [("scrollEnd", "scrollEnd", n - 1), ("scrollMid", "scrollMid", n / 2), ("scrollStart", "scrollStart", 0)] {
        let p = pool.clone();
        let f = fails.clone();
        steps.push(step(&format!("{what} {label}"), move |c| c.click(button), move |c| {
            let ok = c.has_index(target);
            if ok {
                let slots = c.slots().len();
                p.borrow_mut().push((format!("{what} {label}"), slots));
                if slots > 250 {
                    f.borrow_mut().push(format!("{what}: {slots} row slots for {n} items"));
                }
            }
            ok
        }));
    }
}

#[test]
fn native_models_feed_list_view_on_gtk() {
    let bundle = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/gtk-types-models/dist/app.js");
    assert!(bundle.is_file(), "build the fixture first: pnpm --filter @svelters-fixtures/gtk-types-models build");
    // The directory the DirectoryList enumerates: 10k empty files.
    let dir = std::env::temp_dir().join(format!("svelters-n3-dir-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for i in 0..DIR_FILES {
        std::fs::File::create(dir.join(format!("f{i:05}"))).unwrap();
    }
    let dir_s = dir.to_string_lossy().into_owned();
    on_gtk(move || {
        let backend = Rc::new(GtkBackend::new().expect("gtk"));
        let raw = unsafe { BackendRef::from_raw(backend.as_raw()) }.unwrap();
        let cfg = HostConfig { echo_console: false, echo_errors: false, ..HostConfig::default() };
        let host = Host::new(raw, cfg).unwrap();
        // The component runs JS callbacks (the filter and sorter) while it mounts, before `run`.
        unsafe { backend.attach_host(host.host_callbacks()) };
        host.load_bundle(&bundle).unwrap();

        let ctx = Rc::new(Ctx { changes: Rc::default(), counted: RefCell::new(None) });
        let fails: Fails = Rc::default();
        let marks: Rc<RefCell<Vec<(String, f64)>>> = Rc::default();
        let pool: Rc<RefCell<Vec<(String, usize)>>> = Rc::default();
        let mut steps: Vec<Step> = Vec::new();

        steps.push(step("start", |_| {}, |c| c.status().starts_with("mode=store n=0 ") && c.view().is_some()));
        // ---- a Gio.ListStore mutated from JS: one native change per mutation --------------------------
        steps.push(mutation("store fill", "fill", vec![(0, 0, 10)], |c| c.status().starts_with("mode=store n=10 ") && c.has_row("0 S0") && c.has_row("2 S2"), fails.clone()));
        steps.push(mutation("store insert", "insert", vec![(2, 0, 1)], |c| c.status().starts_with("mode=store n=11 ") && c.has_row("2 ins"), fails.clone()));
        // The row that showed S2 at 2 keeps its item and follows it to 3 (epoch-ordered shift, no rebind needed).
        steps.push(check("store rows follow", fails.clone(), |c| (!c.has_row("3 S2") || c.has_row("2 S2")).then(|| format!("after insert: {:?}", c.slots()))));
        steps.push(mutation("store remove", "remove", vec![(0, 1, 0)], |c| c.status().starts_with("mode=store n=10 ") && c.has_row("0 S1") && c.has_row("1 ins"), fails.clone()));
        steps.push(mutation("store splice", "splice", vec![(1, 2, 3)], |c| c.status().starts_with("mode=store n=11 ") && c.has_row("1 x") && c.has_row("3 z"), fails.clone()));
        steps.push(mutation("store sort", "sort", vec![(0, 11, 11)], |c| c.has_row("0 z") && c.has_row("1 y") && c.has_row("3 S9") && !c.has_row("1 x"), fails.clone()));
        steps.push(step("activate", |c| c.view().unwrap().emit_by_name::<()>("activate", &[&1u32]), |c| c.status().contains("activated=1:y ")));
        steps.push(step("select natively", |c| c.selection().unwrap().downcast::<gtk::SingleSelection>().unwrap().set_selected(2), |c| c.status().contains("selected=2 ")));
        // ---- a filter/sort chain with JS functions ----------------------------------------------------------
        steps.push(click_until("chain", "chain", |c| c.status().starts_with("mode=chain n=3 ") && c.has_row("0 e") && c.has_row("1 c") && c.has_row("2 a")));
        steps.push(click_until("chain add", "chainAdd", |c| c.status().starts_with("mode=chain n=4 ") && c.has_row("0 g") && c.has_row("1 e") && c.has_row("3 a")));
        steps.push(click_until("chain refilter", "refilter", |c| c.status().starts_with("mode=chain n=3 ") && c.has_row("0 f") && c.has_row("1 d") && c.has_row("2 b")));
        // ---- reactive getters on items -------------------------------------------------------------------
        steps.push(click_until("adj", "adj", |c| c.status().starts_with("mode=adj n=3 ") && c.has_row("0 v=1") && c.has_row("2 v=3")));
        steps.push(step("adj native change", |c| {
            let m = c.user_model().unwrap();
            m.item(0).and_downcast::<gtk::Adjustment>().unwrap().set_value(42.0);
        }, |c| c.has_row("0 v=42")));
        // ---- the app's own selection model, shown as is; its `selected` is reactive ------------------------
        steps.push(click_until("sel", "sel", |c| c.status().starts_with("mode=sel n=11 ") && c.has_row("0 z")));
        {
            let f = fails.clone();
            steps.push(step("sel natively", |c| c.selection().unwrap().downcast::<gtk::SingleSelection>().unwrap().set_selected(1), move |c| {
                let ok = c.status().ends_with(" sel=1");
                if ok && c.view().unwrap().model().unwrap().downcast::<gtk::SingleSelection>().unwrap().model().is_none() {
                    f.borrow_mut().push("sel: the view does not show the app's selection model".into());
                }
                ok
            }));
        }
        // ---- a 10k-file DirectoryList: lazy loading, n-items growth, pooled rows ----------------------------
        {
            let d = dir_s.clone();
            steps.push(step("type path", move |c| c.find::<gtk::Entry>(|_| true).unwrap().set_text(&d), |_| true));
        }
        steps.push(click_until("dir first rows", "dir", |c| c.status().starts_with("mode=dir ") && c.n() > 0 && c.has_index(0)));
        steps.push(step("dir loaded", |_| {}, |c| c.n() == DIR_FILES as u32 && c.status().contains(&format!("n={DIR_FILES} ")) && c.status().contains("loading=false")));
        big_list(&mut steps, "dir 10k", check("dir slots", fails.clone(), |_| None), DIR_FILES as u32, fails.clone(), pool.clone());
        // ---- StringList 10k / 100k and the JS-array path at the same sizes ---------------------------------
        for (what, button, n) in [("strings 10k", "str10k", 10_000u32), ("strings 100k", "str100k", 100_000), ("array 10k", "arr10k", 10_000), ("array 100k", "arr100k", 100_000)] {
            let open = click_until(&format!("{what} fill"), button, move |c| c.status().contains(&format!(" n={n} ")) && c.has_row("0 Item 0"));
            big_list(&mut steps, what, open, n, fails.clone(), pool.clone());
        }
        steps.push(click_until("back to store", "store", |c| c.status().starts_with("mode=store n=11 ") && c.has_row("0 z")));

        let total = steps.len();
        let queue = Rc::new(RefCell::new(steps.into_iter().collect::<std::collections::VecDeque<_>>()));
        let current: Rc<RefCell<Option<(Step, Instant)>>> = Rc::default();
        let quit = raw;
        let (c2, m2, f2, q2, cur2) = (ctx.clone(), marks.clone(), fails.clone(), queue.clone(), current.clone());
        let done = Rc::new(Cell::new(false));
        let d2 = done.clone();
        glib::timeout_add_local(Duration::from_millis(15), move || {
            let mut cur = cur2.borrow_mut();
            if cur.is_none() {
                let Some(s) = q2.borrow_mut().pop_front() else {
                    if !d2.replace(true) {
                        glib::timeout_add_local_once(Duration::from_millis(100), move || quit.quit());
                    }
                    return glib::ControlFlow::Continue;
                };
                (s.run)(&c2);
                *cur = Some((s, Instant::now()));
                return glib::ControlFlow::Continue;
            }
            let (s, t0) = cur.as_ref().unwrap();
            if (s.until)(&c2) {
                m2.borrow_mut().push((s.name.clone(), t0.elapsed().as_secs_f64() * 1000.0));
                *cur = None;
            } else if t0.elapsed() > Duration::from_secs(60) {
                f2.borrow_mut().push(format!("step `{}` timed out; status `{}`, model {}, slots {:?}", s.name, c2.status(), c2.n(), c2.slots().iter().take(4).collect::<Vec<_>>()));
                q2.borrow_mut().clear();
                *cur = None;
            }
            glib::ControlFlow::Continue
        });
        glib::timeout_add_local_once(Duration::from_secs(600), move || quit.quit());
        host.run().unwrap();

        let errs: Vec<String> = host.take_errors().into_iter().map(|e| format!("{}\n{}", e.message, e.stack.unwrap_or_default())).collect();
        eprintln!("--- timings (ms, {total} steps; action to condition, includes polling at 15 ms) ---");
        for (name, ms) in marks.borrow().iter() {
            eprintln!("{name:28} {ms:9.1}");
        }
        eprintln!("--- row slots: {:?}", pool.borrow());
        for e in &errs {
            eprintln!("--- JS error: {}", e.chars().take(600).collect::<String>());
        }
        let _ = std::fs::remove_dir_all(&dir);
        assert!(errs.is_empty(), "{errs:?}");
        assert!(fails.borrow().is_empty(), "{:#?}", fails.borrow());
        assert_eq!(marks.borrow().len(), total, "every step finished");
    });
}
