use rquickjs::Persistent;

use crate::{build_class, inner::*};

#[derive(Clone)]
pub struct Event<T> {
    _type: Option<String>,
    interface: String,
    bubbles: bool,
    cancelable: bool,
    detail: T,
}

impl<T> Event<T>
where
    T: for<'js> IntoJs<'js>,
{
    pub fn new<'js>(ctx: Ctx<'js>, _type: &'static str, detail: T) -> Result<Event<Value<'js>>> {
        let detail: Value<'js> = detail.into_js(ctx)?;
        Ok(Event {
            _type: Some(_type.to_string()),
            interface: "CustomEvent".to_string(),
            bubbles: false,
            cancelable: false,
            detail: detail,
        })
    }
}

impl<'js> Event<Value<'js>> {
    pub fn custom(ctx: Ctx<'js>) -> Result<Value<'js>> {
        Event {
            _type: None,
            interface: "CustomEvent".to_string(),
            bubbles: false,
            cancelable: false,
            detail: Value::new_undefined(ctx),
        }
        .into_js(ctx)
    }
}

impl<'x> ClassDef for Event<Value<'x>> {
    const CLASS_NAME: &'static str = "Event";

    unsafe fn class_id() -> &'static mut ClassId {
        static mut CLASS_ID: ClassId = ClassId::new();
        &mut CLASS_ID
    }

    const HAS_PROTO: bool = true;

    const HAS_STATIC: bool = true;

    const HAS_REFS: bool = true;

    fn init_proto<'js>(ctx: Ctx<'js>, proto: &Object<'js>) -> Result<()> {
        proto.set(
            "initCustomEvent",
            rquickjs::Func::new(
                "initCustomEvent",
                rquickjs::Method(
                    |event: &mut Event<Value<'js>>,
                     ctx: Ctx<'js>,
                     _type: String,
                     bubbles: bool,
                     cancelable: bool,
                     detail: Value<'js>| {
                        event._type = Some(_type);
                        event.bubbles = bubbles;
                        event.cancelable = cancelable;
                        event.detail = detail;
                    },
                ),
            ),
        )?;

        proto.set(
            "stopPopagation",
            rquickjs::Func::new(
                "stopPopagation",
                rquickjs::Method(|_: &Event<Value<'js>>| {}),
            ),
        )?;
        proto.set(
            "preventDefault",
            rquickjs::Func::new(
                "preventDefault",
                rquickjs::Method(|_: &Event<Value<'js>>| {}),
            ),
        )?;
        proto.prop(
            "type",
            Accessor::new_get(
                rquickjs::Method(move |this: &Event<Value>| this._type.clone()),
                // rquickjs::Method(move |this: &mut Event<Value>, val: String| {
                //     this._type = Some(val);
                // }),
            ),
        )?;
        proto.prop(
            "bubbles",
            Accessor::new_get(
                rquickjs::Method(move |this: &Event<Value>| this.bubbles),
                // rquickjs::Method(move |this: &mut Event<Value>, val: bool| {
                //     this.bubbles = val;
                // }),
            ),
        )?;
        proto.prop(
            "cancelable",
            Accessor::new_get(
                rquickjs::Method(move |this: &Event<Value>| this.cancelable),
                // rquickjs::Method(move |this: &mut Event<Value>, val: bool| {
                //         this.cancelable = val;
                //     }),
            ),
        )?;
        proto.prop(
            "detail",
            Accessor::new_get(
                rquickjs::Method(move |this: &Event<Value<'js>>| this.detail.clone()),
                // rquickjs::Method(move |this: &mut Event<Value<'js>>, val: Value<'js>| {
                //     this.detail = val;
                // }),
            ),
        )?;
        Ok(())
    }

    fn init_static<'js>(ctx: Ctx<'js>, _static: &Object<'js>) -> Result<()> {
        Ok(())
    }

    fn mark_refs(&self, marker: &RefsMarker) {}

    fn into_js_obj<'js>(self, ctx: Ctx<'js>) -> Result<Value<'js>>
    where
        Self: Sized,
    {
        JsClass::<Self>::instance(ctx, self).map(|cls| cls.into_value())
    }

    fn from_js_ref<'js>(ctx: Ctx<'js>, value: Value<'js>) -> Result<&'js Self>
    where
        Self: Sized,
    {
        let value = Object::from_js(ctx, value)?;
        JsClass::<Self>::try_ref(ctx, &value)
    }

    fn from_js_mut<'js>(ctx: Ctx<'js>, value: Value<'js>) -> Result<&'js mut Self>
    where
        Self: Sized,
    {
        let value = Object::from_js(ctx, value)?;
        JsClass::<Self>::try_mut(ctx, &value)
    }

    fn from_js_obj<'js>(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self>
    where
        Self: Clone + Sized,
    {
        let value = Object::from_js(ctx, value)?;
        let instance = JsClass::<Self>::try_ref(ctx, &value)?;
        Ok(instance.clone())
    }
}

impl<'js> rquickjs::IntoJs<'js> for Event<Value<'js>> {
    fn into_js(self, ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
        <Event<Value<'js>> as rquickjs::ClassDef>::into_js_obj(self, ctx)
    }
}

impl<'js> rquickjs::FromJs<'js> for &'js Event<Value<'js>> {
    fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
        <Event<Value<'js>> as rquickjs::ClassDef>::from_js_ref(ctx, value)
    }
}

impl<'js> rquickjs::FromJs<'js> for &'js mut Event<Value<'js>> {
    fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
        <Event<Value<'js>> as rquickjs::ClassDef>::from_js_mut(ctx, value)
    }
}
