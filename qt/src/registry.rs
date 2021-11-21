use rquickjs::{
    qjs, ClassId, Ctx, Function, IntoJs, Object as JsObject, RegisteryKey, Rest, Result,
    TypedMapKey, Value,
};
use std::ffi::CString;

use crate::common::*;

pub trait Registry<'js> {
    fn marshall<T>(self, obj: Ptr<T>) -> Result<Value<'js>>
    where
        Ptr<QObject>: CastFrom<Ptr<T>>;

    fn construct(self, args: &[Value<'js>], id: TypeId) -> Result<()>;

    // fn initialize(self, meta: Ref<QMetaObject>) -> Result<()>;

    // fn set_proto<FN: 'static + for<'x> Fn(Ctx<'x>, &JsObject<'x>) -> Result<()>>(
    //     self,
    //     meta: Ref<QMetaObject>,
    //     func: FN,
    // );
    // fn set_constructor<FN: IntoJs<'js>>(self, meta: Ref<QMetaObject>, func: FN);
    // fn set_default_constructor(self, meta: Ref<QMetaObject>);
}

impl<'js> Registry<'js> for Ctx<'js> {
    fn marshall<T>(self, ptr: Ptr<T>) -> Result<Value<'js>>
    where
        Ptr<QObject>: CastFrom<Ptr<T>>,
    {
        let obj: Ptr<QObject> = unsafe { ptr.cast_into() };
        if obj.is_null() {
            return Err(exception!("Could not cast into QObject"));
        }
        unsafe {
            let info = TypeInfo::new(ptr)?;
            let ty = Type::get(&self, &info);
            let obj = crate::Object::new(ptr);
            tracing::error!(">>>>> Marshall {}", info.name());
            let obj = obj.into_js(self)?;
            obj.as_object().unwrap().set_prototype(&ty.proto(self)?)?;

            Ok(obj)
        }
    }

    fn construct(self, args: &[Value<'js>], meta: &TypeInfo) -> Result<()> {
        let num_args = args.len();
        for constructor in meta.constructors() {
            if num_args != constructor.num_args() {
                continue;
            }
        }
        Err(exception!(
            "Invalid arguments for {} constructor",
            meta.name()
        ))
    }

    // fn set_proto<FN: 'static + for<'x> Fn(Ctx<'x>, &JsObject<'x>) -> Result<()>>(
    //     self,
    //     meta: Ref<QMetaObject>,
    //     func: FN,
    // ) {
    //     let meta = TypeInfo::new(meta);
    //     let iface = Iface::get(&self, class_name);
    //     iface.set_proto = Some(Box::new(func));
    // }

    // fn set_constructor<FN: IntoJs<'js>>(self, meta: Ref<QMetaObject>, func: FN) {
    //     let meta = TypeInfo::new(meta);
    //     let iface = Iface::get(&self, class_name);
    //     let key = self.register(func.into_js(self).expect("Failed to create constructor"));
    //     if let Some(key) = iface.constructor {
    //         self.deregister(key);
    //     }
    //     iface.constructor = Some(key);
    // }

    // fn set_default_constructor(self, meta: Ref<QMetaObject>) {
    //     let constructor = Function::new(self, move |ctx: Ctx<'js>| {
    //         let meta = TypeInfo::new(meta);
    //         let obj = GObject::with_type(class_name, &[])
    //             .map_err(|err| exception!("Could not create {}: {}", class_name.name(), err))?;
    //         ctx.marshall(obj)
    //     })
    //     .expect("Failed to create constructor");
    //     self.set_constructor(class_name, constructor);
    // }

    // fn initialize(self, meta: Ref<QMetaObject>) -> Result<()> {
    //     let meta = TypeInfo::new(meta);
    //     let ty = Type::get(&self, class_name);
    //     let proto = ty.proto(self)?;
    //     let constructor = proto
    //         .get::<_, Value<'js>>("constructor")
    //         .unwrap_or_else(|_| proto.into_value());
    //     tracing::info!("Setting global {} to proto", class_name.name());
    //     self.globals().set(class_name.name(), constructor)
    // }
}

#[derive(Clone)]
struct Type {
    meta: TypeInfo,
    proto_key: Option<RegisteryKey>,
}

impl Type {
    #[allow(clippy::mut_from_ref)]
    fn get<'js: 'a, 'a>(ctx: &'a Ctx<'js>, meta: &TypeInfo) -> &'a mut Self {
        if !meta.is_object() {
            panic!("Type {} is not a QObject", meta.name());
        }

        let key = TypeId(meta.id().into());
        if let Some(desc) = ctx.get_typed_mut(&key) {
            return desc;
        }
        let ty = Self {
            meta: meta.clone(),
            proto_key: None,
        };

        ctx.insert_typed(key.clone(), ty);

        ctx.get_typed_mut(&key).unwrap()
    }

    pub fn proto<'js>(&mut self, ctx: Ctx<'js>) -> Result<JsObject<'js>> {
        unimplemented!();
        // if let Some(key) = self.proto_key {
        //     return Ok(ctx
        //         .get_register(key)
        //         .expect("Proto key set but not found in registery")
        //         .into_object()
        //         .unwrap());
        // }

        // // interfaces implemented by the object
        // let proto = rquickjs::Object::new(ctx)?;
        // for iface in self.class_name.interfaces().into_iter() {
        //     if let Some(iface) = ctx.get_typed(&InterfaceKey(iface)) {
        //         if let Some(set_proto) = &iface.set_proto {
        //             set_proto(ctx, &proto)?;
        //         }
        //     }
        // }

        // // parent class methods and stuff
        // let mut parent = self.class_name.parent();
        // while let Some(parent_type) = parent {
        //     if let Some(cls) = ctx.get_typed(&InterfaceKey(parent_type)) {
        //         if let Some(set_proto) = &cls.set_proto {
        //             set_proto(ctx, &proto)?;
        //         }
        //     }
        //     parent = parent_type.parent();
        // }

        // // the main type
        // if let Some(cls) = ctx.get_typed(&InterfaceKey(self.class_name)) {
        //     if let Some(set_proto) = &cls.set_proto {
        //         set_proto(ctx, &proto)?;
        //     }
        // }
        // if self.class_name.is_a(GObject::static_type()) {
        //     init_gtype(ctx, &proto, self.class_name)?;
        // }

        // let ty = self.class_name;
        // let constructor = Function::new(ctx, move |ctx: Ctx<'js>, args: Rest<Value<'js>>| {
        //     let iface = Iface::get(&ctx, ty);
        //     if let Some(key) = &iface.constructor {
        //         let func = ctx
        //             .get_register(*key)
        //             .expect("Could not find constructor in registery")
        //             .into_function()
        //             .expect("Constructor is not a function");
        //         let args = args.0;
        //         let len = args.len();
        //         let mut args = args.into_iter();
        //         match len {
        //             0 => func.call::<_, Value<'js>>(()),
        //             1 => func.call::<_, Value<'js>>((args.next(),)),
        //             2 => func.call::<_, Value<'js>>((args.next(), args.next())),
        //             3 => func.call::<_, Value<'js>>((args.next(), args.next(), args.next())),
        //             4 => func.call::<_, Value<'js>>((
        //                 args.next(),
        //                 args.next(),
        //                 args.next(),
        //                 args.next(),
        //             )),
        //             5 => func.call::<_, Value<'js>>((
        //                 args.next(),
        //                 args.next(),
        //                 args.next(),
        //                 args.next(),
        //                 args.next(),
        //             )),
        //             6 => func.call::<_, Value<'js>>((
        //                 args.next(),
        //                 args.next(),
        //                 args.next(),
        //                 args.next(),
        //                 args.next(),
        //                 args.next(),
        //             )),
        //             len => Err(exception!(
        //                 "Cannot call constructor with {} arguments (max: 6)",
        //                 len
        //             )),
        //         }
        //     } else {
        //         Err(exception!("{} has no constructor", ty.name()))
        //     }
        // })?;
        // constructor.set_constructor(true);
        // constructor.set_prototype(&proto);

        // // GC keeps destroying the prototypes and causing use after free
        // let mut class_id = ClassId::new();
        // class_id.init();
        // let class_name = CString::new(self.class_name.name())?;
        // let class_def = qjs::JSClassDef {
        //     class_name: class_name.as_ptr(),
        //     finalizer: None,
        //     gc_mark: Some(gc_mark),
        //     call: None,
        //     exotic: std::ptr::null_mut(),
        // };
        // let context = rquickjs::Context::from_ctx(ctx)?;
        // if 0 != unsafe { qjs::JS_NewClass(context.runtime().ptr(), class_id.get(), &class_def) } {
        //     return Err(rquickjs::Error::Unknown);
        // }
        // unsafe { qjs::JS_SetClassProto(ctx.ctx, class_id.get(), (*proto).clone().into_js_value()) }

        // self.proto_key = Some(ctx.register(proto.clone().into_value()));
        // Ok(proto)
    }
}

unsafe extern "C" fn gc_mark(
    _rt: *mut qjs::JSRuntime,
    _val: qjs::JSValue,
    mark_fn: Option<unsafe extern "C" fn(*mut qjs::JSRuntime, *mut qjs::JSGCObjectHeader)>,
) {
}

type ProtoFn = dyn for<'js> Fn(Ctx<'js>, &JsObject<'js>) -> Result<()>;

#[allow(dead_code)]
struct Iface {
    meta: TypeInfo,
    set_proto: Option<Box<ProtoFn>>,
    constructor: Option<RegisteryKey>,
}

impl Iface {
    #[allow(clippy::mut_from_ref)]
    fn get<'js: 'a, 'a>(ctx: &'a Ctx<'js>, meta: TypeInfo) -> &'a mut Self {
        let key = InterfaceKey(meta.id().0);
        if let Some(desc) = ctx.get_typed_mut(&key) {
            return desc;
        }
        let ty = Self {
            meta,
            set_proto: None,
            constructor: None,
        };

        ctx.insert_typed(key.clone(), ty);

        ctx.get_typed_mut(&key).unwrap()
    }
}

impl TypedMapKey for TypeId {
    type Value = Type;
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct InterfaceKey(pub c_int);

impl TypedMapKey for InterfaceKey {
    type Value = Iface;
}
