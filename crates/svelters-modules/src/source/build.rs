//! Q4: building a `.svb` from an app's sources (ASSETS.md §4.2). Pure Rust: the resolver and the
//! compiler of [`SourceProvider`], no JS engine, so it runs from a `build.rs` before
//! `include_bytes!` as well as from a tool.

use super::{ENTRY_ID, SourceConfig, SourceProvider, asset_kind};
use crate::assets::{mime_of, package_name};
use crate::svb::{BundleKind, FileRole, ModuleEntry, SvbWriter, content_hash};
use crate::{ModuleCode, ModuleProvider};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};
use svelters_compiler::acorn;

/// What goes into a bundle besides the module graph.
#[derive(Clone, Debug)]
pub struct BuildOptions {
    /// Ship each module's source (decision 5: true, so the release build can compile edits live).
    pub sources: bool,
    /// Embedder data stored as the index's `meta`, uninterpreted.
    pub meta: Value,
}

impl Default for BuildOptions {
    fn default() -> Self {
        BuildOptions { sources: true, meta: Value::Object(Default::default()) }
    }
}

/// The asset root, and the first segment of every module id, of an app bundle.
const ROOT: &str = crate::assets::APP_ROOT;

/// Builds the release bundle of the app `cfg` describes: the module graph from its entry,
/// compiled for production (`dev` and `hmr` are turned off), each module with its import table;
/// the asset files the graph imports; the package manifests; the sources unless
/// `opts.sources` is off. Module ids are logical: `app/<path in the app>` for the app's files,
/// `app/node_modules/<package>/<path>` for a package's, so they do not depend on the machine
/// that built the bundle.
pub fn build(cfg: SourceConfig, opts: &BuildOptions) -> Result<Vec<u8>, String> {
    let cfg = SourceConfig { dev: false, hmr: false, ..cfg };
    let root = cfg.root.clone();
    let provider = SourceProvider::new(cfg)?;
    let mut ids = Logical { root: root.clone(), seen: BTreeMap::new() };

    let entry = provider.entry();
    let mut w = SvbWriter::new(BundleKind::App, ROOT, &ids.id(&entry)?);
    w.index.meta = opts.meta.clone();
    w.index.version = read_json(&root.join("package.json")).and_then(|j| j.get("version")?.as_str().map(str::to_string));
    w.index.stamp.runtime =
        read_json(&provider.inner.borrow().runtime_dir.join("package.json")).and_then(|j| j.get("version")?.as_str().map(str::to_string));

    let mut queue = VecDeque::from([entry.clone()]);
    let mut visited = BTreeSet::from([entry]);
    let mut manifests = BTreeSet::new();
    while let Some(id) = queue.pop_front() {
        let logical = ids.id(&id)?;
        let (js, map) = match provider.load(&id).map_err(|e| format!("{id}: {e}"))? {
            ModuleCode::Source { js, map } => (js, map),
            ModuleCode::Cached { .. } => return Err(format!("{id}: the source provider gave bytecode")),
        };
        let mut imports = BTreeMap::new();
        for spec in scan_imports(&js).map_err(|e| format!("{id}: {e}"))? {
            if host_served(&spec) {
                continue;
            }
            let dep = provider.resolve(&id, &spec).map_err(|e| format!("{id}: {e}"))?;
            imports.insert(spec, ids.id(&dep)?);
            if visited.insert(dep.clone()) {
                queue.push_back(dep);
            }
        }
        let mut entry = ModuleEntry { imports, ..ModuleEntry::default() };
        entry.js = w.blob(js.as_bytes());
        entry.map = map.map(|m| w.blob(m.as_bytes()));
        if let Some(path) = file_of(&id) {
            let rel = ids.rel(&path)?;
            let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            if asset_kind(&id).is_some() {
                // The ref's path in the app root, as `Assets::register` names it.
                w.file(&rel, &bytes, FileRole::Asset, Some(mime_of(&rel)));
            } else {
                entry.source_hash = Some(content_hash(&bytes));
                if opts.sources {
                    w.file(&rel, &bytes, FileRole::Source, None);
                    entry.source = Some(rel);
                }
            }
            if let Some(dir) = path.ancestors().skip(1).find(|d| d.join("package.json").is_file()) {
                manifests.insert(dir.join("package.json"));
            }
        }
        w.module(&logical, entry);
    }
    for m in manifests {
        let rel = ids.rel(&m)?;
        let bytes = std::fs::read(&m).map_err(|e| format!("{}: {e}", m.display()))?;
        w.file(&rel, &bytes, FileRole::Manifest, Some("application/json"));
    }
    Ok(w.finish())
}

/// Maps the source provider's ids (absolute paths, maybe with a query) to bundle ids.
struct Logical {
    root: PathBuf,
    /// Bundle path -> the file it came from: two files must not get one name (two copies of a package).
    seen: BTreeMap<String, PathBuf>,
}

impl Logical {
    fn id(&mut self, id: &str) -> Result<String, String> {
        let (p, query) = id.split_once('?').map_or((id, None), |(p, q)| (p, Some(q)));
        if !Path::new(p).is_absolute() {
            return Ok(id.to_string()); // generated: `svelters:entry`, `svelters:kinds`
        }
        let rel = self.rel(Path::new(p))?;
        Ok(match query {
            Some(q) => format!("{ROOT}/{rel}?{q}"),
            None => format!("{ROOT}/{rel}"),
        })
    }

    /// `path`'s name in the bundle: relative to the app, or `node_modules/<package>/…`.
    fn rel(&mut self, path: &Path) -> Result<String, String> {
        let slashed = |p: &Path| p.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
        let rel = match path.strip_prefix(&self.root) {
            Ok(rel) => slashed(rel),
            Err(_) => {
                let (dir, name) = path
                    .ancestors()
                    .skip(1)
                    .find_map(|d| package_name(d).map(|n| (d, n)))
                    .ok_or_else(|| format!("{}: outside the app and in no package", path.display()))?;
                format!("node_modules/{name}/{}", slashed(path.strip_prefix(dir).expect("an ancestor")))
            }
        };
        match self.seen.get(&rel) {
            Some(prev) if prev != path => Err(format!("{} and {} would both be {rel} in the bundle (two copies of a package)", prev.display(), path.display())),
            Some(_) => Ok(rel),
            None => {
                self.seen.insert(rel.clone(), path.to_path_buf());
                Ok(rel)
            }
        }
    }
}

/// The file behind a source provider id, if it is one.
fn file_of(id: &str) -> Option<PathBuf> {
    let p = Path::new(id.split_once('?').map_or(id, |(p, _)| p));
    (id != ENTRY_ID && p.is_absolute()).then(|| p.to_path_buf())
}

/// Specifiers the host serves before a provider is asked (`svelters:core`, `svelters:assets`,
/// `svelters:app/*`, DX modules). `svelters:behaviors/*` is the app's own (a dumped file).
fn host_served(spec: &str) -> bool {
    spec.starts_with("svelters:") && !spec.starts_with("svelters:behaviors/")
}

/// The specifiers `js` imports: static imports, re-exports, and `import()` of a string literal.
/// Any other dynamic import cannot be bundled and is an error.
pub(crate) fn scan_imports(js: &str) -> Result<Vec<String>, String> {
    let program = acorn::parse(js, &acorn::Options::default(), &mut Vec::new()).map_err(|e| e.message)?;
    let tree = serde_json::to_value(&program).map_err(|e| e.to_string())?;
    let mut out = BTreeSet::new();
    let mut stack = vec![&tree];
    while let Some(v) = stack.pop() {
        match v {
            Value::Array(a) => stack.extend(a),
            Value::Object(o) => {
                let ty = o.get("type").and_then(Value::as_str).unwrap_or("");
                if matches!(ty, "ImportDeclaration" | "ExportNamedDeclaration" | "ExportAllDeclaration" | "ImportExpression") {
                    match o.get("source") {
                        Some(Value::Object(s)) if s.get("type").and_then(Value::as_str) == Some("Literal") => {
                            if let Some(spec) = s.get("value").and_then(Value::as_str) {
                                out.insert(spec.to_string());
                            }
                        }
                        Some(Value::Null) | None => {}
                        Some(_) if ty == "ImportExpression" => return Err("a dynamic import() of a computed specifier cannot be bundled".into()),
                        Some(_) => {}
                    }
                }
                stack.extend(o.values());
            }
            _ => {}
        }
    }
    Ok(out.into_iter().collect())
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_static_dynamic_and_re_export_specifiers() {
        let js = r#"
            import a from './a.js';
            import './side.js';
            export { b } from "./b.js";
            export * from './c.js';
            const s = "import x from './not-an-import.js'"; // import y from './comment.js'
            const re = /import z from '.\/regex.js'/;
            async function f() { return import('./lazy.js'); }
            import.meta.url;
            export const local = 1;
        "#;
        assert_eq!(scan_imports(js).unwrap(), ["./a.js", "./b.js", "./c.js", "./lazy.js", "./side.js"]);
        assert!(scan_imports("const n = 'x'; import(n);").unwrap_err().contains("computed"));
        assert!(scan_imports("import {").is_err());
    }

    #[test]
    fn host_modules_are_left_to_the_host() {
        assert!(host_served("svelters:core"));
        assert!(host_served("svelters:app/task"));
        assert!(host_served("svelters:assets"));
        assert!(!host_served("svelters:behaviors/demo"));
        assert!(!host_served("svelte"));
    }
}
