use std::marker::PhantomData;

use rquickjs::ClassId;

use super::*;

// #[derive(Debug, Copy, Clone)]
// pub struct Fw;
// pub struct FwCore;
// pub struct FwWidgets;
// pub struct FwWrapper<T>(T);
// pub struct Object(&'static str);

impl TypeDef<Widget> for Fw {
    const CLASS_NAME: &'static str = "Widget";

    unsafe fn get_id() -> &'static mut rquickjs::ClassId {
        static mut CLASSID: ClassId = ClassId::new();
        &mut CLASSID
    }

    fn define<P>(cls: Type<Self, Widget, P>) -> Result<()> {
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
        cls.method0(
            "className",
            |_this: &PushButton| -> rquickjs::Result<String> { Ok("PushButton".to_string()) },
        )?;
        Ok(())
    }
}

impl TypeDef<Object> for Fw {
    const CLASS_NAME: &'static str = "Object";

    unsafe fn get_id() -> &'static mut rquickjs::ClassId {
        static mut CLASSID: ClassId = ClassId::new();
        &mut CLASSID
    }

    fn define<P>(cls: Type<Self, Object, P>) -> Result<()> {
        cls.method0("className", |_this: &Object| -> rquickjs::Result<String> {
            Ok("Object".to_string())
        })?;
        Ok(())
    }
}

impl StaticUpcast<Object> for Object {
    fn static_cast<'a>(&'a self) -> &'a Object {
        self
    }
}

impl StaticUpcast<Object> for Widget {
    fn static_cast<'a>(&'a self) -> &'a Object {
        self.0.static_cast()
    }
}

impl StaticUpcast<Object> for Application {
    fn static_cast<'a>(&'a self) -> &'a Object {
        self.0.static_cast()
    }
}

impl StaticUpcast<Object> for MainWindow {
    fn static_cast<'a>(&'a self) -> &'a Object {
        self.0.static_cast()
    }
}

impl StaticUpcast<Object> for BoxLayout {
    fn static_cast<'a>(&'a self) -> &'a Object {
        self.0.static_cast()
    }
}

impl StaticUpcast<Object> for AbstractButton {
    fn static_cast<'a>(&'a self) -> &'a Object {
        self.0.static_cast()
    }
}

impl StaticUpcast<Object> for PushButton {
    fn static_cast<'a>(&'a self) -> &'a Object {
        self.0.static_cast()
    }
}

impl StaticUpcast<Widget> for Widget {
    fn static_cast<'a>(&'a self) -> &'a Widget {
        self
    }
}

impl StaticUpcast<Widget> for MainWindow {
    fn static_cast<'a>(&'a self) -> &'a Widget {
        self.0.static_cast()
    }
}

impl StaticUpcast<Widget> for BoxLayout {
    fn static_cast<'a>(&'a self) -> &'a Widget {
        self.0.static_cast()
    }
}

impl StaticUpcast<Widget> for AbstractButton {
    fn static_cast<'a>(&'a self) -> &'a Widget {
        self.0.static_cast()
    }
}

impl StaticUpcast<Widget> for PushButton {
    fn static_cast<'a>(&'a self) -> &'a Widget {
        self.0.static_cast()
    }
}

impl StaticUpcast<AbstractButton> for AbstractButton {
    fn static_cast<'a>(&'a self) -> &'a AbstractButton {
        self
    }
}

impl StaticUpcast<AbstractButton> for PushButton {
    fn static_cast<'a>(&'a self) -> &'a AbstractButton {
        self.0.static_cast()
    }
}

impl StaticUpcast<PushButton> for PushButton {
    fn static_cast<'a>(&'a self) -> &'a PushButton {
        self
    }
}

impl StaticUpcast<BoxLayout> for BoxLayout {
    fn static_cast<'a>(&'a self) -> &'a BoxLayout {
        self
    }
}

impl StaticUpcast<MainWindow> for MainWindow {
    fn static_cast<'a>(&'a self) -> &'a MainWindow {
        self
    }
}

impl StaticUpcast<Application> for Application {
    fn static_cast<'a>(&'a self) -> &'a Application {
        self
    }
}

impl Cast<Object> for Object {
    type Output<'a> = DerefWrapper<'a, Object>;

    fn cast<'a>(&'a self) -> Self::Output<'a> {
        DerefWrapper(self.clone(), PhantomData)
    }
}

impl<T, O> Cast<T> for FwWrapper<O>
where
    O: StaticUpcast<T>,
    T: Clone,
{
    type Output<'a> = DerefWrapper<'a, T>;

    fn cast<'a>(&'a self) -> Self::Output<'a> {
        DerefWrapper(self.0.static_cast().clone(), PhantomData)
    }
}
