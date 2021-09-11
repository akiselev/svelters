use rquickjs::{bind, HasRefs, Object, Persistent};

#[allow(unused_imports)]
use crate::inner::*;

#[derive(Clone, Eq, PartialEq)]
pub(in crate) struct SlotInner<'js> {
    pub(in crate) name: String,
    pub(in crate) node: NodeData<'js>,
}

#[derive(Clone, Eq, PartialEq)]
pub struct SlotNode<'js>(pub(super) Inner<SlotInner<'js>>);

impl<'js> SlotNode<'js> {
    // constructor
    pub fn new() -> Self {
        Self(Inner::new(SlotInner {
            name: "default".to_string(),
            node: NodeData::new(),
        }))
    }

    pub(in crate) fn borrow<'a>(&'a self) -> std::cell::Ref<'a, SlotInner<'js>>
    where
        'js: 'a,
    {
        self.0.borrow()
    }

    pub(in crate) fn borrow_mut<'a>(&self) -> std::cell::RefMut<SlotInner<'js>>
    where
        'js: 'a,
    {
        self.0.borrow_mut()
    }

    pub fn get_name(&self) -> String {
        self.borrow().name.clone()
    }

    pub fn set_name(&self, name: String) {
        self.borrow_mut().name = name
    }
}

impl<'x> ClassDef for SlotNode<'x> {
    const CLASS_NAME: &'static str = "SlotNode";

    unsafe fn class_id() -> &'static mut ClassId {
        static mut CLASS_ID: ClassId = ClassId::new();
        &mut CLASS_ID
    }

    const HAS_PROTO: bool = true;

    const HAS_STATIC: bool = true;

    const HAS_REFS: bool = true;

    fn init_proto<'js>(_ctx: Ctx<'js>, _proto: &Object<'js>) -> Result<()> {
        // proto.set(
        //     "createEvent",
        //     rquickjs::Func::new("createEvent", rquickjs::Method(Slot::create_event)),
        // )?;
        // proto.set(
        //     "createElement",
        //     rquickjs::Func::new("createElement", rquickjs::Method(Slot::create_element)),
        // )?;
        // proto.set(
        //     "createElementNS",
        //     rquickjs::Func::new(
        //         "createElementNS",
        //         rquickjs::Method(Slot::create_element_ns),
        //     ),
        // )?;
        // proto.set(
        //     "dispatchEvent",
        //     rquickjs::Func::new("dispatchEvent", rquickjs::Method(Slot::dispatch_event)),
        // )?;
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

impl<'js> rquickjs::IntoJs<'js> for SlotNode<'js> {
    fn into_js(self, ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
        <SlotNode as rquickjs::ClassDef>::into_js_obj(self, ctx)
    }
}

impl<'js, 'a> rquickjs::FromJs<'js> for &'a SlotNode<'js> {
    fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
        <SlotNode as rquickjs::ClassDef>::from_js_ref(ctx, value)
    }
}

impl<'js, 'a> rquickjs::FromJs<'js> for &'a mut SlotNode<'js> {
    fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
        <SlotNode as rquickjs::ClassDef>::from_js_mut(ctx, value)
    }
}
