use std::{borrow::Cow, cell::UnsafeCell, marker::PhantomData};

use gtk4::prelude::GtkWindowExt;
use rquickjs::{ClassDef, Ctx, FromJs, IntoJs, Value};

use crate::{common::*, core::property::Prop, types::application::Application};

pub type ApplicationWindow = Gtk<gtk4::ApplicationWindow>;

interface!(gtk4::ApplicationWindow {
    fn constructor(ctx: Ctx) {
        let globals = ctx.globals();
        let app = globals.get::<_, Application>("application")?;
        Result::Ok(Gtk(gtk4::ApplicationWindow::new(&app.0)))
    }
});

pub type Window = Gtk<gtk4::Window>;

interface!(
    gtk4::Window  {
        fn constructor() {
            Result::Ok(Gtk(gtk4::Window::new()))
        }

        fn show(this: Gtk<gtk4::Window>) {
            this.show();
        }

        fn hide(this: Gtk<gtk4::Window>) {
            this.hide();
        }

        fn destroy(this: Gtk<gtk4::Window>) {
            this.destroy();
        }

    }
);
