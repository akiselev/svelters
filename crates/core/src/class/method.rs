use rquickjs::{Class, Rest};

use crate::inner::*;

pub struct ClassMethod<FW, CLS, TG, A, R, T>(&'static str, T, PhantomData<(CLS, TG, A, R, FW)>);

impl<FW, T, A, R, CLS> ClassMethod<FW, CLS, CLS, A, R, T> {
    pub(in crate) fn new(name: &'static str, func: T) -> Self {
        ClassMethod(name, func, PhantomData)
    }
}

impl<'a, 'b: 'a, X, T, FW, CLS, R> Implements<InitProto<'a, 'b, FW, CLS>>
    for ClassMethod<FW, CLS, CLS, (T,), R, X>
where
    FW: Framework + FrameworkClass<CLS> + Container<CLS> + Container<T>,
    X: 'static + for<'x> Fn(&'x T) -> R,
    ClsBox<FW, CLS>: Cast<T>,
    R: 'static + for<'js> IntoJs<'js>,
    <FW as Container<CLS>>::Type: for<'x> GetRef<'x, CLS>,
    CLS: 'static,
    T: 'static,
{
    fn build(self, input: &InitProto<'a, 'b, FW, CLS>) -> Result<()> {
        println!(
            "Implementing {}::{}",
            <FW as FrameworkClass<CLS>>::get_name(),
            self.0
        );
        input.obj.set(
            self.0,
            rquickjs::Func::new(
                self.0,
                rquickjs::Method(
                    move |this: Class<'b, Obj<FW, CLS>>, ctx: Ctx<'b>, args: Rest<Value<'b>>| {
                        let args = args.into_inner();
                        let args_len = args.len();
                        tuple_match_size(args_len, 0)?;
                        let obj: &Obj<FW, CLS> = this.as_ref();
                        let data = obj.inner.cast();
                        let res = self.1(&*data);
                        Result::<_>::Ok(res.into_js(ctx))
                    },
                ),
            ),
        )?;
        Ok(())
    }
}

impl<'a, 'b: 'a, T, FW, CLS, R, SUPER> Implements<InitSuperProto<'a, 'b, FW, CLS, SUPER>>
    for ClassMethod<FW, SUPER, SUPER, (SUPER,), R, T>
where
    FW: Framework + FrameworkClass<CLS> + Container<CLS> + Container<SUPER> + FrameworkClass<SUPER>,
    ClsBox<FW, CLS>: for<'c> GetRef<'c, CLS>,
    ClsBox<FW, CLS>: Cast<SUPER>,
    SUPER: 'static,
    T: 'static + Fn(&SUPER) -> R,
    R: 'static + for<'js> IntoJs<'js>,
    CLS: 'static,
{
    fn build(self, input: &InitSuperProto<'a, 'b, FW, CLS, SUPER>) -> Result<()> {
        println!(
            "Implementing super {}::{} on {}",
            <FW as FrameworkClass<SUPER>>::get_name(),
            self.0,
            <FW as FrameworkClass<CLS>>::get_name(),
        );
        input.obj.set(
            self.0,
            rquickjs::Func::new(
                self.0,
                rquickjs::Method(
                    move |this: Class<'b, Obj<FW, CLS>>, ctx: Ctx<'b>, args: Rest<Value<'b>>| {
                        println!(
                            "CALLING SUPER METHOD {}::{} ON {}",
                            <FW as FrameworkClass<SUPER>>::get_name(),
                            self.0,
                            <FW as FrameworkClass<CLS>>::get_name(),
                        );
                        let args = args.into_inner();
                        let args_len = args.len();
                        tuple_match_size(args_len, 0)?;
                        let obj: &Obj<FW, CLS> = this.as_ref();
                        let res = self.1(&*obj.as_box().cast());
                        Result::<_>::Ok(res.into_js(ctx))
                    },
                ),
            ),
        )?;
        Ok(())
    }
}
