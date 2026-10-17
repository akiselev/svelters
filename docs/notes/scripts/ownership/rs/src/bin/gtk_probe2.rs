use gtk::glib;
use gtk::prelude::*;
use std::cell::Cell;
use std::rc::Rc;
fn spin() { let c = glib::MainContext::default(); for _ in 0..50 { while c.iteration(false) {} } }
fn fin(o: &impl IsA<glib::Object>) -> Rc<Cell<bool>> { let f = Rc::new(Cell::new(false)); let f2 = f.clone(); o.as_ref().add_weak_ref_notify_local(move || f2.set(true)); f }
fn run(present: bool, focusable: bool) {
    let win = gtk::Window::new();
    let bx = gtk::Box::new(gtk::Orientation::Vertical, 0);
    win.set_child(Some(&bx));
    let btn = gtk::Button::with_label("x");
    btn.set_focusable(focusable);
    let f = fin(&btn);
    bx.append(&btn);
    if present { win.present(); spin(); }
    let is_focus = GtkWindowExt::focus(&win).map(|w| &w == btn.upcast_ref::<gtk::Widget>()).unwrap_or(false);
    let w = btn.downgrade();
    drop(btn);
    bx.remove(&w.upgrade().unwrap());
    spin();
    println!("present={present} focusable={focusable} was_focus={is_focus}: finalized_after_remove={} focus_now={:?}", f.get(), GtkWindowExt::focus(&win).map(|w| w.type_().name()));
    win.destroy(); drop(win); spin();
    println!("   finalized after window gone={}", f.get());
}
fn main() {
    gtk::init().unwrap();
    run(false, true); run(true, true); run(true, false);
    // zombie window: destroy() while held strongly, then present() again
    let win = gtk::Window::new();
    let d = Rc::new(Cell::new(false)); let d2 = d.clone(); win.connect_destroy(move |_| d2.set(true));
    win.present(); spin(); win.destroy(); spin();
    let in_list = gtk::Window::list_toplevels().iter().any(|t| t == win.upcast_ref::<gtk::Widget>());
    println!("held window after destroy: destroy_sig={} in_toplevels={in_list} realized={}", d.get(), win.is_realized());
    win.present(); spin();
    let in_list = gtk::Window::list_toplevels().iter().any(|t| t == win.upcast_ref::<gtk::Widget>());
    println!("re-present zombie: visible={} in_toplevels={in_list}", win.is_visible());
    // close-request -> destroy() with only a weak ref held by us
    let win2 = gtk::Window::new();
    let d = Rc::new(Cell::new(false)); let d2 = d.clone(); win2.connect_destroy(move |_| d2.set(true));
    win2.connect_close_request(|w| { w.destroy(); glib::Propagation::Stop });
    win2.present(); spin();
    let ww = win2.downgrade(); drop(win2);
    ww.upgrade().unwrap().close(); spin();
    println!("destroy from own close-request (weakly held): destroy_sig={} gone={}", d.get(), ww.upgrade().is_none());
}
