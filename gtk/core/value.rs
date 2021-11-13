use std::{borrow::Cow, cell::UnsafeCell, marker::PhantomData};

use glib::Value as GValue;
use rquickjs::{AsProperty, Ctx, Value as JsValue};

use crate::common::*;

pub struct Value<T>(pub T);

impl<T> From<T> for Value<T> {
    fn from(val: T) -> Self {
        Value(val)
    }
}
