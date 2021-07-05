use std::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

#[derive(Debug, Copy, Clone)]
pub struct Fw;
pub struct FwCore;
pub struct FwWidgets;
pub struct FwWrapper<T>(pub T);

#[derive(Debug, Clone)]
pub struct Object(pub String);

impl Default for Object {
    fn default() -> Self {
        Object("Hello Object".to_string())
    }
}

#[derive(Debug, Clone)]
pub struct Widget(pub Object, pub String);

impl Default for Widget {
    fn default() -> Self {
        Widget(Default::default(), "Hello Widget".to_string())
    }
}

#[derive(Debug, Clone)]
pub struct Application(pub Object, pub String);

impl Default for Application {
    fn default() -> Self {
        Application(Default::default(), "Hello Application".to_string())
    }
}

#[derive(Debug, Clone)]
pub struct MainWindow(pub Widget, pub String);

impl Default for MainWindow {
    fn default() -> Self {
        MainWindow(Default::default(), "Hello MainWindow".to_string())
    }
}

#[derive(Debug, Clone)]
pub struct BoxLayout(pub Widget, pub String);

impl Default for BoxLayout {
    fn default() -> Self {
        BoxLayout(Default::default(), "Hello BoxLayout".to_string())
    }
}

#[derive(Debug, Clone)]
pub struct AbstractButton(pub Widget, pub String);

impl Default for AbstractButton {
    fn default() -> Self {
        AbstractButton(Default::default(), "Hello AbstractButton".to_string())
    }
}

#[derive(Debug, Clone)]
pub struct PushButton(pub AbstractButton, pub String);

impl Default for PushButton {
    fn default() -> Self {
        PushButton(Default::default(), "Hello PushButton".to_string())
    }
}

pub trait StaticUpcast<T> {
    fn static_cast<'a>(&'a self) -> &'a T;
}

pub struct DerefWrapper<'a, T>(pub(crate) T, pub(crate) PhantomData<&'a ()>);

impl<'a, T> Deref for DerefWrapper<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a, T> DerefMut for DerefWrapper<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}
