use crate::{build_class, inner::*};

pub struct InitStatic<'a, 'js, CLS> {
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) obj: &'a Object<'js>,
    pub(in crate) framework: PhantomData<CLS>,
}

impl<'a, 'js, CLS> InitStatic<'a, 'js, CLS> {
    pub fn new(ctx: Ctx<'js>, obj: &'a Object<'js>) -> Self {
        Self {
            ctx,
            obj,
            framework: PhantomData,
        }
    }
}

impl<'a, 'b: 'a, BASE, SUPER> Implements<InitStatic<'a, 'b, BASE>> for members::Superclass<SUPER>
where
    BASE: Caster<SUPER>,
    SUPER: Type,
{
    #[inline(always)]
    fn build_phase(self, phase: &mut InitStatic<'a, 'b, BASE>) -> Result<()> {
        build_class!(SUPER with InitStatic::<'_, '_, BASE>::new(phase.ctx, phase.obj));
        Ok(())
    }
}

macro_rules! tup {
    ($($($arg:ident)*,)*) => {
        $(
            impl<'a, 'b: 'a, T, BASE, CLS, R, $($arg,)*> Implements<InitStatic<'a, 'b, BASE>> for members::StaticFn<CLS, ($($arg,)*), R, T>
            where
                BASE: Caster<CLS>,
                CLS: Type,
                T: 'static + Fn($($arg,)*) -> Result<R>,
                R: 'static + for<'js> IntoJs<'js>,
                $($arg: 'static + for<'x> FromJs<'x>,)*
            {
                #[inline(always)]
                fn build_phase(self, phase: &mut InitStatic<'a, 'b, BASE>) -> Result<()> {
                    phase.obj.set(
                        self.0,
                        rquickjs::Func::new(self.0, move |ctx: Ctx<'b>, args: Rest<Value<'b>>| {
                            let args = args.into_inner();
                            let args_len = args.len();
                            tuple_match_size(args_len, {0 $(+ tup!(@one $arg))*})?;

                            let res = self.1(
                                $({ let arg: $arg = FromJs::from_js(ctx, args.get(tup!(@idx $arg)).cloned().unwrap())?; arg },)*
                            );
                            Result::<_>::Ok(res.into_js(ctx))
                        }),
                    )?;
                    Ok(())
                }
            }
        )*
    };
    (@one $($t:tt)*) => { 1 };
    (@idx A) => { 0 };
    (@idx B) => { 1 };
    (@idx C) => { 2 };
    (@idx D) => { 3 };
    (@idx E) => { 4 };
    (@idx F) => { 5 };
    (@idx G) => { 6 };
    (@idx H) => { 7 };
    (@idx I) => { 8 };
    (@idx J) => { 9 };
    (@idx K) => { 10 };
    (@idx L) => { 11 };
    (@idx M) => { 12 };
    (@idx N) => { 13 };
    (@idx O) => { 14 };
    (@idx P) => { 15 };
}

tup! {
    ,
    A,
    A B,
    A B C,
    A B C D,
    A B C D E,
    A B C D E F,
    A B C D E F G,
    A B C D E F G H,
    A B C D E F G H I,
    A B C D E F G H I J,
    A B C D E F G H I J K,
    A B C D E F G H I J K L,
    A B C D E F G H I J K L M,
    A B C D E F G H I J K L M N,
    A B C D E F G H I J K L M N O,
    A B C D E F G H I J K L M N O P,
}
