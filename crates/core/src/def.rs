use anyhow::{anyhow, Result};
use phf::phf_map;

#[derive(Clone, Copy)]
pub struct Event {
    pub name: &'static str,
    pub connect: &'static ConnectFn,
}

#[derive(Clone, Copy)]
pub struct Prop {
    pub name: &'static str,
    pub setter: Option<&'static SetterFn>,
    pub getter: &'static GetterFn,
    // pub binder: Option<&'static BinderFn>,
}

impl Default for Prop {
    fn default() -> Prop {
        Prop {
            name: "",
            getter: &() as &GetterFn,
            setter: None,
            // binder: None,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Method {
    pub name: &'static str,
    pub func: &'static MethodFn,
}

pub type PropMap = phf::Map<&'static str, Prop>;
pub type EventMap = phf::Map<&'static str, Event>;
pub type MethodMap = phf::Map<&'static str, Method>;
pub type WidgetMap = phf::Map<&'static str, &'static Widget>;

pub type Constructor = ();
pub type ConnectFn = ();
pub type GetterFn = ();
pub type SetterFn = ();
pub type MethodFn = ();
pub type BinderFn = ();

pub struct FrameworkSchema {
    widgets: WidgetMap,
}

pub struct Widget {
    pub name: &'static str,
    pub constructor: Option<&'static Constructor>,
    pub superclasses: &'static [&'static Widget],
    pub props: PropMap,
    pub events: EventMap,
    pub methods: MethodMap,
}

impl Default for Widget {
    fn default() -> Widget {
        Widget {
            name: "",
            constructor: None,
            superclasses: &[],
            props: phf_map! {},
            events: phf_map! {},
            methods: phf_map! {},
        }
    }
}

impl Widget {
    pub fn get<T: AsRef<str>>(elements: WidgetMap, name: T) -> Option<&'static Widget> {
        elements.get(name.as_ref()).map(|res| *res)
    }

    pub fn event<T: AsRef<str>>(&self, name: T) -> Option<&'static ConnectFn> {
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

    pub fn method<T: AsRef<str>>(&self, name: T) -> Option<&'static MethodFn> {
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

    pub fn getter<T: AsRef<str>>(&self, name: T) -> Option<&'static GetterFn> {
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

    pub fn setter<T: AsRef<str>>(&self, name: T) -> Option<&'static SetterFn> {
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

    pub fn parents(&self) -> impl Iterator<Item = &'static Widget> {
        self.superclasses.clone().into_iter().map(unref)
    }
}

pub fn get_class<'a>(elements: WidgetMap, tag: &'a str) -> Result<&'static Widget> {
    Widget::get(elements, tag).ok_or_else(|| anyhow!("not_found(tag)"))
}

pub fn get_getter<'a>(
    elements: WidgetMap,
    tag: &'a str,
    name: &'a str,
) -> Result<&'static GetterFn> {
    let cls = get_class(elements, tag)?;
    cls.getter(name).ok_or_else(|| anyhow!("prop(tag, name)"))
}

pub fn get_setter<'a>(
    elements: WidgetMap,
    tag: &'a str,
    name: &'a str,
) -> Result<&'static SetterFn> {
    let cls = get_class(elements, tag)?;
    cls.setter(name).ok_or_else(|| anyhow!("setter(tag, name)"))
}

pub fn get_method<'a>(
    elements: WidgetMap,
    tag: &'a str,
    name: &'a str,
) -> Result<&'static MethodFn> {
    let cls = get_class(elements, tag)?;
    cls.method(name).ok_or_else(|| anyhow!("method(tag, name)"))
}

pub fn get_event<'a>(
    elements: WidgetMap,
    tag: &'a str,
    name: &'a str,
) -> Result<&'static ConnectFn> {
    let cls = get_class(elements, tag)?;
    cls.event(name).ok_or_else(|| anyhow!("event(tag, name)"))
}

fn unref<'a, T>(ptr: &'a &'static T) -> &'static T {
    *ptr
}
