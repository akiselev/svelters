use std::rc::Weak;

use crate::inner::*;

#[derive(Debug, PartialOrd, Ord, PartialEq, Eq)]
#[repr(transparent)]
pub struct Inner<T>(Rc<RefCell<T>>);

impl<T> Clone for Inner<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Inner<T> {
    pub fn new(inner: T) -> Self {
        Self(Rc::new(RefCell::new(inner)))
    }

    pub fn with<FN: for<'a> FnOnce(&'a T) -> R, R>(&self, func: FN) -> R {
        let inner = self.0.borrow();
        func(&inner)
    }

    pub fn with_mut<FN: for<'a> FnOnce(&'a mut T) -> R, R>(&self, func: FN) -> R {
        let mut inner = self.0.borrow_mut();
        func(&mut inner)
    }

    pub fn downgrade(&self) -> WeakInner<T> {
        WeakInner(Rc::downgrade(&self.0))
    }
}

impl<T> Default for Inner<T>
where
    T: Default,
{
    fn default() -> Self {
        Self::new(Default::default())
    }
}

impl<T> std::ops::Deref for Inner<T> {
    type Target = RefCell<T>;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

pub struct WeakInner<T>(Weak<RefCell<T>>);

impl<T> Clone for WeakInner<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> WeakInner<T> {
    pub fn upgrade(&self) -> Option<Inner<T>> {
        self.0.upgrade().map(|inner| Inner(inner))
    }
}
