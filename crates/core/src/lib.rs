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
// Unsize trait for coercing user types
#![feature(unsize)]

pub mod prelude {
    pub use crate::traits::*;

    pub use crate::runtime::document::Document;

    pub use crate::nodes::element::Element;
    pub use crate::nodes::node::Node;
    pub use crate::runtime::event::Event;
    pub use crate::typedef::hslot::BasicSlot;
    pub use crate::typedef::obj::Obj;
    pub use crate::typedef::virt::Virt;

    pub use crate::init::{cleanup_svelters, initialize_svelters};
    pub use crate::metadata::Metadata;
    pub use crate::typedef::builder::TypeBuilder;
    pub use crate::typedef::module::ModuleBuilder;
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
    pub(in crate) mod callbacks;
    pub(in crate) mod dispatcher;
    pub(in crate) mod dispatcher_def;
    pub(in crate) mod doc_def;
    pub(in crate) mod document;
    pub(in crate) mod event;
    pub(in crate) mod subscription;

    pub(in crate) use self::callbacks::*;
    pub(in crate) use self::dispatcher::*;
    pub(in crate) use self::dispatcher_def::*;
    pub(in crate) use self::doc_def::*;
    pub(in crate) use self::document::*;
    pub(in crate) use self::event::*;
    pub(in crate) use self::subscription::*;
}

#[allow(unused_imports)]
pub(in crate) mod nodes {
    pub(in crate) mod element;
    pub(in crate) mod node;

    pub(in crate) use self::element::*;
    pub(in crate) use self::node::*;

    pub(in crate) mod elements {
        pub(in crate) mod element_def;
        pub(in crate) mod element_node;
        pub(in crate) mod element_node_def;
    }

    pub(in crate) use self::elements::element_def::*;
    pub(in crate) use self::elements::element_node::*;
    pub(in crate) use self::elements::element_node_def::*;

    pub(in crate) mod slot {
        pub(in crate) mod slot_def;
        pub(in crate) mod slot_node;
    }

    pub(in crate) use self::slot::slot_def::*;
    pub(in crate) use self::slot::slot_node::*;

    pub(in crate) mod text {
        pub(in crate) mod text_node;
        pub(in crate) mod text_node_def;
    }

    pub(in crate) use self::text::text_node::*;
    pub(in crate) use self::text::text_node_def::*;
}

#[allow(unused_imports)]
pub(in crate) mod typedef {
    pub(in crate) mod builder;
    pub(in crate) mod hslot;
    pub(in crate) mod module;
    pub(in crate) mod obj;
    pub(in crate) mod virt;

    pub(in crate) use self::builder::*;
    pub(in crate) use self::hslot::*;
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
        mod init_caster;
        mod init_element;
        mod init_native_slot;
        mod init_node;
        mod init_proto;

        pub(in crate) use self::constructor::*;
        pub(in crate) use self::init_caster::*;
        pub(in crate) use self::init_element::*;
        pub(in crate) use self::init_native_slot::*;
        pub(in crate) use self::init_node::*;
        pub(in crate) use self::init_proto::*;
    }

    mod class;
    mod loader;

    pub(in crate) use self::class::*;
    pub(in crate) use self::element::*;
    pub(in crate) use self::loader::*;
}

#[allow(unused_imports)]
pub(in crate) mod inner {
    pub(in crate) use std::{
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
    pub(in crate) use typedmap::TypedMapKey;

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

    pub(in crate) use crate::{build_class, exception};

    pub(in crate) type ObjCasterMap<CLS> = HashMap<
        &'static str,
        Box<dyn for<'a> Fn(Ctx<'a>, JsClass<'a, Obj<CLS>>) -> Result<Value<'a>>>,
    >;
    pub(in crate) type VirtCasterMap<CLS> = HashMap<
        &'static str,
        Box<dyn for<'a> Fn(Ctx<'a>, JsClass<'a, Virt<CLS>>) -> Result<Value<'a>>>,
    >;
    pub(in crate) type ElementCasterMap = HashMap<
        &'static str,
        Box<dyn for<'a, 'b> Fn(Ctx<'a>, &'b ElementNode<'a>) -> Result<Value<'a>>>,
    >;
}
