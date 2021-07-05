use std::marker::PhantomData;

use rquickjs::ClassId;

use super::*;

pub struct Temp<T>(pub T);

impl TypeDef<Object> for Fw {
    const CLASS_NAME: &'static str = "Object";

    unsafe fn get_id() -> &'static mut rquickjs::ClassId {
        static mut CLASSID: ClassId = ClassId::new();
        &mut CLASSID
    }

    fn define<P>(cls: Type<Self, Object, P>) -> Result<()> {
        cls.constructor0(|| -> Temp<Object> { Temp(Default::default()) })?;

        cls.method0("className", |_this: &Object| -> String {
            "Object".to_string()
        })?;
        Ok(())
    }
}

impl TypeDef<Widget> for Fw {
    const CLASS_NAME: &'static str = "Widget";

    unsafe fn get_id() -> &'static mut rquickjs::ClassId {
        static mut CLASSID: ClassId = ClassId::new();
        &mut CLASSID
    }

    fn define<P>(cls: Type<Self, Widget, P>) -> Result<()> {
        cls.constructor0(|| -> Temp<Widget> { Temp(Default::default()) })?;

        cls.superclass::<Object>()?;
        cls.method0("className", |_this: &Widget| -> String {
            "Widget".to_string()
        })?;
        Ok(())
    }
}

impl TypeDef<Application> for Fw {
    const CLASS_NAME: &'static str = "Application";

    unsafe fn get_id() -> &'static mut rquickjs::ClassId {
        static mut CLASSID: ClassId = ClassId::new();
        &mut CLASSID
    }

    fn define<P>(cls: Type<Self, Application, P>) -> Result<()> {
        cls.constructor0(|| -> Temp<Application> { Temp(Default::default()) })?;
        cls.method0("status", |this: &Application| -> String { this.1.clone() })?;

        cls.superclass::<Object>()?;
        cls.method0("className", |_this: &Application| -> String {
            "Application".to_string()
        })?;
        Ok(())
    }
}

impl TypeDef<MainWindow> for Fw {
    const CLASS_NAME: &'static str = "MainWindow";

    unsafe fn get_id() -> &'static mut rquickjs::ClassId {
        static mut CLASSID: ClassId = ClassId::new();
        &mut CLASSID
    }

    fn define<P>(cls: Type<Self, MainWindow, P>) -> Result<()> {
        cls.constructor0(|| -> Temp<MainWindow> { Temp(Default::default()) })?;
        cls.method0("title", |this: &MainWindow| -> String { this.1.clone() })?;

        cls.superclass::<Widget>()?;
        cls.method0("className", |_this: &MainWindow| -> String {
            "MainWindow".to_string()
        })?;
        Ok(())
    }
}

impl TypeDef<BoxLayout> for Fw {
    const CLASS_NAME: &'static str = "BoxLayout";

    unsafe fn get_id() -> &'static mut rquickjs::ClassId {
        static mut CLASSID: ClassId = ClassId::new();
        &mut CLASSID
    }

    fn define<P>(cls: Type<Self, BoxLayout, P>) -> Result<()> {
        cls.constructor0(|| -> Temp<BoxLayout> { Temp(Default::default()) })?;
        cls.method0("orientation", |this: &BoxLayout| -> String {
            this.1.clone()
        })?;

        cls.superclass::<Widget>()?;
        cls.method0("className", |_this: &BoxLayout| -> String {
            "BoxLayout".to_string()
        })?;
        Ok(())
    }
}

impl TypeDef<AbstractButton> for Fw {
    const CLASS_NAME: &'static str = "AbstractButton";

    unsafe fn get_id() -> &'static mut rquickjs::ClassId {
        static mut CLASSID: ClassId = ClassId::new();
        &mut CLASSID
    }

    fn define<P>(cls: Type<Self, AbstractButton, P>) -> Result<()> {
        cls.method0("text", |this: &AbstractButton| -> String { this.1.clone() })?;

        cls.superclass::<Widget>()?;
        cls.method0("className", |_this: &AbstractButton| -> String {
            "AbstractButton".to_string()
        })?;
        Ok(())
    }
}

impl TypeDef<PushButton> for Fw {
    const CLASS_NAME: &'static str = "PushButton";

    unsafe fn get_id() -> &'static mut rquickjs::ClassId {
        static mut CLASSID: ClassId = ClassId::new();
        &mut CLASSID
    }

    fn define<P>(cls: Type<Self, PushButton, P>) -> Result<()> {
        cls.constructor0(|| -> Temp<PushButton> { Temp(Default::default()) })?;
        cls.method0("status", |this: &PushButton| -> String { this.1.clone() })?;

        cls.superclass::<AbstractButton>()?;
        cls.method0("className", |_this: &PushButton| -> String {
            "PushButton".to_string()
        })?;
        Ok(())
    }
}
