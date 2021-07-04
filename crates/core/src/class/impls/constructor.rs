use rquickjs::{Class, Rest};

use crate::inner::*;

pub struct ClassConstructor<FW, CLS, A, R, T>(
    pub(crate) T,
    pub(crate) PhantomData<(CLS, A, R, FW)>,
)
where
    FW: Framework + FrameworkClass<CLS> + Container<CLS>;

// unsafe impl<FW, T, A, CLS> Sync for ClassConstructor<FW, CLS, A, T>
// where
//     FW: Framework + FrameworkClass<CLS> + Container<CLS>,
//     T: Constructor<FW, CLS, A, R>,
// {
// }

impl<FW, T, A, R, CLS> ClassConstructor<FW, CLS, A, R, T>
where
    FW: Framework + FrameworkClass<CLS> + Container<CLS>,
{
    pub(in crate) fn new(func: T) -> Self {
        ClassConstructor(func, PhantomData)
    }
}

impl<'a, 'b: 'a, T, FW, CLS, R> Implements<ModuleLoad<'a, 'b>>
    for ClassConstructor<FW, CLS, (), R, T>
where
    FW: Framework + FrameworkClass<CLS> + Container<CLS>,
    ClsBox<FW, CLS>: for<'x> GetRef<'x, CLS>,
    T: Fn() -> R,
{
    fn build(self, input: &ModuleLoad<'a, 'b>) -> Result<()> {
        println!(
            "Setting {}::{} in ModuleEval",
            std::any::type_name::<FW>(),
            std::any::type_name::<CLS>()
        );
        input.module.add(<FW as FrameworkClass<CLS>>::get_name())?;
        Ok(())
    }
}

// impl<T, FW, CLS, R> Constructor<FW, CLS, (), R> for T
// where
//     FW: Container<CLS>,
//     ClsBox<FW, CLS>: for<'a> GetRef<'a, CLS>,
//     T: Fn() -> R,
//     R: IntoContainer<FW, CLS, Type = ClsBox<FW, CLS>>,
// {
//     const NUM_ARGS: usize = 0;

//     fn try_constructor<'js>(
//         &'static self,
//         _ctx: Ctx<'js>,
//         args: &Vec<Value<'js>>,
//     ) -> Result<Obj<FW, CLS>> {
//         let args_len = args.len();
//         tuple_match_size(args_len, 0)?;

//         let result = (*self)();
//         Ok(Obj::new(result.into_framework_type()))
//     }
// }

impl<'a, 'b: 'a, T, FW, CLS, R> Implements<ModuleEval<'a, 'b>>
    for ClassConstructor<FW, CLS, (), R, T>
where
    FW: Framework + FrameworkClass<CLS> + Container<CLS>,
    ClsBox<FW, CLS>: for<'x> GetRef<'x, CLS>,
    T: Fn() -> R,
    CLS: 'static,
    T: 'static,
    R: 'static + IntoContainer<FW, CLS, Type = ClsBox<FW, CLS>>,
{
    fn build(self, input: &ModuleEval<'a, 'b>) -> Result<()> {
        println!(
            "Setting {}::{} in ModuleEval",
            std::any::type_name::<FW>(),
            <FW as FrameworkClass<CLS>>::CLASS_NAME
        );
        rquickjs::Class::<Obj<FW, CLS>>::register(input.ctx)?;
        input.module.set(
            <FW as FrameworkClass<CLS>>::CLASS_NAME,
            rquickjs::Func::new(
                <FW as FrameworkClass<CLS>>::CLASS_NAME,
                rquickjs::Class::<Obj<FW, CLS>>::constructor(
                    move |ctx: Ctx<'b>, args: Rest<Value<'b>>| {
                        let args = args.into_inner();
                        let args_len = args.len();
                        tuple_match_size(args_len, 0)?;

                        let result = self.0();
                        println!(
                            "Calling constructor for {}",
                            <FW as FrameworkClass<CLS>>::CLASS_NAME
                        );
                        Class::instance(ctx, Obj::<FW, CLS>::new(result.into_framework_type()))
                    },
                ),
            ),
        )?;
        Ok(())
    }
}

// macro_rules! tup {
//     ($($($type:ident)*,)*) => {
//         $(
//             impl<'a, 'b: 'a, T, FW, CLS, R, $($type,)*> Implements<ModuleLoad<'a, 'b>> for ClassConstructor<FW, CLS, ($($type,)*), R, T>
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
