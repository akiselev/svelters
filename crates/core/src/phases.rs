use crate::inner::*;

mod class {
    mod constructor_eval;
    mod constructor_load;
    mod init_proto;
    mod init_static;
    mod super_proto;
    mod super_static;

    pub(in crate) use self::constructor_eval::*;
    pub(in crate) use self::constructor_load::*;
    pub(in crate) use self::init_proto::*;
    pub(in crate) use self::init_static::*;
    pub(in crate) use self::super_proto::*;
    pub(in crate) use self::super_static::*;
}

mod module {
    mod loader;

    pub(in crate) use self::loader::*;
}

pub(in crate) use self::class::*;
pub(in crate) use self::module::*;

mod constructor {
    use super::*;

    pub struct ClassConstructor<FW, CLS, A, R, T>(
        pub(crate) T,
        pub(crate) PhantomData<(CLS, A, R, FW)>,
    )
    where
        FW: Framework + TypeDef<CLS> + Wraps<CLS>;

    impl<FW, T, A, R, CLS> ClassConstructor<FW, CLS, A, R, T>
    where
        FW: Framework + TypeDef<CLS> + Wraps<CLS>,
    {
        pub(in crate) fn new(func: T) -> Self {
            ClassConstructor(func, PhantomData)
        }
    }
}

mod prop {
    use super::*;

    pub struct ClassProp<FW, CLS, THIS, PROP, GET, SET> {
        pub(crate) name: &'static str,
        pub(crate) getter: GET,
        pub(crate) setter: SET,
        marker: PhantomData<(FW, CLS, THIS, PROP)>,
    }

    impl<FW, CLS, PROP, GET, SET, THIS> ClassProp<FW, CLS, THIS, PROP, GET, SET>
    where
        GET: for<'a> Fn(&'a THIS) -> Result<PROP>,
        SET: for<'a> Fn(&'a mut THIS, PROP) -> Result<()>,
    {
        pub fn new(name: &'static str, getter: GET, setter: SET) -> Self {
            Self {
                name,
                getter,
                setter,
                marker: PhantomData,
            }
        }
    }

    pub struct ClassReadOnly<FW, CLS, GET> {
        pub(crate) name: &'static str,
        pub(crate) getter: GET,
        marker: PhantomData<(CLS, FW)>,
    }

    impl<FW, CLS, GET> ClassReadOnly<FW, CLS, GET> {
        pub fn new(name: &'static str, getter: GET) -> Self {
            Self {
                name,
                getter,
                marker: PhantomData,
            }
        }
    }
}

mod method {
    use super::*;

    pub struct ClassMethod<FW, CLS, A, R, T>(
        pub(crate) &'static str,
        pub(crate) T,
        PhantomData<(CLS, A, R, FW)>,
    );

    impl<FW, T, A, R, CLS> ClassMethod<FW, CLS, A, R, T> {
        pub(in crate) fn new(name: &'static str, func: T) -> Self {
            ClassMethod(name, func, PhantomData)
        }
    }
}

mod staticfn {
    use super::*;

    pub struct ClassStaticFunc<FW, CLS, A, R, T>(
        pub(crate) &'static str,
        pub(crate) T,
        PhantomData<(CLS, A, R, FW)>,
    )
    where
        FW: Wraps<CLS>;

    impl<FW, T, A, R, CLS> ClassStaticFunc<FW, CLS, A, R, T>
    where
        FW: Wraps<CLS>,
    {
        pub(in crate) fn new(name: &'static str, func: T) -> Self {
            ClassStaticFunc(name, func, PhantomData)
        }
    }
}

mod superclass {
    use super::*;

    pub struct ClassSuper<FW, CLS, SUPER>(PhantomData<(CLS, SUPER, FW)>)
    where
        FW: Wraps<CLS>;

    impl<FW, SUPER, CLS> ClassSuper<FW, CLS, SUPER>
    where
        FW: Wraps<CLS>,
    {
        pub(in crate) fn new() -> Self {
            ClassSuper(PhantomData)
        }
    }
}

pub(in crate) use self::constructor::*;
pub(in crate) use self::method::*;
pub(in crate) use self::prop::*;
pub(in crate) use self::staticfn::*;
pub(in crate) use self::superclass::*;
