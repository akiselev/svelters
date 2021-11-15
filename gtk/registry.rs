use std::{ffi::CString, ptr};

use rquickjs::{
    qjs, ClassId, Ctx, Function, IntoJs, Object as JsObject, RegisteryKey, Rest, TypedMapKey, Value,
};

use crate::{common::*, core::from_gtype::init_gtype};

pub trait Registry<'js> {
    fn marshall<T: IsA<GObject>>(self, obj: T) -> Result<Value<'js>>;

    fn initialize(self, r#type: glib::Type) -> Result<()>;

    fn set_proto<FN: 'static + for<'x> Fn(Ctx<'x>, &JsObject<'x>) -> Result<()>>(
        self,
        r#type: glib::Type,
        func: FN,
    );
    fn set_constructor<FN: IntoJs<'js>>(self, r#type: glib::Type, func: FN);
    fn set_default_constructor(self, r#type: glib::Type);
}

impl<'js> Registry<'js> for Ctx<'js> {
    fn marshall<T: IsA<GObject> + Cast>(self, obj: T) -> Result<Value<'js>> {
        let obj = obj.upcast::<GObject>();
        let ty = obj.type_();
        let obj = crate::Object::new(obj);
        let ty = Type::get(&self, ty);

        rquickjs::WithProto(obj, ty.proto(self)?).into_js(self)
    }

    fn set_proto<FN: 'static + for<'x> Fn(Ctx<'x>, &JsObject<'x>) -> Result<()>>(
        self,
        r#type: glib::Type,
        func: FN,
    ) {
        let iface = Iface::get(&self, r#type);
        iface.set_proto = Some(Box::new(func));
    }

    fn set_constructor<FN: IntoJs<'js>>(self, r#type: glib::Type, func: FN) {
        let iface = Iface::get(&self, r#type);
        let key = self.register(func.into_js(self).expect("Failed to create constructor"));
        iface.constructor = Some(key);
    }

    fn set_default_constructor(self, r#type: glib::Type) {
        let constructor = Function::new(
            self,
            rquickjs::Class::<'js, crate::Object>::constructor(move |ctx: Ctx<'js>| {
                let obj = GObject::with_type(r#type, &[])
                    .map_err(|err| exception!("Could not create {}: {}", r#type.name(), err))?;
                ctx.marshall(obj)
            }),
        )
        .expect("Failed to create constructor");
        // constructor.set_constructor(true);
        self.set_constructor(r#type, constructor);
    }

    fn initialize(self, r#type: glib::Type) -> Result<()> {
        let ty = Type::get(&self, r#type);
        let proto = ty.proto(self)?;
        let constructor = proto
            .get::<_, Value<'js>>("constructor")
            .unwrap_or_else(|_| proto.into_value());
        tracing::info!("Setting global {} to proto", r#type.name());
        self.globals().set(r#type.name(), constructor)
    }
}

#[derive(Clone)]
struct Type {
    r#type: glib::Type,
    proto_key: Option<RegisteryKey>,
}

impl Type {
    fn get<'js: 'a, 'a>(ctx: &'a Ctx<'js>, r#type: glib::Type) -> &'a mut Self {
        if !r#type.is_a(GObject::static_type()) {
            panic!("Type {} is not a GObject", r#type.name());
        }

        let key = TypeKey(r#type);
        if let Some(desc) = ctx.get_typed_mut(&key) {
            return desc;
        }
        let ty = Self {
            r#type,
            proto_key: None,
        };

        ctx.insert_typed(key.clone(), ty.clone());

        ctx.get_typed_mut(&key).unwrap()
    }

    pub fn name(&self) -> &str {
        self.r#type.name()
    }

    pub fn proto<'js>(&mut self, ctx: Ctx<'js>) -> Result<JsObject<'js>> {
        if let Some(key) = self.proto_key {
            return Ok(ctx
                .get_register(key)
                .expect("Proto key set but not found in registery")
                .into_object()
                .unwrap());
        }

        // interfaces implemented by the object
        let proto = rquickjs::Object::new(ctx)?;
        for iface in self.r#type.interfaces().into_iter() {
            if let Some(iface) = ctx.get_typed(&InterfaceKey(iface)) {
                if let Some(set_proto) = &iface.set_proto {
                    set_proto(ctx, &proto)?;
                }
            }
        }

        // parent class methods and stuff
        let mut parent = self.r#type.parent();
        while let Some(parent_type) = parent {
            if let Some(cls) = ctx.get_typed(&InterfaceKey(parent_type)) {
                if let Some(set_proto) = &cls.set_proto {
                    set_proto(ctx, &proto)?;
                }
            }
            parent = parent_type.parent();
        }

        // the main type
        if let Some(cls) = ctx.get_typed(&InterfaceKey(self.r#type)) {
            if let Some(set_proto) = &cls.set_proto {
                set_proto(ctx, &proto)?;
            }
        }
        if self.r#type.is_a(GObject::static_type()) {
            init_gtype(ctx, &proto, self.r#type)?;
        }

        let ty = self.r#type;
        let constructor = Function::new(ctx, move |ctx: Ctx<'js>, args: Rest<Value<'js>>| {
            let iface = Iface::get(&ctx, ty);
            if let Some(key) = &iface.constructor {
                let func = ctx
                    .get_register(*key)
                    .expect("Could not find constructor in registery")
                    .into_function()
                    .expect("Constructor is not a function");
                let args = args.0;
                let len = args.len();
                let mut args = args.into_iter();
                match len {
                    0 => func.call::<_, Value<'js>>(()),
                    1 => func.call::<_, Value<'js>>((args.next(),)),
                    2 => func.call::<_, Value<'js>>((args.next(), args.next())),
                    3 => func.call::<_, Value<'js>>((args.next(), args.next(), args.next())),
                    4 => func.call::<_, Value<'js>>((
                        args.next(),
                        args.next(),
                        args.next(),
                        args.next(),
                    )),
                    5 => func.call::<_, Value<'js>>((
                        args.next(),
                        args.next(),
                        args.next(),
                        args.next(),
                        args.next(),
                    )),
                    6 => func.call::<_, Value<'js>>((
                        args.next(),
                        args.next(),
                        args.next(),
                        args.next(),
                        args.next(),
                        args.next(),
                    )),
                    len => Err(exception!(
                        "Cannot call constructor with {} arguments (max: 6)",
                        len
                    )),
                }
            } else {
                Err(exception!("{} has no constructor", ty.name()))
            }
        })?;
        constructor.set_constructor(true);
        constructor.set_prototype(&proto);

        self.proto_key = Some(ctx.register(proto.clone().into_value()));
        Ok(proto)
    }
}
struct Iface {
    r#type: glib::Type,
    set_proto: Option<Box<dyn for<'js> Fn(Ctx<'js>, &JsObject<'js>) -> Result<()>>>,
    constructor: Option<RegisteryKey>,
}

impl Iface {
    fn get<'js: 'a, 'a>(ctx: &'a Ctx<'js>, r#type: glib::Type) -> &'a mut Self {
        let key = InterfaceKey(r#type);
        if let Some(desc) = ctx.get_typed_mut(&key) {
            return desc;
        }
        let ty = Self {
            r#type,
            set_proto: None,
            constructor: None,
        };

        ctx.insert_typed(key.clone(), ty);

        ctx.get_typed_mut(&key).unwrap()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct TypeKey(pub glib::types::Type);

impl TypedMapKey for TypeKey {
    type Value = Type;
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct InterfaceKey(pub glib::types::Type);

impl TypedMapKey for InterfaceKey {
    type Value = Iface;
}
