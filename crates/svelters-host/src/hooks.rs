//! LD1: what a host adds to module loading: the app's native modules, extra schemes (SV22) and the
//! `svelters:app/*` visibility filter (SV25).

use std::rc::Rc;
use svelters_modules::{ComposedProvider, ExtensionPolicy, ModuleProvider, NativeAccess, NativeModules, NativeRegistry, SchemeHook, Visibility, APP_PREFIX, CORE_ID};

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
    extra: NativeRegistry,
    schemes: Vec<Rc<dyn SchemeHook>>,
    policy: ExtensionPolicy,
}

impl Default for ModuleHooks {
    fn default() -> Self {
        let mut natives = NativeRegistry::new();
        natives.add(ExportModules);
        ModuleHooks { natives, extra: { let mut e = NativeRegistry::new(); e.add(crate::dx::DxModules); e }, schemes: Vec::new(), policy: ExtensionPolicy::open() }
    }
}

impl ModuleHooks {
    /// The hooks of an isolate or extension host: the same natives, but deny by default
    /// ([`ExtensionPolicy::deny_all`]). No `svelters:app/*` module is visible until
    /// [`ModuleHooks::visibility`] allows some, and no added native module loads until
    /// [`ModuleHooks::native_access`] allows it, so a host nobody configured does not see every
    /// export of the app. `ModuleHooks::default()` is the
    /// primary host: everything visible.
    pub fn isolate() -> ModuleHooks {
        ModuleHooks { policy: ExtensionPolicy::deny_all(), ..ModuleHooks::default() }
    }

    /// Serves `scheme:` imports through `hook` (SV22). `svelters` is reserved.
    pub fn scheme(mut self, hook: Rc<dyn SchemeHook>) -> Self {
        self.schemes.push(hook);
        self
    }

    /// Restricts the `svelters:app/*` modules imports may see (SV25).
    pub fn visibility(mut self, v: Visibility) -> Self {
        self.policy.visibility = v;
        self
    }

    /// Adds a set of native modules (a dlopen'd library's, or an embedder's). Its modules are
    /// subject to the visibility filter and the policy's native-module gate.
    pub fn natives(mut self, set: impl NativeModules + 'static) -> Self {
        self.extra.add(set);
        self
    }

    /// Which added native modules may load (`ExtensionPolicy::native_modules`).
    pub fn native_access(mut self, n: NativeAccess) -> Self {
        self.policy.native_modules = n;
        self
    }

    /// The gate for ops that reach an export without an import: the policy's visibility, with no importer.
    pub(crate) fn app_gate(&self) -> Rc<dyn Fn(&str) -> bool> {
        let v = self.policy.visibility.clone();
        Rc::new(move |module| v.permits(module, ""))
    }

    pub(crate) fn compose(&self, inner: Rc<dyn ModuleProvider>) -> Result<Rc<dyn ModuleProvider>, String> {
        let mut p = ComposedProvider::new(inner).natives(self.natives.clone()).extra_natives(self.extra.clone()).policy(self.policy.clone());
        for h in &self.schemes {
            p = p.hook(h.clone())?;
        }
        Ok(Rc::new(p))
    }
}
