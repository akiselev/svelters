use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::inner::*;

#[derive(Clone)]
pub struct ElementProp {
    pub(in crate) inherited: bool,
    pub(in crate) name: &'static str,
    pub(in crate) getter: Rc<dyn for<'js> Fn(This<Value<'js>>, Ctx<'js>) -> Result<Value<'js>>>,
    pub(in crate) setter:
        Option<Rc<dyn for<'js> Fn(This<Value<'js>>, Ctx<'js>, Value<'js>) -> Result<()>>>,
}

#[derive(Clone)]
pub struct ElementType {
    pub(in crate) name: &'static str,
    pub(in crate) inherits: Vec<&'static str>,
    pub(in crate) props: HashMap<&'static str, ElementProp>,
    pub(in crate) constructor: Rc<dyn for<'js> Fn(Ctx<'js>) -> Result<Value<'js>>>,
}

impl ElementType {
    pub fn new<FUNC>(name: &'static str, constructor: FUNC) -> ElementType
    where
        FUNC: 'static + for<'js> Fn(Ctx<'js>) -> Result<Value<'js>>,
    {
        ElementType {
            name,
            inherits: vec![],
            props: HashMap::new(),
            constructor: Rc::new(constructor),
        }
    }

    pub fn inherits<SUPER: Type>(&mut self) {
        self.inherits.push(SUPER::CLASS_NAME);
    }

    pub fn prop<BASE, CLS, PROP, GET, SET>(&mut self, name: &'static str, getter: GET, setter: SET)
    where
        CLS: Type,
        BASE: Type + Caster<CLS>,
        PROP: 'static + std::fmt::Debug + for<'x> IntoJs<'x> + for<'x> FromJs<'x>,
        GET: 'static + for<'x> Fn(&'x CLS) -> Result<PROP>,
        SET: 'static + for<'x> Fn(&'x CLS, PROP) -> Result<()>,
    {
        let prop = ElementProp {
            name,
            inherited: BASE::CLASS_NAME == CLS::CLASS_NAME,
            getter: Rc::new(move |this: This<Value<'_>>, ctx: Ctx<'_>| {
                let this = Element::<BASE>::from_js_ref(ctx, this.0)?;
                this.inner
                    .with_cast(|data| {
                        let val = getter(data)?;
                        rquickjs::Result::Ok(val)
                    })
                    .into_js(ctx)
            }),
            setter: Some(Rc::new(
                move |this: This<Value<'_>>, ctx: Ctx<'_>, value: Value<'_>| {
                    let this = Element::<BASE>::from_js_ref(ctx, this.0)?;
                    let val = PROP::from_js(ctx, value)?;
                    this.inner.with_cast(|data| {
                        setter(data, val)?;
                        Result::<_>::Ok(())
                    })
                },
            )),
        };
        self.props.insert(name, prop);
    }

    pub fn readonly<BASE, CLS, PROP, GET>(&mut self, name: &'static str, getter: GET)
    where
        CLS: Type,
        BASE: Type + Caster<CLS>,
        PROP: 'static + std::fmt::Debug + for<'x> IntoJs<'x> + for<'x> FromJs<'x>,
        GET: 'static + for<'x> Fn(&'x CLS) -> Result<PROP>,
    {
        let prop = ElementProp {
            name,
            inherited: BASE::CLASS_NAME == CLS::CLASS_NAME,
            getter: Rc::new(move |this: This<Value<'_>>, ctx: Ctx<'_>| {
                let this = Element::<BASE>::from_js_ref(ctx, this.0)?;
                this.inner
                    .with_cast(|data| {
                        let val = getter(data)?;
                        rquickjs::Result::Ok(val)
                    })
                    .into_js(ctx)
            }),
            setter: None,
        };
        self.props.insert(name, prop);
    }
}

#[derive(Clone)]
pub struct Document {
    pub(in crate) elements: Rc<RefCell<HashMap<&'static str, ElementType>>>,
}

impl Document {
    pub fn new() -> Document {
        Document {
            elements: Default::default(),
        }
    }
}

impl ClassDef for Document {
    const CLASS_NAME: &'static str = "Document";

    unsafe fn class_id() -> &'static mut ClassId {
        static mut class_id: ClassId = ClassId::new();
        &mut class_id
    }

    const HAS_PROTO: bool = true;

    const HAS_STATIC: bool = true;

    const HAS_REFS: bool = true;

    fn init_proto<'js>(ctx: Ctx<'js>, proto: &Object<'js>) -> Result<()> {
        proto.set(
            "createElement",
            rquickjs::Func::new("createElement", rquickjs::Method(create_element)),
        )?;
        proto.set(
            "createElementNS",
            rquickjs::Func::new("createElementNS", rquickjs::Method(create_element_ns)),
        )?;
        proto.set(
            "dispatchEvent",
            rquickjs::Func::new("dispatchEvent", rquickjs::Method(dispatch_event)),
        )?;
        Ok(())
    }

    fn init_static<'js>(ctx: Ctx<'js>, _static: &Object<'js>) -> Result<()> {
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

impl<'js> rquickjs::IntoJs<'js> for Document {
    fn into_js(self, ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
        <Document as rquickjs::ClassDef>::into_js_obj(self, ctx)
    }
}

impl<'js> rquickjs::FromJs<'js> for &'js Document {
    fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
        <Document as rquickjs::ClassDef>::from_js_ref(ctx, value)
    }
}

impl<'js> rquickjs::FromJs<'js> for &'js mut Document {
    fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
        <Document as rquickjs::ClassDef>::from_js_mut(ctx, value)
    }
}
