use crate::inner::*;

impl<'a, 'b, 'c, CLS, SLOT> Implements<InitElementProto<'a, 'b, 'c, CLS>>
    for members::NativeSlot<CLS, SLOT>
where
    CLS: Type,
    SLOT: NativeSlot,
{
    #[inline(always)]
    fn build(self, phase: &mut InitElementProto<'a, 'b, 'c, CLS>) -> Result<()> {
        let members::ReadOnly { name, getter, .. } = self;

        let res = phase.obj.set(
            name,
            rquickjs::Function::new(rquickjs::Method(
                move |this: &mut Element<'b>, ctx: Ctx<'b>| {
                    let obj = Obj::<CLS>::from_js_ref(ctx, this.obj.clone())?;
                    obj.inner.with_cast(|data| {
                        let val = getter(data)?;
                        rquickjs::Result::Ok(val)
                    })
                },
            ))
            .enumerable(),
        );
        res
    }
}
