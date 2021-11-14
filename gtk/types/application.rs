use std::{borrow::Cow, cell::UnsafeCell, marker::PhantomData};

use gtk4::Application as GtkApplication;
use rquickjs::{ClassDef, Ctx, FromJs, IntoJs, Value};

use crate::common::*;

pub type Application = Gtk<gtk4::Application>;

interface!(Application as gtk4::Application | ty | {});

class_def!(
    Application as "Application" | ty | {
        define::<gtk4::Application, _>(&ty)?;
        ty.implements::<Application>()?;
        ty.implements::<crate::Object>()?;
        ty.register_as::<gtk4::Application>()?;
    }
);
