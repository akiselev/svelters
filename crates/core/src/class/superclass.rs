use rquickjs::{Class, Rest};

use crate::inner::*;

pub struct ClassSuper<FW, CLS, SUPER>(PhantomData<(CLS, SUPER, FW)>)
where
    FW: Container<CLS>;

impl<FW, SUPER, CLS> ClassSuper<FW, CLS, SUPER>
where
    FW: Container<CLS>,
{
    pub(in crate) fn new() -> Self {
        ClassSuper(PhantomData)
    }
}

impl<'a, 'b: 'a, FW, CLS, SUPER> Implements<InitStatic<'a, 'b, FW, CLS>>
    for ClassSuper<FW, CLS, SUPER>
where
    FW: Framework + FrameworkClass<CLS> + Container<CLS> + Container<SUPER> + FrameworkClass<SUPER>,
    CLS: 'static,
{
    fn build(self, input: &InitStatic<'a, 'b, FW, CLS>) -> Result<()> {
        println!(
            "Building SUPER STATIC on {}",
            <FW as FrameworkClass<CLS>>::get_name()
        );
        let phase = InitSuperStatic::<'_, '_, FW, CLS, SUPER>::new(input.ctx, input.obj);
        let cls: JsClass<FW, SUPER, _> = JsClass::new(phase);
        <FW as FrameworkClass<SUPER>>::define(cls)?;
        Ok(())
    }
}

impl<'a, 'b: 'a, FW, CLS, SUPER> Implements<InitProto<'a, 'b, FW, CLS>>
    for ClassSuper<FW, CLS, SUPER>
where
    FW: Framework + FrameworkClass<CLS> + Container<CLS> + Container<SUPER> + FrameworkClass<SUPER>,
    CLS: 'static,
{
    fn build(self, input: &InitProto<'a, 'b, FW, CLS>) -> Result<()> {
        println!(
            "Building SUPER PROTO on {}",
            <FW as FrameworkClass<CLS>>::get_name()
        );
        let phase = InitSuperProto::<'_, '_, FW, CLS, SUPER>::new(input.ctx, input.obj);
        let cls: JsClass<FW, SUPER, _> = JsClass::new(phase);
        <FW as FrameworkClass<SUPER>>::define(cls)?;
        Ok(())
    }
}
