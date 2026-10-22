//! LD1: what a host adds to module loading: the app's native modules, extra schemes (SV22) and the
//! `svelters:app/*` visibility filter (SV25).

use std::rc::Rc;
use svelters_modules::{ComposedProvider, ModuleProvider, NativeModules, NativeRegistry, SchemeHook, Visibility, APP_PREFIX, CORE_ID};

/// The statically registered exports of this binary (the E0 `inventory` modules) and the object
/// core they import, as native modules.
pub(crate) struct ExportModules;

impl NativeModules for ExportModules {
    fn ids(&self) -> Vec<String> {
        let mut ids = vec![CORE_ID.to_string()];
        ids.extend(crate::exports::schema().modules.iter().map(|m| format!("{APP_PREFIX}{}", m.name)));
        ids
    }

    fn source(&self, id: &str) -> Option<String> {
        crate::exports::builtin_module(id)
    }
}

/// Per-host module loading additions, set with `Host::set_module_hooks`.
#[derive(Clone)]
pub struct ModuleHooks {
    natives: NativeRegistry,
    schemes: Vec<Rc<dyn SchemeHook>>,
    visibility: Visibility,
}

impl Default for ModuleHooks {
    fn default() -> Self {
        let mut natives = NativeRegistry::new();
        natives.add(ExportModules);
        ModuleHooks { natives, schemes: Vec::new(), visibility: Visibility::All }
    }
}

impl ModuleHooks {
    /// Serves `scheme:` imports through `hook` (SV22). `svelters` is reserved.
    pub fn scheme(mut self, hook: Rc<dyn SchemeHook>) -> Self {
        self.schemes.push(hook);
        self
    }

    /// Restricts the `svelters:app/*` modules imports may see (SV25).
    pub fn visibility(mut self, v: Visibility) -> Self {
        self.visibility = v;
        self
    }

    /// Adds a set of native modules (a dlopen'd library's, or an embedder's).
    pub fn natives(mut self, set: impl NativeModules + 'static) -> Self {
        self.natives.add(set);
        self
    }

    pub(crate) fn compose(&self, inner: Rc<dyn ModuleProvider>) -> Result<Rc<dyn ModuleProvider>, String> {
        let mut p = ComposedProvider::new(inner).natives(self.natives.clone()).visibility(self.visibility.clone());
        for h in &self.schemes {
            p = p.hook(h.clone())?;
        }
        Ok(Rc::new(p))
    }
}
