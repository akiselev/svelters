use phf::phf_map;
use rquickjs::{
    bind, BuiltinLoader, BuiltinResolver, Context, Ctx, Error, FileResolver, Func, Function,
    IntoJs, Module, NativeLoader, Opt, Persistent, RegisteryKey, Rest, Result, Runtime,
    ScriptLoader, Value,
};

use crate::legacy::traits::Framework;

#[derive(Clone, Copy)]
pub struct Event<F: Framework> {
    pub name: &'static str,
    pub connect: &'static ConnectFn<F>,
}

#[derive(Clone, Copy)]
pub struct Prop<F: Framework> {
    pub name: &'static str,
    pub setter: Option<&'static SetterFn<F>>,
    pub getter: &'static GetterFn<F>,
}

#[derive(Clone, Copy)]
pub struct Method<F: Framework> {
    pub name: &'static str,
    pub func: &'static MethodFn<F>,
}

pub type PropMap<F> = phf::Map<&'static str, Prop<F>>;
pub type EventMap<F> = phf::Map<&'static str, Event<F>>;
pub type MethodMap<F> = phf::Map<&'static str, Method<F>>;
pub type ClassMap<F> = phf::Map<&'static str, &'static Class<F>>;

pub type Constructor<F> =
    dyn for<'a> Fn(Ctx<'a>, Vec<Value<'a>>) -> Result<<F as Framework>::Object> + Send + Sync;
pub type ConnectFn<F> = dyn for<'a> Fn(
        Context,
        &'a mut <F as Framework>::Object,
        RegisteryKey,
    ) -> Result<<F as Framework>::Handler>
    + Send
    + Sync;
pub type GetterFn<F> =
    dyn for<'a> Fn(Ctx<'a>, &'a <F as Framework>::Object) -> Result<Value<'a>> + Send + Sync;
pub type SetterFn<F> = dyn for<'a> Fn(Ctx<'a>, &'a mut <F as Framework>::Object, Value<'a>) -> Result<()>
    + Send
    + Sync;
pub type MethodFn<F> = dyn for<'a> Fn(Ctx<'a>, &'a mut <F as Framework>::Object, Vec<Value<'a>>) -> Result<Value<'a>>
    + Send
    + Sync;
pub type InsertFn<F> = dyn for<'a> Fn(
        Ctx<'a>,
        &'a mut <F as Framework>::Object,
        &'a mut <F as Framework>::Object,
        Opt<&'a mut <F as Framework>::Object>,
    ) -> Result<bool>
    + Send
    + Sync;

pub struct FrameworkSchema<F: Framework> {
    pub classes: &'static ClassMap<F>,
}

impl<F: Framework> FrameworkSchema<F> {
    pub fn new(classes: &'static ClassMap<F>) -> Self {
        Self { classes }
    }
}

pub struct Class<F: Framework> {
    pub name: &'static str,
    pub constructor: Option<&'static Constructor<F>>,
    pub superclasses: &'static [&'static Class<F>],
    pub props: PropMap<F>,
    pub events: EventMap<F>,
    pub methods: MethodMap<F>,
    pub insert: Option<&'static InsertFn<F>>,
}

impl<F: Framework> Class<F> {
    pub fn get<T: AsRef<str>>(
        schema: &'static FrameworkSchema<F>,
        name: T,
    ) -> Option<&'static Class<F>> {
        schema.classes.get(name.as_ref()).map(|res| *res)
    }

    pub fn event<T: AsRef<str>>(&self, name: T) -> Option<&'static ConnectFn<F>> {
        self.events
            .get(name.as_ref())
            .cloned()
            .map(|event| event.connect)
            .or_else(|| {
                for parent in self.parents() {
                    if let Some(event) = parent.event(name.as_ref()) {
                        return Some(event);
                    }
                }
                None
            })
    }

    pub fn method<T: AsRef<str>>(&self, name: T) -> Option<&'static MethodFn<F>> {
        self.methods
            .get(name.as_ref())
            .cloned()
            .map(|method| method.func)
            .or_else(|| {
                for parent in self.parents() {
                    if let Some(method) = parent.method(name.as_ref()) {
                        return Some(method);
                    }
                }
                None
            })
    }

    pub fn getter<T: AsRef<str>>(&self, name: T) -> Option<&'static GetterFn<F>> {
        self.props
            .get(name.as_ref())
            .cloned()
            .map(|prop| prop.getter)
            .or_else(|| {
                for parent in self.parents() {
                    if let Some(getter) = parent.getter(name.as_ref()) {
                        return Some(getter);
                    }
                }
                None
            })
    }

    pub fn setter<T: AsRef<str>>(&self, name: T) -> Option<&'static SetterFn<F>> {
        self.props
            .get(name.as_ref())
            .cloned()
            .map(|prop| prop.setter)
            .flatten()
            .or_else(|| {
                for parent in self.parents() {
                    if let Some(setter) = parent.setter(name.as_ref()) {
                        return Some(setter);
                    }
                }
                None
            })
    }

    pub fn implements<'a>(&self, name: &'a str) -> bool {
        for parent in self.parents() {
            if parent.implements(name) {
                return true;
            }
        }
        return name == self.name;
    }

    pub fn parents(&self) -> impl Iterator<Item = &'static Class<F>> {
        self.superclasses.clone().into_iter().map(unref)
    }
}

pub fn get_class<'a, F: Framework>(
    schema: &'static FrameworkSchema<F>,
    tag: &'a str,
) -> Result<&'static Class<F>> {
    Class::get(schema, tag).ok_or_else(|| Error::Exception {
        message: format!("Element named {} does not exist", tag),
        file: "".to_string(),
        line: -1,
        stack: "".to_string(),
    })
}

pub fn get_getter<'a, F: Framework>(
    schema: &'static FrameworkSchema<F>,
    tag: &'a str,
    name: &'a str,
) -> Result<&'static GetterFn<F>> {
    let cls = get_class(schema, tag)?;
    cls.getter(name).ok_or_else(|| Error::Exception {
        message: format!("Getter for prop {} on element {} not found", name, tag),
        file: "".to_string(),
        line: -1,
        stack: "".to_string(),
    })
}

pub fn get_setter<'a, F: Framework>(
    schema: &'static FrameworkSchema<F>,
    tag: &'a str,
    name: &'a str,
) -> Result<&'static SetterFn<F>> {
    let cls = get_class(schema, tag)?;
    cls.setter(name).ok_or_else(|| Error::Exception {
        message: format!("Setter for prop {} on element {} not found", name, tag),
        file: "".to_string(),
        line: -1,
        stack: "".to_string(),
    })
}

pub fn get_method<'a, F: Framework>(
    schema: &'static FrameworkSchema<F>,
    tag: &'a str,
    name: &'a str,
) -> Result<&'static MethodFn<F>> {
    let cls = get_class(schema, tag)?;
    cls.method(name).ok_or_else(|| Error::Exception {
        message: format!("Method {} on element {} not found", name, tag),
        file: "".to_string(),
        line: -1,
        stack: "".to_string(),
    })
}

pub fn get_event<'a, F: Framework>(
    schema: &'static FrameworkSchema<F>,
    tag: &'a str,
    name: &'a str,
) -> Result<&'static ConnectFn<F>> {
    let cls = get_class(schema, tag)?;
    cls.event(name).ok_or_else(|| Error::Exception {
        message: format!("Eventt {} on element {} not found", name, tag),
        file: "".to_string(),
        line: -1,
        stack: "".to_string(),
    })
}

pub fn get_insert<'a, F: Framework>(
    schema: &'static FrameworkSchema<F>,
    tag: &'a str,
) -> Result<&'static InsertFn<F>> {
    let cls = get_class(schema, tag)?;
    cls.insert.ok_or_else(|| Error::Exception {
        message: format!("Element {} does not support widgets", tag),
        file: "".to_string(),
        line: -1,
        stack: "".to_string(),
    })
}

fn unref<'a, T>(ptr: &'a &'static T) -> &'static T {
    *ptr
}
