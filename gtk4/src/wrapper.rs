use glib::{object::Cast as GtkCast, IsA, ObjectType};
use gtk4::{
    prelude::{ApplicationExt, ApplicationExtManual},
    Application,
};
use svelters_core::prelude::*;

use crate::inner::*;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Gtk<CLS = ()>(pub CLS);

impl Gtk<()> {
    pub fn start<FUNC: 'static + Fn(&Application)>(name: &str, func: FUNC) {
        let app = Application::builder().application_id(name).build();

        // Connect to "activate" signal of `app`
        app.connect_activate(func);

        // Run the application
        app.run();
    }

    pub fn module() -> Self {
        Self(())
    }
}

impl<CLS> Gtk<CLS> {
    pub fn new(inner: CLS) -> Self {
        Gtk(inner)
    }
}

impl<T> Deref for Gtk<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<X, Y> Upcast<Gtk<X>> for Gtk<Y>
where
    X: ObjectType,
    Y: Clone + GtkCast + IsA<X>,
{
    fn upcast(&self) -> Gtk<X> {
        Gtk(self.0.clone().upcast())
    }
}

impl<X, Y> Upcast<X> for Gtk<Y>
where
    X: ObjectType,
    Y: Clone + GtkCast + IsA<X>,
{
    fn upcast(&self) -> X {
        self.0.clone().upcast()
    }
}

impl<X, Y> UpcastRef<X> for Gtk<Y>
where
    X: ObjectType,
    Y: Clone + GtkCast + IsA<X>,
{
    fn upcast_ref(&self) -> &X {
        self.0.upcast_ref()
    }
}
