#![recursion_limit = "4096"]
#![allow(incomplete_features, dead_code)]
#![allow(warnings)]
// GATS and specialization make up the core of this API
#![feature(specialization)]
#![feature(generic_associated_types)]
// Used for static data like type information
#![feature(once_cell)]
// Helps with WithImpl trait that converts from JS to args used by functions
#![feature(negative_impls)]
#![feature(auto_traits)]
// For traits using Fn<ARGS, Output=R> instead of Fn(args...) macros
#![feature(unboxed_closures)]
#![feature(fn_traits)]
// Useful for optimized debugging and whatnot
#![feature(const_type_name)]
#![feature(type_name_of_val)]
#![feature(backtrace)]
// allows bounds of the form `where T: MyTrait<Output: TraitBound>`
//   instead of `<T as MyTrait>::Output: TraitBound`
#![feature(associated_type_bounds)]
// Helps reduce some of the trait bound noise
#![feature(trait_alias)]
// enables println!("{inline} variables") instead of println!("{} variables", inline)
#![feature(format_args_capture)]
// Enables using TypeId in Metadata
#![feature(const_type_id)]
#![feature(const_fn_trait_bound)]
// Enables using size_of in [u8; size_of::<T>()]
#![feature(generic_const_exprs)]
// Unsize trait for coercing user types (used for SmallBox)
#![feature(unsize)]
// Allows creating try blocks in functions that dont return result
#![feature(try_blocks)]
// New implementation of ? operator
// #![feature(try_trait_v2)]

pub mod prelude {
    pub use crate::api::*;
    pub use crate::eldef::*;
    pub use crate::traits::*;

    pub use crate::init::{cleanup_svelters, initialize_svelters};
    pub use crate::metadata::Metadata;

    pub use crate::inner::Result;
}

pub use crate::utils::console;
pub use crate::utils::timing;

// ----------------- INTERNAL -----------------

mod api;
mod eldef;
pub(in crate) mod init;
pub(in crate) mod macros;
pub(in crate) mod metadata;
pub(in crate) mod rt;
pub(in crate) mod traits;
pub(in crate) mod utils;

#[allow(unused_imports)]
pub(in crate) mod inner {
    pub(in crate) use std::{
        any::Any,
        borrow::Cow,
        cell::{Cell, RefCell},
        collections::HashMap,
        convert::{TryFrom, TryInto},
        lazy::OnceCell,
        marker::PhantomData,
        ops::{Add, Deref, DerefMut},
        rc::Rc,
    };

    pub(in crate) use frunk::{
        coproduct::{CNil, Coproduct},
        hlist::{HList, Selector},
        HCons, HNil,
    };
    pub(in crate) use parking_lot::{const_rwlock, RwLock};
    pub(in crate) use rquickjs::{
        Accessor, AsFunction, Class as JsClass, ClassDef, ClassId, Context, Created, Ctx, Error,
        FromJs, IntoJs, Loaded, Module as QJsModule, ModuleDef, Native, Object, Opt, RefsMarker,
        RegisteryKey, Rest, This, Value, WithProto,
    };
    pub type Result<T, E = Error> = std::result::Result<T, E>;
    pub(in crate) use slotmap::{new_key_type, SlotMap};
    pub(in crate) use slotmap::{SecondaryMap, SparseSecondaryMap};
    pub(in crate) use smallbox::{smallbox, space, SmallBox};
    pub(in crate) use typedmap::TypedMapKey;

    pub(in crate) use crate::api::*;
    pub(in crate) use crate::traits::*;
    pub(in crate) use crate::utils::*;

    pub(in crate) use crate::eldef::*;
    pub(in crate) use crate::macros::*;
    pub(in crate) use crate::metadata::*;
    pub(in crate) use crate::rt::*;
    pub(in crate) use crate::utils;

    pub(in crate) use crate::{build, build_class, check_member, exception};

    pub(in crate) struct Casters {
        pub(in crate) obj_to_virt:
            SmallBox<dyn for<'a> Fn(Ctx<'a>, Object<'a>, Rt, i32) -> Result<Value<'a>>, space::S8>,
        pub(in crate) obj_to_impl:
            SmallBox<dyn for<'a> Fn(Ctx<'a>, Object<'a>, Rt, i32) -> Result<Value<'a>>, space::S8>,
        pub(in crate) impl_to_impl:
            SmallBox<dyn for<'a> Fn(Ctx<'a>, Object<'a>) -> Result<Value<'a>>, space::S8>,
    }

    impl Casters {
        pub(in crate) fn new<F1, F2, F3>(obj_to_virt: F1, obj_to_impl: F2, impl_to_impl: F3) -> Self
        where
            F1: 'static + for<'a> Fn(Ctx<'a>, Object<'a>, Rt, i32) -> Result<Value<'a>>,
            F2: 'static + for<'a> Fn(Ctx<'a>, Object<'a>, Rt, i32) -> Result<Value<'a>>,
            F3: 'static + for<'a> Fn(Ctx<'a>, Object<'a>) -> Result<Value<'a>>,
        {
            Self {
                obj_to_virt: smallbox!(obj_to_virt),
                obj_to_impl: smallbox!(obj_to_impl),
                impl_to_impl: smallbox!(impl_to_impl),
            }
        }
    }

    pub(in crate) type CasterMap = HashMap<&'static str, Casters>;
}
