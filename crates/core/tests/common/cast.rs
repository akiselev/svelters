use std::marker::PhantomData;

use super::*;

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

impl<T> IntoType<Fw, T> for Temp<T>
where
    Fw: TypeDef<T>,
{
    type Type = FwWrapper<T>;

    fn into_framework_type(self) -> Self::Type {
        FwWrapper(self.0)
    }
}
