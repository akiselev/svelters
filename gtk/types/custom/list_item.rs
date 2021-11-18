use std::cell::RefCell;

use gdk4::subclass::prelude::*;
use rquickjs::{qjs, Context, Ctx, Value};

use crate::exception;

use super::*;

struct Item {
    ctx: Context,
    value: qjs::JSValue,
}

impl Drop for Item {
    fn drop(&mut self) {
        tracing::error!("Dropping list item");
        unsafe {
            let rt = self.ctx.runtime().ptr();
            qjs::JS_FreeValueRT(rt, self.value);
        }
    }
}

mod imp {

    use rquickjs::Value;

    use super::*;

    #[derive(Default)]
    pub struct ListItemValue(pub(super) RefCell<Option<Item>>);

    #[glib::object_subclass]
    impl ObjectSubclass for ListItemValue {
        const NAME: &'static str = "ListItemValue";
        type Type = super::ListItemValue;
        type ParentType = glib::Object;
    }
    impl ObjectImpl for ListItemValue {}
}
// Public part of the Model type.
glib::wrapper! {
    pub struct ListItemValue(ObjectSubclass<imp::ListItemValue>) @implements gio::ListModel;
}
// Constructor for new instances. This simply calls glib::Object::new()
impl ListItemValue {
    #[allow(clippy::new_without_default)]
    pub fn new<'js>(ctx: Ctx<'js>, value: Value<'js>) -> rquickjs::Result<ListItemValue> {
        let obj = glib::Object::new(&[]).expect("Failed to create ListItemValue");
        {
            let this = imp::ListItemValue::from_instance(&obj);
            let mut cb = this.0.borrow_mut();
            *cb = Some(Item {
                ctx: Context::from_ctx(ctx)?,
                value: value.into_js_value(),
            });
        }
        Ok(obj)
    }

    pub fn get_value<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Value<'js>> {
        let this = imp::ListItemValue::from_instance(self);
        let inner = this.0.borrow();
        if let Some(item) = &*inner {
            Ok(unsafe { Value::from_js_value_const(ctx, item.value) })
        } else {
            Err(exception!("ListItemValue is not set"))
        }
    }
}

crate::interface!(
    ListItemValue['js] {
        fn constructor(ctx: Ctx<'js>, value: Value<'js>) {
            rquickjs::Result::Ok(crate::newtype::Gtk(ListItemValue::new(ctx, value)?))
        }

        fn getValue(this: crate::newtype::Gtk<ListItemValue>, ctx: Ctx<'js>) {
            rquickjs::Result::Ok(this.get_value(ctx)?)
        }
    }
);
