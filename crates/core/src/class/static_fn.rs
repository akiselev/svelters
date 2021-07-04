use rquickjs::{Class, Rest};

use crate::inner::*;

pub struct ClassStaticFunc<FW, CLS, A, R, T>(&'static str, T, PhantomData<(CLS, A, R, FW)>)
where
    FW: Container<CLS>;

unsafe impl<FW, T, A, R, CLS> Sync for ClassStaticFunc<FW, CLS, A, R, T> where FW: Container<CLS> {}

impl<FW, T, A, R, CLS> ClassStaticFunc<FW, CLS, A, R, T>
where
    FW: Container<CLS>,
{
    pub(in crate) fn new(name: &'static str, func: T) -> Self {
        ClassStaticFunc(name, func, PhantomData)
    }
}

impl<'a, 'b: 'a, T, FW, CLS, R> Implements<InitStatic<'a, 'b, FW, CLS>>
    for ClassStaticFunc<FW, CLS, (), R, T>
where
    FW: Framework + FrameworkClass<CLS> + Container<CLS>,
    T: 'static + Fn() -> R,
    R: 'static + for<'js> IntoJs<'js>,
    CLS: 'static,
{
    fn build(self, input: &InitStatic<'a, 'b, FW, CLS>) -> Result<()> {
        println!(
            "Implementing static {}::{} ",
            <FW as FrameworkClass<CLS>>::get_name(),
            self.0,
        );
        input.obj.set(
            self.0,
            rquickjs::Func::new(self.0, move |ctx: Ctx<'b>, args: Rest<Value<'b>>| {
                println!(
                    "CALLING STATIC {}::{} ",
                    <FW as FrameworkClass<CLS>>::get_name(),
                    self.0,
                );
                let args = args.into_inner();
                let args_len = args.len();
                tuple_match_size(args_len, 0)?;

                let res = self.1();
                Result::<_>::Ok(res.into_js(ctx))
            }),
        )?;
        Ok(())
    }
}

impl<'a, 'b: 'a, T, FW, CLS, SUPER, R> Implements<InitSuperStatic<'a, 'b, FW, CLS, SUPER>>
    for ClassStaticFunc<FW, SUPER, (), R, T>
where
    FW: Framework + FrameworkClass<CLS> + Container<CLS> + FrameworkClass<SUPER> + Container<SUPER>,
    T: 'static + Fn() -> R,
    R: 'static + for<'js> IntoJs<'js>,
    CLS: 'static,
    SUPER: 'static,
{
    fn build(self, input: &InitSuperStatic<'a, 'b, FW, CLS, SUPER>) -> Result<()> {
        println!(
            "Implementing super static {}::{} on {}",
            <FW as FrameworkClass<SUPER>>::get_name(),
            self.0,
            <FW as FrameworkClass<CLS>>::get_name(),
        );
        input.obj.set(
            self.0,
            rquickjs::Func::new(self.0, move |ctx: Ctx<'b>, args: Rest<Value<'b>>| {
                println!(
                    "CALLING SUPER STATIC {}::{} ",
                    <FW as FrameworkClass<CLS>>::get_name(),
                    self.0,
                );
                let args = args.into_inner();
                let args_len = args.len();
                tuple_match_size(args_len, 0)?;

                let res = self.1();
                Result::<_>::Ok(res.into_js(ctx))
            }),
        )?;
        Ok(())
    }
}
