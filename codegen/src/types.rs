use std::fmt;

use crate::helpers::get_namespace;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClassDef {
    pub name: String,
    pub path: String,
}

impl ClassDef {
    pub fn namespace(&self) -> &'static str {
        get_namespace(&self.path)
    }
}

impl fmt::Display for ClassDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}::{}", self.namespace(), self.name)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Class {
    pub def: ClassDef,
    pub inherits: Vec<String>,
    pub methods: Vec<Method>,
}

impl fmt::Display for Class {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.def.fmt(f)
    }
}

impl Class {
    pub fn is_abstract(&self) -> bool {
        self.def.name.starts_with("QAbstract")
    }

    pub fn namespace(&self) -> &'static str {
        self.def.namespace()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Method {
    pub name: String,
    pub args: Vec<MethodArg>,
    pub return_type: Option<String>,
    pub tag: Option<String>,
}

impl Method {
    pub fn has_tag(&self, needle: &str) -> bool {
        match &self.tag {
            Some(tag) if tag.contains(needle) => true,
            _ => false,
        }
    }

    pub fn tag_str(&self) -> &str {
        if let Some(tag) = &self.tag {
            tag.as_str()
        } else {
            ""
        }
    }
}

impl fmt::Display for Method {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(_type) = &self.return_type {
            write!(f, "{} ", _type)?;
        }
        write!(f, "{}(", self.name)?;
        for (index, arg) in self.args.iter().enumerate() {
            write!(f, "{}", arg)?;
            if index + 1 < self.args.len() {
                write!(f, ", ")?;
            }
        }
        write!(f, ")")?;
        // if let Some(tag) = &self.tag {
        //     write!(f, "{:>40}", tag)?;
        // }
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MethodArg {
    pub is_const: bool,
    pub is_ptr: bool,
    pub is_ref: bool,
    pub has_default: bool,
    pub has_template: bool,
    pub _type: String,
}

impl fmt::Display for MethodArg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_const {
            write!(f, "const ")?;
        }
        if self.is_ref {
            write!(f, "&")?;
        }
        if self.is_ptr {
            write!(f, "*")?;
        }
        write!(f, "{}", self._type)?;
        if self.has_template {
            write!(f, "<...>")?;
        }
        // if self.has_default {
        //     write!(f, "= [...]")?;
        // }
        Ok(())
    }
}

pub trait Visitor {
    fn visit_class_def(&mut self, _: &ClassDef) {}
    fn visit_class(&mut self, _: &Class) {}
    fn visit_method(&mut self, _: &Method) {}
    fn visit_signal(&mut self, _: &Method) {}
    fn visit_method_arg(&mut self, _: &MethodArg) {}
    fn after_class_defs(&mut self) {}
    fn after_classes(&mut self) {}
    fn after_methods(&mut self) {}
    fn after_method_args(&mut self) {}

    fn iter_class_def<'a, T: Iterator<Item = &'a ClassDef>>(&mut self, iter: T) {
        for item in iter {
            self.on_class_def(item);
        }
    }

    fn iter_class<'a, T: Iterator<Item = &'a Class>>(&mut self, iter: T) {
        for item in iter {
            self.on_class(item);
        }
    }

    fn iter_method<'a, T: Iterator<Item = &'a Method>>(&mut self, iter: T) {
        for item in iter {
            self.on_method(item);
        }
    }

    fn iter_method_arg<'a, T: Iterator<Item = &'a MethodArg>>(&mut self, iter: T) {
        for item in iter {
            self.on_method_arg(item);
        }
    }

    fn on_class_def(&mut self, node: &ClassDef) {
        self.visit_class_def(node);
        self.after_class_defs();
    }

    fn on_class(&mut self, node: &Class) {
        self.visit_class(node);
        node.methods.iter().for_each(|method| {
            self.on_method(method);
        });
        self.after_classes();
    }

    fn on_method(&mut self, node: &Method) {
        self.visit_method(node);
        if node.has_tag("signal") {
            self.visit_signal(node);
        }
        node.args.iter().for_each(|arg| {
            self.on_method_arg(arg);
        });
        self.after_methods();
    }

    fn on_method_arg(&mut self, node: &MethodArg) {
        self.visit_method_arg(node);
        self.after_method_args();
    }
}

pub struct VisitClass<F: FnMut(&Class)> {
    func: F,
}

impl<F: FnMut(&Class)> Visitor for VisitClass<F> {
    fn visit_class(&mut self, class: &Class) {
        (self.func)(class);
    }
}

pub struct VisitMethod<F: FnMut(&Method)> {
    func: F,
}

impl<F: FnMut(&Method)> Visitor for VisitMethod<F> {
    fn visit_method(&mut self, method: &Method) {
        (self.func)(method);
    }
}

pub struct VisitMethodArg<F: FnMut(&MethodArg)> {
    func: F,
}

impl<F: FnMut(&MethodArg)> Visitor for VisitMethodArg<F> {
    fn visit_method_arg(&mut self, class: &MethodArg) {
        (self.func)(class);
    }
}
