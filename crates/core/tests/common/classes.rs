use std::marker::PhantomData;

use rquickjs::ClassId;

use super::*;

impl TypeDef<Object> for Fw {
    const CLASS_NAME: &'static str = "Object";

    unsafe fn get_id() -> &'static mut rquickjs::ClassId {
        static mut CLASSID: ClassId = ClassId::new();
        &mut CLASSID
    }

    fn define<P>(cls: Type<Self, Object, P>) -> Result<()> {
        cls.constructor0(|| -> rquickjs::Result<Object> { Ok(Default::default()) })?;

        cls.method0("className", |_this: &Object| -> rquickjs::Result<String> {
            Ok("Object".to_string())
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
        cls.constructor0(|| -> rquickjs::Result<Widget> { Ok(Default::default()) })?;

        cls.superclass::<Object>()?;
        cls.method0("className", |_this: &Widget| -> rquickjs::Result<String> {
            Ok("Widget".to_string())
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
        cls.constructor0(|| -> rquickjs::Result<Application> { Ok(Default::default()) })?;

        cls.superclass::<Object>()?;
        cls.method0(
            "className",
            |_this: &Application| -> rquickjs::Result<String> { Ok("Application".to_string()) },
        )?;
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
        cls.constructor0(|| -> rquickjs::Result<MainWindow> { Ok(Default::default()) })?;

        cls.superclass::<Widget>()?;
        cls.method0(
            "className",
            |_this: &MainWindow| -> rquickjs::Result<String> { Ok("MainWindow".to_string()) },
        )?;
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
        cls.constructor0(|| -> rquickjs::Result<BoxLayout> { Ok(Default::default()) })?;

        cls.superclass::<Widget>()?;
        cls.method0(
            "className",
            |_this: &BoxLayout| -> rquickjs::Result<String> { Ok("BoxLayout".to_string()) },
        )?;
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
        cls.constructor0(|| -> rquickjs::Result<AbstractButton> { Ok(Default::default()) })?;

        cls.superclass::<Widget>()?;
        cls.method0(
            "className",
            |_this: &AbstractButton| -> rquickjs::Result<String> {
                Ok("AbstractButton".to_string())
            },
        )?;
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
        cls.constructor0(|| -> rquickjs::Result<PushButton> { Ok(Default::default()) })?;

        cls.superclass::<AbstractButton>()?;
        cls.method0(
            "className",
            |_this: &PushButton| -> rquickjs::Result<String> { Ok("PushButton".to_string()) },
        )?;
        Ok(())
    }
}
