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

pub trait MarshallState<const IDX: isize>: Sized {
    type Output;
    fn marshall_state(&self) -> &Self::Output;
}

pub trait InitMarshall<'i, 'js>: Sized {
    type Output;
    fn init_marshall_state(input: &mut InputAccessor<'i, 'js>) -> Result<Self::Output>;
}

pub trait MarshallResult<'js, M> {
    type Type;
    type Marshall<'a>: IntoJs<'js>
    where
        M: 'a;
    fn marshall_result<'a>(state: &'a M, value: Self::Type) -> Result<Self::Marshall<'a>>;
}

/// IDX is the index of the argument (1 indexed). 0 is used for return values
pub trait MarshallArgument<'js, M, const IDX: isize> {
    type Type;
    type Marshall<'a>: Marshall<'a, Self::Type>
    where
        M: 'a;
    fn marshall_arg<'a>(state: &'a M) -> Result<Self::Marshall<'a>>;
}

macro_rules! arg_impls {
    ($($arg:ident)*) => {
        impl<'i, 'js $(, $arg)*> InitMarshall<'i, 'js> for ($($arg,)*)
        where
            $($arg: InitMarshall<'i, 'js>),*
        {
            type Output = ($(<$arg as InitMarshall<'i, 'js>>::Output,)*);
            fn init_marshall_state(input: &mut InputAccessor<'i, 'js>) -> Result<Self::Output> {
                Ok(($(<$arg>::init_marshall_state(input)?,)*))
            }
        }
    };
    ($($($arg:ident)*,)*) => {
        $(
            arg_impls!($($arg)*);
        )*
    }
}

arg_impls! {
    A,
    A B,
    A B D,
    A B D E,
    A B D E G,
    A B D E G H,
}
