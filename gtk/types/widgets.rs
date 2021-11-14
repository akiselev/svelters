use std::{borrow::Cow, cell::UnsafeCell, marker::PhantomData};

use gtk4::prelude::*;
use rquickjs::{ClassDef, Ctx, FromJs, IntoJs, Value};

use crate::common::*;

pub type CssProvider = Gtk<gtk4::CssProvider>;

interface!(
    CssProvider as gtk4::CssProvider | ty | {
        ty.method("setCss", |this: &gtk4::CssProvider, val: String| {
            this.load_from_data(val.as_bytes());
        })?;
        ty.method("mount", |this: &gtk4::CssProvider, priority: u32| {
            let display = gdk4::Display::default().unwrap();
            gtk4::StyleContext::add_provider_for_display(&display, this, priority);
        })?;
        ty.method("unmount", |this: &gtk4::CssProvider| {
            let display = gdk4::Display::default().unwrap();
            gtk4::StyleContext::remove_provider_for_display(&display, this);
        })?;
    }
);

class_def!(
    CssProvider as "CssProvider" | ty | {
        define::<gtk4::CssProvider, _>(&ty)?;
        ty.implements::<CssProvider>()?;
        ty.implements::<crate::Object>()?;
        ty.register_as::<gtk4::CssProvider>()?;
    }
);

pub type Button = Gtk<gtk4::Button>;

interface!(Button as gtk4::Button | ty | {});

class_def!(
    Button as "Button" | ty | {
        define::<gtk4::Button, _>(&ty)?;
        ty.implements::<Button>()?;
        ty.implements::<crate::Object>()?;
        ty.register_as::<gtk4::Button>()?;
    }
);

#[rquickjs::bind(module, public)]
mod x {
    pub struct T {
        pub x: u32,
        #[quickjs(readonly)]
        pub y: u32,
    }

    impl T {
        #[quickjs(constructor)]
        pub fn new() -> T {
            T { x: 100, y: 200 }
        }

        pub fn start(&self) {
            println!("start!");
        }
    }
}
