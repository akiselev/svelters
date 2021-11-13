use std::{borrow::Cow, cell::UnsafeCell, marker::PhantomData};

use rquickjs::{AsProperty, Ctx, Value};

use crate::common::*;

pub struct Member<T> {
    param_name: Cow<'static, str>,

    marker: PhantomData<T>,
}

impl<T> GParam<T> {
    pub fn new(name: &'static str) -> Self {
        Self {
            param_name: name.into(),
            spec: UnsafeCell::new(None),
            marker: PhantomData,
        }
    }
}
