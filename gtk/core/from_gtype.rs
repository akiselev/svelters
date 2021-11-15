use std::{
    collections::HashMap,
    os::raw::{c_long, c_ulong},
};

use convert_case::{Case, Casing};
use glib::{Class, SignalHandlerId};
use rquickjs::{qjs, Context, Ctx, Function, IntoJs, Object as JsObject, Opt, Value};

use crate::{
    common::*,
    core::{enum_prop::EnumProp, obj_prop::ObjectProp, property::Prop},
    registry::Registry,
    Object,
};

pub fn init_gtype<'js>(ctx: Ctx<'js>, proto: &JsObject<'js>, ty: glib::Type) -> Result<()> {
    let cls = Class::<GObject>::from_type(ty).unwrap();
    let params = cls.list_properties();
    let cls_name = ty.name();
    // println!("Props for {}:", cls_name);
    for param in params.into_iter() {
        let param_ty = param.value_type();
        let prop_name = param.name().to_case(Case::Camel);
        use glib::types::Type;
        if param_ty.is_a(Type::I8) {
            // println!("    {} ({}): {}", prop_name, param.name(), Type::I8.name());

            proto.prop(prop_name.as_str(), Prop::<i8>::new(param))?;
        } else if param_ty.is_a(Type::U8) {
            // println!("    {} ({}): {}", prop_name, param.name(), Type::U8.name());

            proto.prop(prop_name.as_str(), Prop::<u8>::new(param))?;
        } else if param_ty.is_a(Type::BOOL) {
            // println!(
            //     "    {} ({}): {}",
            //     prop_name,
            //     param.name(),
            // Type::BOOL.name()
            // );

            proto.prop(prop_name.as_str(), Prop::<bool>::new(param))?;
        } else if param_ty.is_a(Type::I32) {
            // println!("    {} ({}): {}", prop_name, param.name(), Type::I32.name());

            proto.prop(prop_name.as_str(), Prop::<i32>::new(param))?;
        } else if param_ty.is_a(Type::U32) {
            // println!("    {} ({}): {}", prop_name, param.name(), Type::U32.name());

            proto.prop(prop_name.as_str(), Prop::<u32>::new(param))?;
        } else if param_ty.is_a(Type::I_LONG) {
            // println!(
            //     "    {} ({}): {}",
            //     prop_name,
            //     param.name(),
            // Type::I_LONG.name()
            // );

            proto.prop(prop_name.as_str(), Prop::<c_long>::new(param))?;
        } else if param_ty.is_a(Type::U_LONG) {
            // println!(
            //     "    {} ({}): {}",
            //     prop_name,
            //     param.name(),
            // Type::U_LONG.name()
            // );

            proto.prop(prop_name.as_str(), Prop::<c_ulong>::new(param))?;
        } else if param_ty.is_a(Type::I64) {
            // println!("    {} ({}): {}", prop_name, param.name(), Type::I64.name());

            proto.prop(prop_name.as_str(), Prop::<i64>::new(param))?;
        } else if param_ty.is_a(Type::U64) {
            // println!("    {} ({}): {}", prop_name, param.name(), Type::U64.name());

            proto.prop(prop_name.as_str(), Prop::<u64>::new(param))?;
        } else if param_ty.is_a(Type::F32) {
            // println!("    {} ({}): {}", prop_name, param.name(), Type::F32.name());

            proto.prop(prop_name.as_str(), Prop::<f32>::new(param))?;
        } else if param_ty.is_a(Type::F64) {
            // println!("    {} ({}): {}", prop_name, param.name(), Type::F64.name());

            proto.prop(prop_name.as_str(), Prop::<f64>::new(param))?;
        } else if param_ty.is_a(Type::STRING) {
            // println!("    {} ({}): {}", prop_name, param.name(), param_ty.name());

            proto.prop(prop_name.as_str(), Prop::<String>::new(param))?;
        } else if param_ty.is_a(Vec::<String>::static_type()) {
            // println!("    {} ({}): string[]", prop_name, param.name(),);

            proto.prop(prop_name.as_str(), Prop::<Vec<String>>::new(param))?;
        // } else if param_ty.is_a(Type::POINTER) {
        //     // println!(
        //     //     ">>>>>> Type::POINTER not implemented for prop {}::{}",
        //     //     cls_name, prop_name
        //     // );
        // } else if param_ty.is_a(Type::VARIANT) {
        //     // println!(
        //     //     ">>>>>> Type::VARIANT not implemented for prop {}::{}",
        //     //     cls_name, prop_name
        //     // );
        // } else if param_ty.is_a(Type::INTERFACE) {
        //     // println!(
        //     //     ">>>>>> Type::INTERFACE not implemented for prop {}::{}",
        //     //     cls_name, prop_name
        //     // );
        } else if param_ty.is_a(Type::ENUM) {
            // println!("    {} ({}): {}", prop_name, param.name(), param_ty.name());
            proto.prop::<_, _, String>(prop_name.as_str(), EnumProp::new(param))?;
            // ty.enum_prop(prop_name.as_str(), param)?;
        } else if param_ty.is_a(Type::FLAGS) {
            // println!(
            //     ">>>>>> Type::FLAGS not implemented for prop {}::{}",
            //     cls_name, prop_name
            // );
        } else if param_ty.is_a(Type::OBJECT) {
            proto.prop::<_, _, ()>(prop_name.as_str(), ObjectProp::new(param))?;
            // println!("    {} ({}): {}", prop_name, param.name(), param_ty.name());
            // ty.obj_prop(prop_name.as_str(), param)?;
        } else {
            println!(
                ">>>>>> Type::??? not implemented for prop {}::{}",
                cls_name, prop_name
            );
        }
    }
    if ty.is_a(GObject::static_type()) {
        define_event_listeners(ctx, proto)?;
        define_sv_extra(ctx, proto)?;
        proto.prop("isGObject", rquickjs::Accessor::new_get(|| true))?;
    }
    if ty.is_a(gtk4::Widget::static_type()) {
        proto.prop("isGtkWidget", rquickjs::Accessor::new_get(|| true))?;
    }
    Ok(())
}

struct ExtraData {
    _ctx: Context,
    value: qjs::JSValue,
}

struct Events {
    ctx: Context,
    map: HashMap<String, Connection>,
}

#[derive(Default)]
struct Connection {
    id: Option<SignalHandlerId>,
    callbacks: Vec<qjs::JSValue>,
}

fn handle_event(name: String) -> impl Fn(&[GValue]) -> Option<GValue> {
    move |args| {
        if args.is_empty() {
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
                event
                    .set("target", ctx.marshall(this.clone()).unwrap())
                    .unwrap();

                let mut details = vec![];
                for arg in &args[1..] {
                    match G(arg).into_js(ctx) {
                        Ok(val) => details.push(val),
                        Err(err) => {
                            details.push(Value::new_undefined(ctx));
                            tracing::error!(
                                "Could not convert Value of type {} to JS Value in {} handler: {}",
                                arg.type_().name(),
                                name,
                                err
                            );
                        }
                    }
                }
                event.set("details", details).unwrap();
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
        None
    }
}

fn define_event_listeners<'js>(ctx: Ctx<'js>, proto: &JsObject<'js>) -> Result<()> {
    proto.set(
        "addEventListener",
        Function::new(
            ctx,
            rquickjs::Method(
                move |this: &'js Object,
                      ctx: Ctx<'js>,
                      name: String,
                      func: Function<'js>,
                      _opts: Opt<Value<'js>>|
                      -> Result<()> {
                    tracing::info!(
                        "Adding event listener for {}::{}",
                        this.type_().name(),
                        name
                    );

                    let data = this.try_get_or_create_data("svEvents", || -> Result<Events> {
                        Ok(Events {
                            ctx: Context::from_ctx(ctx)?,
                            map: Default::default(),
                        })
                    })?;
                    let entry = data.map.entry(name.clone()).or_default();
                    if entry.id.is_none() {
                        tracing::info!("Connecting {}::{}", this.type_().name(), name);
                        let id = this
                            .connect(name.as_str(), false, handle_event(name.clone()))
                            .map_err(|err| {
                                exception!("Could not connect to signal {}: {}", name, err)
                            })?;
                        entry.id = Some(id);
                    }
                    entry.callbacks.push(func.into_value().into_js_value());

                    Result::Ok(())
                },
            ),
        ),
    )?;

    proto.set(
        "removeEventListener",
        Function::new(
            ctx,
            rquickjs::Method(
                move |this: &'js Object,
                      ctx: Ctx<'js>,
                      name: String,
                      func: Function<'js>,
                      _opts: Opt<Value<'js>>|
                      -> Result<()> {
                    tracing::info!(
                        "Removing event listener for {}::{}",
                        this.type_().name(),
                        name
                    );
                    let func = func.into_value();
                    if let Some(data) = this.get_data::<Events>("svEvents") {
                        if let Some(conn) = data.map.get_mut(&name) {
                            conn.callbacks.retain(|val| unsafe {
                                Value::from_js_value_const(ctx, *val) != func
                            });
                            if conn.callbacks.is_empty() {
                                tracing::info!("Disconnecting {}::{}", this.type_().name(), name);
                                this.disconnect(conn.id.take().unwrap());
                            }
                            return Result::Ok(());
                        }
                    }
                    tracing::warn!("Event {} is not connected", name);
                    Result::Ok(())
                },
            ),
        ),
    )
}

fn define_sv_extra<'js>(_ctx: Ctx<'js>, proto: &JsObject<'js>) -> Result<()> {
    proto.prop(
        "svExtra",
        rquickjs::Accessor::new_get(rquickjs::Method(
            |this: &'js Object, ctx: Ctx<'js>| unsafe {
                tracing::info!("Getting {}::svExtra", this.type_().name());

                if let Some(data) = this.data::<ExtraData>("svExtra") {
                    rquickjs::Result::Ok(Value::from_js_value_const(ctx, data.as_ref().value))
                } else {
                    let value = rquickjs::Object::new(ctx)?;
                    let data = ExtraData {
                        _ctx: Context::from_ctx(ctx)?,
                        value: value.clone().into_value().into_js_value(),
                    };
                    this.set_data("svExtra", data);
                    rquickjs::Result::Ok(value.into_value())
                }
            },
        )),
    )
}
