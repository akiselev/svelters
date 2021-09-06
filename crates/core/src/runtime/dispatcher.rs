use std::collections::HashMap;

use rquickjs::{Function, Persistent};

use crate::inner::*;

#[derive(Clone)]
pub(in crate) struct Dispatcher {
    pub(in crate) inner: Inner<DispInner>,
}

impl std::ops::Deref for Dispatcher {
    type Target = Inner<DispInner>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

pub(in crate) struct DispInner {
    pub(in crate) subs: HashMap<&'static str, Inner<EventSub>>,
}

impl Dispatcher {
    pub fn new() -> Self {
        Self {
            inner: Inner::new(DispInner {
                subs: Default::default(),
            }),
        }
    }
}
