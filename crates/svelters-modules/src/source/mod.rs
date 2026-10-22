//! [`SourceProvider`]: an app loaded from its sources, compiled live (Q1).
//!
//! LD3: the compiler's diagnostic types ([`Diagnostic`], [`Severity`], and the [`FixIt`]/[`TextEdit`]/
//! [`FixHint`] of its fields) are re-exported at the crate root (`svelters_modules::Diagnostic`,
//! `svelters_host::modules::Diagnostic`) with feature `source`, so a host reading [`Check`] needs no
//! direct svelters-compiler dependency.

mod aliases;
mod glob;
pub(crate) mod build;
mod resolve;
mod transform;

use crate::assets::{self, APP_ROOT, Assets};
use crate::{ModuleCode, ModuleId, ModuleProvider, normalize};
use aliases::{Aliases, package_dir};
use resolve::Resolver;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
pub use svelters_compiler::diag::{Diagnostic, FixHint, FixIt, Severity, TextEdit};
use svelters_compiler::semantics::{SpecBackend, kinds_module};
use transform::{Compiler, Kind, Output};

/// The id of the generated entry module (an `App` mount, or an embed entry).
const ENTRY_ID: &str = "svelters:entry";
/// What a component that resolves attributes at run time imports (spreads, `<svelte:element>`).
const KINDS_SPECIFIER: &str = "@svelters/native-runtime/kinds";
const KINDS_ID: &str = "svelters:kinds";
/// LD2: the prefix of a component's sheet module (`svelters-style:<component id>`), which its
/// compiled code imports. Its own module, so a `<style>`-only edit re-runs it alone (H2's
/// self-accepting style module): the sheet is replaced by id and the backend restyles, no remount.
const STYLE_PREFIX: &str = "svelters-style:";

/// What to evaluate first.
#[derive(Clone, Debug)]
pub enum Entry {
    /// A module evaluated as is (`main.js`, a `.svelte.js`).
    Module(PathBuf),
    /// A component mounted at the root (`mount(App, { props: {} })`), as `@svelters/build`'s entry.
    App(PathBuf),
    /// An embed entry (EM1, `svelteConfig({ embed })`): installs `@svelters/native-runtime/embed`
    /// and exports each component under its name; mounts nothing.
    Embed(Vec<(String, PathBuf)>),
}

/// How an app is loaded from source. [`SourceConfig::for_path`] reads it from the app's
/// package.json (`"svelters": { "toolkit", "spec", "specFragments", "embed" }`, the fields
/// `@svelters/build` reads).
#[derive(Clone, Debug)]
pub struct SourceConfig {
    /// The app's directory (its package.json).
    pub root: PathBuf,
    pub entry: Entry,
    /// The toolkit the components are compiled for (`gtk`, `qt`, `appkit`, `winui`, `egui`).
    pub backend: String,
    /// A semantics file instead of the toolkit's built-in one.
    pub spec: Option<PathBuf>,
    /// Spec fragments merged into it (RA1 provider kinds: the `kinds.json` an egui app dumps).
    pub spec_fragments: Vec<PathBuf>,
    /// Svelte's `dev` option, and the `development` export condition instead of `production`.
    pub dev: bool,
    /// Compile components with `--hmr` (with `dev`): `import.meta.hot` wrappers for in-place swaps (H1).
    pub hmr: bool,
    /// `@svelters/native-runtime`'s directory; found from the app when `None`.
    pub runtime_dir: Option<PathBuf>,
    /// Extension confinement (deny by default, `plans/research/ARCH-SECURITY.md` §5): absolute
    /// specifiers are refused, and a path specifier (`./`, `../`) must stay inside the importing
    /// module's package root (symlinks resolved, then re-checked). Off for the primary app host,
    /// on with [`SourceConfig::for_extension`]. Package specifiers still resolve; pinning which
    /// packages and files an extension may load is SEC1/SEC2 (principals, module lock).
    pub confine: bool,
}

impl SourceConfig {
    /// The app at `path`: a directory (its embed entry, or `src/App.svelte`), or a file inside an
    /// app (`.svelte` mounts it, anything else is evaluated as is). The app root is the nearest
    /// directory with a package.json.
    pub fn for_path(path: &Path) -> Result<SourceConfig, String> {
        let path = path.canonicalize().map_err(|e| format!("{}: {e}", path.display()))?;
        let start = if path.is_dir() { path.as_path() } else { path.parent().unwrap_or(&path) };
        let root = start
            .ancestors()
            .find(|d| d.join("package.json").is_file())
            .ok_or_else(|| format!("{}: no package.json in it or above it", path.display()))?
            .to_path_buf();
        let manifest = root.join("package.json");
        let text = std::fs::read_to_string(&manifest).map_err(|e| format!("{}: {e}", manifest.display()))?;
        let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", manifest.display()))?;
        let meta = json.get("svelters").cloned().unwrap_or_default();
        let s = |k: &str| meta.get(k).and_then(|v| v.as_str()).map(str::to_string);
        let entry = if path.is_dir() {
            match meta.get("embed").and_then(|e| e.as_object()) {
                Some(table) => Entry::Embed(
                    table
                        .iter()
                        .map(|(name, p)| Ok((name.clone(), root.join(p.as_str().ok_or_else(|| format!("{}: embed {name:?} is not a path", manifest.display()))?))))
                        .collect::<Result<_, String>>()?,
                ),
                None => Entry::App(path.join("src/App.svelte")),
            }
        } else if Kind::of(&path) == Kind::Component {
            Entry::App(path.clone())
        } else {
            Entry::Module(path.clone())
        };
        Ok(SourceConfig {
            entry,
            backend: s("toolkit").unwrap_or_else(|| "gtk".into()),
            spec: s("spec").map(|p| root.join(p)),
            spec_fragments: meta
                .get("specFragments")
                .and_then(|f| f.as_array())
                .map(|a| a.iter().filter_map(|v| v.as_str()).map(|p| root.join(p)).collect())
                .unwrap_or_default(),
            dev: false,
            hmr: false,
            runtime_dir: None,
            confine: false,
            root,
        })
    }

    /// [`SourceConfig::for_path`] for an extension or isolate host: confined to its packages.
    pub fn for_extension(path: &Path) -> Result<SourceConfig, String> {
        Ok(SourceConfig { confine: true, ..SourceConfig::for_path(path)? })
    }

    /// `@svelters/native-runtime`'s directory: from the app's `node_modules`, through
    /// `@svelters/build`'s (an app built by `svelteConfig` depends on that only), or this
    /// repository's `packages/native-runtime`.
    fn find_runtime(&self) -> Result<PathBuf, String> {
        if let Some(d) = &self.runtime_dir {
            return d.canonicalize().map_err(|e| format!("{}: {e}", d.display()));
        }
        const RUNTIME: &str = "@svelters/native-runtime";
        package_dir(&self.root, RUNTIME)
            .or_else(|| package_dir(&self.root, "@svelters/build").and_then(|b| package_dir(&b, RUNTIME)))
            .or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/native-runtime").canonicalize().ok())
            .ok_or_else(|| format!("{}: {RUNTIME} is not installed (pnpm install)", self.root.display()))
    }
}

/// In-memory file contents by path, read before the disk: an editor's unsaved buffers. Cheap to
/// clone and shareable across threads. Setting or clearing a file does not reload by itself: the
/// next load (a reload, [`SourceProvider::check`]) reads it.
#[derive(Clone, Default)]
pub struct Overlay(Arc<Mutex<HashMap<PathBuf, String>>>);

impl Overlay {
    pub fn set(&self, path: impl AsRef<Path>, text: impl Into<String>) {
        self.0.lock().unwrap().insert(key(path.as_ref()), text.into());
    }

    /// Back to the file on disk.
    pub fn clear(&self, path: impl AsRef<Path>) {
        self.0.lock().unwrap().remove(&key(path.as_ref()));
    }

    pub fn get(&self, path: impl AsRef<Path>) -> Option<String> {
        self.0.lock().unwrap().get(&key(path.as_ref())).cloned()
    }

    /// The overlay's text, else the file's.
    fn read(&self, path: &Path) -> Result<String, String> {
        match self.get(path) {
            Some(t) => Ok(t),
            None => std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display())),
        }
    }
}

/// Overlay keys: canonical when the file exists, else absolute and normalised.
fn key(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| normalize(&std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())))
}

/// The result of [`SourceProvider::check`]: the compiler's diagnostics for one file.
#[derive(Debug)]
pub struct Check {
    pub diagnostics: Vec<Diagnostic>,
    /// A failure that is not a compiler diagnostic (unreadable file, invalid JSON, unsupported kind).
    pub error: Option<String>,
}

impl Check {
    pub fn ok(&self) -> bool {
        self.error.is_none() && !self.diagnostics.iter().any(|d| d.severity == Severity::Error)
    }
}

struct Cached {
    hash: u64,
    js: String,
    map: Option<String>,
    /// LD2: the component's scoped sheet (its `svelters-style:` module registers it).
    css: Option<String>,
}

/// LD2: what an edit of a component changed, by file (`invalidate`), so `modules_at` re-runs only
/// that part: a `<style>`-only edit re-runs the sheet module alone.
#[derive(Clone, Copy)]
struct Changed {
    js: bool,
    css: bool,
}

struct Node {
    path: Option<PathBuf>,
    deps: BTreeSet<ModuleId>,
    /// AS1: an asset ref module (`./icon.svg`, `?url`): constant, so a change of its file is the
    /// asset registry's (no JS runs) and neither `invalidate` nor `modules_at` reports it.
    asset_ref: bool,
}

struct Inner {
    cfg: SourceConfig,
    runtime_dir: PathBuf,
    compiler: Compiler,
    resolver: Resolver,
    aliases: Aliases,
    /// Transform output by file, keyed by a hash of what was transformed.
    cache: HashMap<PathBuf, Cached>,
    /// The module graph of the current load: id -> its file and its imports.
    nodes: BTreeMap<ModuleId, Node>,
    warnings: Vec<String>,
    /// LD2: the components of the last `invalidate` and what changed in them.
    changed: HashMap<PathBuf, Changed>,
    /// AS1: the registry behind `asset://app/…` (`app` is the app root).
    assets: Arc<Assets>,
}

/// An app loaded from source: resolves with Node's ESM rules (`resolve.rs`) and the runtime's alias table, compiles
/// `.svelte`/`.svelte.js` with `svelters-compiler`, keeps source maps in memory (they come with
/// each module's code) and transform output by content, so a reload recompiles only what changed.
pub struct SourceProvider {
    inner: RefCell<Inner>,
    overlay: Overlay,
    assets: Arc<Assets>,
}

impl SourceProvider {
    pub fn new(cfg: SourceConfig) -> Result<SourceProvider, String> {
        let runtime_dir = cfg.find_runtime()?;
        let aliases = Aliases::load(&runtime_dir)?;
        let compiler = Compiler { sem: spec_backend(&cfg)?, dev: cfg.dev, hmr: cfg.hmr && cfg.dev, root: cfg.root.clone() };
        let resolver = resolver(cfg.dev);
        let p = SourceProvider {
            assets: Assets::for_app(&cfg.root),
            inner: RefCell::new(Inner { cfg, runtime_dir, compiler, resolver, aliases, cache: HashMap::new(), nodes: BTreeMap::new(), warnings: Vec::new(), changed: HashMap::new(), assets: Assets::new() }),
            overlay: Overlay::default(),
        };
        p.inner.borrow_mut().assets = p.assets.clone();
        Ok(p)
    }

    /// [`SourceConfig::for_path`] then [`SourceProvider::new`].
    pub fn for_path(path: &Path) -> Result<SourceProvider, String> {
        SourceProvider::new(SourceConfig::for_path(path)?)
    }

    /// AS1: the asset registry (`app` mounted at the app root). Share it with the backend
    /// (`svelters_egui::assets::install`); the host reads it through [`ModuleProvider::assets`].
    pub fn assets(&self) -> &Arc<Assets> {
        &self.assets
    }

    /// The buffers read before the disk. Clone it to share it with an editor.
    pub fn overlay(&self) -> &Overlay {
        &self.overlay
    }

    pub fn config(&self) -> SourceConfig {
        self.inner.borrow().cfg.clone()
    }

    /// Compiles one file (the overlay's text first) for its diagnostics. Touches neither the
    /// running app nor the transform cache.
    pub fn check(&self, path: &Path) -> Check {
        let path = key(path);
        let src = match self.overlay.read(&path) {
            Ok(s) => s,
            Err(e) => return Check { diagnostics: Vec::new(), error: Some(e) },
        };
        let inner = self.inner.borrow();
        let kind = Kind::of(&path);
        match inner.compiler.compile(kind, &path, &src) {
            Some(res) => Check { diagnostics: res.diagnostics, error: None },
            None => Check { diagnostics: Vec::new(), error: inner.compiler.transform(&path, src).err() },
        }
    }

    /// The files of the current module graph (no generated modules).
    pub fn graph_files(&self) -> Vec<PathBuf> {
        self.inner.borrow().nodes.values().filter_map(|n| n.path.clone()).collect()
    }

    /// The modules that import `id` directly (the graph's reverse edges).
    pub fn importers(&self, id: &str) -> Vec<ModuleId> {
        self.inner.borrow().nodes.iter().filter(|(_, n)| n.deps.contains(id)).map(|(k, _)| k.clone()).collect()
    }
}

fn spec_backend(cfg: &SourceConfig) -> Result<SpecBackend, String> {
    let sem = match &cfg.spec {
        Some(p) => SpecBackend::from_path(p)?,
        None => SpecBackend::builtin(&cfg.backend)?,
    };
    cfg.spec_fragments.iter().try_fold(sem, |s, f| s.merge_fragment_path(f))
}

/// Node ESM resolution with Vite's conditions for a client build (`browser`, then the defaults).
fn resolver(dev: bool) -> Resolver {
    Resolver::new(&["browser", "import", "module", if dev { "development" } else { "production" }, "default"], &[".mjs", ".js", ".mts", ".ts", ".json"], &["module", "main"])
}

fn hash_of(parts: &[&str]) -> u64 {
    let mut h = std::hash::DefaultHasher::new();
    parts.hash(&mut h);
    h.finish()
}

impl Inner {
    fn record(&mut self, id: &str, path: Option<PathBuf>) {
        let asset_ref = path.is_some() && matches!(asset_kind(id), Some(AssetKind::Ref));
        self.nodes.entry(id.to_string()).or_insert_with(|| Node { path, deps: BTreeSet::new(), asset_ref });
    }

    fn entry_source(&self) -> String {
        let q = |p: &Path| serde_json::Value::String(p.to_string_lossy().into_owned()).to_string();
        match &self.cfg.entry {
            Entry::Module(p) => format!("export * from {};\n", q(p)),
            Entry::App(p) => format!("import {{ mount }} from '@svelters/native-runtime';\nimport App from {};\nmount(App, {{ props: {{}} }});\n", q(p)),
            Entry::Embed(list) => {
                let mut js = String::from("import '@svelters/native-runtime/embed';\n");
                for (name, p) in list {
                    js.push_str(&format!("export {{ default as {name} }} from {};\n", q(p)));
                }
                js
            }
        }
    }

    fn resolve(&mut self, base: &str, specifier: &str) -> Result<ModuleId, String> {
        let (id, path) = if specifier == KINDS_SPECIFIER {
            (KINDS_ID.to_string(), None)
        } else if let Some(ns) = specifier.strip_prefix("svelters:behaviors/") {
            // EV2b: the app's native behaviours, the module of Svelte actions the app binary dumps
            // (`svelters_egui::dump`), as vite.js maps it.
            if ns.is_empty() || !ns.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-') {
                return Err(format!("{specifier:?}: not a behaviour namespace"));
            }
            let p = self.cfg.root.join(".svelters/app/behaviors").join(format!("{ns}.js"));
            let p = p.canonicalize().map_err(|e| format!("{specifier:?} -> {}: {e} (run the app binary's dump)", p.display()))?;
            (p.to_string_lossy().into_owned(), Some(p))
        } else if let Some(file) = specifier.strip_prefix(STYLE_PREFIX) {
            // LD2: only the component itself imports its sheet (its compiled code ends with the import).
            if unversioned_base(base) != file {
                return Err(format!("cannot find module {specifier:?}: a component's sheet is imported by that component only"));
            }
            (specifier.to_string(), Some(PathBuf::from(file)))
        } else if let Some(p) = self.aliases.get(specifier) {
            let p = p.canonicalize().map_err(|e| format!("alias of {specifier:?} -> {}: {e}", p.display()))?;
            (p.to_string_lossy().into_owned(), Some(p))
        } else {
            let dir = match Path::new(base) {
                b if b.is_absolute() => b.parent().unwrap_or(&self.cfg.root).to_path_buf(),
                _ => self.cfg.root.clone(),
            };
            // The generated entry (host code) names the app's files by absolute path.
            if self.cfg.confine && base != ENTRY_ID && Path::new(specifier).is_absolute() {
                return Err(format!("cannot find module {specifier:?}: absolute imports are refused for extensions"));
            }
            // AS1: Vite's query suffixes on files (`?url`, `?raw`, `?inline`, `?bytes`).
            let (specifier, query) = split_query(specifier)?;
            let p = self.resolver.resolve(&dir, specifier)?;
            if self.cfg.confine && (specifier.starts_with("./") || specifier.starts_with("../")) {
                let scope = dir.ancestors().find(|d| d.join("package.json").is_file()).unwrap_or(&self.cfg.root);
                if !p.starts_with(scope) {
                    return Err(format!("cannot find module {specifier:?}: outside the importing package ({})", scope.display()));
                }
            }
            let mut id = p.to_string_lossy().into_owned();
            if let Some(q) = query.filter(|q| !(*q == "url" && Kind::of(&p) == Kind::Asset)) {
                id.push('?');
                id.push_str(q);
            }
            (id, Some(p))
        };
        self.record(&id, path);
        if let Some(n) = self.nodes.get_mut(base) {
            n.deps.insert(id.clone());
        }
        Ok(id)
    }

    fn load(&mut self, id: &str, overlay: &Overlay) -> Result<ModuleCode, String> {
        if id == ENTRY_ID {
            return Ok(ModuleCode::Source { js: self.entry_source(), map: None });
        }
        if id == KINDS_ID {
            return Ok(ModuleCode::Source { js: kinds_module(self.compiler.sem.spec()), map: None });
        }
        if let Some(kind) = asset_kind(id) {
            let path = Path::new(id.split_once('?').map_or(id, |(p, _)| p));
            return Ok(ModuleCode::Source { js: self.asset_module(kind, path)?, map: None });
        }
        if let Some(file) = id.strip_prefix(STYLE_PREFIX) {
            let key = build::logical_id(&self.cfg.root, Path::new(file));
            let c = self.compiled(Path::new(file), overlay)?;
            return Ok(ModuleCode::Source { js: style_module(&key, c.css.as_deref().unwrap_or("")), map: None });
        }
        let c = self.compiled(Path::new(id), overlay)?;
        Ok(ModuleCode::Source { js: c.js.clone(), map: c.map.clone() })
    }

    /// The transform output of `path` (from the cache when its text is unchanged). A component with
    /// a sheet ends with the import of its `svelters-style:` module.
    fn compiled(&mut self, path: &Path, overlay: &Overlay) -> Result<&Cached, String> {
        let src = overlay.read(path)?;
        let hash = hash_of(&[&src]);
        if self.cache.get(path).is_some_and(|c| c.hash == hash) {
            return Ok(&self.cache[path]);
        }
        let Output { mut js, map, warnings, css } = self.compiler.transform(path, src)?;
        // AU2b: Vite's `import.meta.glob`, expanded against the files there now.
        if let Some(expanded) = glob::expand(&js, path)? {
            js = expanded;
        }
        self.warnings.extend(warnings);
        let css = css.map(|c| c.code);
        if css.is_some() {
            let spec = serde_json::Value::String(format!("{STYLE_PREFIX}{}", path.to_string_lossy())).to_string();
            js.push_str(&format!("\nimport {spec};\n"));
        }
        self.cache.insert(path.to_path_buf(), Cached { hash, js, map, css });
        Ok(&self.cache[path])
    }

    /// AS1: the module of an asset import. A ref module is constant (`asset(ref)`, the handle);
    /// a value module reads the file when it is evaluated, so H1 re-evaluating it after a change
    /// gives the new value.
    fn asset_module(&self, kind: AssetKind, path: &Path) -> Result<String, String> {
        let r = self.assets.register(APP_ROOT, path).map_err(|e| e.to_string())?;
        let lit = |s: &str| serde_json::Value::String(s.to_string()).to_string();
        Ok(match kind {
            AssetKind::Ref => format!("import {{ asset }} from \"{}\";\nexport default asset({});\n", assets::MODULE_ID, lit(r.as_uri())),
            AssetKind::Raw => format!("import {{ text }} from \"{}\";\nexport default text({});\n", assets::MODULE_ID, lit(r.as_uri())),
            AssetKind::Bytes => format!("import {{ bytes }} from \"{}\";\nexport default bytes({});\n", assets::MODULE_ID, lit(r.as_uri())),
            AssetKind::Inline => {
                let b = self.assets.bytes(&r).map_err(|e| e.to_string())?;
                format!("export default {};\n", lit(&assets::data_url(r.mime(), &b)))
            }
        })
    }

    /// Package manifests and spec files whose change reloads (and resets) the loader.
    fn manifests(&self) -> Vec<PathBuf> {
        let mut v = vec![self.cfg.root.join("package.json"), self.runtime_dir.join("package.json")];
        v.extend(self.cfg.spec.iter().cloned());
        v.extend(self.cfg.spec_fragments.iter().cloned());
        v
    }
}

impl ModuleProvider for SourceProvider {
    /// A module entry is the file itself (its namespace is the entry's); an app or embed entry is
    /// generated.
    fn entry(&self) -> ModuleId {
        let mut inner = self.inner.borrow_mut();
        let (id, path) = match &inner.cfg.entry {
            Entry::Module(p) => (p.to_string_lossy().into_owned(), Some(p.clone())),
            _ => (ENTRY_ID.to_string(), None),
        };
        inner.record(&id, path);
        id
    }

    fn resolve(&self, base: &str, specifier: &str) -> Result<ModuleId, String> {
        self.inner.borrow_mut().resolve(base, specifier)
    }

    fn load(&self, id: &str) -> Result<ModuleCode, String> {
        self.inner.borrow_mut().load(id, &self.overlay)
    }

    fn begin(&self) {
        self.inner.borrow_mut().nodes.clear();
    }

    /// The graph's files outside `node_modules` (the app, workspace packages such as the runtime)
    /// and the manifests. Installed packages are not watched: reinstalling changes a package.json.
    fn watch_files(&self) -> Vec<PathBuf> {
        let inner = self.inner.borrow();
        let mut files: BTreeSet<PathBuf> = inner
            .nodes
            .values()
            .filter_map(|n| n.path.clone())
            .filter(|p| !p.components().any(|c| c.as_os_str() == "node_modules"))
            .collect();
        files.extend(inner.manifests().into_iter().map(|p| key(&p)));
        files.into_iter().collect()
    }

    fn invalidate(&self, changed: &[PathBuf]) -> bool {
        let mut inner = self.inner.borrow_mut();
        let manifests: Vec<PathBuf> = inner.manifests().iter().map(|p| key(p)).collect();
        let changed: Vec<PathBuf> = changed.iter().map(|p| key(p)).collect();
        if changed.iter().any(|p| manifests.contains(p)) {
            // A manifest or spec changed: resolution and every compiled component may differ.
            inner.resolver.clear_cache();
            let reset = Aliases::load(&inner.runtime_dir).and_then(|a| Ok((a, spec_backend(&inner.cfg)?)));
            match reset {
                Ok((aliases, sem)) => {
                    inner.aliases = aliases;
                    inner.compiler.sem = sem;
                }
                Err(e) => inner.warnings.push(e),
            }
            inner.cache.clear();
            return true;
        }
        // LD2: a component edit is compiled now, to tell a `<style>`-only change (its sheet module
        // re-runs, no remount) from one to its code (H1's component swap).
        inner.changed.clear();
        let components: Vec<PathBuf> = changed.iter().filter(|p| inner.nodes.contains_key(&*p.to_string_lossy()) && Kind::of(p) == Kind::Component).cloned().collect();
        for p in components {
            let before = inner.cache.get(&p).map(|c| (c.js.clone(), c.css.clone()));
            if let (Some((js, css)), Ok(c)) = (before, inner.compiled(&p, &self.overlay)) {
                let parts = Changed { js: c.js != js, css: c.css != css };
                inner.changed.insert(p, parts);
            }
        }
        changed.iter().any(|p| inner.nodes.values().any(|n| !n.asset_ref && n.path.as_ref() == Some(p)))
    }

    fn assets(&self) -> Option<Arc<Assets>> {
        Some(self.assets.clone())
    }

    fn hmr(&self) -> bool {
        self.inner.borrow().compiler.hmr
    }

    fn modules_at(&self, changed: &[PathBuf]) -> Vec<ModuleId> {
        let inner = self.inner.borrow();
        let changed: Vec<PathBuf> = changed.iter().map(|p| key(p)).collect();
        inner
            .nodes
            .iter()
            .filter(|(_, n)| !n.asset_ref && n.path.as_ref().is_some_and(|p| changed.contains(p)))
            .filter(|(id, n)| {
                // LD2: the sheet module re-runs when its sheet changed; the component when its code
                // did, or when nothing did (as before LD2).
                let parts = n.path.as_ref().and_then(|p| inner.changed.get(p)).copied().unwrap_or(Changed { js: true, css: true });
                if id.starts_with(STYLE_PREFIX) { parts.css } else { parts.js || !parts.css }
            })
            .map(|(id, _)| id.clone())
            .collect()
    }

    fn importers(&self, id: &str) -> Vec<ModuleId> {
        SourceProvider::importers(self, id)
    }

    fn take_warnings(&self) -> Vec<String> {
        std::mem::take(&mut self.inner.borrow_mut().warnings)
    }

    fn describe(&self) -> String {
        let inner = self.inner.borrow();
        let show = |p: &Path| p.strip_prefix(&inner.cfg.root).unwrap_or(p).display().to_string();
        match &inner.cfg.entry {
            Entry::Module(p) | Entry::App(p) => show(p),
            Entry::Embed(list) => list.iter().map(|(_, p)| show(p)).collect::<Vec<_>>().join(", "),
        }
    }
}

/// LD2: a component's sheet module: registers its scoped sheet as text (what the Vite plugin emits
/// for egui, `registerStyles(id, { css })`; the backend's style engine parses and installs it, `:state()`
/// resolved against the host's registry) and accepts its own updates, so an edit replaces the sheet by
/// id and the backend restyles. Q4b: the id is the component's logical id (`app/src/App.svelte`).
fn style_module(file: &str, css: &str) -> String {
    let lit = |s: &str| serde_json::Value::String(s.to_string()).to_string();
    format!(
        "import {{ registerStyles }} from \"@svelters/native-runtime/style\";\nregisterStyles({}, {{ css: {} }});\nif (import.meta.hot) import.meta.hot.accept();\n",
        lit(file),
        lit(css)
    )
}

/// A module id without the `?v=<n>` the host adds to re-evaluated modules (H1).
fn unversioned_base(base: &str) -> &str {
    base.split_once("?v=").map_or(base, |(b, _)| b)
}

/// What an asset import gives (ASSETS.md §1.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AssetKind {
    /// The `Asset` handle (an asset file, `?url`).
    Ref,
    /// `?raw`: the text.
    Raw,
    /// `?inline`: a `data:` URL.
    Inline,
    /// `?bytes`: a `Uint8Array`.
    Bytes,
}

/// The asset kind of module `id` (a file path, maybe with a query suffix), or `None` for code.
fn asset_kind(id: &str) -> Option<AssetKind> {
    match id.split_once('?') {
        Some((_, "url")) => Some(AssetKind::Ref),
        Some((_, "raw")) => Some(AssetKind::Raw),
        Some((_, "inline")) => Some(AssetKind::Inline),
        Some((_, "bytes")) => Some(AssetKind::Bytes),
        Some(_) => None,
        None => (Path::new(id).is_absolute() && Kind::of(Path::new(id)) == Kind::Asset).then_some(AssetKind::Ref),
    }
}

/// `./a.svg?raw` -> (`./a.svg`, `Some("raw")`). Other queries are refused.
fn split_query(specifier: &str) -> Result<(&str, Option<&str>), String> {
    match specifier.split_once('?') {
        None => Ok((specifier, None)),
        Some((p, q @ ("url" | "raw" | "inline" | "bytes"))) => Ok((p, Some(q))),
        Some((_, q)) => Err(format!(
            "cannot import {specifier:?}: the module loader supports the query suffixes ?url, ?raw, ?inline and ?bytes, not ?{q} (no ?worker, no ?sharedworker)"
        )),
    }
}
