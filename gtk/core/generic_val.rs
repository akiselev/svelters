use std::os::raw::{c_long, c_ulong};

use glib::EnumValue;
use rquickjs::IntoJs;

use crate::{common::*, errors::HandleValueError, registry::Registry};

impl<'js> IntoJs<'js> for G<glib::Value> {
    fn into_js(self, ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
        G(&self.0).into_js(ctx)
    }
}

impl<'a, 'js> IntoJs<'js> for G<&'a glib::Value> {
    fn into_js(self, ctx: rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
        let value = self.0;
        let param_ty = value.type_();

        use glib::types::Type;
        if param_ty.is_a(Type::I8) {
            match value.get::<i8>() {
                Ok(val) => val.into_js(ctx),
                Err(err) => err.handle_value_error(ctx, param_ty),
            }
        } else if param_ty.is_a(Type::U8) {
            match value.get::<u8>() {
                Ok(val) => val.into_js(ctx),
                Err(err) => err.handle_value_error(ctx, param_ty),
            }
        } else if param_ty.is_a(Type::BOOL) {
            match value.get::<bool>() {
                Ok(val) => val.into_js(ctx),
                Err(err) => err.handle_value_error(ctx, param_ty),
            }
        } else if param_ty.is_a(Type::I32) {
            match value.get::<i32>() {
                Ok(val) => val.into_js(ctx),
                Err(err) => err.handle_value_error(ctx, param_ty),
            }
        } else if param_ty.is_a(Type::U32) {
            match value.get::<u32>() {
                Ok(val) => val.into_js(ctx),
                Err(err) => err.handle_value_error(ctx, param_ty),
            }
        } else if param_ty.is_a(Type::I_LONG) {
            match value.get::<c_long>() {
                Ok(val) => val.into_js(ctx),
                Err(err) => err.handle_value_error(ctx, param_ty),
            }
        } else if param_ty.is_a(Type::U_LONG) {
            match value.get::<c_ulong>() {
                Ok(val) => val.into_js(ctx),
                Err(err) => err.handle_value_error(ctx, param_ty),
            }
        } else if param_ty.is_a(Type::I64) {
            match value.get::<i64>() {
                Ok(val) => val.into_js(ctx),
                Err(err) => err.handle_value_error(ctx, param_ty),
            }
        } else if param_ty.is_a(Type::U64) {
            match value.get::<u64>() {
                Ok(val) => val.into_js(ctx),
                Err(err) => err.handle_value_error(ctx, param_ty),
            }
        } else if param_ty.is_a(Type::F32) {
            match value.get::<f32>() {
                Ok(val) => val.into_js(ctx),
                Err(err) => err.handle_value_error(ctx, param_ty),
            }
        } else if param_ty.is_a(Type::F64) {
            match value.get::<f64>() {
                Ok(val) => val.into_js(ctx),
                Err(err) => err.handle_value_error(ctx, param_ty),
            }
        } else if param_ty.is_a(Type::STRING) {
            match value.get::<String>() {
                Ok(val) => val.into_js(ctx),
                Err(err) => err.handle_value_error(ctx, param_ty),
            }
        } else if param_ty.is_a(Vec::<String>::static_type()) {
            match value.get::<Vec<String>>() {
                Ok(val) => val.into_js(ctx),
                Err(err) => err.handle_value_error(ctx, param_ty),
            }
        } else if param_ty.is_a(Type::ENUM) {
            let value = EnumValue::from_value(value).ok_or_else(|| {
                exception!(
                    "Could not convert glib Value to EnumValue for enum {}",
                    param_ty.name()
                )
            })?;
            value.nick().into_js(ctx)
        } else if param_ty.is_a(Type::OBJECT) {
            match value.get::<GObject>() {
                Ok(obj) => ctx.marshall(obj),
                Err(_) => Ok(rquickjs::Value::new_null(ctx)),
            }
        // } else if param_ty.is_a(Type::POINTER) {
        // } else if param_ty.is_a(Type::VARIANT) {
        // } else if param_ty.is_a(Type::INTERFACE) {
        // } else if param_ty.is_a(Type::FLAGS) {
        } else {
            panic!(">>>>>> IntoJs not implemented for type {}", param_ty.name());
        }
    }
}
