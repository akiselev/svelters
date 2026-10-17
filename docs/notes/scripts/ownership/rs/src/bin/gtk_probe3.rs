use gtk::glib;
use gtk::prelude::*;
use std::cell::Cell;
use std::rc::Rc;
fn spin() { let c = glib::MainContext::default(); for _ in 0..50 { while c.iteration(false) {} } }
fn fin(o: &impl IsA<glib::Object>) -> Rc<Cell<bool>> { let f = Rc::new(Cell::new(false)); let f2 = f.clone(); o.as_ref().add_weak_ref_notify_local(move || f2.set(true)); f }
fn run(present: bool, in_handler: bool, emit: bool) {
    let win = gtk::Window::new();
    let bx = gtk::Box::new(gtk::Orientation::Vertical, 0);
    win.set_child(Some(&bx));
    let btn = gtk::Button::with_label("x");
    let f = fin(&btn);
    bx.append(&btn);
    if in_handler { let bx = bx.clone(); btn.connect_clicked(move |b| bx.remove(b)); }
    if present { win.present(); spin(); }
    let w = btn.downgrade();
    drop(btn);
    if emit { w.upgrade().unwrap().emit_clicked(); }
    if !in_handler { bx.remove(&w.upgrade().unwrap()); }
    let t = std::time::Instant::now();
    while t.elapsed() < std::time::Duration::from_millis(1500) { glib::MainContext::default().iteration(false); std::thread::sleep(std::time::Duration::from_millis(5)); }
    let rc = w.upgrade().map(|b| b.ref_count() - 1);
    println!("present={present} in_handler={in_handler} emit={emit}: finalized={} extra_refs={rc:?}", f.get());
    win.destroy(); drop(win); spin();
}
fn main() {
    gtk::init().unwrap();
    run(false, false, true); run(true, false, true); run(false, true, true); run(true, true, true);
}
