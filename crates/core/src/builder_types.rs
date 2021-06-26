use rquickjs::{Class, Rest};

use crate::inner::*;

pub struct ClassConstructor<FW, const CLS: &'static str, A, R, T>(T, PhantomData<(A, R, FW)>)
where
    FW: Container<CLS>,
    T: Constructor<FW, CLS, A, R>;

impl<FW, T, A, R, const CLS: &'static str> ClassConstructor<FW, CLS, A, R, T>
where
    FW: Container<CLS>,
    T: Constructor<FW, CLS, A, R>,
{
    pub(in crate) fn new(func: T) -> Self {
        ClassConstructor(func, PhantomData)
    }
}

impl<FW, T, A, R, const CLS: &'static str> JsModuleDef for ClassConstructor<FW, CLS, A, R, T>
where
    FW: Container<CLS>,
    T: Constructor<FW, CLS, A, R>,
    <T as Constructor<FW, CLS, A, R>>::Args: for<'a> FromJs<'a>,
    Obj<FW, CLS>: ClassDef + 'static,
{
    fn load<'js>(ctx: Ctx<'js>, exports: &Module<'js, Created>) -> Result<()> {
        exports.add(CLS)?;
        Ok(())
    }

    fn eval<'js>(
        &'static self,
        ctx: Ctx<'js>,
        exports: &Module<'js, Loaded<Native>>,
    ) -> Result<()> {
        exports.set(
            CLS,
            rquickjs::Func::new(
                CLS,
                rquickjs::Class::<Obj<FW, CLS>>::constructor(
                    move |ctx: Ctx<'js>, args: Rest<Value<'js>>| {
                        let args = args.into_inner();
                        let res = self.0.try_constructor(ctx, &args)?;
                        Class::instance(ctx, res)
                    },
                ),
            ),
        )?;
        Ok(())
    }
}

// fn transform<'a: 'c, 'c, FW, T, A, R, const CLS: &'static str>(
//     cls: ClassConstructor<'a, FW, CLS, A, R, T>,
//     func: impl for<'b> FnOnce(
//         ClassConstructor<'b, FW, CLS, A, R, T>,
//     ) -> ClassConstructor<'b, FW, CLS, A, R, T>,
// ) -> ClassConstructor<'a, FW, CLS, A, R, T>
// where
//     FW: Container<CLS>,
//     T: Constructor<'a, FW, CLS, A, R>,
//     <T as Constructor<'a, FW, CLS, A, R>>::Args: FromJs<'a>,
//     Obj<FW, CLS>: 'static,
// {
//     func(cls)
// }
