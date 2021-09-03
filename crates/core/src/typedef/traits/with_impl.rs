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
impl<CLS> !NotCoreType for Virt<CLS> {}
impl<'js> !NotCoreType for Element<'js> {}
impl<T: Type> !NotUserType for T {}
impl<T: Type> !NotUserType for Obj<T> {}
impl<T: Type> !NotUserType for Virt<T> {}
impl<T> !NotRef for &T {}
impl<T> !NotRef for &mut T {}
impl<'js, T> !NotFromJsType for T where T: FromJs<'js> {}

pub trait WithImpl<CLS>: Sized {
    fn num_args() -> Range<usize>;

    /// Get it from input
    fn with_obj<'i, 'js, FUNC: FnOnce<Self, Output: ToResult<'js>>>(
        func: FUNC,
        accessor: &mut InputAccessor<'i, 'js>,
    ) -> Result<Value<'js>>;

    fn with_virt<'i, 'js, FUNC: FnOnce<Self, Output: ToResult<'js>>>(
        func: FUNC,
        accessor: &mut InputAccessor<'i, 'js>,
    ) -> Result<Value<'js>>;

    fn with_element<'i, 'js, FUNC: FnOnce<Self, Output: ToResult<'js>>>(
        func: FUNC,
        accessor: &mut InputAccessor<'i, 'js>,
    ) -> Result<Value<'js>>;
}

macro_rules! tup {
    ($($($arg:ident)*,)*) => {
        $(
            #[allow(unused_variables, non_snake_case)]
            impl<BASE, CLS, $($arg),*> WithImpl<BASE> for (&CLS, $($arg,)*)
            where
                CLS: Type,
                BASE: Clone + Type + Caster<CLS>,
                $($arg: for<'js> FromImpl<'js>),*
            {
                fn num_args() -> Range<usize> {
                    $(let $arg = $arg::num_args();)*
                    0usize $(+ $arg.start)* .. 0usize $(.saturating_add($arg.end))*
                }

                fn with_obj<'i, 'js, FUNC: FnOnce<Self, Output: ToResult<'js>>>(func: FUNC, accessor: &mut InputAccessor<'i, 'js>) -> Result<Value<'js>> {
                    let this: &Obj<BASE> = accessor.this()?;
                    this.inner.with_cast(|_this: &CLS| {
                        $(
                            let $arg = $arg::from_impl(accessor)?;
                        )*
                        let result = {
                            func.call_once((_this, $($arg,)*))
                        };
                        result.to_result(accessor.ctx())
                    })
                }

                fn with_virt<'i, 'js, FUNC:  FnOnce<Self, Output: ToResult<'js>>>(func: FUNC, accessor: &mut InputAccessor<'i, 'js>) -> Result<Value<'js>> {
                    let this: Virt<BASE> = accessor.this()?;
                    $(
                        let $arg = $arg::from_impl(accessor)?;
                    )*
                    this.inner.with_cast(move |this| {
                        func.call_once((this, $($arg,)*)).to_result(accessor.ctx())
                    })
                }

                fn with_element<'i, 'js, FUNC:  FnOnce<Self, Output: ToResult<'js>>>(func: FUNC, accessor: &mut InputAccessor<'i, 'js>) -> Result<Value<'js>> {
                    let this: &Element<'js> = accessor.this()?;
                    let this: &Obj<BASE> = FromJs::from_js(ctx, this.inner)?;
                    $(
                        let $arg = $arg::from_impl(accessor)?;
                    )*
                    this.inner.with_cast(move |this| {
                        func.call_once((this, $($arg,)*)).to_result(accessor.ctx())
                    })
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
