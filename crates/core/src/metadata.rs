use std::any::{Any, TypeId};
use std::{collections::HashMap, rc::Rc};

use rquickjs::Function;

use crate::inner::*;

/// Helper class to access type metadata
#[derive(Debug, Copy, Clone)]
pub struct Class<CLS>(PhantomData<CLS>);

impl<CLS> Class<CLS> {
    pub fn new() -> Self {
        Class(PhantomData)
    }
}

impl<CLS: Type> Class<CLS> {
    pub fn name(&self) -> &'static str {
        CLS::CLASS_NAME
    }
}

/// Type metadata for a type registered with Svelters
pub struct Metadata {
    pub(in crate) type_id: TypeId,
    pub(in crate) obj_id: ClassId,
    pub(in crate) virt_id: ClassId,
}

impl Metadata {
    pub const fn new<CLS: Type + Any>() -> Metadata {
        Metadata {
            obj_id: ClassId::new(),
            virt_id: ClassId::new(),
            type_id: TypeId::of::<CLS>(),
        }
    }

    pub(in crate) unsafe fn init(&mut self) -> Result<()> {
        self.obj_id.init();
        self.virt_id.init();
        Ok(())
    }
}

#[derive(Clone)]
pub struct ElementProp {
    pub(in crate) inherited: bool,
    pub(in crate) name: &'static str,
    pub(in crate) getter: Rc<dyn for<'js> Fn(Value<'js>, Ctx<'js>) -> Result<Value<'js>>>,
    pub(in crate) setter:
        Option<Rc<dyn for<'js> Fn(Value<'js>, Ctx<'js>, Value<'js>) -> Result<()>>>,
}

#[derive(Clone)]
pub struct ElementEvent {
    pub(in crate) inherited: bool,
    pub(in crate) name: &'static str,
    pub(in crate) subscribe: Rc<dyn SubscribeFn>,
}

#[derive(Clone)]
pub struct ElementType {
    pub(in crate) name: &'static str,
    pub(in crate) inherits: Vec<&'static str>,
    pub(in crate) props: HashMap<&'static str, ElementProp>,
    pub(in crate) events: HashMap<&'static str, ElementEvent>,
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
            events: HashMap::new(),
            constructor: Rc::new(constructor),
        }
    }

    pub fn inherits<SUPER: Type>(&mut self) {
        self.inherits.push(SUPER::CLASS_NAME);
    }

    pub fn prop<BASE, CLS, PROP, GET, SET>(&mut self, name: &'static str, getter: GET, setter: SET)
    where
        CLS: Type,
        BASE: Type + Cast<CLS>,
        PROP: 'static + for<'x> IntoJs<'x> + for<'x> FromJs<'x>,
        GET: 'static + for<'x> Fn(&'x CLS) -> Result<PROP>,
        SET: 'static + for<'x> Fn(&'x CLS, PROP) -> Result<()>,
    {
        let prop = ElementProp {
            name,
            inherited: BASE::CLASS_NAME != CLS::CLASS_NAME,
            getter: Rc::new(move |this: Value<'_>, ctx: Ctx<'_>| {
                let el = ElementNode::from_js(ctx, this)?;
                let this = el.borrow();
                let obj = Obj::<BASE>::from_js_ref(ctx, this.obj.clone().into_value())?;
                let ptr = obj.borrow().unwrap();
                let cls = ptr.cast();
                let val = getter(cls.as_ref())?;
                val.into_js(ctx)
            }),
            setter: Some(Rc::new(
                move |this: Value<'_>, ctx: Ctx<'_>, value: Value<'_>| {
                    let el = ElementNode::from_js(ctx, this)?;
                    let this = el.borrow();
                    let obj = Obj::<BASE>::from_js_ref(ctx, this.obj.clone().into_value())?;
                    let val = PROP::from_js(ctx, value)?;
                    let ptr = obj.borrow().unwrap();
                    let cls = ptr.cast();

                    setter(cls.as_ref(), val)?;
                    Result::<_>::Ok(())
                },
            )),
        };
        self.props.insert(name, prop);
    }

    pub fn readonly<BASE, CLS, PROP, GET>(&mut self, name: &'static str, getter: GET)
    where
        CLS: Type,
        BASE: Type + Cast<CLS>,
        PROP: 'static + for<'x> IntoJs<'x> + for<'x> FromJs<'x>,
        GET: 'static + for<'x> Fn(&'x CLS) -> Result<PROP>,
    {
        let prop = ElementProp {
            name,
            inherited: BASE::CLASS_NAME != CLS::CLASS_NAME,
            getter: Rc::new(move |this: Value<'_>, ctx: Ctx<'_>| {
                let el = ElementNode::from_js(ctx, this)?;
                let this = el.borrow();
                let obj = Obj::<BASE>::from_js_ref(ctx, this.obj.clone().into_value())?;
                let ptr = obj.borrow().unwrap();
                let cls = ptr.cast();
                let val = getter(cls.as_ref())?;
                val.into_js(ctx)
            }),
            setter: None,
        };
        self.props.insert(name, prop);
    }

    #[inline(always)]
    pub fn event<BASE: Type, CLS: Type, FUNC>(&mut self, name: &'static str, func: FUNC)
    where
        CLS: Type,
        BASE: Type + Cast<CLS>,
        FUNC: 'static + SubscribeFn,
    {
        let event = ElementEvent {
            name,
            inherited: BASE::CLASS_NAME != CLS::CLASS_NAME,
            subscribe: Rc::new(move |el: Element<'_>, callback: Function<'_>| func(el, callback)),
        };
        self.events.insert(name, event);
    }
}
