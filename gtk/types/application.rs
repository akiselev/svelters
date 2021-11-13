use std::{borrow::Cow, cell::UnsafeCell, marker::PhantomData};

use gtk4::Application as GtkApplication;
use rquickjs::{ClassDef, Ctx, FromJs, IntoJs, Value};

use crate::common::*;

pub type Application = Gtk<gtk4::Application>;

interface!(
    Application | ty | {
        ty.prop::<String>("title", "title")?;
    }
);

class_def!(
    Application as "Application" | ty | {
        ty.implements::<Application>()?;
        ty.implements::<crate::Object>()?;
    }
);
