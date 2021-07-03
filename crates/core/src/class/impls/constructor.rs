use rquickjs::{Class, Rest};

use crate::inner::*;

pub struct ClassConstructor<FW, const CLS: &'static str, F>(F, PhantomData<FW>)
where
    FW: Container<CLS>;

unsafe impl<FW, F, const CLS: &'static str> Sync for ClassConstructor<FW, CLS, F> where
    FW: Container<CLS>
{
}

impl<FW, F, const CLS: &'static str> ClassConstructor<FW, CLS, F>
where
    FW: Container<CLS>,
{
    pub(in crate) const fn new(func: F) -> Self {
        ClassConstructor(func, PhantomData)
    }
}

impl<FW, F, R, const CLS: &'static str> JsModuleDef for ClassConstructor<FW, CLS, F>
where
    FW: Container<CLS>,
    ClsBox<FW, CLS>: for<'a> GetRef<'a, ClsTy<FW, CLS>>,
    F: 'static + Fn() -> R,
    R: IntoContainer<FW, CLS, Type = ClsBox<FW, CLS>>,
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
                        let args_len = args.len();
                        tuple_match_size(args_len, 0)?;
                        let result = self.0();
                        let wrapped: Obj<FW, CLS> = Obj::new(result.into_framework_type());
                        Result::Ok(Class::instance(ctx, wrapped))
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

fn tuple_match_size(actual: usize, expected: usize) -> Result<()> {
    if actual == expected {
        Ok(())
    } else {
        Err(Error::new_from_js_message(
            "array",
            "tuple",
            if actual < expected {
                "Not enough values"
            } else {
                "Too many values"
            },
        ))
    }
}
