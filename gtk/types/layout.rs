use std::{borrow::Cow, cell::UnsafeCell, marker::PhantomData};

use rquickjs::{ClassDef, Ctx, FromJs, IntoJs, Value};

use crate::{common::*, core::enums::Enum};

pub type Box = Gtk<gtk4::Box>;

interface!(Box as gtk4::Box | ty | {});

class_def!(
    Box as "Box" | ty | {
        define::<gtk4::Box, _>(&ty)?;
        ty.implements::<Box>()?;
        ty.implements::<crate::Object>()?;
        ty.register_as::<gtk4::Box>()?;
    }
);
