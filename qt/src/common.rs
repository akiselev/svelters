pub use std::{
    ffi::{CStr, CString},
    os::raw::c_int,
};

pub use cpp_core::{CastFrom, CastInto, CppBox, DynamicCast, Ptr, Ref, StaticUpcast};
pub use qt_core::{
    q_meta_type::Type as QType, QMetaMethod, QMetaObject, QMetaProperty, QMetaType, QObject, QPtr,
    QVariant, q_variant::{Type as VariantType}
};

pub type Result<T, E = rquickjs::Error> = std::result::Result<T, E>;

pub use crate::macros::*;
pub use crate::meta::*;
pub use crate::newtype::Qt;
pub use crate::registry::Registry;

pub trait Stringify {
    fn stringify(&self) -> Result<String>;
}
