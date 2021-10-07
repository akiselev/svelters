use rquickjs::HasRefs;

use crate::inner::*;

#[derive(Clone)]
pub struct Event<'js> {
    pub(in crate) _type: Option<String>,
    pub(in crate) interface: String,
    pub(in crate) bubbles: bool,
    pub(in crate) cancelable: bool,
    pub(in crate) target: Value<'js>,
    pub(in crate) detail: Value<'js>,
}

impl<'js> Event<'js> {
    pub fn new(
        ctx: Ctx<'js>,
        _type: &'static str,
        target: Value<'js>,
        detail: Value<'js>,
    ) -> Result<Event<'js>> {
        let detail: Value<'js> = detail.into_js(ctx)?;
        Ok(Event {
            _type: Some(_type.to_string()),
            interface: "CustomEvent".to_string(),
            bubbles: false,
            cancelable: false,
            target,
            detail,
        })
    }
}

impl<'js> HasRefs for Event<'js> {
    fn mark_refs(&self, marker: &RefsMarker) {
        self.detail.as_object().map(|obj| obj.mark(marker));
        self.target.as_object().map(|obj| obj.mark(marker));
    }
}

impl<'js> Event<'js> {
    pub fn custom(ctx: Ctx<'js>) -> Result<Value<'js>> {
        Event {
            _type: None,
            interface: "CustomEvent".to_string(),
            bubbles: false,
            cancelable: false,
            target: Value::new_undefined(ctx),
            detail: Value::new_undefined(ctx),
        }
        .into_js(ctx)
    }
}

impl<'x> ClassDef for Event<'x> {
    const CLASS_NAME: &'static str = "Event";

    unsafe fn class_id() -> &'static mut ClassId {
        static mut CLASS_ID: ClassId = ClassId::new();
        &mut CLASS_ID
    }

    const HAS_PROTO: bool = true;

    const HAS_STATIC: bool = true;

    const HAS_REFS: bool = true;

    fn init_proto<'js>(_ctx: Ctx<'js>, proto: &Object<'js>) -> Result<()> {
        proto.set(
            "initCustomEvent",
            rquickjs::Func::new(
                "initCustomEvent",
                rquickjs::Method(
                    |event: &mut Event<'js>,
                     _ctx: Ctx<'js>,
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
            rquickjs::Func::new("stopPopagation", rquickjs::Method(|_: &Event<'js>| {})),
        )?;
        proto.set(
            "preventDefault",
            rquickjs::Func::new("preventDefault", rquickjs::Method(|_: &Event<'js>| {})),
        )?;
        proto.prop(
            "type",
            Accessor::new_get(
                rquickjs::Method(move |this: &Event| this._type.clone()),
                // rquickjs::Method(move |this: &mut Event, val: String| {
                //     this._type = Some(val);
                // }),
            ),
        )?;
        proto.prop(
            "bubbles",
            Accessor::new_get(
                rquickjs::Method(move |this: &Event| this.bubbles),
                // rquickjs::Method(move |this: &mut Event, val: bool| {
                //     this.bubbles = val;
                // }),
            ),
        )?;
        proto.prop(
            "cancelable",
            Accessor::new_get(
                rquickjs::Method(move |this: &Event| this.cancelable),
                // rquickjs::Method(move |this: &mut Event, val: bool| {
                //         this.cancelable = val;
                //     }),
            ),
        )?;
        proto.prop(
            "detail",
            Accessor::new_get(
                rquickjs::Method(move |this: &Event<'js>| this.detail.clone()),
                // rquickjs::Method(move |this: &mut Event<'js>, val: Value<'js>| {
                //     this.detail = val;
                // }),
            ),
        )?;
        proto.prop(
            "target",
            Accessor::new_get(
                rquickjs::Method(move |this: &Event<'js>| this.target.clone()),
                // rquickjs::Method(move |this: &mut Event<'js>, val: Value<'js>| {
                //     this.detail = val;
                // }),
            ),
        )?;
        Ok(())
    }

    fn init_static<'js>(_ctx: Ctx<'js>, _static: &Object<'js>) -> Result<()> {
        Ok(())
    }

    fn mark_refs(&self, _marker: &RefsMarker) {}

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

impl<'js> rquickjs::IntoJs<'js> for Event<'js> {
    fn into_js(self, ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
        <Event<'js> as rquickjs::ClassDef>::into_js_obj(self, ctx)
    }
}

impl<'js> rquickjs::FromJs<'js> for &'js Event<'js> {
    fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
        <Event<'js> as rquickjs::ClassDef>::from_js_ref(ctx, value)
    }
}

impl<'js> rquickjs::FromJs<'js> for &'js mut Event<'js> {
    fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
        <Event<'js> as rquickjs::ClassDef>::from_js_mut(ctx, value)
    }
}
