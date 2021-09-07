use qt_core::{cpp_core::Ref, QString};
use rquickjs::Coerced;

use crate::inner::*;

pub enum Q<T: qt_core::cpp_core::CppDeletable> {
    Box(CppBox<T>),
    Ptr(Ptr<T>),
}

impl<T: qt_core::cpp_core::CppDeletable> Q<T> {
    pub fn as_ptr(&self) -> Ptr<T> {
        match self {
            Q::Box(this) => unsafe { this.as_ptr() },
            Q::Ptr(this) => unsafe { this.clone() },
        }
    }
    pub fn to_ref(&self) -> Ref<T> {
        match self {
            Q::Box(this) => unsafe { this.as_ref() },
            Q::Ptr(this) => unsafe { this.as_ref().unwrap() },
        }
    }
}

impl<T: qt_core::cpp_core::CppDeletable> std::ops::Deref for Q<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            Q::Box(this) => &this,
            Q::Ptr(this) => &this,
        }
    }
}

impl<T: qt_core::cpp_core::CppDeletable> AsRef<T> for Q<T> {
    fn as_ref(&self) -> &T {
        match self {
            Q::Box(this) => &this,
            Q::Ptr(this) => &this,
        }
    }
}

impl<'js> FromJs<'js> for Q<QString> {
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        let val = Coerced::<String>::from_js(ctx, value)?;
        Ok(Q::Box(QString::from_std_str(val.0)))
    }
}

impl<'js> IntoJs<'js> for Q<QString> {
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        self.as_ptr().to_std_string().into_js(ctx)
    }
}
