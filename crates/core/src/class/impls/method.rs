use rquickjs::{Class, Rest};

use crate::inner::*;

pub struct ClassMethod<FW, CLS, A, R, T>(&'static str, T, PhantomData<(CLS, A, R, FW)>)
where
    FW: Container<CLS>,
    T: Method<FW, CLS, A, R>;

unsafe impl<FW, T, A, R, CLS> Sync for ClassMethod<FW, CLS, A, R, T>
where
    FW: Container<CLS>,
    T: Method<FW, CLS, A, R>,
{
}

impl<FW, T, A, R, CLS> ClassMethod<FW, CLS, A, R, T>
where
    FW: Container<CLS>,
    T: Method<FW, CLS, A, R>,
{
    pub(in crate) fn new(name: &'static str, func: T) -> Self {
        ClassMethod(name, func, PhantomData)
    }
}

impl<FW, T, A, R, CLS> TypeDef for ClassMethod<FW, CLS, A, R, T>
where
    FW: Container<CLS>,
    T: 'static + Method<FW, CLS, A, R>,
    R: 'static + for<'js> IntoJs<'js>,
    A: 'static,
    <T as Method<FW, CLS, A, R>>::Args: for<'a> FromJs<'a>,
{
    fn init_proto<'js>(&'static self, _ctx: Ctx<'js>, exports: &Object<'js>) -> Result<()> {
        println!("Initializing {}::{}", std::any::type_name::<CLS>(), self.0);
        exports.set(
            self.0,
            rquickjs::Func::new(
                self.0,
                rquickjs::Method(
                    move |this: Class<'js, Obj<FW, CLS>>, ctx: Ctx<'js>, args: Rest<Value<'js>>| {
                        let args = args.into_inner();
                        let res = self.1.try_call(&this, ctx, &args)?;
                        Result::<_>::Ok(res.into_js(ctx))
                    },
                ),
            ),
        )?;
        Ok(())
    }
}
