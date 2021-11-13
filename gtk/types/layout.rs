use std::{borrow::Cow, cell::UnsafeCell, marker::PhantomData};

use rquickjs::{ClassDef, Ctx, FromJs, IntoJs, Value};

use crate::common::*;

pub type Box = Gtk<gtk4::Box>;

interface!(
    Box | ty | {
        ty.prop::<String>("title", "title")?;
    }
);

class_def!(
    Box as "Box" | ty | {
        ty.implements::<Box>()?;
        ty.implements::<crate::Object>()?;
    }
);
