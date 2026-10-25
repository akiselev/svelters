//! Typed services: values the app installs on a [`Document`](crate::Document) once (presto-text's
//! text system, an icon set, the platform policy) and kinds read from their contexts, so kinds need
//! no thread-locals. One value per type.
//!
//! The measure, prepare and record contexts read a service through a shared borrow
//! ([`NodeView::service`](crate::NodeView::service)); a service that must change while kinds use it
//! (a shaping cache) keeps its mutable part in a `RefCell` (`doc.set_service(RefCell::new(ts))`, then
//! `cx.service::<RefCell<TextSystem>>()`). [`EventCx`](crate::EventCx) and the document also give
//! `&mut` access.

use std::any::{Any, TypeId};
use std::collections::HashMap;

/// The services of a document, keyed by type.
#[derive(Default)]
pub struct Services {
    map: HashMap<TypeId, Box<dyn Any>>,
}

impl Services {
    /// Installs `v`, returning the value it replaces.
    pub fn insert<T: 'static>(&mut self, v: T) -> Option<T> {
        self.map.insert(TypeId::of::<T>(), Box::new(v)).and_then(|old| old.downcast::<T>().ok()).map(|b| *b)
    }
    pub fn get<T: 'static>(&self) -> Option<&T> {
        self.map.get(&TypeId::of::<T>()).and_then(|b| b.downcast_ref::<T>())
    }
    pub fn get_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.map.get_mut(&TypeId::of::<T>()).and_then(|b| b.downcast_mut::<T>())
    }
    pub fn remove<T: 'static>(&mut self) -> Option<T> {
        self.map.remove(&TypeId::of::<T>()).and_then(|b| b.downcast::<T>().ok()).map(|b| *b)
    }
    pub fn contains<T: 'static>(&self) -> bool {
        self.map.contains_key(&TypeId::of::<T>())
    }
}

impl std::fmt::Debug for Services {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Services({})", self.map.len())
    }
}
