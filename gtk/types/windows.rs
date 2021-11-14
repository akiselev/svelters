use std::{borrow::Cow, cell::UnsafeCell, marker::PhantomData};

use gtk4::prelude::GtkWindowExt;
use rquickjs::{ClassDef, Ctx, FromJs, IntoJs, Value};

use crate::{common::*, core::property::Prop};

pub type ApplicationWindow = Gtk<gtk4::ApplicationWindow>;

interface!(ApplicationWindow as gtk4::ApplicationWindow | ty | {});

class_def!(
    ApplicationWindow as "ApplicationWindow" | ty | {
        define::<gtk4::ApplicationWindow, _>(&ty)?;
        ty.implements::<ApplicationWindow>()?;
        ty.implements::<Window>()?;
        ty.implements::<crate::Object>()?;
        ty.register_as::<gtk4::ApplicationWindow>()?;
    }
);

pub type Window = Gtk<gtk4::Window>;

interface!(
    Window as gtk4::Window | ty | {
        ty.method("show", |this: &gtk4::Window| {
            this.show();
        })?;
        ty.method("hide", |this: &gtk4::Window| {
            this.hide();
        })?;
        ty.method("destroy", |this: &gtk4::Window| {
            this.destroy();
        })?;
    }
);

class_def!(
    Window as "Window" | ty | {
        define::<gtk4::Window, _>(&ty)?;
        ty.implements::<Window>()?;
        ty.implements::<crate::Object>()?;
        ty.register_as::<gtk4::Window>()?;
    }
);
