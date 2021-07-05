use crate::inner::*;

pub struct ConstructorEval<'a, 'js, FW, CLS> {
    pub(in crate) ctx: Ctx<'js>,
    pub(in crate) module: &'a Module<'js, Loaded<Native>>,
    pub(in crate) framework: PhantomData<(FW, CLS)>,
}

impl<'a, 'js, FW: Wraps<CLS>, CLS> ConstructorEval<'a, 'js, FW, CLS> {
    pub fn new(ctx: Ctx<'js>, module: &'a Module<'js, Loaded<Native>>) -> Self {
        Self {
            ctx,
            module,
            framework: PhantomData,
        }
    }
}

impl<'a, 'b: 'a, T, FW, CLS, R> Implements<ConstructorEval<'a, 'b, FW, CLS>>
    for ClassConstructor<FW, CLS, (), R, T>
where
    FW: Framework + TypeDef<CLS> + Wraps<CLS>,
    Wrapped<FW, CLS>: for<'x> GetRef<'x, CLS>,
    T: Fn() -> R,
    CLS: 'static,
    T: 'static,
    R: 'static + IntoContainer<FW, CLS, Type = Wrapped<FW, CLS>>,
{
    fn build(self, input: &ConstructorEval<'a, 'b, FW, CLS>) -> Result<()> {
        println!(
            "-- Setting {}::{} in ConstructorEval",
            std::any::type_name::<FW>(),
            <FW as TypeDef<CLS>>::CLASS_NAME
        );
        JsClass::<Obj<FW, CLS>>::register(input.ctx)?;
        input.module.set(
            <FW as TypeDef<CLS>>::CLASS_NAME,
            rquickjs::Func::new(
                <FW as TypeDef<CLS>>::CLASS_NAME,
                JsClass::<Obj<FW, CLS>>::constructor(move |ctx: Ctx<'b>, args: Rest<Value<'b>>| {
                    let args = args.into_inner();
                    let args_len = args.len();
                    tuple_match_size(args_len, 0)?;

                    let result = self.0();
                    println!(
                        "Calling constructor for {}",
                        <FW as TypeDef<CLS>>::CLASS_NAME
                    );
                    JsClass::instance(ctx, Obj::<FW, CLS>::new(result.into_framework_type()))
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
