use std::marker::PhantomData;

use glib::object::{ObjectRef, UnsafeFrom};
use glib::value::{
    FromValue, FromValueOptional, ToValueOptional, ValueType, ValueTypeChecker,
    ValueTypeMismatchOrNoneError,
};
use glib::{ObjectType, ToValue};
pub use gtk4::prelude::{ObjectExt, WidgetExt};

pub use gtk4::glib::{Cast, IsA, ParamFlags, ParamSpec, StaticType};
pub use gtk4::glib::{Object as GObject, Value as GValue};
pub use gtk4::Widget as GtkWidget;

use rquickjs::{
    AsFunction, AsProperty, ClassDef, Constructor, Ctx, FromInput, FromJs, IntoAtom, IntoJs,
    Object as JsObject, ObjectDef, This, TypedMapKey, Value,
};

use crate::core::method::InterfaceMethod;
use crate::core::{enum_prop::EnumProp, obj_prop::ObjectProp, property::Prop};

pub use crate::exception;

pub type Result<T> = rquickjs::Result<T>;

pub use crate::errors::HandleParamError;

pub use crate::core::from_gtype::define;

pub use crate::obj_data::ObjectData;

pub struct JsConverter {
    pub type_: glib::types::Type,
    pub construct: Box<dyn for<'js> Fn(Ctx<'js>, GObject) -> Result<Value<'js>>>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TypeKey(pub glib::types::Type);

impl TypedMapKey for TypeKey {
    type Value = JsConverter;
}

pub trait AsObject {
    fn as_object(&self) -> &GObject;
}

pub trait Interface<'js, T> {
    fn define(ty: &Type<'js, T>) -> Result<()>;
}

pub struct Type<'js, T> {
    name: &'static str,
    ctx: Ctx<'js>,
    pub proto: JsObject<'js>,
    marker: PhantomData<T>,
}

impl<'js, T> Type<'js, T>
where
    T: 'static,
{
    pub fn new(ctx: Ctx<'js>, proto: JsObject<'js>, name: &'static str) -> Self {
        Self {
            name,
            ctx,
            proto,
            marker: PhantomData,
        }
    }

    pub fn register_as<U>(&self) -> Result<()>
    where
        U: StaticType + ObjectType,
        T: From<U> + for<'a> IntoJs<'a>,
    {
        let type_ = U::static_type();
        self.ctx.insert_typed(
            TypeKey(type_),
            JsConverter {
                type_,
                construct: Box::new(move |ctx, val| {
                    let this = match val.dynamic_cast::<U>() {
                        Ok(val) => val,
                        Err(_) => {
                            return Err(exception!("Could not cast Object to {}", type_.name()));
                        }
                    };
                    let this = T::from(this);
                    this.into_js(ctx)
                }),
            },
        );
        Ok(())
    }

    pub fn custom_prop<V, P>(&self, name: &str, prop: V) -> Result<()>
    where
        V: AsProperty<'js, P>,
    {
        self.proto.prop(name, prop)
    }

    pub fn method<FN, A, R, X>(&self, name: &str, func: FN) -> Result<()>
    where
        FN: Fn<A, Output = R> + 'static,
        R: IntoJs<'js>,
        T: 'static + CastRef<X>,
        &'js T: FromInput<'js>,
        X: 'static,
        InterfaceMethod<T, X, FN>: AsFunction<'js, A, R>,
    {
        let method = InterfaceMethod(func, PhantomData::<(T, X)>);
        self.proto.set(name, rquickjs::Func::new(name, method))
    }

    pub fn raw_method<FN, A, R>(&self, name: &str, func: FN) -> Result<()>
    where
        FN: 'static + Fn<A, Output = R>,
        rquickjs::Method<FN>: AsFunction<'js, A, R>,
    {
        self.proto
            .set(name, rquickjs::Func::new(name, rquickjs::Method(func)))
    }

    pub fn raw_prop<V, P>(&self, name: &str, prop: V) -> Result<()>
    where
        V: AsProperty<'js, P>,
    {
        self.proto.prop(name, prop)
    }

    pub fn set<V>(&self, name: &str, prop: V) -> Result<()>
    where
        V: IntoJs<'js>,
    {
        self.proto.set(name, prop)
    }

    pub fn param<P>(&self, name: &str, spec: ParamSpec) -> Result<()>
    where
        T: 'static + AsObject,
        P: 'static + IntoJs<'js> + FromJs<'js> + for<'a> FromValue<'a> + ToValue,
        &'js T: FromJs<'js>,
        Prop<T, P>: AsProperty<'js, P>,
    {
        self.proto.prop(name, Prop::<T, P>::new(spec))
    }

    pub fn enum_prop(&self, name: &str, spec: ParamSpec) -> Result<()>
    where
        T: 'static + AsObject,
        &'js T: FromJs<'js>,
        EnumProp<T, String>: AsProperty<'js, String>,
    {
        self.proto.prop(name, EnumProp::<T, String>::new(spec))
    }

    pub fn obj_prop(&self, name: &str, spec: ParamSpec) -> Result<()>
    where
        T: 'static + AsObject,
        &'js T: FromJs<'js>,
        ObjectProp<T>: AsProperty<'js, ()>,
    {
        self.proto.prop(name, ObjectProp::<T>::new(spec))
    }

    pub fn implements<I>(&self) -> Result<()>
    where
        I: Interface<'js, T>,
    {
        <I as Interface<'js, T>>::define(self)
    }
}

pub macro interface {
    ($cls:ty as $inner:ty |$bld:ident| { $($tail:tt)* }) => {
        impl<'js, T> Interface<'js, T> for $cls where T: 'static + ClassDef + CastRef<$inner>, &'js T: FromJs<'js> {
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
                let $bld = Type::<'js, $cls>::new(ctx, proto.clone(), $name);
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

pub trait CastRef<T> {
    fn cast_ref(&self) -> Option<&T>;
}

impl<T, U> CastRef<U> for Gtk<T>
where
    U: ObjectType,
    T: Cast,
{
    fn cast_ref(&self) -> Option<&U> {
        self.0.dynamic_cast_ref::<U>()
    }
}

impl<T> From<T> for Gtk<T> {
    fn from(val: T) -> Self {
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
