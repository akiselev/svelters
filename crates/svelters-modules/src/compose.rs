//! A provider that routes by scheme: native modules, host-registered schemes, then the inner
//! provider (SV22, SV25).

use crate::native::{NativeRegistry, APP_PREFIX, CORE_ID};
use crate::{ModuleCode, ModuleId, ModuleProvider};
use std::path::PathBuf;
use std::rc::Rc;

/// A host-registered scheme (`cadatomic:`), served synchronously because QuickJS's loader is.
pub trait SchemeHook {
    /// The scheme without the colon (`cadatomic`).
    fn scheme(&self) -> &str;

    /// Maps `specifier` (it starts with `<scheme>:`), imported by `base`, to a module id. The
    /// default is the specifier itself.
    fn resolve(&self, _base: &str, specifier: &str) -> Result<ModuleId, String> {
        Ok(specifier.to_string())
    }

    /// The code of module `id`.
    fn load(&self, id: &str) -> Result<ModuleCode, String>;
}

/// Which `svelters:app/*` modules an importer may see (SV25). `svelters:core` is always visible,
/// because the generated modules import it.
#[derive(Clone)]
pub enum Visibility {
    All,
    /// Only these module names (`task` for `svelters:app/task`).
    Allow(Vec<String>),
    /// Everything but these.
    Deny(Vec<String>),
    /// Decided per import: `(module name, importer id)`.
    Filter(Rc<dyn Fn(&str, &str) -> bool>),
}

impl Visibility {
    /// Whether `module` (`task` for `svelters:app/task`) is visible to `importer`. The ops path
    /// (`construct`, `callStatic`) has no importer and passes `""`.
    pub fn permits(&self, module: &str, importer: &str) -> bool {
        match self {
            Visibility::All => true,
            Visibility::Allow(names) => names.iter().any(|n| n == module),
            Visibility::Deny(names) => !names.iter().any(|n| n == module),
            Visibility::Filter(f) => f(module, importer),
        }
    }
}

/// Which native modules from added sets (a dlopen'd library's, an embedder's; not the app's own
/// exports, which `Visibility` governs) an importer may load.
#[derive(Clone)]
pub enum NativeAccess {
    All,
    /// Only these ids (`svelters:app/extra`).
    Allow(Vec<String>),
    /// None.
    Deny,
}

impl NativeAccess {
    fn permits(&self, id: &str) -> bool {
        match self {
            NativeAccess::All => true,
            NativeAccess::Allow(ids) => ids.iter().any(|i| i == id),
            NativeAccess::Deny => false,
        }
    }
}

/// What a host's modules may reach: the first fields of the extension security policy
/// (ARCH-SECURITY, SEC0, designs the rest). Deny by default for an extension or isolate host
/// ([`ExtensionPolicy::deny_all`]); the primary host uses [`ExtensionPolicy::open`].
///
/// SEC0 slot: a file-system scope. `SourceProvider` loads any path it resolves, so an extension
/// host must reach it only through a [`ComposedProvider`] carrying this policy, never directly.
#[derive(Clone)]
#[non_exhaustive]
pub struct ExtensionPolicy {
    /// The `svelters:app/*` modules an import may see (SV25).
    pub visibility: Visibility,
    /// The native modules of added sets an import may load.
    pub native_modules: NativeAccess,
}

impl ExtensionPolicy {
    /// The primary host: nothing is restricted.
    pub fn open() -> ExtensionPolicy {
        ExtensionPolicy { visibility: Visibility::All, native_modules: NativeAccess::All }
    }

    /// An extension or isolate host: no app module and no added native module is visible.
    pub fn deny_all() -> ExtensionPolicy {
        ExtensionPolicy { visibility: Visibility::Allow(Vec::new()), native_modules: NativeAccess::Deny }
    }

    pub fn visibility(mut self, v: Visibility) -> Self {
        self.visibility = v;
        self
    }

    pub fn native_modules(mut self, n: NativeAccess) -> Self {
        self.native_modules = n;
        self
    }
}

/// Routing, in order: `svelters:core` and `svelters:app/*` go to the native modules (through the
/// visibility filter); a registered scheme goes to its hook; the rest goes to the inner provider.
pub struct ComposedProvider {
    inner: Rc<dyn ModuleProvider>,
    natives: NativeRegistry,
    extra: NativeRegistry,
    hooks: Vec<Rc<dyn SchemeHook>>,
    policy: ExtensionPolicy,
}

impl ComposedProvider {
    pub fn new(inner: Rc<dyn ModuleProvider>) -> ComposedProvider {
        ComposedProvider { inner, natives: NativeRegistry::new(), extra: NativeRegistry::new(), hooks: Vec::new(), policy: ExtensionPolicy::open() }
    }

    /// The app's own native modules, governed by `policy.visibility`.
    pub fn natives(mut self, natives: NativeRegistry) -> Self {
        self.natives = natives;
        self
    }

    /// Added native sets (dlopen'd, embedder's): governed by `policy.visibility` and then
    /// `policy.native_modules`.
    pub fn extra_natives(mut self, natives: NativeRegistry) -> Self {
        self.extra = natives;
        self
    }

    /// Registers a scheme. `svelters` is reserved, and a scheme registers once.
    pub fn hook(mut self, hook: Rc<dyn SchemeHook>) -> Result<Self, String> {
        let s = hook.scheme();
        if s.is_empty() || s.contains(':') || s == "svelters" {
            return Err(format!("`{s}` is not a usable scheme for a loader hook"));
        }
        if self.hooks.iter().any(|h| h.scheme() == s) {
            return Err(format!("scheme `{s}` is registered twice"));
        }
        self.hooks.push(hook);
        Ok(self)
    }

    pub fn visibility(mut self, v: Visibility) -> Self {
        self.policy.visibility = v;
        self
    }

    pub fn policy(mut self, p: ExtensionPolicy) -> Self {
        self.policy = p;
        self
    }

    fn hook_for(&self, id: &str) -> Option<&Rc<dyn SchemeHook>> {
        let (scheme, _) = id.split_once(':')?;
        self.hooks.iter().find(|h| h.scheme() == scheme)
    }
}

impl ModuleProvider for ComposedProvider {
    fn entry(&self) -> ModuleId {
        self.inner.entry()
    }

    fn resolve(&self, base: &str, specifier: &str) -> Result<ModuleId, String> {
        if specifier == CORE_ID {
            return Ok(specifier.to_string());
        }
        if let Some(module) = specifier.strip_prefix(APP_PREFIX) {
            let visible = self.policy.visibility.permits(module, base);
            let served = self.natives.serves(specifier) || (self.extra.serves(specifier) && self.policy.native_modules.permits(specifier));
            if !visible || !served {
                // The same message for "hidden" and "absent": a filter does not reveal what it hides.
                return Err(format!("no such module: `{specifier}`"));
            }
            return Ok(specifier.to_string());
        }
        // DX1: an added native module outside `svelters:app/*` (`svelters:settings`), gated like the others.
        if self.extra.serves(specifier) {
            if !self.policy.native_modules.permits(specifier) {
                return Err(format!("no such module: `{specifier}`"));
            }
            return Ok(specifier.to_string());
        }
        if let Some(h) = self.hook_for(specifier) {
            return h.resolve(base, specifier);
        }
        self.inner.resolve(base, specifier)
    }

    fn load(&self, id: &str) -> Result<ModuleCode, String> {
        if id == CORE_ID || id.starts_with(APP_PREFIX) {
            // The id passed `resolve`; a load by id alone (a cached graph) is gated again.
            // A module id reached by another route than `resolve` must not load either. A per-import
            // `Filter` needs an importer, so it is checked at `resolve` only.
            if let Some(module) = id.strip_prefix(APP_PREFIX) {
                if !matches!(self.policy.visibility, Visibility::Filter(_)) && !self.policy.visibility.permits(module, "") {
                    return Err(format!("no such module: `{id}`"));
                }
            }
            if id != CORE_ID && !self.natives.serves(id) {
                if !(self.extra.serves(id) && self.policy.native_modules.permits(id)) {
                    return Err(format!("no such module: `{id}`"));
                }
                return self.extra.load(id);
            }
            return self.natives.load(id);
        }
        if self.extra.serves(id) {
            if !self.policy.native_modules.permits(id) {
                return Err(format!("no such module: `{id}`"));
            }
            return self.extra.load(id);
        }
        if let Some(h) = self.hook_for(id) {
            return h.load(id);
        }
        self.inner.load(id)
    }

    fn begin(&self) {
        self.inner.begin()
    }

    fn map_dir(&self) -> Option<PathBuf> {
        self.inner.map_dir()
    }

    fn watch_files(&self) -> Vec<PathBuf> {
        self.inner.watch_files()
    }

    fn invalidate(&self, changed: &[PathBuf]) -> bool {
        self.inner.invalidate(changed)
    }

    fn take_warnings(&self) -> Vec<String> {
        self.inner.take_warnings()
    }

    fn describe(&self) -> String {
        self.inner.describe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::StaticModules;

    struct Inner;
    impl ModuleProvider for Inner {
        fn entry(&self) -> ModuleId {
            "main.js".into()
        }
        fn resolve(&self, _: &str, s: &str) -> Result<ModuleId, String> {
            Ok(format!("inner/{s}"))
        }
        fn load(&self, id: &str) -> Result<ModuleCode, String> {
            Ok(ModuleCode::Source { js: format!("// {id}"), map: None })
        }
        fn watch_files(&self) -> Vec<PathBuf> {
            vec![]
        }
        fn invalidate(&self, _: &[PathBuf]) -> bool {
            false
        }
        fn describe(&self) -> String {
            "inner".into()
        }
    }

    struct Api;
    impl SchemeHook for Api {
        fn scheme(&self) -> &str {
            "cadatomic"
        }
        fn load(&self, id: &str) -> Result<ModuleCode, String> {
            match id {
                "cadatomic:api/doc" => Ok(ModuleCode::Source { js: "export const doc = 1;".into(), map: None }),
                _ => Err(format!("no such module: `{id}`")),
            }
        }
    }

    fn composed(v: Visibility) -> ComposedProvider {
        let mut n = NativeRegistry::new();
        n.add(StaticModules(vec![
            ("svelters:core".into(), "export {}".into()),
            ("svelters:app/task".into(), "export const t = 1".into()),
            ("svelters:app/secret".into(), "export const s = 1".into()),
        ]));
        ComposedProvider::new(Rc::new(Inner)).natives(n).hook(Rc::new(Api)).unwrap().visibility(v)
    }

    #[test]
    fn routes_by_scheme() {
        let p = composed(Visibility::All);
        assert_eq!(p.resolve("main.js", "svelters:app/task").unwrap(), "svelters:app/task");
        assert_eq!(p.resolve("main.js", "cadatomic:api/doc").unwrap(), "cadatomic:api/doc");
        assert_eq!(p.resolve("main.js", "./a.js").unwrap(), "inner/./a.js");
        assert!(matches!(p.load("cadatomic:api/doc").unwrap(), ModuleCode::Source { .. }));
        assert!(p.load("cadatomic:api/nope").is_err());
        assert!(p.load("svelters:app/task").is_ok());
        assert!(p.load("x.js").is_ok());
    }

    #[test]
    fn visibility_hides_app_modules() {
        let p = composed(Visibility::Allow(vec!["task".into()]));
        assert!(p.resolve("main.js", "svelters:app/task").is_ok());
        assert_eq!(p.resolve("main.js", "svelters:app/secret").unwrap_err(), "no such module: `svelters:app/secret`");
        assert!(p.resolve("main.js", "svelters:core").is_ok());
        let p = composed(Visibility::Filter(Rc::new(|m, importer| m != "secret" || importer == "trusted.js")));
        assert!(p.resolve("trusted.js", "svelters:app/secret").is_ok());
        assert!(p.resolve("main.js", "svelters:app/secret").is_err());
    }

    #[test]
    fn load_rechecks_visibility_of_app_exports() {
        let p = composed(Visibility::Allow(vec!["task".into()]));
        assert!(p.load("svelters:app/task").is_ok());
        assert_eq!(p.load("svelters:app/secret").unwrap_err(), "no such module: `svelters:app/secret`");
        assert!(p.load("svelters:core").is_ok());
    }

    #[test]
    fn absent_and_hidden_look_alike() {
        let p = composed(Visibility::Deny(vec!["secret".into()]));
        assert_eq!(
            p.resolve("m", "svelters:app/secret").unwrap_err().replace("secret", "x"),
            p.resolve("m", "svelters:app/x").unwrap_err()
        );
    }

    #[test]
    fn reserved_and_duplicate_schemes_are_refused() {
        struct S(&'static str);
        impl SchemeHook for S {
            fn scheme(&self) -> &str {
                self.0
            }
            fn load(&self, _: &str) -> Result<ModuleCode, String> {
                Err(String::new())
            }
        }
        assert!(ComposedProvider::new(Rc::new(Inner)).hook(Rc::new(S("svelters"))).is_err());
        assert!(ComposedProvider::new(Rc::new(Inner)).hook(Rc::new(S("a"))).unwrap().hook(Rc::new(S("a"))).is_err());
    }
}
