use crate::inner::*;

mod constructor {
    use super::*;

    pub struct Constructor<CLS, ARGS, RET, THIS>(
        pub(crate) THIS,
        pub(crate) PhantomData<(CLS, ARGS, RET)>,
    )
    where
        CLS: Type;

    impl<THIS, ARGS, RET, CLS> Constructor<CLS, ARGS, RET, THIS>
    where
        CLS: Type,
    {
        pub(in crate) fn new(func: THIS) -> Self {
            Constructor(func, PhantomData)
        }
    }
}

mod prop {
    use super::*;

    pub struct Property<CLS, PROP, GET, SET> {
        pub(crate) name: &'static str,
        pub(crate) getter: GET,
        pub(crate) setter: SET,
        marker: PhantomData<(CLS, PROP)>,
    }

    impl<CLS, PROP, GET, SET> Property<CLS, PROP, GET, SET> {
        pub fn new(name: &'static str, getter: GET, setter: SET) -> Self {
            Self {
                name,
                getter,
                setter,
                marker: PhantomData,
            }
        }
    }

    pub struct ReadOnly<CLS, PROP, GET> {
        pub(crate) name: &'static str,
        pub(crate) getter: GET,
        marker: PhantomData<(CLS, PROP)>,
    }

    impl<CLS, PROP, GET> ReadOnly<CLS, PROP, GET> {
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

    pub struct Method<CLS, ARGS, RET, THIS> {
        pub(crate) name: &'static str,
        pub(crate) func: THIS,
        marker: PhantomData<(CLS, ARGS, RET)>,
    }

    impl<THIS, ARGS, RET, CLS> Method<CLS, ARGS, RET, THIS> {
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

    pub struct StaticFn<CLS, ARGS, RET, THIS>(
        pub(crate) &'static str,
        pub(crate) THIS,
        PhantomData<(CLS, ARGS, RET)>,
    )
    where
        CLS: Type;

    impl<THIS, ARGS, RET, CLS> StaticFn<CLS, ARGS, RET, THIS>
    where
        CLS: Type,
    {
        pub(in crate) fn new(name: &'static str, func: THIS) -> Self {
            StaticFn(name, func, PhantomData)
        }
    }
}

mod superclass {
    use super::*;

    pub struct Superclass<CLS, SUPER>(PhantomData<(CLS, SUPER)>)
    where
        CLS: Repr;

    impl<SUPER, CLS> Superclass<CLS, SUPER>
    where
        CLS: Repr,
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
