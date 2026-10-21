//! Slot operations of the generated tables (`dispatch::SLOT_OPS`): a single-child property (`Content`, `Child`,
//! `ContextFlyout`, ...) or a collection of elements (`UIElementCollection`, `ItemCollection`, `IVector<T>`,
//! `IObservableVector<T>`). The generator emits one getter per collection slot that returns it as
//! `IVector<T>` for the slot's element type `T`; [`many_ops!`] turns that into the type-erased [`ManyOps`] the
//! backend drives (insert, remove, read back), casting each child to `T`.
use windows_core::{IInspectable, Result};

pub struct ManyOps {
    pub len: fn(&IInspectable) -> Result<u32>,
    pub get: fn(&IInspectable, u32) -> Result<IInspectable>,
    pub index_of: fn(&IInspectable, &IInspectable) -> Result<Option<u32>>,
    pub insert_at: fn(&IInspectable, u32, &IInspectable) -> Result<()>,
    pub append: fn(&IInspectable, &IInspectable) -> Result<()>,
    pub remove_at: fn(&IInspectable, u32) -> Result<()>,
}

pub enum SlotOps {
    /// A single-child property: set (the child is cast to the property's type; `None` clears), read back.
    Single { set: fn(&IInspectable, Option<&IInspectable>) -> Result<()>, get: fn(&IInspectable) -> Option<IInspectable> },
    /// A collection property of elements.
    Many(ManyOps),
}

/// `ManyOps` over a generated `fn(&IInspectable) -> Result<IVector<$t>>`.
macro_rules! many_ops {
    ($items:path, $t:ty) => {
        $crate::slots::SlotOps::Many($crate::slots::ManyOps {
            len: |p| $items(p)?.Size(),
            get: |p, i| windows_core::Interface::cast::<windows_core::IInspectable>(&$items(p)?.GetAt(i)?),
            index_of: |p, c| {
                let c = windows_core::Interface::cast::<$t>(c)?;
                let mut i = 0u32;
                Ok(if $items(p)?.IndexOf(&c, &mut i)? { Some(i) } else { None })
            },
            insert_at: |p, i, c| $items(p)?.InsertAt(i, &windows_core::Interface::cast::<$t>(c)?),
            append: |p, c| $items(p)?.Append(&windows_core::Interface::cast::<$t>(c)?),
            remove_at: |p, i| $items(p)?.RemoveAt(i),
        })
    };
}
pub(crate) use many_ops;
