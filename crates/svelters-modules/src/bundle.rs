//! A directory with a built bundle (`dist/app.js` and its chunks), as Vite writes it.

use crate::{AssetSource, Assets, ModuleCode, ModuleId, ModuleProvider};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Serves a built bundle. Module ids are slash-separated paths relative to the bundle's directory;
/// only relative imports are allowed, and none may leave the directory (symlinks included). Source
/// maps are the `<module>.map` files next to the modules.
pub struct BundleProvider {
    root: PathBuf,
    entry: String,
    /// AS1: `app` is `<bundle dir>/assets` (what `vite build` copies the imported assets to).
    assets: Arc<Assets>,
}

impl BundleProvider {
    /// The bundle whose entry module is `entry` (e.g. `dist/app.js`).
    pub fn new(entry: &Path) -> Result<BundleProvider, String> {
        let entry = entry.canonicalize().map_err(|e| format!("{}: {e}", entry.display()))?;
        let root = entry.parent().ok_or("entry has no parent directory")?.to_path_buf();
        let name = entry.file_name().and_then(|n| n.to_str()).ok_or("entry file name is not UTF-8")?.to_string();
        let assets = Assets::new();
        let dir = root.join("assets");
        if dir.is_dir() {
            assets.mount(crate::assets::APP_ROOT, AssetSource::Dir(dir)).map_err(|e| e.to_string())?;
        }
        Ok(BundleProvider { root, entry: name, assets })
    }

    /// The bundle's entry file.
    pub fn entry_path(&self) -> PathBuf {
        self.root.join(&self.entry)
    }
}

impl ModuleProvider for BundleProvider {
    fn entry(&self) -> ModuleId {
        self.entry.clone()
    }

    fn resolve(&self, base: &str, specifier: &str) -> Result<ModuleId, String> {
        if !(specifier.starts_with("./") || specifier.starts_with("../")) {
            return Err("only relative imports inside the bundle directory are supported".into());
        }
        let mut parts: Vec<&str> = base.rsplit_once('/').map_or(Vec::new(), |(dir, _)| dir.split('/').collect());
        for seg in specifier.split('/') {
            match seg {
                "" | "." => {}
                ".." => {
                    if parts.pop().is_none() {
                        return Err("import escapes the bundle directory".into());
                    }
                }
                s => parts.push(s),
            }
        }
        Ok(parts.join("/"))
    }

    fn load(&self, id: &str) -> Result<ModuleCode, String> {
        let path = self.root.join(id).canonicalize().map_err(|e| e.to_string())?;
        // Symlinks must not lead out of the bundle directory.
        if !path.starts_with(&self.root) {
            return Err("resolves outside the bundle directory".into());
        }
        let js = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        Ok(ModuleCode::Source { js, map: None })
    }

    fn map_dir(&self) -> Option<PathBuf> {
        Some(self.root.clone())
    }

    fn assets(&self) -> Option<Arc<Assets>> {
        Some(self.assets.clone())
    }

    fn watch_files(&self) -> Vec<PathBuf> {
        let entry = self.entry_path();
        let mut map = entry.clone().into_os_string();
        map.push(".map");
        vec![entry, PathBuf::from(map)]
    }

    /// The entry or its map changed, and the entry exists: a build that clears its output
    /// directory first is mid-write, and the file's reappearance is another event.
    fn invalidate(&self, changed: &[PathBuf]) -> bool {
        let files = self.watch_files();
        let names: Vec<_> = files.iter().filter_map(|f| f.file_name()).collect();
        changed.iter().any(|p| p.file_name().is_some_and(|n| names.contains(&n))) && files[0].is_file()
    }

    fn describe(&self) -> String {
        self.entry_path().display().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider() -> BundleProvider {
        BundleProvider { root: PathBuf::from("/b"), entry: "app.js".into(), assets: Assets::new() }
    }

    #[test]
    fn resolves_relative_imports_inside_the_directory() {
        let p = provider();
        assert_eq!(p.resolve("app.js", "./chunks/a.js").unwrap(), "chunks/a.js");
        assert_eq!(p.resolve("chunks/a.js", "../b.js").unwrap(), "b.js");
        assert!(p.resolve("app.js", "../x.js").unwrap_err().contains("escapes"));
        assert!(p.resolve("app.js", "svelte").unwrap_err().contains("only relative"));
    }
}
