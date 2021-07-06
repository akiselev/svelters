use std::{
    cell::RefCell,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    rc::Rc,
};

#[derive(Debug, Clone, Copy)]
pub struct FW;
#[derive(Debug, Clone, Copy)]
pub struct Fw<T>(pub T);
pub struct FwCore;
pub struct FwWidgets;
pub struct FwWrapper<T>(pub T);

#[derive(Debug)]
pub struct Object(pub RefCell<String>);

impl Default for Object {
    fn default() -> Self {
        Object(RefCell::new("Hello Object".to_string()))
    }
}

#[derive(Debug)]
pub struct Widget(pub Fw<Rc<Object>>, pub RefCell<String>);

impl Default for Widget {
    fn default() -> Self {
        Widget(Default::default(), RefCell::new("Hello Widget".to_string()))
    }
}

#[derive(Debug)]
pub struct Application(pub Fw<Rc<Object>>, pub RefCell<String>);

impl Default for Application {
    fn default() -> Self {
        Application(
            Default::default(),
            RefCell::new("Hello Application".to_string()),
        )
    }
}

#[derive(Debug)]
pub struct MainWindow(pub Fw<Rc<Widget>>, pub RefCell<String>);

impl Default for MainWindow {
    fn default() -> Self {
        MainWindow(
            Default::default(),
            RefCell::new("Hello MainWindow".to_string()),
        )
    }
}

#[derive(Debug)]
pub struct BoxLayout(pub Fw<Rc<Widget>>, pub RefCell<String>);

impl Default for BoxLayout {
    fn default() -> Self {
        BoxLayout(
            Default::default(),
            RefCell::new("Hello BoxLayout".to_string()),
        )
    }
}

#[derive(Debug)]
pub struct AbstractButton(pub Fw<Rc<Widget>>, pub RefCell<String>);

impl Default for AbstractButton {
    fn default() -> Self {
        AbstractButton(
            Default::default(),
            RefCell::new("Hello AbstractButton".to_string()),
        )
    }
}

#[derive(Debug)]
pub struct PushButton(pub Fw<Rc<AbstractButton>>, pub RefCell<String>);

impl Default for PushButton {
    fn default() -> Self {
        PushButton(
            Default::default(),
            RefCell::new("Hello PushButton".to_string()),
        )
    }
}

pub trait StaticUpcast<T> {
    fn static_cast<'a>(&'a self) -> T;
}

pub struct RefWrapper<'a, T>(pub(crate) T, pub(crate) PhantomData<&'a ()>);

impl<'a, T> AsRef<T> for RefWrapper<'a, T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}

impl<'a, T> AsMut<T> for RefWrapper<'a, T> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.0
    }
}
