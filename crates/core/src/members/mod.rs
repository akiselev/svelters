use crate::inner::*;

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
pub struct AsMethod<FUNC> {
    pub(in crate) func: FUNC,
}

impl<FUNC> AsMethod<FUNC> {
    pub fn new(func: FUNC) -> Self {
        Self { func }
    }
}

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

pub struct ElementCtx<CLS, FUNC> {
    pub(in crate) name: &'static str,
    pub(in crate) element: FUNC,
    pub(in crate) cls: Class<CLS>,
}

impl<CLS, FUNC> ElementCtx<CLS, FUNC> {
    pub(in crate) fn new(name: &'static str, element: FUNC) -> Self {
        ElementCtx {
            name,
            element,
            cls: Class::new(),
        }
    }
}

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

mod caster;
mod constructor;
mod method;
mod property;
mod readonly;
mod static_fn;
mod superclass;

pub use self::caster::*;
pub use self::constructor::*;
pub use self::method::*;
pub use self::property::*;
pub use self::readonly::*;
pub use self::static_fn::*;
pub use self::superclass::*;
