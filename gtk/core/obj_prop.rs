use glib::ToValue;

use rquickjs::{qjs, qjs::c_int, AsFunction, AsProperty, Ctx, FromJs, Function, IntoJs, Value};

use crate::{common::*, registry::Registry, Object};

pub struct ObjectProp {
    spec: ParamSpec,
}

impl ObjectProp {
    pub fn new(spec: ParamSpec) -> Self {
        Self { spec }
    }
}

impl<'js> AsProperty<'js, ()> for ObjectProp {
    fn config(self, ctx: Ctx<'js>) -> Result<(c_int, Value<'js>, Value<'js>, Value<'js>)> {
        Ok((
            (qjs::JS_PROP_HAS_GET | qjs::JS_PROP_HAS_SET | qjs::JS_PROP_HAS_ENUMERABLE) as c_int,
            Value::new_undefined(ctx),
            Function::new::<_, (), ()>(ctx, Getter::new(self.spec.clone())).into_js(ctx)?,
            Function::new::<_, (), ()>(ctx, Setter::new(self.spec)).into_js(ctx)?,
        ))
    }
}

struct Getter {
    param: ParamSpec,
}

impl Getter {
    fn new(param: ParamSpec) -> Self {
        Self { param }
    }
}

impl<'js> AsFunction<'js, (), ()> for Getter {
    fn num_args() -> std::ops::Range<usize> {
        0..0
    }

    fn call(&self, input: &rquickjs::Input<'js>) -> rquickjs::Result<Value<'js>> {
        let acc = input.access();
        let this = acc.this::<&Object>()?;
        let _ = this.span().enter();
        self.param.log_getter();

        let ctx = acc.ctx();
        let value = match this.property(self.param.name()) {
            Ok(val) => val,
            Err(err) => return Err(exception!("Could not get property: {}", err.message)),
        };
        let obj = match value.get::<GObject>() {
            Ok(value) => value,
            Err(err) => return err.handle_param_error(acc.ctx(), &self.param),
        };
        ctx.marshall(obj)
    }
}

struct Setter {
    param: ParamSpec,
}

impl Setter {
    fn new(param: ParamSpec) -> Self {
        Self { param }
    }
}

impl<'js> AsFunction<'js, (), ()> for Setter {
    fn num_args() -> std::ops::Range<usize> {
        0..0
    }

    fn call(&self, input: &rquickjs::Input<'js>) -> rquickjs::Result<Value<'js>> {
        let mut acc = input.access();
        let this = acc.this::<&Object>()?;
        let _ = this.span().enter();
        self.param.log_setter();

        if acc.len() < 1 {
            return Err(exception!("Setter must have at least one argument"));
        }
        let value = acc.arg::<Value<'js>>()?;
        let value = if value.is_null() || value.is_undefined() {
            tracing::warn!("Object GValue is null or undefined");
            self.param.default_value().clone()
        } else {
            let obj = match Object::from_js(acc.ctx(), value) {
                Ok(val) => val,
                Err(err) => return Err(exception!("Not a GObject: {}", err)),
            };
            obj.to_value()
        };
        match this.set_property(self.param.name(), value) {
            Ok(val) => val,
            Err(err) => return Err(exception!("Could not set property: {}", err.message)),
        };
        Ok(Value::new_undefined(acc.ctx()))
    }
}
