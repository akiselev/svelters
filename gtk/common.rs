pub use gtk4::glib::{Cast, IsA, ParamFlags, ParamSpec, StaticType};
pub use gtk4::glib::{Object as GObject, Value as GValue};
pub use gtk4::prelude::{ObjectExt, WidgetExt};
pub use gtk4::Widget as GtkWidget;

pub type Result<T> = rquickjs::Result<T>;

pub use crate::core::logging::*;
pub use crate::errors::HandleParamError;
pub use crate::exception;
pub use crate::newtype::{Gtk, G};
pub use crate::obj_data::ObjectData;

use crate::registry::Registry;

pub macro interface {
    (($ctx:ident, $ty:ty) => fn constructor($($args:tt)*) $code:block $($tail:tt)*) => {
        let constructor = rquickjs::Function::new($ctx, |$($args)*| {
            $code
        }).expect("Could not convert constructor into Javascript Function");
        constructor.set_constructor(true);
        $ctx.set_constructor(<$ty>::static_type(), constructor);
        interface!(($ctx, $ty) => $($tail)*);
    },
    (($ctx:ident, $ty:ty) => $skip:tt $($tail:tt)*) => {
        interface!(($ctx, $ty) => $($tail)*);
    },
    (($ctx:ident, $ty:ty) => ) => {},

    (($ctx:ident, $proto:ident): fn constructor($($args:tt)*) $code:block $($tail:tt)*) => {
        interface!(($ctx, $proto): $($tail)*);
    },

    (($ctx:ident, $proto:ident): fn $name:ident($($args:tt)*) $code:block $($tail:tt)*) => {
        $proto.set(stringify!($name), rquickjs::Function::new($ctx, rquickjs::Method(|$($args)*| {
            $code
        })))?;
        interface!(($ctx, $proto): $($tail)*);
    },

    // Empty interface (usually a placeholder)
    (($ctx:ident, $proto:ident): ) => {},

    ($ty:ty { $($tail:tt)* }) => {
        #[allow(unused_variables)]
        fn init_proto<'js>(ctx: rquickjs::Ctx<'js>, proto: &rquickjs::Object<'js>) -> rquickjs::Result<()> {
            interface!((ctx, proto): $($tail)*);
            Ok(())
        }

        fn init<'js>(ctx: rquickjs::Ctx<'js>) {
            crate::Registry::set_proto(ctx, <$ty as glib::StaticType>::static_type(), init_proto);
            interface!((ctx, $ty) => $($tail)*);
        }

        inventory::submit! {
            crate::InterfaceDeclaration::new(init)
        }
    }
}

pub macro methods {
    (($ctx:ident, $proto:ident): fn $name:ident($($args:tt)*) { $($tt:tt)* }) => {
        $proto.set(stringify!($name), rquickjs::Function::new(rquickjs::Method(|$($args)*| {
            $(tt)*
        })))?;
    },

    ($ab:tt => fn $name:ident($($args:tt)*) { $($tt:tt)* } $($tail:tt)*) => {
        methods!($ab: fn $name($($args)*) { $($tt)* });
        methods!($ab => $($tail)*);
    }
}

#[macro_export]
macro_rules! exception {
    ($($tokens:tt)*) => {
        {
            let backtrace = std::backtrace::Backtrace::capture().to_string();
            use std::convert::TryInto;
            rquickjs::Error::Exception {
                message: format!(
                    $($tokens)*
                ),
                file: file!().to_string(),
                line: line!().try_into().unwrap(),
                stack: backtrace.clone(),
                rust_stack: Some(backtrace),
            }
        }
    };
}
