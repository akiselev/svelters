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
    FW: Framework + FrameworkClass<CLS> + Container<CLS>,
    CLS: 'static,
{
    fn build(self, input: &InitStatic<'a, 'b, FW, CLS>) -> Result<()> {
        let cls: JsClass<FW, CLS, _> = JsClass::new(input);
        <FW as FrameworkClass<CLS>>::define(cls)?;
        Ok(())
    }
}
