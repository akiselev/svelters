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
    pub use crate::traits::*;

    pub use crate::runtime::document::Document;

    pub use crate::api::element::Element;
    pub use crate::api::node::Node;
    pub use crate::runtime::event::Event;
    pub use crate::typedef::hslot::BasicSlot;
    pub use crate::typedef::impls::Impl;
    pub use crate::typedef::obj::Obj;
    pub use crate::typedef::virt::Virt;

    pub use crate::init::{cleanup_svelters, initialize_svelters};
    pub use crate::metadata::Metadata;
    pub use crate::typedef::builder::ClassBuilder;
    pub use crate::typedef::builder::TypeBuilder;
    pub use crate::typedef::module::ModuleBuilder;

    pub use crate::inner::Result;
}

pub use crate::utils::console;
pub use crate::utils::timing;

// ----------------- INTERNAL -----------------

pub(in crate) mod init;
pub(in crate) mod macros;
pub(in crate) mod members;
pub(in crate) mod metadata;
pub(in crate) mod rt;
pub(in crate) mod traits;
pub(in crate) mod utils;

#[allow(unused_imports)]
pub(in crate) mod runtime {
    pub(in crate) mod dispatcher;
    pub(in crate) mod document;
    pub(in crate) mod event;
    pub(in crate) mod subscription;

    pub(in crate) use self::dispatcher::*;
    pub(in crate) use self::document::*;
    pub(in crate) use self::event::*;
    pub(in crate) use self::subscription::*;
}

// #[allow(unused_imports)]
pub(in crate) mod api {
    pub(in crate) mod element;
    pub(in crate) mod node;

    pub(in crate) use self::element::*;
    pub(in crate) use self::node::*;
}

// #[allow(unused_imports)]
pub(in crate) mod nodes {
    pub(in crate) mod element_node;
    pub(in crate) mod slot_node;
    pub(in crate) mod text_node;
    pub(in crate) use self::element_node::*;
    pub(in crate) use self::slot_node::*;
    pub(in crate) use self::text_node::*;
}

#[allow(unused_imports)]
pub(in crate) mod typedef {
    pub(in crate) mod builder;
    pub(in crate) mod hslot;
    pub(in crate) mod impls;
    pub(in crate) mod module;
    pub(in crate) mod obj;
    pub(in crate) mod virt;

    pub(in crate) use self::builder::*;
    pub(in crate) use self::hslot::*;
    pub(in crate) use self::impls::*;
    pub(in crate) use self::module::*;
    pub(in crate) use self::obj::*;
    pub(in crate) use self::virt::*;

    pub(in crate) mod traits {
        pub(in crate) mod from_impl;
        pub(in crate) mod method_impl;
        pub(in crate) mod to_result;
    }

    pub(in crate) use self::traits::from_impl::*;
    pub(in crate) use self::traits::method_impl::*;
    pub(in crate) use self::traits::to_result::*;
}

#[allow(unused_imports)]
pub(in crate) mod phases {
    mod element {
        mod constructor;
        mod init_element;
        mod init_native_slot;
        mod init_node;
        mod init_proto;

        pub(in crate) use self::constructor::*;
        pub(in crate) use self::init_element::*;
        pub(in crate) use self::init_native_slot::*;
        pub(in crate) use self::init_node::*;
        pub(in crate) use self::init_proto::*;
    }

    mod queries {
        pub(in crate) mod has_member;
        pub(in crate) mod inherits_member;

        pub(in crate) use self::has_member::*;
        pub(in crate) use self::inherits_member::*;
    }

    mod class;
    mod loader;

    pub(in crate) use self::class::*;
    pub(in crate) use self::element::*;
    pub(in crate) use self::loader::*;
    pub use self::queries::*;
}

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
    pub(in crate) use crate::nodes::*;
    pub(in crate) use crate::phases::*;
    pub(in crate) use crate::traits::*;
    pub(in crate) use crate::typedef::*;
    pub(in crate) use crate::utils::*;

    pub(in crate) use crate::macros::*;
    pub(in crate) use crate::members;
    pub(in crate) use crate::metadata::*;
    pub(in crate) use crate::rt::*;
    pub(in crate) use crate::runtime::*;
    pub(in crate) use crate::utils;

    pub(in crate) use crate::{build, build_class, check_member, exception};

    // pub(in crate) type CasterCallback<'js, CLS> =
    //     SmallBox<dyn Fn(Ctx<'js>, &CLS) -> Result<Value<'js>>, space::S4>;
    // pub(in crate) type Casters = SecondaryMap<TypeKey, SmallBox<dyn Any, space::S4>>;
    // pub(in crate) type CasterMap<CLS> = SparseSecondaryMap<
    //     TypeKey,
    //     SmallBox<
    //         dyn for<'a> Fn(Ctx<'a>, Rt, Idx, CasterCallback<'a, CLS>) -> Result<Value<'a>>,
    //         space::S8,
    //     >,
    // >;
    pub(in crate) struct Casters {
        pub(in crate) obj_to_virt:
            SmallBox<dyn for<'a> Fn(Ctx<'a>, Object<'a>, Rt, Idx) -> Result<Value<'a>>, space::S8>,
        pub(in crate) obj_to_impl:
            SmallBox<dyn for<'a> Fn(Ctx<'a>, Object<'a>, Rt, Idx) -> Result<Value<'a>>, space::S8>,
        pub(in crate) impl_to_impl:
            SmallBox<dyn for<'a> Fn(Ctx<'a>, Object<'a>) -> Result<Value<'a>>, space::S8>,
    }

    impl Casters {
        pub(in crate) fn new<F1, F2, F3>(obj_to_virt: F1, obj_to_impl: F2, impl_to_impl: F3) -> Self
        where
            F1: 'static + for<'a> Fn(Ctx<'a>, Object<'a>, Rt, Idx) -> Result<Value<'a>>,
            F2: 'static + for<'a> Fn(Ctx<'a>, Object<'a>, Rt, Idx) -> Result<Value<'a>>,
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
