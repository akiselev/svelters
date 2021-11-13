use std::marker::PhantomData;

pub use gtk4::prelude::{ObjectExt, WidgetExt};

pub use gtk4::glib::Object as GObject;
pub use gtk4::glib::{Cast, IsA, ParamFlags, ParamSpec};
pub use gtk4::Widget as GtkWidget;

pub use crate::exception;

pub type Result<T> = rquickjs::Result<T>;

use rquickjs::{AsProperty, Ctx, IntoAtom, Object as JsObject, ObjectDef, Value};

pub trait AsObject {
    fn as_object(&self) -> &GObject;
}

pub trait Interface<T> {
    fn define<'js>(ctx: Ctx<'js>, proto: Value<'js>) -> Result<()>;
}

pub struct Type<'js, T> {
    ctx: Ctx<'js>,
    proto: JsObject<'js>,
    marker: PhantomData<T>,
}

impl<'js, T> Type<'js, T> {
    pub fn new(ctx: Ctx<'js>, proto: JsObject<'js>) -> Self {
        Self {
            ctx,
            proto,
            marker: PhantomData,
        }
    }

    pub fn prop<K, V, P>(&self, name: K, prop: V) -> Result<()>
    where
        K: IntoAtom<'js>,
        V: AsProperty<'js, P>,
    {
        self.proto.prop(name, prop)
    }
}

macro class_def {
    ($cls:ident as $name:literal |$bld:ident| { $($tail:tt)* }) => {
        impl rquickjs::ClassDef for $cls {
            const CLASS_NAME: &'static str = $name;

            unsafe fn class_id() -> &'static mut rquickjs::ClassId {
                static mut CLASS_ID: rquickjs::ClassId = rquickjs::ClassId::new();
                &mut CLASS_ID
            }

            const HAS_PROTO: bool = true;

            fn init_proto<'js>(
                ctx: rquickjs::Ctx<'js>,
                proto: &rquickjs::Object<'js>,
            ) -> rquickjs::Result<()> {
                let $bld = Type::new(ctx, proto);
                $($tail)*
                Ok(())
            }
        }
    },
    ($cls:ident |$bld:ident| { $($tail:tt)* }) => {
        impl rquickjs::ClassDef for $cls {
            const CLASS_NAME: &'static str = stringify!($cls);

            unsafe fn class_id() -> &'static mut rquickjs::ClassId {
                static mut CLASS_ID: rquickjs::ClassId = rquickjs::ClassId::new();
                &mut CLASS_ID
            }

            const HAS_PROTO: bool = true;

            fn init_proto<'js>(
                ctx: rquickjs::Ctx<'js>,
                proto: &rquickjs::Object<'js>,
            ) -> rquickjs::Result<()> {
                let $bld = Type::<$cls>::new(ctx, proto.clone());
                $($tail)*
                Ok(())
            }
        }
    },
}

class_def!(Button | ty | {});

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
