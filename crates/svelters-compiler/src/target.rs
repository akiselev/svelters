//! [`RuntimeTarget`]: what an emitted module runs on (a JS engine plus the host that provides
//! `__svelters_host`), next to the GUI platform ([`PlatformSemantics`](crate::semantics::PlatformSemantics)).
//! Deliberately thin (plans/research/REACTIVITY.md sections 9.2 and 9.5, decision 13 #11): the emitted
//! component JS is the same on QuickJS and Node today, so a target only names the runtime imports,
//! maps module specifiers and describes its dev hooks. Engine capabilities join when the first one
//! changes generated code.

use std::borrow::Cow;

/// The modules generated code imports as `$` (the reactive core) and `$n` (the native runtime).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeImports {
    pub core: String,
    pub native: String,
}

impl Default for RuntimeImports {
    fn default() -> Self {
        RuntimeImports { core: "@svelters/native-runtime/core".into(), native: "@svelters/native-runtime".into() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hmr {
    /// Vite's `import.meta.hot`.
    Vite,
    /// The host reloads the bundle in-process (M2b).
    HostReload,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceMaps {
    /// Next to the bundle (`app.js.map`).
    File,
    /// Kept in memory by the dev loader.
    InMemory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DevHooks {
    pub hmr: Hmr,
    pub source_maps: SourceMaps,
}

pub trait RuntimeTarget {
    /// `quickjs-host`, `node-vite`, `node-testhost`.
    fn id(&self) -> &str;
    fn runtime_imports(&self) -> RuntimeImports {
        RuntimeImports::default()
    }
    /// A module specifier as this target resolves it: `svelters:gtk/gio` is kept for the QuickJS
    /// loader's built-in modules; a target without them maps it to a package path.
    fn specifier<'a>(&self, s: &'a str) -> Cow<'a, str> {
        Cow::Borrowed(s)
    }
    fn dev(&self) -> DevHooks;
}

/// The svelters host: QuickJS-NG with built-in `svelters:*` modules, reloaded in-process.
#[derive(Debug, Clone, Copy, Default)]
pub struct QuickJsHost;

impl RuntimeTarget for QuickJsHost {
    fn id(&self) -> &str {
        "quickjs-host"
    }
    fn dev(&self) -> DevHooks {
        DevHooks { hmr: Hmr::HostReload, source_maps: SourceMaps::File }
    }
}

/// Node with Vite (the build plugin, the Node TestHost): the same imports, Vite's HMR.
#[derive(Debug, Clone, Copy, Default)]
pub struct NodeVite;

impl RuntimeTarget for NodeVite {
    fn id(&self) -> &str {
        "node-vite"
    }
    fn dev(&self) -> DevHooks {
        DevHooks { hmr: Hmr::Vite, source_maps: SourceMaps::InMemory }
    }
}

/// A built-in target by id.
pub fn builtin_target(id: &str) -> Option<&'static dyn RuntimeTarget> {
    match id {
        "quickjs-host" => Some(&QuickJsHost),
        "node-vite" | "node-testhost" => Some(&NodeVite),
        _ => None,
    }
}
