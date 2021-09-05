use rquickjs::{HasRefs, IntoJsByRef};

use crate::inner::*;

pub struct Global<T>(pub T);

impl<T> Global<T> {
    pub fn new(var: T) -> Self { Self(var) }
}

impl<T> std::ops::Deref for Global<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub fn tuple_match_size(actual: usize, expected: usize) -> Result<()> {
    if actual == expected {
        Ok(())
    } else {
        Err(Error::new_from_js_message(
            "array",
            "tuple",
            if actual < expected {
                "Not enough values"
            } else {
                "Too many values"
            },
        ))
    }
}
