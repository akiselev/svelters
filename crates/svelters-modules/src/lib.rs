//! The svelters module loader (Q1, `plans/research/QUICKJS-RUNTIME.md` §3).
//!
//! A [`ModuleProvider`] is what a JS engine asks for modules: it resolves specifiers to module ids
//! and returns each module's code. It knows nothing about the engine; `svelters-host`'s QuickJS
//! adapter (`quickjs.rs`) is a thin `Resolver`/`Loader` over it. Two providers:
//!
//! - [`BundleProvider`]: a directory with a built bundle (`dist/app.js`, from Vite), the M2 path.
//!   Imports are relative and stay inside the directory; source maps are the `.map` files next to it.
//! - [`SourceProvider`] (feature `source`): the app's sources compiled live. `oxc_resolver` resolves
//!   (Node ESM rules, the runtime package's alias table), `svelters-compiler` compiles `.svelte` and
//!   `.svelte.js`, JSON and assets become modules, source maps stay in memory. Its module graph
//!   decides which file changes reload the app, and its [`Overlay`] serves unsaved editor buffers.
//!
//! Host built-ins (`svelters:core`, `svelters:app/*`) are served by the host before a provider is
//! asked.

use std::path::{Path, PathBuf};

mod bundle;
mod compose;
#[cfg(feature = "dlopen")]
mod dylib;
mod native;
#[cfg(feature = "source")]
mod source;

pub use bundle::BundleProvider;
pub use compose::{ComposedProvider, SchemeHook, Visibility};
#[cfg(feature = "dlopen")]
pub use dylib::DylibModules;
pub use native::{NativeModules, NativeRegistry, StaticModules, APP_PREFIX, CORE_ID};
#[cfg(feature = "source")]
pub use source::{Check, Entry, Overlay, SourceConfig, SourceProvider};

/// A module's name in the engine: what [`ModuleProvider::resolve`] returns and
/// [`ModuleProvider::load`] takes. Source modules use their absolute path.
pub type ModuleId = String;

/// A module's code.
#[derive(Clone, Debug)]
pub enum ModuleCode {
    /// JavaScript (an ES module) and its source map (JSON), if it has one.
    Source { js: String, map: Option<String> },
    /// Precompiled bytecode. Only the engine named by `engine` reads it (Q2/Q4 caches).
    Cached { engine: &'static str, bytes: Vec<u8> },
}

/// Where an engine gets its modules from.
///
/// Called on the engine's thread, from inside module loading: implementations use interior
/// mutability. Errors are messages for the user (compiler diagnostics already rendered).
pub trait ModuleProvider {
    /// The id of the entry module.
    fn entry(&self) -> ModuleId;

    /// Resolves `specifier`, imported by module `base`.
    fn resolve(&self, base: &str, specifier: &str) -> Result<ModuleId, String>;

    /// The code of module `id`.
    fn load(&self, id: &str) -> Result<ModuleCode, String>;

    /// A fresh load starts (a new runtime): the graph's edges are rebuilt by the load that follows.
    fn begin(&self) {}

    /// The directory whose `<module>.map` files map this provider's modules (a bundle's), or `None`
    /// when maps come with [`ModuleCode::Source`].
    fn map_dir(&self) -> Option<PathBuf> {
        None
    }

    /// The files whose change reloads the app (the module graph's files, the package manifests).
    fn watch_files(&self) -> Vec<PathBuf>;

    /// Called with changed paths (watcher events); true when any of them affects the loaded app,
    /// so a reload is due. Drops what the change makes stale (resolver caches).
    fn invalidate(&self, changed: &[PathBuf]) -> bool;

    /// Warnings collected since the last call (compiler warnings, skipped styles), rendered.
    fn take_warnings(&self) -> Vec<String> {
        Vec::new()
    }

    /// What is loaded, for log lines (`dist/app.js`, `src/App.svelte`).
    fn describe(&self) -> String;
}

/// `path` with `.` and `..` folded, without touching the file system.
pub(crate) fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            c => out.push(c),
        }
    }
    out
}
