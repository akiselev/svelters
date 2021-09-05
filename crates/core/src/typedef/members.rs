use crate::inner::Class;

pub struct Member<CLS, MEM> {
    pub(in crate) name: &'static str,
    pub(in crate) class: Class<CLS>,
    pub(in crate) member: MEM,
}

impl<CLS, FUNC> Member<CLS, FUNC> {
    pub fn new(name: &'static str, member: FUNC) -> Member<CLS, FUNC> {
        Member {
            name,
            class: Class::new(),
            member: member,
        }
    }
}

mod constructor {
    use crate::inner::*;

    pub struct Constructor<CLS, ARGS, THIS>(pub(crate) THIS, pub(crate) PhantomData<(CLS, ARGS)>)
    where
        CLS: Type;

    impl<THIS, ARGS, CLS> Constructor<CLS, ARGS, THIS>
    where
        CLS: Type,
    {
        pub(in crate) fn new(func: THIS) -> Self {
            Constructor(func, PhantomData)
        }
    }
}

mod prop {
    use crate::inner::*;

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
    use crate::inner::*;

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

    pub struct AsMethod<FUNC> {
        pub(in crate) func: FUNC,
    }

    impl<FUNC> AsMethod<FUNC> {
        pub fn new(func: FUNC) -> Self {
            Self { func }
        }
    }
}

mod staticfn {
    use crate::inner::*;

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
    use crate::inner::*;

    pub struct Superclass<SUPER>(PhantomData<SUPER>);

    impl<SUPER> Superclass<SUPER> {
        pub(in crate) fn new() -> Self {
            Superclass(PhantomData)
        }
    }
}

mod element {
    use crate::inner::*;

    pub struct Element<CLS, FUNC> {
        pub(in crate) name: &'static str,
        pub(in crate) func: FUNC,
        marker: PhantomData<CLS>,
    }

    impl<CLS, FUNC> Element<CLS, FUNC> {
        pub(in crate) fn new(name: &'static str, func: FUNC) -> Self {
            Element {
                name,
                func,
                marker: PhantomData,
            }
        }
    }
}

mod native_slot {
    use crate::inner::*;

    pub struct NativeSlot<CLS, SLOT> {
        pub(in crate) slot: SLOT,
        marker: PhantomData<CLS>,
    }

    impl<CLS, SLOT> NativeSlot<CLS, SLOT> {
        pub(in crate) fn new(slot: SLOT) -> Self {
            NativeSlot {
                slot,
                marker: PhantomData,
            }
        }
    }
}

mod event {
    use crate::inner::*;

    pub struct Event<CLS, FUNC> {
        pub(in crate) name: &'static str,
        pub(in crate) func: FUNC,
        marker: PhantomData<CLS>,
    }

    impl<CLS, FUNC> Event<CLS, FUNC> {
        pub(in crate) fn new(name: &'static str, func: FUNC) -> Self {
            Event {
                name,
                func,
                marker: PhantomData,
            }
        }
    }
}

pub use self::constructor::*;
pub use self::element::*;
pub use self::event::*;
pub use self::method::*;
pub use self::native_slot::*;
pub use self::prop::*;
pub use self::staticfn::*;
pub use self::superclass::*;
