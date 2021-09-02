use std::rc::Rc;

use rquickjs::ClassId;

use super::*;

pub struct Temp<T>(pub T);

impl Type for Fw<Rc<Object>> {
    const CLASS_NAME: &'static str = "Object";

    unsafe fn metadata() -> &'static mut Metadata {
        static mut META: Metadata = Metadata::new();
        &mut META
    }

    #[inline(always)]
    fn define<P>(cls: &mut TypeBuilder<Self, P>) -> Result<()> {
        cls.constructor0(|| -> Self { Default::default() })?;
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
        cls.method0("testMethod", |_this: &Self| -> String {
            "__OBJECT__".to_string()
        })?;
        cls.method0("className", |_this: &Self| -> String {
            "Object".to_string()
        })?;
        cls.method0("className2", |_this: &Self| -> String {
            "Object".to_string()
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
        cls.superclass::<Fw<Rc<Object>>>()?;
        cls.constructor0(|| -> Self { Default::default() })?;

        cls.readonly("ro", |this: &Self| {
            let val = this.0 .1.borrow();
            Ok(format!("Readonly {}", val))
        })?;

        cls.method0("className", |_this: &Self| -> String {
            "Widget".to_string()
        })?;
        cls.method0("className2", |_this: &Self| -> String {
            "Widget".to_string()
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
        cls.superclass::<Fw<Rc<Object>>>()?;
        cls.constructor0(|| -> Self { Default::default() })?;
        cls.method0("status", |this: &Self| -> String {
            this.0 .1.borrow().clone()
        })?;

        cls.readonly("ro", |this: &Self| {
            let val = this.0 .1.borrow();
            Ok(format!("Readonly2 {}", val))
        })?;

        cls.method0("className", |_this: &Self| -> String {
            "Application".to_string()
        })?;
        cls.method0("className2", |_this: &Self| -> String {
            "Application".to_string()
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
        cls.superclass::<Fw<Rc<Widget>>>()?;
        cls.constructor0(|| -> Self { Default::default() })?;
        cls.method0("title", |this: &Self| -> String {
            this.0 .1.borrow().clone()
        })?;

        cls.method0("className", |_this: &Self| -> String {
            "MainWindow".to_string()
        })?;
        cls.method0("className2", |_this: &Self| -> String {
            "MainWindow".to_string()
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
        cls.superclass::<Fw<Rc<Widget>>>()?;
        cls.constructor0(|| -> Self { Default::default() })?;
        cls.method0("orientation", |this: &Self| -> String {
            this.0 .1.borrow().clone()
        })?;

        cls.prop(
            "type",
            |this: &Self| Ok(format!("Overriden")),
            |this: &Self, val: String| Ok(()),
        )?;

        cls.method0("className", |_this: &Self| -> String {
            "BoxLayout".to_string()
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
        cls.method0("text", |this: &Self| -> String {
            this.0 .1.borrow().clone()
        })?;

        cls.method0("className", |_this: &Self| -> String {
            "AbstractButton".to_string()
        })?;
        cls.method0("className2", |_this: &Self| -> String {
            "AbstractButton".to_string()
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
        cls.superclass::<Fw<Rc<AbstractButton>>>()?;
        cls.constructor0(|| -> Self { Default::default() })?;
        cls.method0("status", |this: &Self| -> String {
            this.0 .1.borrow().clone()
        })?;

        cls.method0("className", |_this: &Self| -> String {
            "PushButton".to_string()
        })?;
        Ok(())
    }
}
