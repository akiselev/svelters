use crate::{build_phase, inner::*};

pub struct Obj<FW, CLS>
where
    FW: Framework,
    CLS: Repr<FW> + Type<FW>,
{
    pub(in crate) fw: PhantomData<FW>,
    pub(in crate) inner: ReprOf<FW, CLS>,
}

impl<'a, FW, CLS> Obj<FW, CLS>
where
    FW: Framework,
    CLS: Type<FW>,
{
    pub fn new(inner: ReprOf<FW, CLS>) -> Self {
        Self {
            fw: PhantomData,
            inner,
        }
    }

    pub fn as_inner<'b>(&'a self) -> &'b ReprOf<FW, CLS>
    where
        'a: 'b,
    {
        &self.inner
    }
}

impl<'a, FW, CLS: 'static> ClassDef for Obj<FW, CLS>
where
    FW: Framework,
    CLS: Type<FW>,
{
    const CLASS_NAME: &'static str = std::any::type_name::<CLS>();

    unsafe fn class_id() -> &'static mut ClassId {
        <CLS as Type<FW>>::get_id()
    }

    const HAS_PROTO: bool = true;

    const HAS_STATIC: bool = true;

    const HAS_REFS: bool = true;

    fn init_proto<'js>(ctx: Ctx<'js>, proto: &Object<'js>) -> Result<()> {
        build_phase!(FW::CLS with InitProto::<FW, CLS>::new(ctx, proto));
        Ok(())
    }

    fn init_static<'js>(ctx: Ctx<'js>, _static: &Object<'js>) -> Result<()> {
        build_phase!(FW::CLS with InitStatic::<FW, CLS>::new(ctx, _static));
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
