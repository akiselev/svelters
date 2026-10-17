//! GTK4 ownership experiments for svelters. Prints "ok"/"FAIL"/"info" lines per case.
use gtk::glib;
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

thread_local!(static FAILS: Cell<u32> = Cell::new(0));
macro_rules! check {
    ($c:expr) => {{
        let ok = $c;
        println!("  {} {}", if ok { "ok  " } else { "FAIL" }, stringify!($c));
        if !ok { FAILS.with(|f| f.set(f.get() + 1)); }
    }};
}

fn spin() {
    // time-based drain: GTK holds transient refs from internal timeouts (e.g. ~250ms after a click)
    let ctx = glib::MainContext::default();
    let t = std::time::Instant::now();
    while t.elapsed() < std::time::Duration::from_millis(600) {
        while ctx.iteration(false) {}
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

fn finalize_flag(o: &impl IsA<glib::Object>) -> Rc<Cell<bool>> {
    let f = Rc::new(Cell::new(false));
    let f2 = f.clone();
    // weak-ref notify = finalize-time hook (fires on last unref, after dispose)
    o.as_ref().add_weak_ref_notify_local(move || f2.set(true));
    f
}

fn destroy_flag(w: &impl IsA<gtk::Widget>) -> Rc<Cell<bool>> {
    let f = Rc::new(Cell::new(false));
    let f2 = f.clone();
    w.as_ref().connect_destroy(move |_| f2.set(true)); // dispose-time hook
    f
}

fn case_detach_reinsert() {
    println!("CASE detach_reinsert");
    let win = gtk::Window::new();
    let bx = gtk::Box::new(gtk::Orientation::Vertical, 0);
    win.set_child(Some(&bx));
    let a = gtk::Button::with_label("a"); // our strong ref == the handle table's `hold`
    let a_fin = finalize_flag(&a);
    bx.append(&a);
    win.present();
    spin();
    bx.remove(&a);
    check!(a.parent().is_none());
    // parent window destroyed while `a` is detached
    let win_fin = finalize_flag(&win);
    win.destroy();
    drop(win);
    drop(bx);
    spin();
    check!(win_fin.get());
    check!(!a_fin.get());
    // re-insert into another toplevel
    let win2 = gtk::Window::new();
    let bx2 = gtk::Box::new(gtk::Orientation::Vertical, 0);
    win2.set_child(Some(&bx2));
    bx2.append(&a);
    win2.present();
    spin();
    check!(a.parent().as_ref() == Some(bx2.upcast_ref()) && a.is_mapped());
    // without our ref, removal finalizes immediately
    let b = gtk::Button::with_label("b");
    let b_fin = finalize_flag(&b);
    bx2.append(&b);
    let bw = b.downgrade();
    drop(b);
    bx2.remove(&bw.upgrade().unwrap());
    check!(b_fin.get());
    win2.destroy();
    spin();
}

fn case_native_close() {
    println!("CASE native_close");
    // (a) table holds window weakly (current P3 design), children strongly
    let win = gtk::Window::new();
    let bx = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let btn = gtk::Button::with_label("x");
    bx.append(&btn);
    win.set_child(Some(&bx));
    let (wd, wf) = (destroy_flag(&win), finalize_flag(&win));
    let (bd, bf) = (destroy_flag(&bx), finalize_flag(&bx));
    let ww = win.downgrade();
    win.present();
    drop(win);
    spin();
    ww.upgrade().unwrap().close(); // native close: default close-request handler destroys
    spin();
    check!(wd.get() && wf.get());
    check!(!bd.get() && !bf.get()); // child survives (we hold it), not even disposed
    check!(bx.parent().is_none()); // ... but orphaned: a leak unless the backend drops the subtree
    check!(btn.parent().as_ref() == Some(bx.upcast_ref())); // grandchild still under it
    drop(btn);
    drop(bx);
    check!(bf.get());
    // (b) table holds the window strongly: does close still dispose it?
    let win = gtk::Window::new();
    let (wd, wf) = (destroy_flag(&win), finalize_flag(&win));
    win.present();
    spin();
    win.close();
    spin();
    println!("  info strong-held window after close: destroy-signal={} finalized={} visible={}", wd.get(), wf.get(), win.is_visible());
    // (c) veto: close-request handler returns Stop -> stays alive
    let win2 = gtk::Window::new();
    win2.connect_close_request(|_| glib::Propagation::Stop);
    win2.present();
    spin();
    win2.close();
    spin();
    check!(win2.is_visible());
    win2.destroy();
    // (d) hide-on-close
    let win3 = gtk::Window::new();
    win3.set_hide_on_close(true);
    let d3 = destroy_flag(&win3);
    win3.present();
    spin();
    win3.close();
    spin();
    check!(!win3.is_visible() && !d3.get());
    win3.destroy();
    drop(win);
    spin();
}

fn case_borrowed() {
    println!("CASE borrowed");
    let sw = gtk::ScrolledWindow::new();
    let label = gtk::Label::new(Some("x")); // not scrollable -> wrapped in an auto Viewport
    sw.set_child(Some(&label));
    let vp = sw.child().unwrap(); // a "borrowed" handle: we did not create it
    check!(vp.is::<gtk::Viewport>());
    let vp_fin = finalize_flag(&vp);
    let vpw = vp.downgrade();
    drop(vp); // borrowed handles hold WEAK refs
    sw.set_child(None::<&gtk::Widget>);
    spin();
    check!(vp_fin.get() && vpw.upgrade().is_none());
    // a borrowed handle holding a STRONG ref keeps the internal object alive, orphaned
    sw.set_child(Some(&label));
    let vp2 = sw.child().unwrap();
    let vp2_fin = finalize_flag(&vp2);
    sw.set_child(None::<&gtk::Widget>);
    spin();
    check!(!vp2_fin.get() && vp2.parent().is_none());
    // is `label` (the user's widget, which we hold) still inside the dead viewport?
    println!("  info label parent after viewport detach: {:?}", label.parent().map(|p| p.type_().name()));
    drop(vp2);
    check!(vp2_fin.get());
    // Entry's internal GtkText dies when the Entry is finalized
    let entry = gtk::Entry::new();
    let text = entry.first_child().unwrap();
    println!("  info entry internal child: {}", text.type_().name());
    let tf = finalize_flag(&text);
    let tw = text.downgrade();
    drop(text);
    drop(entry);
    check!(tf.get() && tw.upgrade().is_none());
}

fn case_delete_in_signal() {
    println!("CASE delete_in_signal");
    let win = gtk::Window::new();
    let bx = gtk::Box::new(gtk::Orientation::Vertical, 0);
    win.set_child(Some(&bx));
    let hold: Rc<RefCell<Option<gtk::Button>>> = Rc::new(RefCell::new(Some(gtk::Button::with_label("x"))));
    let btn = hold.borrow().clone().unwrap();
    let fin = finalize_flag(&btn);
    bx.append(&btn);
    let second = Rc::new(Cell::new(false));
    let fin_during = Rc::new(Cell::new(false));
    {
        let (hold, bx) = (hold.clone(), bx.clone());
        btn.connect_clicked(move |b| {
            bx.remove(b); // parent drops its ref
            hold.borrow_mut().take(); // table drops its ref: emission still holds one
        });
    }
    {
        let (second, fin, fin_during) = (second.clone(), fin.clone(), fin_during.clone());
        btn.connect_clicked(move |b| {
            second.set(true);
            fin_during.set(fin.get());
            let _ = b.label(); // touch it
        });
    }
    win.present();
    spin();
    let w = btn.downgrade();
    drop(btn);
    w.upgrade().unwrap().emit_clicked();
    spin();
    check!(second.get());
    check!(!fin_during.get()); // object outlived the emission
    if let Some(b) = w.upgrade() {
        println!("  info button still alive after emission: refcount={} (incl. this upgrade) parent={:?}", b.ref_count(), b.parent().map(|p| p.type_().name()));
    }
    check!(fin.get()); // finalized after it
    // destroy a window from its own close-request
    let win2 = gtk::Window::new();
    let d = destroy_flag(&win2);
    win2.connect_close_request(|w| {
        w.destroy();
        glib::Propagation::Stop
    });
    win2.present();
    spin();
    let ww2 = win2.downgrade();
    drop(win2); // table holds toplevels weakly
    ww2.upgrade().unwrap().close();
    spin();
    check!(d.get() && ww2.upgrade().is_none());
    win.destroy();
    spin();
}

fn case_dispose_vs_finalize() {
    println!("CASE dispose_vs_finalize");
    // gtk_window_destroy on a window we still hold: disposed but not finalized
    let win = gtk::Window::new();
    let bx = gtk::Box::new(gtk::Orientation::Vertical, 0);
    win.set_child(Some(&bx));
    let d = destroy_flag(&win);
    let f = finalize_flag(&win);
    win.present();
    spin();
    win.destroy();
    spin();
    println!("  info held window after destroy(): destroy-signal={} finalized={} child-still-set={}", d.get(), f.get(), win.child().is_some());
    // using a disposed-but-alive window: does it warn/crash?
    win.set_title(Some("after"));
    let c = gtk::Label::new(Some("late"));
    win.set_child(Some(&c));
    println!("  info set_child on disposed window accepted: {}", win.child().is_some());
    drop(win);
    spin();
    println!("  info finalized after last ref: {}", f.get());
}

fn main() {
    gtk::init().unwrap();
    case_detach_reinsert();
    case_native_close();
    case_borrowed();
    case_delete_in_signal();
    case_dispose_vs_finalize();
    let n = FAILS.with(|f| f.get());
    println!("FAILURES {n}");
    std::process::exit(if n == 0 { 0 } else { 1 });
}
