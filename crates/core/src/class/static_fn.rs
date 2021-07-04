use crate::inner::*;

pub trait StaticFunc<FW, CLS, A, R>
where
    FW: Container<CLS>,
    Self::Args: for<'js> FromJs<'js>,
    Self::Return: for<'js> IntoJs<'js>,
{
    const NUM_ARGS: usize;
    type Args;
    type Return;

    fn try_call<'js>(&'static self, ctx: Ctx<'js>, args: &Vec<Value<'js>>) -> Result<Self::Return>;
}

impl<T, FW, CLS, R> StaticFunc<FW, CLS, (), R> for T
where
    FW: Container<CLS>,
    T: Fn() -> R,
    R: for<'js> IntoJs<'js>,
{
    const NUM_ARGS: usize = 0;

    type Args = ();
    type Return = R;

    fn try_call<'js>(
        &'static self,
        _ctx: Ctx<'js>,
        args: &Vec<Value<'js>>,
    ) -> Result<Self::Return> {
        let args_len = args.len();
        tuple_match_size(args_len, 0)?;
        let result = (*self)();
        Ok(result)
    }
}

macro_rules! tup {
    ($($($type:ident)*,)*) => {
        $(
            impl<T, FW, CLS, R, $($type,)*> StaticFunc<FW, CLS, ($($type,)*), R> for T
            where
                FW: Container<CLS>,
                T: Fn($($type,)*) -> R,
                $($type: for<'x> FromJs<'x>,)*
                R: for<'js> IntoJs<'js>,
            {
                const NUM_ARGS: usize = 0 $(+ tup!(@one $type))*;

                type Args = ($($type,)*);

                type Return = R;

                fn try_call<'js>(&'static self, ctx: Ctx<'js>, args: &Vec<Value<'js>>) -> Result<Self::Return>
                {
                    let args_len = args.len();
                    tuple_match_size(args_len, <Self as StaticFunc<FW, CLS, ($($type,)*), R>>::NUM_ARGS)?;
                    let result = (*self)(
                        $({
                            let arg: $type = FromJs::from_js(ctx, args.get(tup!(@idx $type)).cloned().unwrap())?;
                            arg
                        },)*
                    );
                    Ok(result)
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
