use glib::Object as GObject;
use rquickjs::{AsFunction, ClassDef, Constructor, Ctx};

pub trait CastRef<T> {
    fn cast_ref(&self) -> Option<&T>;
}

pub trait AsObject {
    fn as_object(&self) -> &GObject;
}
