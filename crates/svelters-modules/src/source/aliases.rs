//! The runtime's alias table: `"svelters": { "aliases": { ... } }` in
//! `@svelters/native-runtime`'s package.json. `vite.js` reads the same table, so the Vite build and
//! this loader cannot drift.
//!
//! A key is a specifier, matched exactly or with `.js` appended (`svelte/internal/client/context`
//! and `svelte/internal/client/context.js`), or a pattern with one `*` that matches one path
//! segment (`svelters:gtk/*`). A value is `./<path>`, relative to the runtime package, or
//! `<package>/<path>`: a file inside a package the runtime depends on, found through
//! `node_modules` without its `exports` map (Svelte hides the internal modules the runtime needs).
//! A pattern's value repeats the `*`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub(crate) struct Aliases {
    exact: HashMap<String, PathBuf>,
    /// (prefix, suffix, target with `*`)
    patterns: Vec<(String, String, String)>,
}

impl Aliases {
    /// Reads the table of the runtime package in `runtime_dir`.
    pub(crate) fn load(runtime_dir: &Path) -> Result<Aliases, String> {
        let manifest = runtime_dir.join("package.json");
        let text = std::fs::read_to_string(&manifest).map_err(|e| format!("{}: {e}", manifest.display()))?;
        let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", manifest.display()))?;
        let table = json
            .pointer("/svelters/aliases")
            .and_then(|t| t.as_object())
            .ok_or_else(|| format!("{}: no \"svelters\": {{ \"aliases\": ... }} table", manifest.display()))?;
        let mut packages: HashMap<String, PathBuf> = HashMap::new();
        let mut target = |value: &str| -> Result<String, String> {
            if let Some(rel) = value.strip_prefix("./") {
                return Ok(runtime_dir.join(rel).to_string_lossy().into_owned());
            }
            let (pkg, rest) = split_package(value).ok_or_else(|| format!("{}: alias target {value:?} names no package", manifest.display()))?;
            let dir = match packages.get(pkg) {
                Some(d) => d.clone(),
                None => {
                    let d = package_dir(runtime_dir, pkg)
                        .ok_or_else(|| format!("{}: package {pkg:?} of alias target {value:?} is not installed (pnpm install)", manifest.display()))?;
                    packages.insert(pkg.to_string(), d.clone());
                    d
                }
            };
            Ok(dir.join(rest).to_string_lossy().into_owned())
        };
        let mut aliases = Aliases { exact: HashMap::new(), patterns: Vec::new() };
        for (key, value) in table {
            let value = value.as_str().ok_or_else(|| format!("{}: alias {key:?} is not a string", manifest.display()))?;
            let to = target(value)?;
            match key.split_once('*') {
                Some((prefix, suffix)) => aliases.patterns.push((prefix.to_string(), suffix.to_string(), to)),
                None => {
                    aliases.exact.insert(key.clone(), PathBuf::from(to));
                }
            }
        }
        Ok(aliases)
    }

    /// The file `specifier` is aliased to, if any.
    pub(crate) fn get(&self, specifier: &str) -> Option<PathBuf> {
        if let Some(p) = self.exact.get(specifier).or_else(|| specifier.strip_suffix(".js").and_then(|s| self.exact.get(s))) {
            return Some(p.clone());
        }
        self.patterns.iter().find_map(|(prefix, suffix, to)| {
            let mid = specifier.strip_prefix(prefix.as_str())?.strip_suffix(suffix.as_str())?;
            (!mid.is_empty() && !mid.contains('/')).then(|| PathBuf::from(to.replace('*', mid)))
        })
    }
}

/// `@scope/name/rest` -> (`@scope/name`, `rest`); `name/rest` -> (`name`, `rest`).
fn split_package(spec: &str) -> Option<(&str, &str)> {
    let n = if spec.starts_with('@') { 2 } else { 1 };
    let mut idx = 0;
    for _ in 0..n {
        idx += spec[idx..].find('/')? + 1;
    }
    Some((&spec[..idx - 1], &spec[idx..]))
}

/// The directory of package `name` as Node finds it from `from`: the first
/// `<ancestor>/node_modules/<name>` (symlinks resolved).
pub(crate) fn package_dir(from: &Path, name: &str) -> Option<PathBuf> {
    from.ancestors().find_map(|a| crate::canonicalize(&a.join("node_modules").join(name)).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_package_names() {
        assert_eq!(split_package("svelte/src/x.js"), Some(("svelte", "src/x.js")));
        assert_eq!(split_package("@svelters/types/gtk/*/index.js"), Some(("@svelters/types", "gtk/*/index.js")));
        assert_eq!(split_package("svelte"), None);
    }

    #[test]
    fn matches_exact_keys_with_or_without_js_and_patterns() {
        let a = Aliases {
            exact: [("svelte/internal/client/context".to_string(), PathBuf::from("/s/context.js"))].into(),
            patterns: vec![("svelters:gtk/".into(), String::new(), "/t/gtk/*/index.js".into())],
        };
        assert_eq!(a.get("svelte/internal/client/context"), Some(PathBuf::from("/s/context.js")));
        assert_eq!(a.get("svelte/internal/client/context.js"), Some(PathBuf::from("/s/context.js")));
        assert_eq!(a.get("svelters:gtk/gio"), Some(PathBuf::from("/t/gtk/gio/index.js")));
        assert_eq!(a.get("svelters:gtk/"), None);
        assert_eq!(a.get("svelters:gtk/a/b"), None);
        assert_eq!(a.get("svelte"), None);
    }
}
