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
