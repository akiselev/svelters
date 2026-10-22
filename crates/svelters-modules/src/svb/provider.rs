//! [`SvbProvider`]: an app served from a `.svb` bundle, with no file system.

use super::{BundleKind, ENGINE, Svb};
use crate::{AssetSource, Assets, ModuleCode, ModuleId, ModuleProvider};
use std::cell::Cell;
use std::path::PathBuf;
use std::sync::Arc;

/// Serves a release bundle: module ids are the bundle's logical ids (`app/src/App.svelte`),
/// imports resolve through each module's import table (nothing is resolved at startup), code is
/// the bundle's bytecode when it was written by this engine and the bundle is trusted, else its
/// JS (which the host compiles through its on-disk bytecode cache, Q2). The bundle's files are
/// mounted as its asset root.
pub struct SvbProvider {
    svb: Arc<Svb>,
    assets: Arc<Assets>,
    engine_build: Option<String>,
    trusted: bool,
    bytecode_loads: Cell<u64>,
}

impl SvbProvider {
    /// A provider over `svb`, with its files mounted as asset root `svb.index().root`. Bundle
    /// bytecode is not used until [`SvbProvider::engine_build`] names the running engine.
    pub fn new(svb: Arc<Svb>) -> Result<SvbProvider, String> {
        let assets = Assets::new();
        assets.mount(&svb.index().root, AssetSource::Bundle(svb.clone())).map_err(|e| e.to_string())?;
        let trusted = svb.index().kind == BundleKind::App;
        Ok(SvbProvider { svb, assets, engine_build: None, trusted, bytecode_loads: Cell::new(0) })
    }

    /// The running engine's stamp (`svelters_host::bytecode::stamp()`): the bundle's bytecode is
    /// read only when its stamp is exactly this.
    pub fn engine_build(mut self, stamp: impl Into<String>) -> SvbProvider {
        self.engine_build = Some(stamp.into());
        self
    }

    /// Whether the bundle's bytecode may be read at all. QuickJS bytecode is trusted input only
    /// (QUICKJS-RUNTIME §3.5): on by default for an app bundle, off for an extension bundle
    /// (ARCH-SECURITY §5), whose JS is compiled by the host instead.
    pub fn trusted(mut self, trusted: bool) -> SvbProvider {
        self.trusted = trusted;
        self
    }

    pub fn bundle(&self) -> &Arc<Svb> {
        &self.svb
    }

    /// Modules served as bundle bytecode so far.
    pub fn bytecode_loads(&self) -> u64 {
        self.bytecode_loads.get()
    }

    fn bytecode_usable(&self) -> bool {
        self.trusted && self.engine_build.is_some() && self.svb.index().stamp.engine_build == self.engine_build
    }
}

impl ModuleProvider for SvbProvider {
    fn entry(&self) -> ModuleId {
        self.svb.index().entry.clone()
    }

    fn resolve(&self, base: &str, specifier: &str) -> Result<ModuleId, String> {
        let m = self.svb.index().modules.get(base).ok_or_else(|| format!("{base}: not in the bundle"))?;
        m.imports.get(specifier).cloned().ok_or_else(|| format!("cannot find module {specifier:?}: not in the bundle's import table of {base}"))
    }

    fn load(&self, id: &str) -> Result<ModuleCode, String> {
        let m = self.svb.index().modules.get(id).ok_or_else(|| format!("{id}: not in the bundle"))?;
        if let Some(b) = m.bytecode.filter(|_| self.bytecode_usable()) {
            self.bytecode_loads.set(self.bytecode_loads.get() + 1);
            return Ok(ModuleCode::Cached { engine: ENGINE, bytes: self.svb.blob(b).to_vec() });
        }
        let js = self.svb.module_js(id)?;
        let map = m.map.map(|b| String::from_utf8_lossy(self.svb.blob(b)).into_owned());
        Ok(ModuleCode::Source { js, map })
    }

    fn assets(&self) -> Option<Arc<Assets>> {
        Some(self.assets.clone())
    }

    fn watch_files(&self) -> Vec<PathBuf> {
        Vec::new()
    }

    fn invalidate(&self, _changed: &[PathBuf]) -> bool {
        false
    }

    fn describe(&self) -> String {
        let ix = self.svb.index();
        format!("{} bundle {} ({} modules)", ix.root, ix.build, ix.modules.len())
    }
}

#[cfg(test)]
mod tests {
    use super::super::{FileRole, ModuleEntry, SvbWriter};
    use super::*;
    use crate::AssetRef;

    fn bundle(kind: BundleKind, stamp: Option<&str>) -> Arc<Svb> {
        let mut w = SvbWriter::new(kind, "app", "app/main.js");
        let js = w.blob(b"import { x } from './x.js';");
        let x = w.blob(b"export const x = 1;");
        let bc = w.blob(b"BYTECODE");
        w.module("app/main.js", ModuleEntry { js, imports: [("./x.js".into(), "app/x.js".into())].into(), ..ModuleEntry::default() });
        w.module("app/x.js", ModuleEntry { js: x, bytecode: Some(bc), ..ModuleEntry::default() });
        w.file("src/icon.svg", b"<svg/>", FileRole::Asset, None);
        w.index.stamp.engine_build = stamp.map(str::to_string);
        Arc::new(Svb::from_bytes(w.finish()).unwrap())
    }

    #[test]
    fn resolves_through_the_import_table_and_serves_assets() {
        let p = SvbProvider::new(bundle(BundleKind::App, None)).unwrap();
        assert_eq!(p.entry(), "app/main.js");
        assert_eq!(p.resolve("app/main.js", "./x.js").unwrap(), "app/x.js");
        assert!(p.resolve("app/main.js", "./y.js").unwrap_err().contains("import table"));
        assert!(matches!(p.load("app/x.js").unwrap(), ModuleCode::Source { js, .. } if js == "export const x = 1;"));
        let r = AssetRef::new("app", "src/icon.svg").unwrap();
        assert_eq!(&*p.assets().unwrap().bytes(&r).unwrap(), b"<svg/>");
        assert!(p.assets().unwrap().bytes(&AssetRef::new("app", "src/none.svg").unwrap()).is_err());
    }

    #[test]
    fn bytecode_needs_the_same_engine_and_trust() {
        let cached = |p: &SvbProvider| matches!(p.load("app/x.js").unwrap(), ModuleCode::Cached { .. });
        let p = SvbProvider::new(bundle(BundleKind::App, Some("e1"))).unwrap();
        assert!(!cached(&p), "no running engine named");
        let p = p.engine_build("e2");
        assert!(!cached(&p), "another engine");
        let p = p.engine_build("e1");
        assert!(cached(&p));
        assert_eq!(p.bytecode_loads(), 1);
        assert!(!cached(&p.trusted(false)));
        let ext = SvbProvider::new(bundle(BundleKind::Extension, Some("e1"))).unwrap().engine_build("e1");
        assert!(!cached(&ext), "extension bytecode is ignored unless trusted");
        assert!(cached(&ext.trusted(true)));
    }
}
