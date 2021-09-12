use crate::{
    inner::*,
    members::{AsMethod, Member},
};

impl<'js, BASE, CLS, FUNC, ARGS, RET> AsFunction<'js, ARGS, RET>
    for ObjImpl<BASE, Member<CLS, AsMethod<FUNC>>>
where
    BASE: Type + Upcast<CLS>,
    RET: for<'js_> ToResult<'js_>,
    ARGS: for<'js_> FromImpl<'js, BASE>,
    FUNC: 'static + Fn<ARGS, Output = RET>,
{
    fn num_args() -> std::ops::Range<usize> {
        ARGS::num_args()
    }

    fn call(&self, input: &rquickjs::Input<'js>) -> Result<Value<'js>> {
        input.check_num_args::<Self, _, _>()?;
        let mut accessor = input.access();
        let result = self.member.func.call(ARGS::from_obj(&mut accessor)?);
        result.to_result(accessor.ctx())
    }
}

impl<'js, BASE, CLS, FUNC, ARGS, RET> AsFunction<'js, ARGS, RET>
    for VirtImpl<BASE, Member<CLS, AsMethod<FUNC>>>
where
    BASE: Type + Upcast<CLS>,
    RET: for<'js_> ToResult<'js_>,
    ARGS: for<'js_> FromImpl<'js, BASE>,
    FUNC: 'static + Fn<ARGS, Output = RET>,
{
    fn num_args() -> std::ops::Range<usize> {
        ARGS::num_args()
    }

    fn call(&self, input: &rquickjs::Input<'js>) -> Result<Value<'js>> {
        input.check_num_args::<Self, _, _>()?;
        let mut accessor = input.access();
        let result = self.member.func.call(ARGS::from_virt(&mut accessor)?);
        result.to_result(accessor.ctx())
    }
}

impl<'js, BASE, CLS, FUNC, ARGS, RET> AsFunction<'js, ARGS, RET>
    for ElementImpl<BASE, Member<CLS, AsMethod<FUNC>>>
where
    BASE: Type + Upcast<CLS>,
    RET: for<'js_> ToResult<'js_>,
    ARGS: for<'js_> FromImpl<'js, BASE>,
    FUNC: 'static + Fn<ARGS, Output = RET>,
{
    fn num_args() -> std::ops::Range<usize> {
        ARGS::num_args()
    }

    fn call(&self, input: &rquickjs::Input<'js>) -> Result<Value<'js>> {
        input.check_num_args::<Self, _, _>()?;
        let mut accessor = input.access();
        let result = self.member.func.call(ARGS::from_element(&mut accessor)?);
        result.to_result(accessor.ctx())
    }
}
