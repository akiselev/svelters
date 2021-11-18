use std::os::raw::{c_long, c_ulong};

use glib::{subclass::SignalId, EnumClass, EnumValue, ToValue, Type};
use itertools::Itertools;
use rquickjs::{FromJs, IntoJs};

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

impl<'a, 'js> FromJs<'js> for G<glib::Value> {
    fn from_js(ctx: rquickjs::Ctx<'js>, value: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
        match value.type_of() {
            rquickjs::Type::Undefined | rquickjs::Type::Null => {
                let value = glib::Value::from_type(glib::Object::static_type());
                Ok(G(value))
            }
            rquickjs::Type::Bool => {
                let res = value.as_bool().unwrap();
                Ok(G(glib::Value::from(&res)))
            }
            rquickjs::Type::Int => {
                let res = value.as_int().unwrap();
                Ok(G(glib::Value::from(&res)))
            }
            rquickjs::Type::Float => {
                let res = value.as_float().unwrap();
                Ok(G(glib::Value::from(&res)))
            }
            rquickjs::Type::String => {
                let res = value.as_string().unwrap();
                Ok(G(glib::Value::from(&res.to_string()?)))
            }
            rquickjs::Type::Object => {
                let obj: crate::Object = FromJs::from_js(ctx, value)?;
                Ok(G(glib::Value::from(&obj.inner())))
            }
            rquickjs::Type::Array => todo!(),
            _ => unimplemented!(),
            // rquickjs::Type::Symbol => todo!(),
            // rquickjs::Type::Function => todo!(),
            // rquickjs::Type::Module => todo!(),
        }
    }
}

pub fn return_value_from_js<'js>(
    param_ty: glib::Type,
    ctx: rquickjs::Ctx<'js>,
    value: rquickjs::Value<'js>,
) -> rquickjs::Result<glib::Value> {
    if param_ty.is_a(Type::I8) {
        let inner: i8 = FromJs::from_js(ctx, value)?;
        Ok(inner.to_value())
    } else if param_ty.is_a(Type::U8) {
        let inner: u8 = FromJs::from_js(ctx, value)?;
        Ok(inner.to_value())
    } else if param_ty.is_a(Type::BOOL) {
        let inner: bool = FromJs::from_js(ctx, value)?;
        Ok(inner.to_value())
    } else if param_ty.is_a(Type::I32) {
        let inner: i32 = FromJs::from_js(ctx, value)?;
        Ok(inner.to_value())
    } else if param_ty.is_a(Type::U32) {
        let inner: u32 = FromJs::from_js(ctx, value)?;
        Ok(inner.to_value())
    } else if param_ty.is_a(Type::I_LONG) {
        let inner: c_long = FromJs::from_js(ctx, value)?;
        Ok(inner.to_value())
    } else if param_ty.is_a(Type::U_LONG) {
        let inner: c_ulong = FromJs::from_js(ctx, value)?;
        Ok(inner.to_value())
    } else if param_ty.is_a(Type::I64) {
        let inner: i64 = FromJs::from_js(ctx, value)?;
        Ok(inner.to_value())
    } else if param_ty.is_a(Type::U64) {
        let inner: u64 = FromJs::from_js(ctx, value)?;
        Ok(inner.to_value())
    } else if param_ty.is_a(Type::F32) {
        let inner: f32 = FromJs::from_js(ctx, value)?;
        Ok(inner.to_value())
    } else if param_ty.is_a(Type::F64) {
        let inner: f64 = FromJs::from_js(ctx, value)?;
        Ok(inner.to_value())
    } else if param_ty.is_a(Type::STRING) {
        let inner: String = FromJs::from_js(ctx, value)?;
        Ok(inner.to_value())
    } else if param_ty.is_a(Vec::<String>::static_type()) {
        let inner: Vec<String> = FromJs::from_js(ctx, value)?;
        Ok(inner.to_value())
    } else if param_ty.is_a(Type::ENUM) {
        let inner: String = FromJs::from_js(ctx, value)?;
        let cls = EnumClass::new(param_ty)
            .ok_or_else(|| exception!("{} is not an enum", param_ty.name()))?;
        if let Some(val) = cls.to_value_by_nick(inner.as_str()) {
            Ok(val)
        } else {
            Err(exception!(
                "\"{}\" is not a valid value for enum {} (\"{}\")",
                inner,
                param_ty.name(),
                cls.values().iter().map(|val| val.nick()).join("\", \"")
            ))
        }
    } else if param_ty.is_a(Type::OBJECT) || param_ty.is_a(Type::INTERFACE) {
        let obj = crate::Object::from_js(ctx, value)?;
        Ok(obj.inner().to_value())
        // } else if param_ty.is_a(Type::POINTER) {
        // } else if param_ty.is_a(Type::VARIANT) {
        // } else if param_ty.is_a(Type::FLAGS) {
    } else {
        Err(exception!(
            "Invalid return type for signal handler: expected {}, found {}",
            param_ty.name(),
            value.type_name()
        ))
    }
}
