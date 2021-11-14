use std::{borrow::Cow, cell::UnsafeCell, collections::HashMap, marker::PhantomData};

use glib::{value::FromValue, ObjectType, SignalHandlerId};
use rquickjs::{qjs, ClassDef, Context, Ctx, FromJs, Function, IntoJs, Method, MutFn, Opt, Value};

use crate::common::*;

struct ExtraData {
    ctx: Context,
    value: qjs::JSValue,
}

struct Events {
    ctx: Context,
    map: HashMap<String, Connection>,
}

struct Connection {
    id: Option<SignalHandlerId>,
    callbacks: Vec<qjs::JSValue>,
}

impl Default for Connection {
    fn default() -> Self {
        Self {
            id: Default::default(),
            callbacks: Default::default(),
        }
    }
}

#[derive(Clone)]
pub struct Object(GObject);

impl<'js, T> Interface<'js, T> for Object
where
    T: 'static + AsObject + CastRef<GObject>,
    &'js T: FromJs<'js>,
{
    fn define(ty: &Type<'js, T>) -> Result<()> {
        ty.custom_prop(
            "asGObject",
            rquickjs::Accessor::new_get(rquickjs::Method(|this: &'js T| {
                rquickjs::Result::Ok(Object(this.as_object().clone()))
            })),
        )?;
        ty.custom_prop(
            "isGObject",
            rquickjs::Accessor::new_get(|| rquickjs::Result::Ok(true)),
        )?;
        define_sv_extra(ty)?;
        define_event_listeners(ty)?;
        Ok(())
    }
}

fn handle_event(name: String) -> impl Fn(&[GValue]) -> Option<GValue> {
    move |args| {
        if args.len() == 0 {
            panic!("Signal handler received no arguments");
        }
        let this: GObject = args[0]
            .get::<GObject>()
            .expect("Could not get signal target");
        let events = this
            .get_data::<Events>("svEvents")
            .expect("No event data found");
        if let Some(conn) = events.map.get(&name) {
            events.ctx.with(|ctx| {
                let event = rquickjs::Object::new(ctx).unwrap();
                event.set("type", &name).unwrap();
                for cb in conn.callbacks.iter() {
                    let func = unsafe { Value::from_js_value_const(ctx, *cb) };
                    let func = func.as_function().expect("Callback is not a function");
                    if let Err(err) = func.call::<_, Value>((event.clone(),)) {
                        tracing::error!("Callback error in {}: {}", name, err);
                    }
                }
            })
        } else {
            tracing::error!("Could not find connection for {}", name);
        }
        // let args = &args[1..];
        None
    }
}

fn define_event_listeners<'js, T>(ty: &Type<'js, T>) -> Result<()>
where
    T: 'static + AsObject + CastRef<GObject>,
    &'js T: FromJs<'js>,
{
    ty.raw_method(
        "addEventListener",
        move |this: &'js T,
              ctx: Ctx<'js>,
              name: String,
              func: Function<'js>,
              opts: Opt<Value<'js>>|
              -> Result<()> {
            let obj = this.as_object();
            tracing::info!("Adding event listener for {}::{}", obj.type_().name(), name);

            let data = obj.try_get_or_create_data("svEvents", || -> Result<Events> {
                Ok(Events {
                    ctx: Context::from_ctx(ctx)?,
                    map: Default::default(),
                })
            })?;
            let entry = data.map.entry(name.clone()).or_default();
            if entry.id.is_none() {
                tracing::info!("Connecting {}::{}", obj.type_().name(), name);
                let id = obj
                    .connect(name.as_str(), false, handle_event(name.clone()))
                    .map_err(|err| exception!("Could not connect to signal {}: {}", name, err))?;
                entry.id = Some(id);
            }
            entry.callbacks.push(func.into_value().into_js_value());

            Result::Ok(())
        },
    )?;

    ty.raw_method(
        "removeEventListener",
        move |this: &'js T,
              ctx: Ctx<'js>,
              name: String,
              func: Function<'js>,
              opts: Opt<Value<'js>>|
              -> Result<()> {
            let obj = this.as_object();
            tracing::info!(
                "Removing event listener for {}::{}",
                obj.type_().name(),
                name
            );
            let func = func.into_value();
            if let Some(data) = obj.get_data::<Events>("svEvents") {
                if let Some(conn) = data.map.get_mut(&name) {
                    conn.callbacks
                        .retain(|val| unsafe { Value::from_js_value_const(ctx, *val) != func });
                    if conn.callbacks.len() == 0 {
                        tracing::info!("Disconnecting {}::{}", obj.type_().name(), name);
                        obj.disconnect(conn.id.take().unwrap());
                    }
                    return Result::Ok(());
                }
            }
            tracing::warn!("Event {} is not connected", name);
            Result::Ok(())
        },
    )
}

fn define_sv_extra<'js, T>(ty: &Type<'js, T>) -> Result<()>
where
    T: 'static + AsObject,
    &'js T: FromJs<'js>,
{
    ty.custom_prop(
        "svExtra",
        rquickjs::Accessor::new_get(rquickjs::Method(|this: &'js T, ctx: Ctx<'js>| unsafe {
            let obj = this.as_object();
            tracing::info!("Getting {}::svExtra", obj.type_().name());

            if let Some(data) = obj.data::<ExtraData>("svExtra") {
                rquickjs::Result::Ok(Value::from_js_value_const(ctx, data.as_ref().value))
            } else {
                let value = rquickjs::Object::new(ctx)?;
                let data = ExtraData {
                    ctx: Context::from_ctx(ctx)?,
                    value: value.clone().into_value().into_js_value(),
                };
                obj.set_data("svExtra", data);
                rquickjs::Result::Ok(value.into_value())
            }
        })),
    )
}

impl<U> CastRef<U> for Object
where
    U: ObjectType,
{
    fn cast_ref(&self) -> Option<&U> {
        self.0.dynamic_cast_ref::<U>()
    }
}

impl rquickjs::ClassDef for Object {
    const CLASS_NAME: &'static str = "GObject";

    unsafe fn class_id() -> &'static mut rquickjs::ClassId {
        static mut CLASS_ID: rquickjs::ClassId = rquickjs::ClassId::new();
        &mut CLASS_ID
    }

    const HAS_PROTO: bool = true;

    fn init_proto<'js>(
        ctx: rquickjs::Ctx<'js>,
        proto: &rquickjs::Object<'js>,
    ) -> rquickjs::Result<()> {
        let ty = Type::<'js, Object>::new(ctx, proto.clone(), "GObject");
        ty.implements::<Self>()?;
        ty.register_as::<GObject>()?;
        Ok(())
    }
}

impl Object {
    pub fn new<T>(obj: T) -> Self
    where
        T: IsA<GObject> + Cast,
    {
        Object(obj.upcast::<GObject>())
    }

    pub fn inner(self) -> GObject {
        self.0
    }
}

impl std::ops::Deref for Object {
    type Target = GObject;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> From<T> for Object
where
    T: IsA<GObject> + Cast,
{
    fn from(val: T) -> Self {
        Object::new(val)
    }
}

impl<'js> IntoJs<'js> for Object {
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        self.into_js_obj(ctx)
    }
}

impl<'js> FromJs<'js> for &'js Object {
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        Object::from_js_ref(ctx, value)
    }
}

impl<'js> FromJs<'js> for &'js mut Object {
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        Object::from_js_mut(ctx, value)
    }
}

impl<'js> FromJs<'js> for Object {
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        Object::from_js_obj(ctx, value)
    }
}

impl AsObject for Object {
    fn as_object(&self) -> &GObject {
        &self.0
    }
}
