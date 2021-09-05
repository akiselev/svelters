use std::rc::Rc;

use rquickjs::{Function, This};

use super::*;

impl Type for Fw<Rc<Object>> {
    const CLASS_NAME: &'static str = "Object";

    unsafe fn metadata() -> &'static mut Metadata {
        static mut META: Metadata = Metadata::new();
        &mut META
    }

    #[inline(always)]
    fn define<P>(cls: &mut TypeBuilder<Self, P>) -> Result<()> {
        cls.as_element("object", || -> Result<Self> {Ok(Default::default())})?;
        cls.constructor0(|| -> Result<Self> { Ok(Default::default()) })?;
        cls.prop(
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
        )?;
        cls.method0("testMethod", |_this: &Self| -> Result<String> {
            Ok("__OBJECT__".to_string())
        })?;
        cls.static_fn0("staticFn", || -> Result<String> {
            Ok("__STATIC__".to_string())
        })?;
        cls.static_fn0("staticFn2", || -> Result<String> {
            Ok("__STATIC__".to_string())
        })?;
        cls.method0("className", |_this: &Self| -> Result<String> {
            Ok("Object".to_string())
        })?;
        cls.method0("className2", |_this: &Self| -> Result<String> {
            Ok("Object".to_string())
        })?;
        cls.method0("nestedFn", |_this: &Self| -> Result<String> {
            Ok("Object".to_string())
        })?;
        Ok(())
    }
}

impl Type for Fw<Rc<Widget>> {
    const CLASS_NAME: &'static str = "Widget";

    unsafe fn metadata() -> &'static mut Metadata {
        static mut META: Metadata = Metadata::new();
        &mut META
    }

    #[inline(always)]
    fn define<P>(cls: &mut TypeBuilder<Self, P>) -> Result<()> {
        cls.as_element("widget", || -> Result<Self> {Ok(Default::default())})?;
        cls.superclass::<Fw<Rc<Object>>>()?;
        cls.static_fn0("staticFn2", || -> Result<String> {
            Ok("WIDGET".to_string())
        })?;

        cls.constructor0(|| -> Result<Self> { Ok(Default::default()) })?;

        cls.readonly("ro", |this: &Self| {
            let val = this.0 .1.borrow();
            Ok(Ok(format!("Readonly {}", val)))
        })?;

        cls.method0("className", |_this: &Self| -> Result<String> {
            Ok("Widget".to_string())
        })?;
        cls.method0("className2", |_this: &Self| -> Result<String> {
            Ok("Widget".to_string())
        })?;

        Ok(())
    }
}

impl Type for Fw<Rc<Application>> {
    const CLASS_NAME: &'static str = "Application";

    unsafe fn metadata() -> &'static mut Metadata {
        static mut META: Metadata = Metadata::new();
        &mut META
    }

    #[inline(always)]
    fn define<P>(cls: &mut TypeBuilder<Self, P>) -> Result<()> {
        cls.as_element("application", || -> Result<Self> {Ok(Default::default())})?;
        cls.superclass::<Fw<Rc<Object>>>()?;
        cls.static_fn0("staticFn2", || -> Result<String> {
            Ok("APPLICATION".to_string())
        })?;

        cls.constructor0(|| -> Result<Self> { Ok(Default::default()) })?;
        cls.method0("status", |this: &Self| -> Result<String> {
            Ok(this.0 .1.borrow().clone())
        })?;

        cls.readonly("ro", |this: &Self| {
            let val = this.0 .1.borrow();
            Ok(Ok(format!("Readonly2 {}", val)))
        })?;

        cls.method0("className", |_this: &Self| -> Result<String> {
            Ok("Application".to_string())
        })?;
        cls.method0("className2", |_this: &Self| -> Result<String> {
            Ok("Application".to_string())
        })?;
        Ok(())
    }
}

impl Type for Fw<Rc<MainWindow>> {
    const CLASS_NAME: &'static str = "MainWindow";

    unsafe fn metadata() -> &'static mut Metadata {
        static mut META: Metadata = Metadata::new();
        &mut META
    }

    #[inline(always)]
    fn define<P>(cls: &mut TypeBuilder<Self, P>) -> Result<()> {
        cls.as_element("mainWindow", || -> Result<Self> {Ok(Default::default())})?;
        cls.superclass::<Fw<Rc<Widget>>>()?;
        cls.static_fn0("staticFn2", || -> Result<String> {
            Ok("MAINWINDOW".to_string())
        })?;

        cls.constructor0(|| -> Result<Self> { Ok(Default::default()) })?;
        cls.method0("title", |this: &Self| -> Result<String> {
            Ok(this.0 .1.borrow().clone())
        })?;

        cls.method0("className", |_this: &Self| -> Result<String> {
            Ok("MainWindow".to_string())
        })?;
        cls.method0("className2", |_this: &Self| -> Result<String> {
            Ok("MainWindow".to_string())
        })?;
        cls.method0("nestedFn", |_this: &Self| -> Result<String> {
            Ok("NESTED".to_string())
        })?;
        Ok(())
    }
}

impl Type for Fw<Rc<BoxLayout>> {
    const CLASS_NAME: &'static str = "BoxLayout";

    unsafe fn metadata() -> &'static mut Metadata {
        static mut META: Metadata = Metadata::new();
        &mut META
    }

    #[inline(always)]
    fn define<P>(cls: &mut TypeBuilder<Self, P>) -> Result<()> {
        cls.as_element("boxLayout", || -> Result<Self> {Ok(Default::default())})?;
        cls.superclass::<Fw<Rc<Widget>>>()?;
        cls.static_fn0("staticFn2", || -> Result<String> {
            Ok("BOXLAYOUT".to_string())
        })?;

        cls.constructor0(|| -> Result<Self> { Ok(Default::default()) })?;
        cls.method0("orientation", |this: &Self| -> Result<String> {
            Ok(this.0 .1.borrow().clone())
        })?;
        cls.method1("addWidget", |this: &Self, widget: Virt<Fw<Rc<Widget>>>| -> Result<String> {
            Ok(widget.0.1.borrow().clone())
        })?;
        cls.method1("addObject", |this: &Self, widget: Virt<Fw<Rc<Object>>>| -> Result<String> {
            let func: Function = widget.get("className2")?;
            func.call((This(widget.as_object().clone()), ))
        })?;

        cls.prop(
            "type",
            |_this: &Self| Ok(format!("Overriden")),
            |_this: &Self, _val: String| Ok(()),
        )?;

        cls.method0("className", |_this: &Self| -> Result<String> {
            Ok("BoxLayout".to_string())
        })?;
        Ok(())
    }
}

impl Type for Fw<Rc<AbstractButton>> {
    const CLASS_NAME: &'static str = "AbstractButton";

    unsafe fn metadata() -> &'static mut Metadata {
        static mut META: Metadata = Metadata::new();
        &mut META
    }

    #[inline(always)]
    fn define<P>(cls: &mut TypeBuilder<Self, P>) -> Result<()> {
        cls.superclass::<Fw<Rc<Widget>>>()?;
        cls.static_fn0("staticFn2", || -> Result<String> {
            Ok("ABSTRACTBUTTON".to_string())
        })?;

        cls.method0("text", |this: &Self| -> Result<String> {
            Ok(this.0 .1.borrow().clone())
        })?;

        cls.method0("className", |_this: &Self| -> Result<String> {
            Ok("AbstractButton".to_string())
        })?;
        cls.method0("className2", |_this: &Self| -> Result<String> {
            Ok("AbstractButton".to_string())
        })?;
        Ok(())
    }
}

impl Type for Fw<Rc<PushButton>> {
    const CLASS_NAME: &'static str = "PushButton";

    unsafe fn metadata() -> &'static mut Metadata {
        static mut META: Metadata = Metadata::new();
        &mut META
    }

    #[inline(always)]
    fn define<P>(cls: &mut TypeBuilder<Self, P>) -> Result<()> {
        cls.as_element("pushButton", || -> Result<Self> {Ok(Default::default())})?;
        cls.superclass::<Fw<Rc<AbstractButton>>>()?;
        cls.static_fn0("staticFn2", || -> Result<String> {
            Ok("PUSHBUTTON".to_string())
        })?;

        cls.constructor0(|| -> Result<Self> { Ok(Default::default()) })?;
        cls.method0("status", |this: &Self| -> Result<String> {
            Ok(this.0 .1.borrow().clone())
        })?;

        cls.method0("className", |_this: &Self| -> Result<String> {
            Ok("PushButton".to_string())
        })?;
        Ok(())
    }
}
