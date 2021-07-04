use crate::inner::*;

pub struct Obj<FW, const CLS: &'static str>
where
    FW: FrameworkClass<CLS> + Container<CLS> + Framework,
{
    fw: PhantomData<FW>,
    inner: ClsBox<FW, CLS>,
}

impl<'a, FW, const CLS: &'static str> Obj<FW, CLS>
where
    FW: FrameworkClass<CLS> + Container<CLS>,
    ClsBox<FW, CLS>: GetRef<'a, ClsTy<FW, CLS>>,
    ClsTy<FW, CLS>: 'static,
{
    pub fn new(inner: ClsBox<FW, CLS>) -> Self {
        Self {
            fw: PhantomData,
            inner,
        }
    }

    pub fn as_box<'b>(&'a self) -> &'b ClsBox<FW, CLS>
    where
        'a: 'b,
    {
        &self.inner
    }

    pub fn as_inner<'b>(&'a self) -> &'b ClsTy<FW, CLS>
    where
        'a: 'b,
    {
        self.inner.get_ref()
    }
}

impl<'a, FW, const CLS: &'static str> ClassDef for Obj<FW, CLS>
where
    FW: Framework + Container<CLS> + FrameworkClass<CLS>,
{
    const CLASS_NAME: &'static str = CLS;

    unsafe fn class_id() -> &'static mut ClassId {
        <FW as FrameworkClass<CLS>>::get_id()
    }

    const HAS_PROTO: bool = true;

    const HAS_STATIC: bool = true;

    const HAS_REFS: bool = true;

    fn init_proto<'js>(ctx: Ctx<'js>, proto: &Object<'js>) -> Result<()> {
        println!(
            "Initializing {}::{} proto",
            std::any::type_name::<FW>(),
            CLS
        );
        let phase = InitProto::<'_, '_, FW, CLS>::new(ctx, proto);
        let cls = JsClass::new(phase);
        <FW as FrameworkClass<CLS>>::define(cls)?;
        Ok(())
    }

    fn init_static<'js>(ctx: Ctx<'js>, _static: &Object<'js>) -> Result<()> {
        println!(
            "Initializing {}::{} static",
            std::any::type_name::<FW>(),
            CLS
        );
        let phase = InitStatic::<'_, '_, FW, CLS>::new(ctx, _static);
        let cls = JsClass::new(phase);
        <FW as FrameworkClass<CLS>>::define(cls)?;
        Ok(())
    }

    fn mark_refs(&self, _marker: &RefsMarker) {}

    fn into_js_obj<'js>(self, ctx: Ctx<'js>) -> Result<Value<'js>>
    where
        Self: Sized,
    {
        rquickjs::Class::<Self>::instance(ctx, self).map(|cls| cls.into_value())
    }

    fn from_js_ref<'js>(ctx: Ctx<'js>, value: Value<'js>) -> Result<&'js Self>
    where
        Self: Sized,
    {
        let value = Object::from_js(ctx, value)?;
        rquickjs::Class::<Self>::try_ref(ctx, &value)
    }

    fn from_js_mut<'js>(ctx: Ctx<'js>, value: Value<'js>) -> Result<&'js mut Self>
    where
        Self: Sized,
    {
        let value = Object::from_js(ctx, value)?;
        rquickjs::Class::<Self>::try_mut(ctx, &value)
    }

    fn from_js_obj<'js>(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self>
    where
        Self: Clone + Sized,
    {
        let value = Object::from_js(ctx, value)?;
        let instance = rquickjs::Class::<Self>::try_ref(ctx, &value)?;
        Ok(instance.clone())
    }
}
