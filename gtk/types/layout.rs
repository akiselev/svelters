use gtk4::prelude::*;
use rquickjs::Opt;

use crate::{common::*, core::enums::Enum};

pub type Box = Gtk<gtk4::Box>;

interface!(gtk4::Box {
    fn constructor(orientation: Enum<gtk4::Orientation>) {
        Gtk(gtk4::Box::new(orientation.into(), 0))
    }

    fn append(this: Gtk<gtk4::Box>, widget: Gtk<gtk4::Widget>) {
        this.append(widget.as_ref());
    }

    fn remove(this: Gtk<gtk4::Box>, widget: Gtk<gtk4::Widget>) {
        this.remove(widget.as_ref());
    }

    fn insertChildAfter(this: Gtk<gtk4::Box>, widget: Gtk<gtk4::Widget>, anchor: Opt<Gtk<gtk4::Widget>>) {
        let anchor = anchor.into_inner();
        this.insert_child_after(widget.as_ref(), anchor.map(Gtk::inner).as_ref())
    }
});
