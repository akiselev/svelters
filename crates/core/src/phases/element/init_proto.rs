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

impl<'a, 'b, 'c, CLS> Implements<InitElementProto<'a, 'b, 'c, CLS>> for JsClassLoader<CLS>
where
    CLS: Type + Clone,
{
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
                    move |value: JsClass<'b, ElementNode<'b>>, ctx: Ctx<'b>, target: String| {
                        // TODO: FIX CASTER
                        // let inner = this.borrow();
                        // let func: Function = this.get("castAs")?;

                        // let res: Value = Function::call(&func, (This(this), target))?;
                        // Result::<_>::Ok(res)
                        // Result::<_>::Ok(Value::new_undefined(ctx))
                        let this: &ElementNode<'b> = value.as_ref();
                        let rt = this.rt.inner.deref().borrow();
                        let obj = Obj::<CLS>::from_rt(&this.rt, this.idx)
                            .ok_or_else(|| exception!("Could not find Obj<{}>", CLS::CLASS_NAME))?;
                        if CLS::CLASS_NAME == target {
                            Virt::<CLS>::new(
                                this.rt.clone(),
                                this.idx,
                                (*obj).clone(),
                                value.as_object().clone(),
                            )
                            .into_js(ctx)
                        } else {
                            let obj = rt.types.get(Class::<CLS>::typekey()).unwrap();
                            let caster = obj.casters.get(target.as_str()).ok_or_else(|| {
                                exception!("Could not cast {} to {}", CLS::CLASS_NAME, target)
                            })?;
                            caster(ctx, value.as_object().clone(), this.rt.clone(), this.idx)
                        }
                    },
                ),
            ),
        );
        Ok(phase)
    }
}
