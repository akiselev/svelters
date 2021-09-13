use rquickjs::{Func, Function};

use crate::{build_class, inner::*};

pub struct InitElementProto<'a, 'b, 'js, BASE> {
    pub(in crate) depth: isize,
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) obj: &'a Object<'b>,
    pub(in crate) framework: PhantomData<BASE>,
}

impl<'a, 'b, 'js, BASE> InitElementProto<'a, 'b, 'js, BASE> {
    pub fn new(ctx: Ctx<'js>, obj: &'a Object<'b>, depth: isize) -> Self {
        Self {
            ctx,
            obj,
            depth,
            framework: PhantomData,
        }
    }
}

impl<'a, 'b, 'c, CLS> Implements<InitElementProto<'a, 'b, 'c, CLS>> for JsClassLoader<CLS>
where
    CLS: Type + Clone,
{
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
                        let this: &ElementNode<'b> = value.as_ref();
                        let rt = this.rt.metadata.deref().borrow();
                        let obj = Obj::<CLS>::from_rt(&this.rt, this.idx)?;
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
                            (caster.obj_to_virt)(
                                ctx,
                                value.as_object().clone(),
                                this.rt.clone(),
                                this.idx,
                            )
                        }
                    },
                ),
            ),
        );
        phase.obj.set(
            "asImpl",
            Func::new(
                "asImpl",
                rquickjs::Method(
                    move |value: JsClass<'b, ElementNode<'b>>, ctx: Ctx<'b>, target: String| {
                        let this: &ElementNode<'b> = value.as_ref();
                        let rt = this.rt.metadata.deref().borrow();
                        let obj = Obj::<CLS>::from_rt(&this.rt, this.idx)?;
                        if CLS::CLASS_NAME == target {
                            Impl::<CLS>::new((*obj).clone()).into_js(ctx)
                        } else {
                            let obj = rt.types.get(Class::<CLS>::typekey()).unwrap();
                            let caster = obj.casters.get(target.as_str()).ok_or_else(|| {
                                exception!(
                                    "Could not get interface {} from {}",
                                    target,
                                    CLS::CLASS_NAME
                                )
                            })?;
                            (caster.obj_to_impl)(
                                ctx,
                                value.as_object().clone(),
                                this.rt.clone(),
                                this.idx,
                            )
                        }
                    },
                ),
            ),
        );
        Ok(phase)
    }
}
