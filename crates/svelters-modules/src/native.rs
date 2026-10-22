//! Native modules: the Rust exports a binary registers, served as ES modules.

use crate::{ModuleCode, ModuleId};
use std::rc::Rc;

/// The built-in module that the generated `svelters:app/*` modules import.
pub const CORE_ID: &str = "svelters:core";

/// The prefix of the modules an app binary exports.
pub const APP_PREFIX: &str = "svelters:app/";

/// A set of native modules. `svelters-host` implements it over its statically registered
/// (`inventory`) exports; a dlopen'd library registers into a [`NativeRegistry`] the same way.
pub trait NativeModules {
    /// The ids this set serves (`svelters:core`, `svelters:app/task`).
    fn ids(&self) -> Vec<ModuleId>;

    /// The ES module source of `id`, or `None` when this set does not serve it.
    fn source(&self, id: &str) -> Option<String>;
}

/// Several [`NativeModules`] sets, asked in order of registration.
#[derive(Clone, Default)]
pub struct NativeRegistry {
    sets: Vec<Rc<dyn NativeModules>>,
}

impl NativeRegistry {
    pub fn new() -> NativeRegistry {
        NativeRegistry::default()
    }

    pub fn add(&mut self, set: impl NativeModules + 'static) {
        self.sets.push(Rc::new(set));
    }

    pub fn ids(&self) -> Vec<ModuleId> {
        let mut ids: Vec<ModuleId> = self.sets.iter().flat_map(|s| s.ids()).collect();
        ids.sort();
        ids.dedup();
        ids
    }

    pub fn serves(&self, id: &str) -> bool {
        self.sets.iter().any(|s| s.ids().iter().any(|i| i == id))
    }

    pub fn load(&self, id: &str) -> Result<ModuleCode, String> {
        self.sets
            .iter()
            .find_map(|s| s.source(id))
            .map(|js| ModuleCode::Source { js, map: None })
            .ok_or_else(|| format!("no such module: this binary exports no such module `{id}`"))
    }
}

/// A fixed table of modules, for tests and for hosts that build their modules by hand.
pub struct StaticModules(pub Vec<(String, String)>);

impl NativeModules for StaticModules {
    fn ids(&self) -> Vec<ModuleId> {
        self.0.iter().map(|(id, _)| id.clone()).collect()
    }

    fn source(&self, id: &str) -> Option<String> {
        self.0.iter().find(|(i, _)| i == id).map(|(_, js)| js.clone())
    }
}
