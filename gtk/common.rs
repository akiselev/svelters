use std::marker::PhantomData;

use glib::object::{ObjectRef, UnsafeFrom};
use glib::translate::ToGlibPtr;
use glib::value::{
    FromValue, FromValueOptional, ToValueOptional, ValueType, ValueTypeChecker,
    ValueTypeMismatchOrNoneError,
};
use glib::{ObjectType, ToValue};
pub use gtk4::prelude::{ObjectExt, WidgetExt};

pub use gtk4::glib::Object as GObject;
pub use gtk4::glib::{Cast, IsA, ParamFlags, ParamSpec, StaticType};
pub use gtk4::Widget as GtkWidget;

use crate::core::property::Prop;
pub use crate::exception;

pub type Result<T> = rquickjs::Result<T>;

use rquickjs::{AsProperty, Ctx, FromJs, IntoAtom, IntoJs, Object as JsObject, ObjectDef, Value};

pub trait AsObject {
    fn as_object(&self) -> &GObject;
}

pub trait Interface<'js, T> {
    fn define(ty: &Type<'js, T>) -> Result<()>;
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

    pub fn custom_prop<V, P>(&self, name: &'static str, prop: V) -> Result<()>
    where
        V: AsProperty<'js, P>,
    {
        self.proto.prop(name, prop)
    }

    pub fn prop<P>(&self, name: &'static str, param_name: &'static str) -> Result<()>
    where
        T: 'static + AsObject,
        P: 'static + IntoJs<'js> + FromJs<'js> + for<'a> FromValue<'a> + ToValue,
        &'js T: FromJs<'js>,
        Prop<T, P>: AsProperty<'js, P>,
    {
        self.proto.prop(name, Prop::<T, P>::new(param_name))
    }

    pub fn implements<I>(&self) -> Result<()>
    where
        I: Interface<'js, T>,
    {
        <I as Interface<'js, T>>::define(self)
    }
}

// pub fn define_params<T: IsClass + StaticType>()

pub macro interface {
    ($cls:ty |$bld:ident| { $($tail:tt)* }) => {
        impl<'js, T> Interface<'js, T> for $cls where T: 'static + AsObject, &'js T: FromJs<'js> {
            fn define($bld: &Type<'js, T>) -> Result<()> {
                $($tail)*
                Ok(())
            }
        }
    }
}

pub macro class_def {
    ($cls:ty as $name:literal |$bld:ident| { $($tail:tt)* }) => {
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
                let $bld = Type::<'js, $cls>::new(ctx, proto.clone());
                $($tail)*
                Ok(())
            }
        }

        impl<'js> rquickjs::IntoJs<'js> for $cls {
            fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
                self.into_js_obj(ctx)
            }
        }

        impl<'js> rquickjs::FromJs<'js> for &'js $cls {
            fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
                <$cls>::from_js_ref(ctx, value)
            }
        }

        impl<'js> rquickjs::FromJs<'js> for &'js mut $cls {
            fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
                <$cls>::from_js_mut(ctx, value)
            }
        }

        impl<'js> rquickjs::FromJs<'js> for $cls {
            fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
                <$cls>::from_js_obj(ctx, value)
            }
        }
    },
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Gtk<T>(pub T);

impl<T> Gtk<T> {
    pub fn new(val: T) -> Self {
        Gtk(val)
    }
}

unsafe impl<'a, T> FromValue<'a> for Gtk<T>
where
    T: FromValue<'a>,
{
    type Checker = <T as FromValue<'a>>::Checker;

    unsafe fn from_value(value: &'a glib::Value) -> Self {
        Gtk(T::from_value(value))
    }
}

impl<'a, T> ValueType for Gtk<T>
where
    T: ValueType,
{
    type Type = <T as ValueType>::Type;
}

impl<T> ToValue for Gtk<T>
where
    T: ToValue,
{
    fn to_value(&self) -> glib::Value {
        self.0.to_value()
    }

    fn value_type(&self) -> glib::Type {
        self.0.value_type()
    }
}

impl<T> ToValueOptional for Gtk<T>
where
    T: ToValueOptional,
{
    fn to_value_optional(s: Option<&Self>) -> glib::Value {
        T::to_value_optional(s.map(|inner| &inner.0))
    }
}

impl<'a, T, U> ToGlibPtr<'a, *mut U> for Gtk<T>
where
    T: ObjectType,
    T: ToGlibPtr<'a, *mut U>,
{
    type Storage = <T as ToGlibPtr<'a, *mut U>>::Storage;

    fn to_glib_none(&'a self) -> glib::translate::Stash<'a, *mut U, Self> {
        self.to_glib_none()
    }

    fn to_glib_container(&'a self) -> glib::translate::Stash<'a, *mut U, Self> {
        self.to_glib_container()
    }

    fn to_glib_full(&self) -> *mut U {
        self.to_glib_full()
    }
}

impl<T> UnsafeFrom<ObjectRef> for Gtk<T>
where
    T: UnsafeFrom<ObjectRef>,
{
    unsafe fn unsafe_from(t: ObjectRef) -> Self {
        Gtk(T::unsafe_from(t))
    }
}

impl<T> Into<ObjectRef> for Gtk<T>
where
    T: Into<ObjectRef>,
{
    fn into(self) -> ObjectRef {
        self.0.into()
    }
}

impl<T> StaticType for Gtk<T>
where
    T: StaticType,
{
    fn static_type() -> glib::Type {
        T::static_type()
    }
}

impl<T: IsA<GObject>> AsObject for Gtk<T> {
    fn as_object(&self) -> &GObject {
        &self.0.upcast_ref::<GObject>()
    }
}
