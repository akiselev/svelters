use glib::{
    value::{ValueTypeMismatchError, ValueTypeMismatchOrNoneError},
    ParamSpec,
};

use rquickjs::{Ctx, Value};

use crate::exception;

pub trait HandleParamError {
    fn handle_param_error<'js>(
        self,
        ctx: Ctx<'js>,
        param: &ParamSpec,
    ) -> rquickjs::Result<Value<'js>>;
}

impl HandleParamError for ValueTypeMismatchOrNoneError {
    fn handle_param_error<'js>(
        self,
        ctx: Ctx<'js>,
        param: &ParamSpec,
    ) -> rquickjs::Result<Value<'js>> {
        match self {
            ValueTypeMismatchOrNoneError::WrongValueType(err) => err.handle_param_error(ctx, param),
            ValueTypeMismatchOrNoneError::UnexpectedNone => {
                tracing::debug!(
                    "Returning null for prop {}::{}",
                    param.owner_type().name(),
                    param.name()
                );
                Ok(Value::new_undefined(ctx))
            }
        }
    }
}

impl HandleParamError for ValueTypeMismatchError {
    fn handle_param_error<'js>(
        self,
        _ctx: Ctx<'js>,
        param: &ParamSpec,
    ) -> rquickjs::Result<Value<'js>> {
        tracing::error!(
            "Value Type Mismatch error for prop {}::{}: {}",
            param.owner_type().name(),
            param.name(),
            self
        );
        Err(exception!(
            "Value type mismatch error on prop {}::{}: expected {}, found {}",
            param.owner_type().name(),
            param.name(),
            self.requested_type().name(),
            self.actual_type().name()
        ))
    }
}

pub trait HandleValueError {
    fn handle_value_error(self, ctx: Ctx, ty: glib::Type) -> rquickjs::Result<Value>;
}

impl HandleValueError for ValueTypeMismatchOrNoneError {
    fn handle_value_error(self, ctx: Ctx, ty: glib::Type) -> rquickjs::Result<Value> {
        match self {
            ValueTypeMismatchOrNoneError::WrongValueType(err) => err.handle_value_error(ctx, ty),
            ValueTypeMismatchOrNoneError::UnexpectedNone => {
                tracing::debug!("Returning null for {}", ty.name());
                Ok(Value::new_undefined(ctx))
            }
        }
    }
}

impl HandleValueError for ValueTypeMismatchError {
    fn handle_value_error(self, _ctx: Ctx, ty: glib::Type) -> rquickjs::Result<Value> {
        tracing::error!("Value Type Mismatch error for {}: {}", ty.name(), self);
        Err(exception!(
            "Value type mismatch error for {}: expected {}, found {}",
            ty.name(),
            self.requested_type().name(),
            self.actual_type().name()
        ))
    }
}
