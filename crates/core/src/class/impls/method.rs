use rquickjs::{Class, Rest};

use crate::inner::*;

pub struct ClassMethod<FW, CLS, A, R, T>(&'static str, T, PhantomData<(CLS, A, R, FW)>)
where
    FW: Framework + FrameworkClass<CLS> + Container<CLS>,
    T: Method<FW, CLS, A, R>;

unsafe impl<FW, T, A, R, CLS> Sync for ClassMethod<FW, CLS, A, R, T>
where
    FW: Framework + FrameworkClass<CLS> + Container<CLS>,
    T: Method<FW, CLS, A, R>,
{
}

impl<FW, T, A, R, CLS> ClassMethod<FW, CLS, A, R, T>
where
    FW: Framework + FrameworkClass<CLS> + Container<CLS>,
    T: Method<FW, CLS, A, R>,
{
    pub(in crate) fn new(name: &'static str, func: T) -> Self {
        ClassMethod(name, func, PhantomData)
    }
}
