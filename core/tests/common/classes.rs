use std::rc::Rc;

use rquickjs::{Function, This};

use super::*;

impl Type for Fw<Rc<Object>> {
    const CLASS_NAME: &'static str = "Object";

    unsafe fn metadata() -> &'static mut Metadata {
        static mut META: Metadata = Metadata::new::<Fw<Rc<Object>>>();
        &mut META
    }

    #[inline(always)]
    fn define<P>(builder: TypeBuilder<Self, P>) -> Result<P> {
        builder
            .class()?
            .as_element("object", || -> Result<Self> { Ok(Default::default()) })?
            .constructor0(|| -> Result<Self> { Ok(Default::default()) })?
            .prop(
                "type",
                |this: &Self| {
                    let val = this.0 .0.borrow();
                    Ok(format!("Getter {}", val))
                },
                |this: &Self, val: String| {
                    if val != "invalid" {
                        let mut cell = this.0 .0.borrow_mut();
                        *cell = val;
                    }
                    Ok(())
                },
            )?
            .method0("testMethod", |_this: &Self| -> Result<String> {
                Ok("__OBJECT__".to_string())
            })?
            .static_fn0("staticFn", || -> Result<String> {
                Ok("__STATIC__".to_string())
            })?
            .static_fn0("staticFn2", || -> Result<String> {
                Ok("__STATIC__".to_string())
            })?
            .method0("className", |_this: &Self| -> Result<String> {
                Ok("Object".to_string())
            })?
            .method0("className2", |_this: &Self| -> Result<String> {
                Ok("Object".to_string())
            })?
            .method0("nestedFn", |_this: &Self| -> Result<String> {
                Ok("Object".to_string())
            })?
            .build()
    }
}

impl Type for Fw<Rc<Widget>> {
    const CLASS_NAME: &'static str = "Widget";

    unsafe fn metadata() -> &'static mut Metadata {
        static mut META: Metadata = Metadata::new::<Fw<Rc<Widget>>>();
        &mut META
    }

    #[inline(always)]
    fn define<P>(builder: TypeBuilder<Self, P>) -> Result<P> {
        builder
            .class()?
            .as_element("widget", || -> Result<Self> { Ok(Default::default()) })?
            .superclass::<Fw<Rc<Object>>>()?
            .static_fn0("staticFn2", || -> Result<String> {
                Ok("WIDGET".to_string())
            })?
            .constructor0(|| -> Result<Self> { Ok(Default::default()) })?
            .readonly("ro", |this: &Self| {
                let val = this.0 .1.borrow();
                Ok(Ok(format!("Readonly {}", val)))
            })?
            .method0("className", |_this: &Self| -> Result<String> {
                Ok("Widget".to_string())
            })?
            .method0("className2", |_this: &Self| -> Result<String> {
                Ok("Widget".to_string())
            })?
            .build()
    }
}

impl Type for Fw<Rc<Application>> {
    const CLASS_NAME: &'static str = "Application";

    unsafe fn metadata() -> &'static mut Metadata {
        static mut META: Metadata = Metadata::new::<Fw<Rc<Application>>>();
        &mut META
    }

    #[inline(always)]
    fn define<P>(builder: TypeBuilder<Self, P>) -> Result<P> {
        builder
            .class()?
            .as_element("application", || -> Result<Self> { Ok(Default::default()) })?
            .superclass::<Fw<Rc<Object>>>()?
            .static_fn0("staticFn2", || -> Result<String> {
                Ok("APPLICATION".to_string())
            })?
            .constructor0(|| -> Result<Self> { Ok(Default::default()) })?
            .method0("status", |this: &Self| -> Result<String> {
                Ok(this.0 .1.borrow().clone())
            })?
            .readonly("ro", |this: &Self| {
                let val = this.0 .1.borrow();
                Ok(Ok(format!("Readonly2 {}", val)))
            })?
            .method0("className", |_this: &Self| -> Result<String> {
                Ok("Application".to_string())
            })?
            .method0("className2", |_this: &Self| -> Result<String> {
                Ok("Application".to_string())
            })?
            .build()
    }
}

impl Type for Fw<Rc<MainWindow>> {
    const CLASS_NAME: &'static str = "MainWindow";

    unsafe fn metadata() -> &'static mut Metadata {
        static mut META: Metadata = Metadata::new::<Fw<Rc<MainWindow>>>();
        &mut META
    }

    #[inline(always)]
    fn define<P>(builder: TypeBuilder<Self, P>) -> Result<P> {
        builder
            .class()?
            .as_element("mainWindow", || -> Result<Self> { Ok(Default::default()) })?
            .superclass::<Fw<Rc<Widget>>>()?
            .static_fn0("staticFn2", || -> Result<String> {
                Ok("MAINWINDOW".to_string())
            })?
            .constructor0(|| -> Result<Self> { Ok(Default::default()) })?
            .method0("title", |this: &Self| -> Result<String> {
                Ok(this.0 .1.borrow().clone())
            })?
            .method0("className", |_this: &Self| -> Result<String> {
                Ok("MainWindow".to_string())
            })?
            .method0("className2", |_this: &Self| -> Result<String> {
                Ok("MainWindow".to_string())
            })?
            .method0("nestedFn", |_this: &Self| -> Result<String> {
                Ok("NESTED".to_string())
            })?
            .build()
    }
}

impl Type for Fw<Rc<BoxLayout>> {
    const CLASS_NAME: &'static str = "BoxLayout";

    unsafe fn metadata() -> &'static mut Metadata {
        static mut META: Metadata = Metadata::new::<Fw<Rc<BoxLayout>>>();
        &mut META
    }

    #[inline(always)]
    fn define<P>(builder: TypeBuilder<Self, P>) -> Result<P> {
        builder
            .class()?
            .as_element("boxLayout", || -> Result<Self> { Ok(Default::default()) })?
            .superclass::<Fw<Rc<Widget>>>()?
            .static_fn0("staticFn2", || -> Result<String> {
                Ok("BOXLAYOUT".to_string())
            })?
            .constructor0(|| -> Result<Self> { Ok(Default::default()) })?
            .method0("orientation", |this: &Self| -> Result<String> {
                Ok(this.0 .1.borrow().clone())
            })?
            .method1(
                "addWidget",
                |_this: &Self, widget: Virt<Fw<Rc<Widget>>>| -> Result<String> {
                    // Err(rquickjs::Error::Unknown)
                    // Ok(widget.0 .1.borrow().clone())
                    Ok("ADD_WIDGET_RESULT".to_string())
                },
            )?
            .method1(
                "addObject",
                |_this: &Self, widget: Virt<Fw<Rc<Object>>>| -> Result<String> {
                    let func: Function = widget.get("className2")?;
                    func.call((This(widget.as_object().clone()),))
                },
            )?
            .prop(
                "type",
                |_this: &Self| Ok(format!("Overriden")),
                |_this: &Self, _val: String| Ok(()),
            )?
            .method0("className", |_this: &Self| -> Result<String> {
                Ok("BoxLayout".to_string())
            })?
            .build()
    }
}

impl Type for Fw<Rc<AbstractButton>> {
    const CLASS_NAME: &'static str = "AbstractButton";

    unsafe fn metadata() -> &'static mut Metadata {
        static mut META: Metadata = Metadata::new::<Fw<Rc<AbstractButton>>>();
        &mut META
    }

    #[inline(always)]
    fn define<P>(builder: TypeBuilder<Self, P>) -> Result<P> {
        builder
            .class()?
            .superclass::<Fw<Rc<Widget>>>()?
            .static_fn0("staticFn2", || -> Result<String> {
                Ok("ABSTRACTBUTTON".to_string())
            })?
            .method0("text", |this: &Self| -> Result<String> {
                Ok(this.0 .1.borrow().clone())
            })?
            .method0("className", |_this: &Self| -> Result<String> {
                Ok("AbstractButton".to_string())
            })?
            .method0("className2", |_this: &Self| -> Result<String> {
                Ok("AbstractButton".to_string())
            })?
            .build()
    }
}

impl Type for Fw<Rc<PushButton>> {
    const CLASS_NAME: &'static str = "PushButton";

    unsafe fn metadata() -> &'static mut Metadata {
        static mut META: Metadata = Metadata::new::<Fw<Rc<PushButton>>>();
        &mut META
    }

    #[inline(always)]
    fn define<P>(builder: TypeBuilder<Self, P>) -> Result<P> {
        builder
            .class()?
            .as_element("pushButton", || -> Result<Self> { Ok(Default::default()) })?
            .superclass::<Fw<Rc<AbstractButton>>>()?
            .static_fn0("staticFn2", || -> Result<String> {
                Ok("PUSHBUTTON".to_string())
            })?
            .constructor0(|| -> Result<Self> { Ok(Default::default()) })?
            .method0("status", |this: &Self| -> Result<String> {
                Ok(this.0 .1.borrow().clone())
            })?
            .method0("className", |_this: &Self| -> Result<String> {
                Ok("PushButton".to_string())
            })?
            .build()
    }
}
