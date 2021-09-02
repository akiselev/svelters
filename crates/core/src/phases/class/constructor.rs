use crate::inner::*;

pub struct ConstructorEval<'a, 'js, CLS> {
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) module: &'a QJsModule<'js, Loaded<Native>>,
    pub(in crate) framework: PhantomData<CLS>,
}

impl<'a, 'js, CLS> ConstructorEval<'a, 'js, CLS> {
    pub fn new(ctx: Ctx<'js>, module: &'a QJsModule<'js, Loaded<Native>>) -> Self {
        Self {
            ctx,
            module,
            framework: PhantomData,
        }
    }
}

pub struct ConstructorLoad<'a, 'js, CLS> {
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) module: &'a QJsModule<'js, Created>,
    pub(in crate) framework: PhantomData<CLS>,
}

impl<'a, 'js, CLS> ConstructorLoad<'a, 'js, CLS> {
    pub fn new(ctx: Ctx<'js>, module: &'a QJsModule<'js, Created>) -> Self {
        Self {
            ctx,
            module,
            framework: PhantomData,
        }
    }
}

macro_rules! tup {
    ($($($arg:ident)*,)*) => {
        $(
            impl<'a, 'b: 'a, T, CLS, $($arg,)*> Implements<ConstructorEval<'a, 'b, CLS>> for Constructor<CLS, ($($arg,)*), T>
            where
                CLS: Type,
                T: 'static + Fn($($arg,)*) -> Result<CLS>,
                $($arg: 'static + for<'x> FromJs<'x>,)*
            {
                #[inline(always)]
                fn build(self, phase: &mut ConstructorEval<'a, 'b, CLS>) -> Result<()> {
                    JsClass::<Obj<CLS>>::register(phase.ctx)?;
                    JsClass::<Virt<CLS>>::register(phase.ctx)?;
                    phase.module.set(
                        <CLS as Type>::CLASS_NAME,
                        rquickjs::Func::new(
                            <CLS as Type>::CLASS_NAME,
                            JsClass::<Obj<CLS>>::constructor(move |ctx: Ctx<'b>, args: Rest<Value<'b>>| {
                                let args = args.into_inner();
                                let args_len = args.len();
                                tuple_match_size(args_len, {0 $(+ tup!(@one $arg))*})?;

                                let result = self.0(
                                    $({ let arg: $arg = FromJs::from_js(ctx, args.get(tup!(@idx $arg)).cloned().unwrap())?; arg },)*
                                )?;
                                JsClass::instance(ctx, Obj::<CLS>::new(result))
                            }),
                        ),
                    )?;
                    Ok(())
                }
            }

            impl<'a, 'b: 'a, T, CLS, R, $($arg,)*> Implements<ConstructorLoad<'a, 'b, CLS>> for Constructor<CLS, ($($arg,)*), T>
            where
                CLS: Type,
                T: Fn($($arg,)*) -> R,
            {
                #[inline(always)]
                fn build(self, phase: &mut ConstructorLoad<'a, 'b, CLS>) -> Result<()> {
                    phase.module.add(<CLS as Type>::name())?;
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
