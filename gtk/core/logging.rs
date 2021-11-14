use glib::{ParamSpec, StaticType};
use tracing::Span;

pub trait LogClass {
    fn span(&self) -> Span {
        tracing::info_span!("Class", name = std::any::type_name::<Self>())
    }
}

impl LogClass for crate::Object {
    fn span(&self) -> Span {
        tracing::info_span!("In Object", name = "Object")
    }
}

impl<T> LogClass for crate::Gtk<T>
where
    T: StaticType,
{
    fn span(&self) -> Span {
        let name = T::static_type().name();
        tracing::info_span!("In Gtk<{}>", name, name = name)
    }
}

pub trait LogParam {
    fn log_setter(&self);
    fn log_getter(&self);
}

impl LogParam for ParamSpec {
    fn log_setter(&self) {
        tracing::info!("Setting {}::{} ", self.owner_type().name(), self.name());
    }

    fn log_getter(&self) {
        tracing::info!("Getting {}::{} ", self.owner_type().name(), self.name());
    }
}
