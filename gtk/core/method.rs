use std::{marker::PhantomData, ops::Range};

use rquickjs::{AsFunction, FromInput, FromJs, Input, IntoJs, ParallelSend, Result, This, Value};

use crate::common::*;

pub struct InterfaceMethod<CLS, A, F>(pub F, pub PhantomData<(CLS, A)>);

macro_rules! as_function_impls {
    ($($(#[$meta:meta])* $($arg:ident)*,)*) => {
        $(
            // for Fn()
            $(#[$meta])*
            impl<'js, FN, R, T, A $(, $arg)*> AsFunction<'js, (&A, $($arg,)*), R> for InterfaceMethod<T, A, FN>
            where
                FN: Fn(&A, $($arg),*) -> R + ParallelSend + 'static,
                R: IntoJs<'js>,
                T: 'static + CastRef<A>,
                &'js T: FromJs<'js>,
                $($arg: FromInput<'js>,)*
            {
                #[allow(non_snake_case)]
                fn num_args() -> Range<usize> {
                    $(let $arg = $arg::num_args();)*
                    0usize $(+ $arg.start)* .. 0usize $(.saturating_add($arg.end))*
                }

                #[allow(unused_mut)]
                fn call(&self, input: &Input<'js>) -> Result<Value<'js>> {
                    input.check_num_args::<Self, _, _>()?;
                    let mut accessor = input.access();
                    let this = accessor.this::<&T>()?;
                    let this = this.cast_ref().ok_or_else(|| exception!("Could not cast object to {}", std::any::type_name::<A>()))?;
                    (self.0)(
                        this,
                        $($arg::from_input(&mut accessor)?,)*
                    ).into_js(accessor.ctx())
                }
            }
        )*
    };
}

as_function_impls! {
    ,
    B,
    B D,
    B D E,
    B D E G,
    B D E G H,
    B C D E G H I,
    B C D E G H I J,
    B C D E G H I J K,
    B C D E G H I J K L,
    #[cfg(feature = "max-args-11")]
    B C D E G H I J K L M,
    #[cfg(feature = "max-args-12")]
    B C D E G H I J K L M N,
    #[cfg(feature = "max-args-13")]
    B C D E G H I J K L M N O,
    #[cfg(feature = "max-args-14")]
    B C D E G H I J K L M N O P,
    #[cfg(feature = "max-args-15")]
    B C D E G H I J K L M N O P U,
    #[cfg(feature = "max-args-16")]
    B C D E G H I J K L M N O P U V,
}
