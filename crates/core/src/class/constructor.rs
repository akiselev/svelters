use crate::inner::*;

pub trait Constructor<FW, CLS, A, R>
where
    FW: Framework + FrameworkClass<CLS> + Container<CLS>,
{
    const NUM_ARGS: usize;

    fn try_constructor<'js>(
        &'static self,
        ctx: Ctx<'js>,
        args: &Vec<Value<'js>>,
    ) -> Result<Obj<FW, CLS>>;
}

// impl<T, FW, CLS, R> Constructor<FW, CLS, (), R> for T
// where
//     FW: Framework + FrameworkClass<CLS> + Container<CLS>,
//     ClsBox<FW, CLS>: for<'a> GetRef<'a, CLS>,
//     T: Fn() -> R,
//     R: IntoContainer<FW, CLS>,
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

// macro_rules! tup {
//     ($($($type:ident)*,)*) => {
//         $(
//             impl<T, FW, CLS, R, $($type,)*> Constructor<FW, CLS, ($($type,)*), R> for T
//             where
//                 FW: Framework + FrameworkClass<CLS> + Container<CLS>,
//                 T:  Fn($($type,)*) -> R,
//                 $($type: for<'x> FromJs<'x>,)*
//                 ClsBox<FW, CLS>: for<'a> GetRef<'a, CLS>,
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

// fn tuple_match_size(actual: usize, expected: usize) -> Result<()> {
//     if actual == expected {
//         Ok(())
//     } else {
//         Err(Error::new_from_js_message(
//             "array",
//             "tuple",
//             if actual < expected {
//                 "Not enough values"
//             } else {
//                 "Too many values"
//             },
//         ))
//     }
// }
