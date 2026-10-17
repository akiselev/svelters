//! M5 on GTK, end to end: the real Svelte `ListView` (fixtures/lists-stress, built by
//! `pnpm build`/`pnpm test`) running in the QuickJS host on GtkListView. The test clicks the
//! fixture's buttons by label and watches the native model (items-changed notifications), the row
//! slots, the visible row text, selection and activation, handle accounting, and timings.
//! Needs a display (xvfb-run); `GSK_RENDERER=cairo` avoids the GPU path. Timings are printed, never
//! asserted (the machine is shared).

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

const WIN: u64 = 1;

fn walk(w: &gtk::Widget, f: &mut dyn FnMut(&gtk::Widget)) {
    f(w);
    let mut c = w.first_child();
    while let Some(x) = c {
        walk(&x, f);
        c = x.next_sibling();
    }
}

struct Ctx {
    backend: Rc<GtkBackend>,
    changes: Rc<RefCell<Vec<(u32, u32, u32)>>>,
    connected: Cell<bool>,
}

impl Ctx {
    fn win(&self) -> gtk::Widget {
        self.backend.object(WIN).unwrap().downcast().unwrap()
    }

    fn click(&self, label: &str) {
        let mut found = None;
        walk(&self.win(), &mut |w| {
            if let Some(b) = w.downcast_ref::<gtk::Button>() {
                if b.label().as_deref() == Some(label) {
                    found = Some(b.clone());
                }
            }
        });
        found.unwrap_or_else(|| panic!("no button `{label}`")).emit_clicked();
    }

    fn status(&self) -> String {
        let mut s = String::new();
        walk(&self.win(), &mut |w| {
            if let Some(l) = w.downcast_ref::<gtk::Label>() {
                if l.text().starts_with("items=") {
                    s = l.text().to_string();
                }
            }
        });
        s
    }

    fn view(&self) -> Option<gtk::ListView> {
        let mut v = None;
        walk(&self.win(), &mut |w| {
            if let Some(x) = w.downcast_ref::<gtk::ListView>() {
                v = Some(x.clone());
            }
        });
        v
    }

    fn model(&self) -> Option<gtk::gio::ListModel> {
        let sel = self.view()?.model()?;
        sel.downcast_ref::<gtk::SingleSelection>().and_then(|s| s.model())
    }

    fn model_len(&self) -> u32 {
        self.model().map(|m| m.n_items()).unwrap_or(0)
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

    fn has_index(&self, index: usize) -> bool {
        let p = format!("{index} ");
        self.slots().iter().any(|s| s.starts_with(&p) && s.len() > p.len())
    }

    fn connect_counter(&self) {
        if let Some(m) = self.model() {
            let c = self.changes.clone();
            m.connect_items_changed(move |_, p, r, a| c.borrow_mut().push((p, r, a)));
            self.connected.set(true);
        }
    }
}

struct Step {
    name: &'static str,
    run: Box<dyn Fn(&Ctx)>,
    until: Box<dyn Fn(&Ctx) -> bool>,
}

fn step(name: &'static str, run: impl Fn(&Ctx) + 'static, until: impl Fn(&Ctx) -> bool + 'static) -> Step {
    Step { name, run: Box::new(run), until: Box::new(until) }
}

fn click_until(name: &'static str, button: &'static str, until: impl Fn(&Ctx) -> bool + 'static) -> Step {
    step(name, move |c| c.click(button), until)
}

#[test]
fn svelte_list_view_on_gtk() {
    let bundle = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/lists-stress/dist/app.js");
    assert!(bundle.is_file(), "build the fixture first: pnpm --filter @svelters-fixtures/lists-stress build");
    on_gtk(move || {
        let backend = Rc::new(GtkBackend::new().expect("gtk"));
        let raw = unsafe { BackendRef::from_raw(backend.as_raw()) }.unwrap();
        let cfg = HostConfig { echo_console: false, echo_errors: false, ..HostConfig::default() };
        let host = Host::new(raw, cfg).unwrap();
        host.load_bundle(&bundle).unwrap();

        let ctx = Rc::new(Ctx { backend: backend.clone(), changes: Rc::default(), connected: Cell::new(false) });
        let fails: Rc<RefCell<Vec<String>>> = Rc::default();
        let marks: Rc<RefCell<Vec<(String, f64)>>> = Rc::default();
        let handles: Rc<RefCell<Vec<(&'static str, usize)>>> = Rc::default();

        let n = |c: &Ctx| c.changes.borrow().len();
        let mut steps: Vec<Step> = Vec::new();
        {
            let h = handles.clone();
            steps.push(step("start", |_| {}, move |c| {
                let ok = c.status() == "items=0 selected=-1 activated=-" && c.view().is_some();
                if ok && h.borrow().is_empty() {
                    h.borrow_mut().push(("start", c.backend.live_handles()));
                }
                ok
            }));
        }
        steps.push(click_until("fill10k", "fill10k", |c| c.status().starts_with("items=10000 ") && c.model_len() == 10000 && c.has_row("0 Item 0")));
        {
            let f = fails.clone();
            steps.push(step("slots after fill10k", |_| {}, move |c| {
                let slots = c.slots().len();
                if slots > 250 {
                    f.borrow_mut().push(format!("{slots} row slots for 10000 items"));
                }
                c.connect_counter();
                true
            }));
        }
        {
            let f = fails.clone();
            steps.push(step("rename: no notifications", |c| c.click("rename"), move |c| {
                // Same keys, new item objects: the bound rows update, the model is untouched.
                let ok = c.has_row("0 Item 0!");
                if ok && !c.changes.borrow().is_empty() {
                    f.borrow_mut().push(format!("rename raised {:?}", c.changes.borrow()));
                }
                ok
            }));
        }
        let check = |name: &'static str, button: &'static str, expect: Vec<(u32, u32, u32)>, status: &'static str, fails: Rc<RefCell<Vec<String>>>| {
            step(name, move |c| {
                c.changes.borrow_mut().clear();
                c.click(button)
            }, move |c| {
                let done = c.status().starts_with(status);
                if done && *c.changes.borrow() != expect {
                    fails.borrow_mut().push(format!("{button}: expected {expect:?}, got {:?}", c.changes.borrow()));
                }
                done
            })
        };
        steps.push(check("append", "append", vec![(10000, 0, 1)], "items=10001 ", fails.clone()));
        steps.push(check("prepend", "prepend", vec![(0, 0, 1)], "items=10002 ", fails.clone()));
        steps.push(check("removeFirst", "removeFirst", vec![(0, 1, 0)], "items=10001 ", fails.clone()));
        steps.push(check("removeMiddle", "removeMiddle", vec![(5, 3, 0)], "items=9998 ", fails.clone()));
        steps.push(check("moveFirst", "moveFirst", vec![(0, 1, 0), (9997, 0, 1)], "items=9998 ", fails.clone()));
        for (name, button) in [("reverse", "reverse"), ("sortDesc", "sortDesc")] {
            let f = fails.clone();
            steps.push(step(name, move |c| {
                c.changes.borrow_mut().clear();
                c.click(button)
            }, move |c| {
                let k = n(c);
                // The notifications are final once the list shows the new order.
                let done = k > 0 && c.model_len() == 9998;
                if done && k > 4 {
                    f.borrow_mut().push(format!("{button} raised {k} notifications"));
                }
                done
            }));
        }
        {
            let f = fails.clone();
            steps.push(step("odd filter", |c| {
                c.changes.borrow_mut().clear();
                c.click("odd")
            }, move |c| {
                let done = c.model_len() < 9998 && c.status().starts_with("items=");
                if done && n(c) != 1 {
                    f.borrow_mut().push(format!("filter raised {} notifications, want 1", n(c)));
                }
                done
            }));
        }
        steps.push(step("scrollEnd", |c| c.click("scrollEnd"), |c| c.has_index(c.model_len() as usize - 1)));
        steps.push(step("scrollStart", |c| c.click("scrollStart"), |c| c.has_index(0)));
        steps.push(step("select", |c| c.view().unwrap().model().unwrap().downcast::<gtk::SingleSelection>().unwrap().set_selected(5), |c| c.status().contains("selected=5 ")));
        steps.push(step("activate", |c| c.view().unwrap().emit_by_name::<()>("activate", &[&3u32]), |c| {
            let s = c.status();
            s.contains("activated=3:")
        }));
        steps.push(click_until("fill100k", "fill100k", |c| c.status().starts_with("items=100000 ") && c.model_len() == 100000 && c.has_index(0)));
        steps.push(step("scrollEnd 100k", |c| c.click("scrollEnd"), |c| c.has_index(99999)));
        steps.push(step("scrollMid 100k", |c| c.click("scrollMid"), |c| c.has_index(50000)));
        steps.push(click_until("clear", "clear", |c| c.status().starts_with("items=0 ") && c.model_len() == 0));
        {
            let h = handles.clone();
            steps.push(step("handles before cycles", |_| {}, move |c| {
                h.borrow_mut().push(("after first clear", c.backend.live_handles()));
                true
            }));
        }
        for i in 0..3 {
            steps.push(click_until(["cycle fill 1", "cycle fill 2", "cycle fill 3"][i], "fill10k", |c| c.model_len() == 10000 && c.has_index(0)));
            steps.push(step(["cycle end 1", "cycle end 2", "cycle end 3"][i], |c| c.click("scrollEnd"), |c| c.has_index(9999)));
            steps.push(click_until(["cycle clear 1", "cycle clear 2", "cycle clear 3"][i], "clear", |c| c.model_len() == 0));
        }
        {
            let h = handles.clone();
            steps.push(step("handles after cycles", |_| {}, move |c| {
                h.borrow_mut().push(("after cycles", c.backend.live_handles()));
                true
            }));
        }
        steps.push(click_until("hide list", "toggle", |c| c.view().is_none()));
        {
            let h = handles.clone();
            steps.push(step("handles hidden", |_| {}, move |c| {
                h.borrow_mut().push(("hidden", c.backend.live_handles()));
                true
            }));
        }
        steps.push(click_until("show list", "toggle", |c| c.view().is_some()));
        {
            let h = handles.clone();
            steps.push(step("handles shown again", |_| {}, move |c| {
                h.borrow_mut().push(("shown again", c.backend.live_handles()));
                true
            }));
        }

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
                m2.borrow_mut().push((s.name.to_string(), t0.elapsed().as_secs_f64() * 1000.0));
                *cur = None;
            } else if t0.elapsed() > Duration::from_secs(40) {
                f2.borrow_mut().push(format!("step `{}` timed out; status `{}`, model {}, slots {:?}", s.name, c2.status(), c2.model_len(), c2.slots().iter().take(3).collect::<Vec<_>>()));
                q2.borrow_mut().clear();
                *cur = None;
            }
            glib::ControlFlow::Continue
        });
        glib::timeout_add_local_once(Duration::from_secs(300), move || quit.quit());
        host.run().unwrap();

        let errs: Vec<String> = host.take_errors().into_iter().map(|e| format!("{}\n{}", e.message, e.stack.unwrap_or_default())).collect();
        eprintln!("--- timings (ms, {total} steps; click to condition, includes polling at 15 ms) ---");
        for (name, ms) in marks.borrow().iter() {
            eprintln!("{name:28} {ms:9.1}");
        }
        eprintln!("--- handles: {:?}", handles.borrow());
        assert!(errs.is_empty(), "{errs:?}");
        assert!(fails.borrow().is_empty(), "{:#?}", fails.borrow());
        assert_eq!(marks.borrow().len(), total, "every step finished");
        let hs = handles.borrow();
        let get = |k: &str| hs.iter().find(|(n, _)| *n == k).unwrap().1;
        assert_eq!(get("hidden") + 1, get("start"), "hiding the list released the view and every row handle");
        assert_eq!(get("shown again"), get("start"), "showing it again restores exactly the handles of an empty list");
        assert!(get("after cycles") <= get("after first clear") + 40, "handles grew across fill/scroll/clear cycles: {hs:?}");
    });
}
