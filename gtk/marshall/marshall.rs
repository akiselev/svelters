use std::marker::PhantomData;

use rquickjs::{qjs::c_char, Ctx, InputAccessor, IntoJs, Value};

use crate::common::*;

pub trait Marshall<'a, T>: Sized {
    fn use_ref(&self) -> bool {
        true
    }

    fn marshall_as_owned(self) -> Result<T> {
        panic!("Attempted to use Marshall::marshall_as_owned on a ref argument");
    }
    fn marshall_as_ref(&self) -> Result<T> {
        panic!("Attempted to use Marshall::marshall_as_ref on an owned argument");
    }
}

pub trait InitMarshallArg<'i, 'js, const IDX: usize>: Sized {
    type Output;
    fn init_marshall_arg(input: &mut InputAccessor<'i, 'js>) -> Result<Self::Output>;
}

pub trait InitMarshallResult<'i, 'js>: Sized {
    fn init_marshall_result(input: &mut InputAccessor<'i, 'js>) -> Result<Self>;
}

pub trait MarshallResult<'js> {
    type Type;
    type Marshall<'a>: IntoJs<'js>;
    fn marshall_result<'a>(self, value: Self::Type) -> Result<Self::Marshall<'a>>;
}

pub trait MarshallArgument<'js> {
    type Type;
    type Marshall<'a>: Marshall<'a, Self::Type>;
    fn marshall_arg<'a>(self) -> Result<Self::Marshall<'a>>;
}

pub struct DefaultMarshall<C, T>(PhantomData<(C, T)>);

impl<'i, 'js, C, T> InitMarshallResult<'i, 'js> for DefaultMarshall<C, T> {
    fn init_marshall_result(input: &mut InputAccessor<'i, 'js>) -> Result<Self> {
        Ok(DefaultMarshall(PhantomData))
    }
}

impl<'js, C, T> MarshallResult<'js> for DefaultMarshall<C, T>
where
    T: IntoJs<'js>,
    C: for<'a> Marshall<'a, T>,
{
    type Type = C;

    type Marshall<'a> = DefaultMarshallInner<C, T>;

    fn marshall_result<'a>(self, value: Self::Type) -> Result<Self::Marshall<'a>> {
        Ok(DefaultMarshallInner(value, PhantomData))
    }
}

pub struct DefaultMarshallInner<C, T>(C, PhantomData<T>);

impl<'js, C, T> IntoJs<'js> for DefaultMarshallInner<C, T>
where
    T: IntoJs<'js>,
    C: for<'a> Marshall<'a, T>,
{
    fn into_js(self, ctx: Ctx<'js>) -> rquickjs::Result<Value<'js>> {
        let res = if self.0.use_ref() {
            self.0.marshall_as_ref()
        } else {
            self.0.marshall_as_owned()
        }?;
        res.into_js(ctx)
    }
}

macro_rules! arg_impls {
    ($($arg:ident)*: $t:ident=$num:literal) => {
        impl<'i, 'js $(, $arg)*> InitMarshallArg<'i, 'js, $num> for ($($arg,)*)
        where
            $t: InitMarshallArg<'i, 'js, 0>
        {
            type Output = <$t as InitMarshallArg<'i, 'js, 0>>::Output;
            fn init_marshall_arg(input: &mut InputAccessor<'i, 'js>) -> Result<Self::Output> {
                <$t>::init_marshall_arg(input)
            }
        }
    };
    ($($arg:ident)*: $t:ident=$num:literal $($tt:tt)*) => {
        arg_impls!($($arg)*: $t=$num);
        arg_impls!($($arg)*: $($tt)*);
    };
    ($($($arg:ident)*: ($($tt:tt)*),)*) => {
        $(
            arg_impls!($($arg)*: $($tt)*);
        )*
    }
}

arg_impls! {
    A: (A=0),
    A B: (A=0 B=1),
    A B D: (A=0 B=1 D=2),
    A B D E: (A=0 B=1 D=2 E=3),
    A B D E G: (A=0 B=1 D=2 E=3 G=4),
    A B D E G H: (A=0 B=1 D=2 E=3 G=4 H=5),
}
