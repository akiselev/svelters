use std::collections::{hash_map::Entry, HashMap, HashSet};

use crate::{constants::IMPLEMENTED_TYPES, types::*};

pub struct GetSimpleSignals {
    pub include_abstract: bool,
    pub signals: Vec<Class>,
}

struct GetSimpleSignalsInner {
    methods: Vec<Method>,
}

impl Default for GetSimpleSignals {
    fn default() -> GetSimpleSignals {
        GetSimpleSignals {
            signals: vec![],
            include_abstract: true,
        }
    }
}

impl Visitor for GetSimpleSignals {
    fn visit_class(&mut self, cls: &Class) {
        let mut inner = GetSimpleSignalsInner { methods: vec![] };
        inner.on_class(cls);
        let cls = Class {
            def: cls.def.clone(),
            methods: inner.methods,
            inherits: cls.inherits.clone(),
        };
        self.signals.push(cls);
    }
}

impl Visitor for GetSimpleSignalsInner {
    fn visit_signal(&mut self, method: &Method) {
        let mut is_simple = true;
        for arg in method.args.iter() {
            if arg._type.starts_with("Q") && !IMPLEMENTED_TYPES.contains(&arg._type.as_str()) {
                is_simple = false;
            }
        }
        if is_simple {
            self.methods.push(method.clone());
        }
    }
}

pub struct ArgTypes {
    pub include_abstract: bool,
    pub types: HashSet<String>,
}

impl Default for ArgTypes {
    fn default() -> ArgTypes {
        ArgTypes {
            types: HashSet::new(),
            include_abstract: true,
        }
    }
}

impl Visitor for ArgTypes {
    fn visit_signal(&mut self, method: &Method) {
        for item in method.args.iter() {
            self.types.insert(item._type.clone());
        }
    }
}
