use std::{borrow::Cow, cell::UnsafeCell, marker::PhantomData};

use rquickjs::{ClassDef, Ctx, FromJs, IntoJs, Value};

use crate::common::*;

pub type Button = Gtk<gtk4::Button>;

interface!(
    Button | ty | {
        ty.prop::<String>("title", "title")?;
    }
);

class_def!(
    Button as "Button" | ty | {
        ty.implements::<Button>()?;
        ty.implements::<crate::Object>()?;
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
    }
}
