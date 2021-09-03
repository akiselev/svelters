use std::ops::Range;

use rquickjs::InputAccessor;

use crate::inner::*;

pub trait FromImpl<'js>: Sized {
    /// Get possible number of arguments
    fn num_args() -> Range<usize>;

    /// Get it from input
    fn from_impl<'i>(accessor: &mut InputAccessor<'i, 'js>) -> Result<Self>;
}

/// Get context from input
impl<'js> FromImpl<'js> for Ctx<'js> {
    fn num_args() -> Range<usize> {
        0..0
    }

    fn from_impl<'i>(accessor: &mut InputAccessor<'i, 'js>) -> Result<Self> {
        Ok(accessor.ctx())
        //Ok(Ctx::from_ptr(accessor.ctx().ctx))
    }
}

/// Get the `this` from input
impl<'js, T> FromImpl<'js> for This<T>
where
    T: FromJs<'js>,
{
    fn num_args() -> Range<usize> {
        0..0
    }

    fn from_impl<'i>(accessor: &mut InputAccessor<'i, 'js>) -> Result<Self> {
        accessor.this().map(Self)
    }
}

/// Get the next optional argument from input
impl<'js, T> FromImpl<'js> for Opt<T>
where
    T: FromJs<'js>,
{
    fn num_args() -> Range<usize> {
        0..1
    }

    fn from_impl<'i>(accessor: &mut InputAccessor<'i, 'js>) -> Result<Self> {
        if accessor.len() > 0 {
            accessor.arg().map(Self)
        } else {
            Ok(Self(None))
        }
    }
}

/// Get the rest arguments from input
impl<'js, T> FromImpl<'js> for Rest<T>
where
    T: FromJs<'js>,
{
    fn num_args() -> Range<usize> {
        0..usize::MAX
    }

    fn from_impl<'i>(accessor: &mut InputAccessor<'i, 'js>) -> Result<Self> {
        accessor.args().map(Self)
    }
}

/// Get the next argument from input
impl<'js, T> FromImpl<'js> for T
where
    T: FromJs<'js> + NotUserType + NotQJsType + NotCoreType + NotTuple,
{
    fn num_args() -> Range<usize> {
        1..1
    }

    fn from_impl<'i>(accessor: &mut InputAccessor<'i, 'js>) -> Result<Self> {
        accessor.arg()
    }
}
