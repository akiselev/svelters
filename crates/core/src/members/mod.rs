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
pub struct AsMethod<FUNC> {
    pub(in crate) func: FUNC,
}

impl<FUNC> AsMethod<FUNC> {
    pub fn new(func: FUNC) -> Self {
        Self { func }
    }
}

mod constructor;
mod def_element;
mod def_event;
mod def_native_slot;
mod method;
mod property;
mod readonly;
mod static_fn;
mod superclass;

pub use self::constructor::*;
pub use self::def_element::*;
pub use self::def_event::*;
pub use self::def_native_slot::*;
pub use self::method::*;
pub use self::property::*;
pub use self::readonly::*;
pub use self::static_fn::*;
pub use self::superclass::*;
