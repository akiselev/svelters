use crate::inner::*;

mod constructor {
    use super::*;

    pub struct Constructor<FW, CLS, ARGS, RET, THIS>(
        pub(crate) THIS,
        pub(crate) PhantomData<(CLS, ARGS, RET, FW)>,
    )
    where
        FW: Framework,
        CLS: Type<FW>;

    impl<FW, THIS, ARGS, RET, CLS> Constructor<FW, CLS, ARGS, RET, THIS>
    where
        FW: Framework,
        CLS: Type<FW>,
    {
        pub(in crate) fn new(func: THIS) -> Self {
            Constructor(func, PhantomData)
        }
    }
}

mod prop {
    use super::*;

    pub struct Property<FW, CLS, PROP, GET, SET> {
        pub(crate) name: &'static str,
        pub(crate) getter: GET,
        pub(crate) setter: SET,
        marker: PhantomData<(FW, CLS, PROP)>,
    }

    impl<FW, CLS, PROP, GET, SET> Property<FW, CLS, PROP, GET, SET> {
        pub fn new(name: &'static str, getter: GET, setter: SET) -> Self {
            Self {
                name,
                getter,
                setter,
                marker: PhantomData,
            }
        }
    }

    pub struct ReadOnly<FW, CLS, PROP, GET> {
        pub(crate) name: &'static str,
        pub(crate) getter: GET,
        marker: PhantomData<(FW, CLS, PROP)>,
    }

    impl<FW, CLS, PROP, GET> ReadOnly<FW, CLS, PROP, GET> {
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

    pub struct Method<FW, CLS, ARGS, RET, THIS> {
        pub(crate) name: &'static str,
        pub(crate) func: THIS,
        marker: PhantomData<(CLS, ARGS, RET, FW)>,
    }

    impl<FW, THIS, ARGS, RET, CLS> Method<FW, CLS, ARGS, RET, THIS> {
        pub(in crate) fn new(name: &'static str, func: THIS) -> Self {
            Method {
                name,
                func,
                marker: PhantomData,
            }
        }
    }
}

mod staticfn {
    use super::*;

    pub struct StaticFn<FW, CLS, ARGS, RET, THIS>(
        pub(crate) &'static str,
        pub(crate) THIS,
        PhantomData<(CLS, ARGS, RET, FW)>,
    )
    where
        CLS: Type<FW>;

    impl<FW, THIS, ARGS, RET, CLS> StaticFn<FW, CLS, ARGS, RET, THIS>
    where
        CLS: Type<FW>,
    {
        pub(in crate) fn new(name: &'static str, func: THIS) -> Self {
            StaticFn(name, func, PhantomData)
        }
    }
}

mod superclass {
    use super::*;

    pub struct Superclass<FW, CLS, SUPER>(PhantomData<(CLS, SUPER, FW)>)
    where
        CLS: Repr<FW>;

    impl<FW, SUPER, CLS> Superclass<FW, CLS, SUPER>
    where
        CLS: Repr<FW>,
    {
        pub(in crate) fn new() -> Self {
            Superclass(PhantomData)
        }
    }
}

pub use self::constructor::*;
pub use self::method::*;
pub use self::prop::*;
pub use self::staticfn::*;
pub use self::superclass::*;
