use std::cell::RefCell;

use gdk4::{prelude::ListModelExt, subclass::prelude::*};
use rquickjs::{qjs, Context, Ctx, IntoJs, Value};

use crate::{exception, newtype::Gtk};

use super::*;

struct Callback {
    ctx: Context,
    value: qjs::JSValue,
}

impl Callback {
    #[tracing::instrument(skip(self))]
    fn get_n_items(&self) -> rquickjs::Result<u32> {
        self.ctx.with(|ctx| {
            let func = unsafe { Value::from_js_value_const(ctx, self.value) };
            let func = func
                .as_function()
                .ok_or_else(|| exception!("Callback is not a function"))?;
            rquickjs::Function::call(func, ())
        })
    }

    #[tracing::instrument(skip(self))]
    fn get_item(&self, position: u32) -> rquickjs::Result<Option<crate::Object>> {
        self.ctx.with(|ctx| {
            let func = unsafe { Value::from_js_value_const(ctx, self.value) };
            let func = func
                .as_function()
                .ok_or_else(|| exception!("Callback is not a function"))?;
            rquickjs::Function::call(func, (position,))
        })
    }
}

impl Drop for Callback {
    fn drop(&mut self) {
        tracing::error!("Dropping list model callback");
        unsafe {
            let rt = self.ctx.runtime().ptr();
            qjs::JS_FreeValueRT(rt, self.value);
        }
    }
}

mod imp {

    use glib::StaticType;
    use rquickjs::Value;

    use super::*;

    #[derive(Default)]
    pub struct CallbackListModel(pub(super) RefCell<Option<Callback>>);

    #[glib::object_subclass]
    impl ObjectSubclass for CallbackListModel {
        const NAME: &'static str = "CallbackListModel";
        type Type = super::CallbackListModel;
        type ParentType = glib::Object;
        type Interfaces = (gio::ListModel,);
    }
    impl ObjectImpl for CallbackListModel {}
    impl ListModelImpl for CallbackListModel {
        fn item_type(&self, this: &Self::Type) -> glib::Type {
            ListItemValue::static_type()
        }
        fn n_items(&self, obj: &Self::Type) -> u32 {
            let this = CallbackListModel::from_instance(obj);
            let cb = this.0.borrow();
            if let Some(cb) = &*cb {
                match cb.get_n_items() {
                    Ok(val) => val,
                    Err(err) => {
                        tracing::error!("Error in CallbackListModel function: {}", err);
                        0
                    }
                }
            } else {
                tracing::error!("CallbackListModel is not set");
                0
            }
        }
        fn item(&self, obj: &Self::Type, position: u32) -> Option<glib::Object> {
            let this = CallbackListModel::from_instance(obj);
            let cb = this.0.borrow();
            if let Some(cb) = &*cb {
                match cb.get_item(position) {
                    Ok(val) => val.map(crate::Object::inner),
                    Err(err) => {
                        tracing::error!("Error in CallbackListModel function: {}", err);
                        None
                    }
                }
            } else {
                tracing::error!("CallbackListModel is not set");
                None
            }
        }
    }
}
// Public part of the Model type.
glib::wrapper! {
    pub struct CallbackListModel(ObjectSubclass<imp::CallbackListModel>) @implements gio::ListModel;
}
// Constructor for new instances. This simply calls glib::Object::new()
impl CallbackListModel {
    #[allow(clippy::new_without_default)]
    pub fn new<'js>(
        ctx: Ctx<'js>,
        func: rquickjs::Function<'js>,
    ) -> rquickjs::Result<CallbackListModel> {
        let obj = glib::Object::new(&[]).expect("Failed to create CallbackListModel");
        {
            let this = imp::CallbackListModel::from_instance(&obj);
            let mut cb = this.0.borrow_mut();
            *cb = Some(Callback {
                ctx: Context::from_ctx(ctx)?,
                value: func.into_value().into_js_value(),
            });
        }
        Ok(obj)
    }
}

crate::interface!(
    CallbackListModel['js] {
        fn constructor(ctx: Ctx<'js>, func: rquickjs::Function<'js>) {
            rquickjs::Result::Ok(Gtk(CallbackListModel::new(ctx, func)?))
        }

        fn numItems(this: Gtk<CallbackListModel>) {
            rquickjs::Result::Ok(this.n_items())
        }

        fn getItem(this: Gtk<CallbackListModel>, position: u32) {
            rquickjs::Result::Ok(this.item(position).map(Gtk))
        }
    }
);

crate::interface!(
    gtk4::ListView {
        fn constructor(model: rquickjs::Opt<Gtk<gtk4::SelectionModel>>, factory: rquickjs::Opt<Gtk<gtk4::ListItemFactory>>) {
            rquickjs::Result::Ok(Gtk(gtk4::ListView::new(model.0.map(|inner| inner.inner()).as_ref(), factory.0.map(|inner| inner.inner()).as_ref())))
        }
    }
);

crate::interface!(
    gtk4::NoSelection {
        fn constructor(model: rquickjs::Opt<Gtk<gtk4::gio::ListModel>>) {
            rquickjs::Result::Ok(Gtk(gtk4::NoSelection::new(model.0.map(|inner| inner.inner()).as_ref())))
        }
    }
);

crate::interface!(
    gtk4::SingleSelection {
        fn constructor(model: rquickjs::Opt<Gtk<gtk4::gio::ListModel>>) {
            rquickjs::Result::Ok(Gtk(gtk4::SingleSelection::new(model.0.map(|inner| inner.inner()).as_ref())))
        }
    }
);

crate::interface!(
    gtk4::MultiSelection {
        fn constructor(model: rquickjs::Opt<Gtk<gtk4::gio::ListModel>>) {
            rquickjs::Result::Ok(Gtk(gtk4::MultiSelection::new(model.0.map(|inner| inner.inner()).as_ref())))
        }
    }
);
