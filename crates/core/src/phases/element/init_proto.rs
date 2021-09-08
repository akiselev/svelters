use rquickjs::{Func, Function};

use crate::{build_class, inner::*};

pub struct InitElementProto<'a, 'b, 'js, BASE> {
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) obj: &'a Object<'b>,
    pub(in crate) framework: PhantomData<BASE>,
}

impl<'a, 'b, 'js, BASE> InitElementProto<'a, 'b, 'js, BASE> {
    pub fn new(ctx: Ctx<'js>, obj: &'a Object<'b>) -> Self {
        Self {
            ctx,
            obj,
            framework: PhantomData,
        }
    }
}

impl<'a, 'b, 'c, CLS> Implements<InitElementProto<'a, 'b, 'c, CLS>> for JsClassLoader<CLS> {
    #[inline(always)]
    fn build_phase(
        self,
        phase: InitElementProto<'a, 'b, 'c, CLS>,
    ) -> Result<InitElementProto<'a, 'b, 'c, CLS>> {
        phase.obj.set(
            "castAs",
            Func::new(
                "castAs",
                rquickjs::Method(
                    move |this: ElementNode<'b>, _ctx: Ctx<'b>, target: rquickjs::String<'b>| {
                        let inner = this.borrow();
                        let func: Function = inner.obj.get("castAs")?;

                        let res: Value = Function::call(&func, (This(inner.obj.clone()), target))?;
                        Result::<_>::Ok(res)
                    },
                ),
            ),
        );
        Ok(phase)
    }
}
