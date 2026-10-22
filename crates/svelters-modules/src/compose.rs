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
    fn permits(&self, module: &str, importer: &str) -> bool {
        match self {
            Visibility::All => true,
            Visibility::Allow(names) => names.iter().any(|n| n == module),
            Visibility::Deny(names) => !names.iter().any(|n| n == module),
            Visibility::Filter(f) => f(module, importer),
        }
    }
}

/// Routing, in order: `svelters:core` and `svelters:app/*` go to the native modules (through the
/// visibility filter); a registered scheme goes to its hook; the rest goes to the inner provider.
pub struct ComposedProvider {
    inner: Rc<dyn ModuleProvider>,
    natives: NativeRegistry,
    hooks: Vec<Rc<dyn SchemeHook>>,
    visibility: Visibility,
}

impl ComposedProvider {
    pub fn new(inner: Rc<dyn ModuleProvider>) -> ComposedProvider {
        ComposedProvider { inner, natives: NativeRegistry::new(), hooks: Vec::new(), visibility: Visibility::All }
    }

    pub fn natives(mut self, natives: NativeRegistry) -> Self {
        self.natives = natives;
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
        self.visibility = v;
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
            if !self.visibility.permits(module, base) || !self.natives.serves(specifier) {
                // The same message for "hidden" and "absent": a filter does not reveal what it hides.
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
            return self.natives.load(id);
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
