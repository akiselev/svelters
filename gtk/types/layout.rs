use std::{borrow::Cow, cell::UnsafeCell, marker::PhantomData};

use gtk4::prelude::BoxExt;
use rquickjs::{ClassDef, Ctx, FromJs, IntoJs, Opt, Value};

use crate::{common::*, core::enums::Enum};

pub type Box = Gtk<gtk4::Box>;

interface!(
    Box as gtk4::Box | ty | {
        ty.method(
            "append",
            |this: &gtk4::Box, widget: GtkRef<gtk4::Widget>| {
                this.append(widget.as_ref());
            },
        )?;
        ty.method(
            "remove",
            |this: &gtk4::Box, widget: GtkRef<gtk4::Widget>| {
                this.remove(widget.as_ref());
            },
        )?;
        ty.method(
            "insertChildAfter",
            |this: &gtk4::Box, widget: GtkRef<gtk4::Widget>, anchor: Opt<GtkRef<gtk4::Widget>>| {
                let anchor = anchor.into_inner();
                this.insert_child_after(widget.as_ref(), anchor.map(GtkRef::inner).as_ref())
            },
        )?;
    }
);

class_def!(
    Box as "Box" | ty | {
        define::<gtk4::Box, _>(&ty)?;
        ty.implements::<Box>()?;
        ty.implements::<crate::Widget>()?;
        ty.implements::<crate::Object>()?;
        ty.register_as::<gtk4::Box>()?;
    }
);
