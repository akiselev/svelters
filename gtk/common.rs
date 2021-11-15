pub use gtk4::glib::{Cast, IsA, ParamFlags, ParamSpec, StaticType};
pub use gtk4::glib::{Object as GObject, Value as GValue};
pub use gtk4::prelude::{ObjectExt, WidgetExt};
pub use gtk4::Widget as GtkWidget;

pub type Result<T> = rquickjs::Result<T>;

pub use crate::core::logging::*;
pub use crate::errors::HandleParamError;
pub use crate::macros::*;
pub use crate::newtype::{Gtk, G};
pub use crate::obj_data::ObjectData;

pub use crate::registry::Registry;
