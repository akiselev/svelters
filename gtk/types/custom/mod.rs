use glib::StaticType;
use rquickjs::Ctx;

use crate::Registry;

mod list_item;
mod list_model;

pub use list_item::*;
pub use list_model::*;

pub(crate) fn initialize_custom_types(ctx: Ctx) -> rquickjs::Result<()> {
    Ok(())
}

pub(crate) fn initialize_custom_globals(ctx: Ctx) -> rquickjs::Result<()> {
    ctx.initialize(CallbackListModel::static_type())?;
    ctx.initialize(ListItemValue::static_type())?;
    Ok(())
}
