use crate::inner::*;

pub struct ConstructorEval<'a, 'js, FW, CLS> {
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) module: &'a QJsModule<'js, Loaded<Native>>,
    pub(in crate) framework: PhantomData<(FW, CLS)>,
}

impl<'a, 'js, CLS: Repr<FW>, FW> ConstructorEval<'a, 'js, FW, CLS> {
    pub fn new(ctx: Ctx<'js>, module: &'a QJsModule<'js, Loaded<Native>>) -> Self {
        Self {
            ctx,
            module,
            framework: PhantomData,
        }
    }
}

impl<'a, 'b: 'a, T, FW, CLS, R> Implements<ConstructorEval<'a, 'b, FW, CLS>>
    for Constructor<FW, CLS, (), R, T>
where
    FW: Framework,
    CLS: Type<FW>,
    T: 'static + Fn() -> R,
    R: 'static + IntoRepr<FW, CLS>,
{
    fn build(self, phase: &mut ConstructorEval<'a, 'b, FW, CLS>) -> Result<()> {
        JsClass::<Obj<FW, CLS>>::register(phase.ctx)?;
        phase.module.set(
            <CLS as Type<FW>>::CLASS_NAME,
            rquickjs::Func::new(
                <CLS as Type<FW>>::CLASS_NAME,
                JsClass::<Obj<FW, CLS>>::constructor(move |ctx: Ctx<'b>, args: Rest<Value<'b>>| {
                    let args = args.into_inner();
                    let args_len = args.len();
                    tuple_match_size(args_len, 0)?;

                    let result = self.0();
                    JsClass::instance(ctx, Obj::<FW, CLS>::new(result.into_repr()))
                }),
            ),
        )?;
        Ok(())
    }
}

// macro_rules! tup {
//     ($($($type:ident)*,)*) => {
//         $(
//             impl<'a, 'b: 'a, T, FW, CLS, R, $($type,)*> Implements<ConstructorLoad<'a, 'b>> for ClassConstructor<FW, CLS, ($($type,)*), R, T>
//             where
//                 FW: Framework + FrameworkClass<CLS> + Container<CLS>,
//                 T:  Fn($($type,)*) -> R,
//                 $($type: for<'x> FromJs<'x>,)*
//                 ClsBox<FW, CLS>: for<'x> GetRef<'x, CLS>,
//                 R: IntoContainer<FW, CLS>,
//                 Obj<FW, CLS>: ClassDef,
//                 CLS: 'static
//             {
//                 const NUM_ARGS: usize = 0 $(+ tup!(@one $type))*;

//                 fn try_constructor<'js>(&'static self, ctx: Ctx<'js>, args: &Vec<Value<'js>>) -> Result<Obj<FW, CLS>>
//                 {
//                     let args_len = args.len();
//                     tuple_match_size(args_len, Self::NUM_ARGS)?;
//                     let result = (*self)(
//                         $({ let arg: $type = FromJs::from_js(ctx, args.get(tup!(@idx $type)).cloned().unwrap())?; arg },)*
//                     );
//                     Ok(Obj::new(result.into_framework_type()))
//                 }
//             }
//         )*
//     };
//     (@one $($t:tt)*) => { 1 };
//     (@idx A) => { 0 };
//     (@idx B) => { 1 };
//     (@idx C) => { 2 };
//     (@idx D) => { 3 };
//     (@idx E) => { 4 };
//     (@idx F) => { 5 };
//     (@idx G) => { 6 };
//     (@idx H) => { 7 };
//     (@idx I) => { 8 };
//     (@idx J) => { 9 };
//     (@idx K) => { 10 };
//     (@idx L) => { 11 };
//     (@idx M) => { 12 };
//     (@idx N) => { 13 };
//     (@idx O) => { 14 };
//     (@idx P) => { 15 };
// }

// tup! {
//     A,
//     A B,
//     A B C,
//     A B C D,
//     A B C D E,
//     A B C D E F,
//     A B C D E F G,
//     A B C D E F G H,
//     A B C D E F G H I,
//     A B C D E F G H I J,
//     A B C D E F G H I J K,
//     A B C D E F G H I J K L,
//     A B C D E F G H I J K L M,
//     A B C D E F G H I J K L M N,
//     A B C D E F G H I J K L M N O,
//     A B C D E F G H I J K L M N O P,
// }
