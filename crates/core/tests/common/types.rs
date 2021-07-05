use std::{marker::PhantomData, ops::Deref};

#[derive(Debug, Copy, Clone)]
pub struct Fw;
pub struct FwCore;
pub struct FwWidgets;
pub struct FwWrapper<T>(pub T);

#[derive(Debug, Clone)]
pub struct Object(pub String);

#[derive(Debug, Clone)]
pub struct Widget(pub Object);

#[derive(Debug, Clone)]
pub struct Application(pub Object);

#[derive(Debug, Clone)]
pub struct MainWindow(pub Widget);

#[derive(Debug, Clone)]
pub struct BoxLayout(pub Widget);

#[derive(Debug, Clone)]
pub struct AbstractButton(pub Widget);

#[derive(Debug, Clone)]
pub struct PushButton(pub AbstractButton);

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
