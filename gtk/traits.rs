use glib::Object as GObject;

use crate::common::Type;

pub trait CastRef<T> {
    fn cast_ref(&self) -> Option<&T>;
}

pub trait AsObject {
    fn as_object(&self) -> &GObject;
}

pub trait Interface<'js, T> {
    fn define(ty: &Type<'js, T>) -> rquickjs::Result<()>;
}
