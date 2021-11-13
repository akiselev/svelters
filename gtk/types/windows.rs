use std::{borrow::Cow, cell::UnsafeCell, marker::PhantomData};

use rquickjs::{ClassDef, Ctx, FromJs, IntoJs, Value};

use crate::{common::*, core::property::Prop};

pub type ApplicationWindow = Gtk<gtk4::ApplicationWindow>;

interface!(
    ApplicationWindow | ty | {
        ty.prop::<String>("title", "title")?;
    }
);

class_def!(
    ApplicationWindow as "ApplicationWindow" | ty | {
        ty.implements::<ApplicationWindow>()?;
        ty.implements::<crate::Object>()?;
    }
);
