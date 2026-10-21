//! The non-GUI classes of `svelters:<tk>/<ns>` modules as the compiler needs them for
//! `native_read_not_observable` (REACTIVITY.md section 8.4): which class an import names and the
//! observability of its getters. Read from `metadata/semantics/types/<tk>.json`
//! (`tools/metadata/semantics/gen_observe_outputs.py`).

use super::observe::Observability;
use super::spec::{ObsClass, ObsTiming, ValueCost};
use serde::Deserialize;
use std::collections::BTreeMap;

pub const TYPES_SCHEMA: &str = "svelters-semantics-types/v0";

/// A class exported by a `svelters:` module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeRef {
    pub module: String,
    pub export: String,
    /// The observe facts' owner key (`Gtk:TextBuffer`, `QtCore:QTimer`).
    pub key: String,
}

/// A getter of a class (own or inherited) and its observability (`None`: no fact, unknown).
#[derive(Debug, Clone, PartialEq)]
pub struct TypeProp {
    pub js: String,
    pub native: String,
    /// The class that declares it.
    pub owner: String,
    pub observability: Option<Observability>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawProp {
    native: String,
    class: ObsClass,
    #[serde(default)]
    missed: Vec<String>,
    #[serde(default)]
    value_cost: ValueCost,
}

#[derive(Debug, Clone, Deserialize)]
struct RawType {
    #[serde(default)]
    parents: Vec<String>,
    #[serde(default)]
    props: BTreeMap<String, RawProp>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TypeTable {
    schema: String,
    pub toolkit: String,
    /// module -> export -> owner key
    modules: BTreeMap<String, BTreeMap<String, String>>,
    types: BTreeMap<String, RawType>,
}

impl TypeTable {
    pub fn parse(json: &str) -> Result<TypeTable, String> {
        let t: TypeTable = serde_json::from_str(json).map_err(|e| format!("type table: {e}"))?;
        if t.schema != TYPES_SCHEMA {
            return Err(format!("type table: schema {:?}, expected {TYPES_SCHEMA}", t.schema));
        }
        Ok(t)
    }

    pub fn native_type(&self, module: &str, export: &str) -> Option<TypeRef> {
        let key = self.modules.get(module)?.get(export)?;
        Some(TypeRef { module: module.to_string(), export: export.to_string(), key: key.clone() })
    }

    /// `prop` (a getter name) of `ty` or of its nearest parent that has it (depth first, extends first).
    pub fn type_property(&self, ty: &TypeRef, prop: &str) -> Option<TypeProp> {
        let mut stack = vec![ty.key.clone()];
        let mut seen = std::collections::HashSet::new();
        while let Some(k) = stack.pop() {
            if !seen.insert(k.clone()) {
                continue;
            }
            let Some(t) = self.types.get(&k) else { continue };
            if let Some(p) = t.props.get(prop) {
                return Some(TypeProp {
                    js: prop.to_string(),
                    native: p.native.clone(),
                    owner: k,
                    observability: Some(Observability {
                        class: p.class,
                        missed: p.missed.clone(),
                        coarse: false,
                        value_cost: p.value_cost,
                        timing: ObsTiming::Sync,
                        mechanism: String::new(),
                        // native reads only use the class (diagnostics); the type tables carry no strength
                        verified: Default::default(),
                        fact: String::new(),
                    }),
                });
            }
            stack.extend(t.parents.iter().rev().cloned());
        }
        None
    }
}
