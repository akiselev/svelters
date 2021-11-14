use std::{borrow::Cow, cell::UnsafeCell, marker::PhantomData};

use rquickjs::{ClassDef, Ctx, FromJs, IntoJs, Value};

use crate::common::*;

pub type Widget = Gtk<gtk4::Widget>;

interface!(Widget as gtk4::Widget | ty | {});

class_def!(
    Widget as "Widget" | ty | {
        ty.implements::<Widget>()?;
        ty.implements::<crate::Object>()?;
        ty.register_as::<gtk4::Widget>()?;
    }
);
