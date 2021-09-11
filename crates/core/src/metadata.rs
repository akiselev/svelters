use std::any::{Any, TypeId};
use std::{collections::HashMap, rc::Rc};

use rquickjs::Function;
use smallvec::SmallVec;

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

    pub(in crate) fn metadata() -> &'static Metadata {
        unsafe { CLS::metadata() }
    }

    pub(in crate) fn typekey() -> TypeKey {
        unsafe { CLS::metadata().typekey() }
    }
}

/// Type metadata for a type registered with Svelters
pub struct Metadata {
    pub(in crate) _typekey: Option<TypeKey>,
    pub(in crate) type_id: TypeId,
    pub(in crate) obj_id: ClassId,
    pub(in crate) virt_id: ClassId,
}

impl Metadata {
    pub const fn new<CLS: Type + Any>() -> Metadata {
        Metadata {
            obj_id: ClassId::new(),
            _typekey: None,
            virt_id: ClassId::new(),
            type_id: TypeId::of::<CLS>(),
        }
    }

    pub(in crate) unsafe fn init(&mut self, _typekey: TypeKey) -> Result<()> {
        self.obj_id.init();
        self.virt_id.init();
        self._typekey = Some(_typekey);
        Ok(())
    }

    pub(in crate) fn typekey(&self) -> TypeKey {
        self._typekey.unwrap()
    }
}

pub struct PropertyDef {
    pub(in crate) inherited: bool,
    pub(in crate) name: &'static str,
    pub(in crate) getter:
        SmallBox<dyn for<'js> Fn(Ctx<'js>, &Rt, Idx) -> Result<Value<'js>>, space::S4>,
    pub(in crate) setter:
        Option<SmallBox<dyn for<'js> Fn(Ctx<'js>, &Rt, Idx, Value<'js>) -> Result<()>, space::S4>>,
}

pub struct EventDef {
    pub(in crate) inherited: bool,
    pub(in crate) name: &'static str,
    pub(in crate) subscribe: Rc<dyn SubscribeFn>,
}

pub struct TypeDef {
    pub(in crate) typeid: TypeId,
    pub(in crate) name: &'static str,
    pub(in crate) metadata: &'static Metadata,
    pub(in crate) inherits: SmallVec<[&'static str; 8]>,
    pub(in crate) props: HashMap<&'static str, PropertyDef>,
    pub(in crate) events: HashMap<&'static str, EventDef>,
    pub(in crate) element_proto: Option<RegisteryKey>,
    pub(in crate) casters: CasterMap,
}

impl TypeDef {
    pub fn new<CLS: Type>() -> TypeDef {
        TypeDef {
            typeid: TypeId::of::<CLS>(),
            name: CLS::CLASS_NAME,
            metadata: unsafe { CLS::metadata() },
            inherits: SmallVec::new(),
            props: HashMap::new(),
            events: HashMap::new(),
            casters: HashMap::new(),
            element_proto: None,
        }
    }

    pub fn inherits<SUPER: Type>(&mut self) {
        self.inherits.push(SUPER::CLASS_NAME);
    }

    pub fn prop<BASE, CLS, PROP, GET, SET>(&mut self, name: &'static str, getter: GET, setter: SET)
    where
        CLS: Type,
        BASE: Type + Upcast<CLS>,
        PROP: 'static + for<'x> IntoJs<'x> + for<'x> FromJs<'x>,
        GET: 'static + for<'x> Fn(&'x CLS) -> Result<PROP>,
        SET: 'static + for<'x> Fn(&'x CLS, PROP) -> Result<()>,
    {
        let prop = PropertyDef {
            name,
            inherited: BASE::CLASS_NAME != CLS::CLASS_NAME,
            getter: smallbox!(getter_fn::<BASE, CLS, PROP, _>(getter)),
            setter: Some(smallbox!(setter_fn::<BASE, CLS, PROP, _>(setter))),
        };
        self.props.insert(name, prop);
    }

    pub fn readonly<'js, BASE, CLS, PROP, GET>(&mut self, name: &'static str, getter: GET)
    where
        CLS: Type,
        BASE: Type + Upcast<CLS>,
        PROP: 'static + for<'x> IntoJs<'x> + for<'x> FromJs<'x>,
        GET: 'static + for<'x> Fn(&'x CLS) -> Result<PROP>,
    {
        let prop = PropertyDef {
            name,
            inherited: BASE::CLASS_NAME != CLS::CLASS_NAME,
            getter: smallbox!(getter_fn::<BASE, CLS, PROP, _>(getter)),
            setter: None,
        };
        self.props.insert(name, prop);
    }

    #[inline(always)]
    pub fn event<BASE: Type, CLS: Type, FUNC>(&mut self, name: &'static str, func: FUNC)
    where
        CLS: Type,
        BASE: Type + Upcast<CLS>,
        FUNC: 'static + SubscribeFn,
    {
        let event = EventDef {
            name,
            inherited: BASE::CLASS_NAME != CLS::CLASS_NAME,
            subscribe: Rc::new(move |el: Element<'_>, callback: Function<'_>| func(el, callback)),
        };
        self.events.insert(name, event);
    }
}

fn getter_fn<BASE, CLS, PROP, GET>(
    getter: GET,
) -> impl for<'js> Fn(Ctx<'js>, &Rt, Idx) -> Result<Value<'js>>
where
    CLS: Type,
    BASE: Type + Upcast<CLS>,
    PROP: 'static + for<'js> IntoJs<'js> + for<'js> FromJs<'js>,
    GET: 'static + for<'x> Fn(&'x CLS) -> Result<PROP>,
{
    move |ctx: Ctx, rt: &Rt, idx: Idx| {
        let obj = Obj::<BASE>::from_rt(&rt, idx).unwrap();
        let cls = obj.upcast();
        let val = getter(&cls)?;
        val.into_js(ctx)
    }
}

fn setter_fn<BASE, CLS, PROP, SET>(
    setter: SET,
) -> impl for<'js> Fn(Ctx<'js>, &Rt, Idx, Value<'js>) -> Result<()>
where
    CLS: Type,
    BASE: Type + Upcast<CLS>,
    PROP: 'static + for<'js> IntoJs<'js> + for<'js> FromJs<'js>,
    SET: 'static + for<'x> Fn(&'x CLS, PROP) -> Result<()>,
{
    move |ctx: Ctx, rt: &Rt, idx: Idx, value: Value| {
        let obj = Obj::<BASE>::from_rt(&rt, idx).unwrap();
        let cls = obj.upcast();
        let val = PROP::from_js(ctx, value)?;
        setter(&cls, val)
    }
}
