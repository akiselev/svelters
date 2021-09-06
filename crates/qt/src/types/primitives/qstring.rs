use qt_core::QString;
use rquickjs::Coerced;

use crate::inner::*;

pub struct Q<T: qt_core::cpp_core::CppDeletable>(pub CppBox<T>);

impl<T: qt_core::cpp_core::CppDeletable> std::ops::Deref for Q<T> {
    type Target = CppBox<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'js> FromJs<'js> for Q<QString> {
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        let val = Coerced::<String>::from_js(ctx, value)?;
        Ok(Q(QString::from_std_str(val.0)))
    }
}

impl<'js> IntoJs<'js> for Q<QString> {
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        self.0.to_std_string().into_js(ctx)
    }
}
