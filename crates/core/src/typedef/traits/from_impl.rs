use std::ops::Range;

use rquickjs::InputAccessor;

use crate::inner::*;

#[derive(Debug, Clone, Copy)]
pub struct ObjImpl<CLS, T>(pub(in crate) T, pub(in crate) Class<CLS>);

impl<CLS, T> std::ops::Deref for ObjImpl<CLS, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<CLS, T> std::ops::DerefMut for ObjImpl<CLS, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Debug, Clone, Copy)]
pub struct VirtImpl<CLS, T>(pub(in crate) T, pub(in crate) Class<CLS>);

impl<CLS, T> std::ops::Deref for VirtImpl<CLS, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<CLS, T> std::ops::DerefMut for VirtImpl<CLS, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ElementImpl<CLS, T>(pub(in crate) T, pub(in crate) Class<CLS>);

impl<CLS, T> std::ops::Deref for ElementImpl<CLS, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<CLS, T> std::ops::DerefMut for ElementImpl<CLS, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub auto trait NotFromJsType {}

pub auto trait NotUserType {}

pub auto trait NotCoreType {}

pub auto trait NotQJsType {}

pub auto trait NotRef {}

impl<'js> !NotQJsType for Ctx<'js> {}
impl<T> !NotQJsType for This<T> {}
impl<T> !NotQJsType for Opt<T> {}
impl<T> !NotQJsType for Rest<T> {}
impl<CLS> !NotCoreType for Obj<CLS> {}
impl<'js, CLS> !NotCoreType for Virt<'js, CLS> {}
impl<'js> !NotCoreType for Element<'js> {}
impl<T: Type> !NotUserType for T {}
impl<T: Type> !NotUserType for Obj<T> {}
impl<'js, T: Type> !NotUserType for Virt<'js, T> {}
impl<T> !NotRef for &T {}
impl<T> !NotRef for &mut T {}
impl<'js, T> !NotFromJsType for T where T: FromJs<'js> {}

pub trait FromImpl<'js, CLS>: Sized {
    /// Get possible number of arguments
    fn num_args() -> Range<usize>;

    /// Get it from input
    fn from_obj<'i>(accessor: &mut InputAccessor<'i, 'js>) -> Result<Self>;
    fn from_virt<'i>(accessor: &mut InputAccessor<'i, 'js>) -> Result<Self>;
    fn from_element<'i>(accessor: &mut InputAccessor<'i, 'js>) -> Result<Self>;
}

/// Get context from input
impl<'js, CLS> FromImpl<'js, CLS> for Ctx<'js> {
    fn num_args() -> Range<usize> {
        0..0
    }

    fn from_obj<'i>(accessor: &mut InputAccessor<'i, 'js>) -> Result<Self> {
        Ok(accessor.ctx())
    }

    fn from_virt<'i>(accessor: &mut InputAccessor<'i, 'js>) -> Result<Self> {
        Ok(accessor.ctx())
    }

    fn from_element<'i>(accessor: &mut InputAccessor<'i, 'js>) -> Result<Self> {
        Ok(accessor.ctx())
    }
}

/// Get the `this` from input
impl<'js, CLS, T> FromImpl<'js, CLS> for This<T>
where
    T: FromJs<'js>,
{
    fn num_args() -> Range<usize> {
        0..0
    }

    fn from_obj<'i>(accessor: &mut InputAccessor<'i, 'js>) -> Result<Self> {
        accessor.this().map(Self)
    }

    fn from_virt<'i>(accessor: &mut InputAccessor<'i, 'js>) -> Result<Self> {
        accessor.this().map(Self)
    }

    fn from_element<'i>(accessor: &mut InputAccessor<'i, 'js>) -> Result<Self> {
        accessor.this().map(Self)
    }
}

/// Get the next optional argument from input
impl<'js, CLS, T> FromImpl<'js, CLS> for Opt<T>
where
    T: FromJs<'js>,
{
    fn num_args() -> Range<usize> {
        0..1
    }

    fn from_obj<'i>(accessor: &mut InputAccessor<'i, 'js>) -> Result<Self> {
        if accessor.len() > 0 {
            accessor.arg().map(Self)
        } else {
            Ok(Self(None))
        }
    }

    fn from_virt<'i>(accessor: &mut InputAccessor<'i, 'js>) -> Result<Self> {
        if accessor.len() > 0 {
            accessor.arg().map(Self)
        } else {
            Ok(Self(None))
        }
    }

    fn from_element<'i>(accessor: &mut InputAccessor<'i, 'js>) -> Result<Self> {
        if accessor.len() > 0 {
            accessor.arg().map(Self)
        } else {
            Ok(Self(None))
        }
    }
}

/// Get the rest arguments from input
impl<'js, CLS, T> FromImpl<'js, CLS> for Rest<T>
where
    T: FromJs<'js>,
{
    fn num_args() -> Range<usize> {
        0..usize::MAX
    }

    fn from_obj<'i>(accessor: &mut InputAccessor<'i, 'js>) -> Result<Self> {
        accessor.args().map(Self)
    }

    fn from_virt<'i>(accessor: &mut InputAccessor<'i, 'js>) -> Result<Self> {
        accessor.args().map(Self)
    }

    fn from_element<'i>(accessor: &mut InputAccessor<'i, 'js>) -> Result<Self> {
        accessor.args().map(Self)
    }
}

/// Get the next argument from input
default impl<'js, CLS, T> FromImpl<'js, CLS> for T
where
    T: FromJs<'js> + NotUserType + NotQJsType + NotCoreType + NotTuple,
{
    fn num_args() -> Range<usize> {
        1..1
    }

    fn from_obj<'i>(accessor: &mut InputAccessor<'i, 'js>) -> Result<Self> {
        accessor.arg()
    }

    fn from_virt<'i>(accessor: &mut InputAccessor<'i, 'js>) -> Result<Self> {
        accessor.arg()
    }

    fn from_element<'i>(accessor: &mut InputAccessor<'i, 'js>) -> Result<Self> {
        accessor.arg()
    }
}

macro_rules! tup {
    ($($($arg:ident)*,)*) => {
        $(
            #[allow(unused_variables)]
            impl<'js: 'c, 'c, BASE, $($arg),*> FromImpl<'js, BASE> for ($($arg,)*)
            where
                BASE: Type,
                $($arg: FromImpl<'js, BASE>),*
            {
                #[allow(non_snake_case)]
                fn num_args() -> Range<usize> {
                    $(let $arg = $arg::num_args();)*
                    0usize $(+ $arg.start)* .. 0usize $(.saturating_add($arg.end))*
                }

                fn from_obj<'i>(accessor: &mut InputAccessor<'i, 'js>) -> Result<Self> {
                    Ok(($(
                        $arg::from_obj(accessor)?,
                    )*))
                }

                fn from_virt<'i>(accessor: &mut InputAccessor<'i, 'js>) -> Result<Self> {
                    Ok(($(
                        $arg::from_virt(accessor)?,
                    )*))
                }

                fn from_element<'i>(accessor: &mut InputAccessor<'i, 'js>) -> Result<Self> {
                    Ok(($(
                        $arg::from_element(accessor)?,
                    )*))
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
