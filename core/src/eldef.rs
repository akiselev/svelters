use phf::Map;
use smallvec::SmallVec;

use crate::inner::*;

pub type InterfaceDefFn = SmallBox<dyn Fn(&mut ElementDef), space::S8>;
pub type ElementDefFn = Rc<dyn for<'a> Fn(TypeBuilder<'a>) -> ElementDef>;
pub type ElementContainer = SmallBox<dyn IElement, space::S8>;

pub type ConstructorFn = SmallBox<dyn for<'js> Fn(Ctx<'js>) -> Result<ElementContainer>, space::S8>;
pub type PropGetterFn =
    SmallBox<dyn for<'js> Fn(Ctx<'js>, &dyn IElement) -> Result<Value<'js>>, space::S8>;
pub type PropSetterFn =
    SmallBox<dyn for<'js> Fn(Ctx<'js>, &mut dyn IElement, Value<'js>) -> Result<()>, space::S8>;
pub type MethodFn = SmallBox<
    dyn for<'js> Fn(Ctx<'js>, &mut dyn IElement, Rest<Value<'js>>) -> Result<Value<'js>>,
    space::S8,
>;

pub struct ElementDef {
    pub(in crate) tag_name: &'static str,
    pub(in crate) constructor: ConstructorFn,
    pub(in crate) members: HashMap<&'static str, Member>,
    pub(in crate) slot: Option<SmallBox<dyn INativeSlot, space::S4>>,
}

impl ElementDef {
    #[inline(always)]
    pub fn new<'a, FUNC: 'static + for<'js> Fn(Ctx<'js>) -> Result<ElementContainer>>(
        builder: TypeBuilder<'a>,
        tag_name: &'static str,
        constructor: FUNC,
    ) -> ElementBuilder<'a> {
        ElementBuilder {
            builder,
            def: ElementDef {
                tag_name,
                constructor: smallbox!(constructor),
                members: Default::default(),
                slot: None,
            },
        }
    }

    pub fn method<FUNC: 'static + IMethod>(&mut self, name: &'static str, func: FUNC) -> &mut Self {
        tracing::info!("Added method {}::{}", self.tag_name, name);
        if self
            .members
            .insert(name, Member::Method(smallbox!(func)))
            .is_some()
        {
            panic!(
                "Member {} on element {} already defined",
                name, self.tag_name
            );
        }
        self
    }

    pub fn prop<FUNC: 'static + IProperty>(&mut self, name: &'static str, func: FUNC) -> &mut Self {
        tracing::info!("Added property {}::{}", self.tag_name, name);
        if self
            .members
            .insert(name, Member::Property(smallbox!(func)))
            .is_some()
        {
            panic!(
                "Member {} on element {} already defined",
                name, self.tag_name
            );
        }
        self
    }

    pub fn slot<FUNC>(&mut self, func: FUNC) -> &mut Self
    where
        FUNC: INativeSlot,
    {
        tracing::info!("Added slot to {}", self.tag_name);
        if self.slot.is_some() {
            panic!("Slot on element {} already defined", self.tag_name);
        }
        self.slot = Some(smallbox!(func));
        self
    }
}

pub(in crate) enum Member {
    Property(SmallBox<dyn IProperty, space::S8>),
    Method(SmallBox<dyn IMethod, space::S8>),
}

pub trait IProperty {
    fn name<'a>(&'a self) -> Cow<'a, str>;
    fn get<'js>(&self, ctx: Ctx<'js>, element: &dyn IElement) -> Result<Value<'js>> {
        Err(exception!("{} does not implement a getter", self.name()))
    }
    fn set<'js>(&self, ctx: Ctx<'js>, element: &dyn IElement, value: Value<'js>) -> Result<()> {
        Err(exception!("{} does not implement a setter", self.name()))
    }
}

pub trait IMethod {
    fn call<'js, 'a>(
        &self,
        ctx: Ctx<'js>,
        element: &'a dyn IElement,
        args: Vec<Value<'js>>,
    ) -> Result<Value<'js>>;
}

pub struct ElementBuilder<'a> {
    builder: TypeBuilder<'a>,
    def: ElementDef,
}

impl<'a> ElementBuilder<'a> {
    pub fn build(self) -> ElementDef {
        self.def
    }

    pub fn with_interface(mut self, name: &'static str) -> Self {
        if let Some(interface_fn) = self.builder.rt.interfaces.get(name) {
            (interface_fn)(&mut self.def);
        } else {
            panic!("Interface {} not found", name);
        }
        self
    }

    pub fn method<FUNC: 'static + IMethod>(&mut self, name: &'static str, func: FUNC) -> &mut Self {
        self.def.method(name, func);
        self
    }

    pub fn prop<FUNC: 'static + IProperty>(&mut self, name: &'static str, func: FUNC) -> &mut Self {
        self.def.prop(name, func);
        self
    }
}
