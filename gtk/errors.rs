use glib::{
    value::{
        FromValue, FromValueOptional, ToValueOptional, ValueType, ValueTypeChecker,
        ValueTypeMismatchError, ValueTypeMismatchOrNoneError,
    },
    ParamSpec,
};

use rquickjs::{
    AsFunction, AsProperty, ClassDef, Constructor, Ctx, FromInput, FromJs, IntoAtom, IntoJs,
    Object as JsObject, ObjectDef, This, TypedMapKey, Value,
};

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
            ValueTypeMismatchOrNoneError::UnexpectedNone => Ok(Value::new_undefined(ctx)),
        }
    }
}

impl HandleParamError for ValueTypeMismatchError {
    fn handle_param_error<'js>(
        self,
        ctx: Ctx<'js>,
        param: &ParamSpec,
    ) -> rquickjs::Result<Value<'js>> {
        Err(exception!(
            "Value type mismatch error on prop {}::{}: expected {}, found {}",
            param.owner_type().name(),
            param.name(),
            self.requested_type().name(),
            self.actual_type().name()
        ))
    }
}
