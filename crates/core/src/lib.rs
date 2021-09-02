#![feature(generic_associated_types)]
#![feature(const_type_name)]
#![feature(type_name_of_val)]
#![feature(specialization)]
#![feature(once_cell)]
#![feature(unboxed_closures)]
#![feature(fn_traits)]
#![recursion_limit = "4096"]
#![allow(incomplete_features, dead_code)]

pub mod prelude {
    pub use crate::metadata::Metadata;
    pub use crate::module::ModuleBuilder;
    pub use crate::obj::Obj;
    pub use crate::traits::*;
    pub use crate::typedef::TypeBuilder;
    pub use crate::virt::Virt;
}

#[cfg(feature = "unstable")]
pub mod types {
    pub use phases::Constructor;
}

pub(in crate) mod builders;
pub(in crate) mod macros;
pub(in crate) mod metadata;
pub(in crate) mod module;
pub(in crate) mod obj;
pub(in crate) mod traits;
pub(in crate) mod typedef;
pub(in crate) mod utils;
pub(in crate) mod virt;

// ----------------- INTERNAL -----------------

pub(in crate) mod phases {
    mod class {
        mod constructor;
        mod init_proto;
        mod init_static;
        mod init_virt;

        pub(in crate) use self::constructor::*;
        pub(in crate) use self::init_proto::*;
        pub(in crate) use self::init_static::*;
        pub(in crate) use self::init_virt::*;
    }

    mod slot {
        mod init_caster;
        mod init_super;

        pub(in crate) use self::init_super::*;
    }

    mod module {
        mod loader;

        pub(in crate) use self::loader::*;
    }

    pub(in crate) use self::class::*;
    pub(in crate) use self::module::*;
    pub(in crate) use self::slot::*;
}

#[allow(unused_imports)]
pub(in crate) mod inner {
    pub(in crate) use std::{
        borrow::Cow,
        cell::Cell,
        convert::{TryFrom, TryInto},
        marker::PhantomData,
        ops::{Add, Deref, DerefMut},
    };

    pub(in crate) use frunk::{
        hlist::{HList, Selector},
        HCons, HNil,
    };
    pub(in crate) use parking_lot::{const_rwlock, RwLock};
    pub(in crate) use rquickjs::{
        Accessor, AsFunction, Class as JsClass, ClassDef, ClassId, Context, Created, Ctx, Error,
        FromJs, IntoJs, Loaded, Module as QJsModule, ModuleDef, Native, Object, RefsMarker, Rest,
        Result, Value,
    };

    pub(in crate) use crate::builders::*;
    pub(in crate) use crate::metadata::*;
    pub(in crate) use crate::module::*;
    pub(in crate) use crate::obj::*;
    pub(in crate) use crate::phases::*;
    pub(in crate) use crate::traits::*;
    pub(in crate) use crate::typedef::*;
    pub(in crate) use crate::utils::*;
    pub(in crate) use crate::virt::*;

    pub(in crate) use crate::macros::*;
}

// ------------------ LEGACY ------------------
mod legacy;

pub use crate::legacy::console;
pub use crate::legacy::document;
pub use crate::legacy::timing;
