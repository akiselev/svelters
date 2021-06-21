pub mod console;
pub mod document;

mod def;
mod traits;
pub use crate::def::*;
pub use crate::traits::*;

pub(in crate) mod inner;
