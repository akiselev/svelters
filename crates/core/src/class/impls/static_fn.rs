use rquickjs::Rest;

use crate::inner::*;

pub struct ClassStaticFunc<FW, CLS, A, R, T>(&'static str, T, PhantomData<(CLS, A, R, FW)>)
where
    FW: Container<CLS>,
    T: StaticFunc<FW, CLS, A, R>;

unsafe impl<FW, T, A, R, CLS> Sync for ClassStaticFunc<FW, CLS, A, R, T>
where
    FW: Container<CLS>,
    T: StaticFunc<FW, CLS, A, R>,
{
}

impl<FW, T, A, R, CLS> ClassStaticFunc<FW, CLS, A, R, T>
where
    FW: Container<CLS>,
    T: StaticFunc<FW, CLS, A, R>,
{
    pub(in crate) fn new(name: &'static str, func: T) -> Self {
        ClassStaticFunc(name, func, PhantomData)
    }
}
