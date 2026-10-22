//! Node ESM resolution for the loader: relative and absolute paths, bare specifiers through
//! `node_modules`, package.json `exports` (conditions, `*` subpath patterns) and `imports` (`#`),
//! the `module`/`main` fields, extensions and directory indexes, symlinks resolved (pnpm).
//!
//! Our own instead of `oxc_resolver`: that crate turns on serde_json's `preserve_order` for every
//! crate in the build, which reorders every JSON map the workspace generates. `exports` priority is
//! key order, so package.json is read here with an order-keeping value type ([`Json`]).

use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// A JSON value whose objects keep their key order.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Json {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(o) => o.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    fn as_str(&self) -> Option<&str> {
        match self {
            Json::String(s) => Some(s),
            _ => None,
        }
    }
}

impl<'de> Deserialize<'de> for Json {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Json, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Json;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("JSON")
            }
            fn visit_unit<E>(self) -> Result<Json, E> {
                Ok(Json::Null)
            }
            fn visit_bool<E>(self, b: bool) -> Result<Json, E> {
                Ok(Json::Bool(b))
            }
            fn visit_i64<E>(self, n: i64) -> Result<Json, E> {
                Ok(Json::Number(n as f64))
            }
            fn visit_u64<E>(self, n: u64) -> Result<Json, E> {
                Ok(Json::Number(n as f64))
            }
            fn visit_f64<E>(self, n: f64) -> Result<Json, E> {
                Ok(Json::Number(n))
            }
            fn visit_str<E>(self, s: &str) -> Result<Json, E> {
                Ok(Json::String(s.to_string()))
            }
            fn visit_string<E>(self, s: String) -> Result<Json, E> {
                Ok(Json::String(s))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Json, A::Error> {
                let mut v = Vec::new();
                while let Some(x) = a.next_element()? {
                    v.push(x);
                }
                Ok(Json::Array(v))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Json, A::Error> {
                let mut v: Vec<(String, Json)> = Vec::new();
                while let Some((k, x)) = a.next_entry::<String, Json>()? {
                    // A repeated key keeps its first position and its last value, as JSON.parse.
                    match v.iter_mut().find(|(e, _)| *e == k) {
                        Some(e) => e.1 = x,
                        None => v.push((k, x)),
                    }
                }
                Ok(Json::Object(v))
            }
        }
        d.deserialize_any(V)
    }
}

pub(crate) struct Resolver {
    /// Export conditions, matched in the package's key order; `default` always matches.
    conditions: Vec<String>,
    extensions: Vec<String>,
    main_fields: Vec<String>,
    /// package.json by directory (`None`: there is none, or it does not parse).
    manifests: RefCell<HashMap<PathBuf, Option<Rc<Json>>>>,
}

impl Resolver {
    pub(crate) fn new(conditions: &[&str], extensions: &[&str], main_fields: &[&str]) -> Resolver {
        let own = |v: &[&str]| v.iter().map(|s| s.to_string()).collect();
        Resolver { conditions: own(conditions), extensions: own(extensions), main_fields: own(main_fields), manifests: RefCell::default() }
    }

    /// Forgets the package.json files read so far (one changed, or packages were installed).
    pub(crate) fn clear_cache(&self) {
        self.manifests.borrow_mut().clear();
    }

    /// Resolves `spec` imported from a module in directory `dir` to a canonical file path.
    pub(crate) fn resolve(&self, dir: &Path, spec: &str) -> Result<PathBuf, String> {
        let not_found = || format!("cannot find module {spec:?} from {}", dir.display());
        if spec.starts_with("./") || spec.starts_with("../") || spec == "." || spec == ".." {
            return self.file_or_dir(&dir.join(spec)).ok_or_else(not_found);
        }
        if Path::new(spec).is_absolute() {
            return self.file_or_dir(Path::new(spec)).ok_or_else(not_found);
        }
        if spec.starts_with('#') {
            return self.package_imports(dir, spec);
        }
        self.bare(dir, spec)
    }

    fn manifest(&self, dir: &Path) -> Option<Rc<Json>> {
        if let Some(m) = self.manifests.borrow().get(dir) {
            return m.clone();
        }
        let m = std::fs::read_to_string(dir.join("package.json")).ok().and_then(|t| serde_json::from_str::<Json>(&t).ok()).map(Rc::new);
        self.manifests.borrow_mut().insert(dir.to_path_buf(), m.clone());
        m
    }

    fn file(&self, p: &Path) -> Option<PathBuf> {
        if p.is_file() { p.canonicalize().ok() } else { None }
    }

    /// A file as is, with an extension appended, or a directory (its package.json main, its index).
    fn file_or_dir(&self, p: &Path) -> Option<PathBuf> {
        let p = crate::normalize(p);
        if let Some(f) = self.file(&p) {
            return Some(f);
        }
        for ext in &self.extensions {
            let mut s = p.clone().into_os_string();
            s.push(ext);
            if let Some(f) = self.file(Path::new(&s)) {
                return Some(f);
            }
        }
        // TypeScript's convention: `./util.js` names `util.ts` (`.mjs`: `.mts`), the file it compiles to
        let name = p.to_string_lossy();
        if let Some(stem) = name.strip_suffix(".js").or_else(|| name.strip_suffix(".mjs"))
            && let Some(f) = self.file(Path::new(&format!("{stem}{}", if name.ends_with(".mjs") { ".mts" } else { ".ts" })))
        {
            return Some(f);
        }
        if p.is_dir() {
            if let Some(m) = self.manifest(&p) {
                for field in &self.main_fields {
                    if let Some(main) = m.get(field).and_then(Json::as_str)
                        && let Some(f) = self.file_or_dir(&p.join(main))
                    {
                        return Some(f);
                    }
                }
            }
            return self.extensions.iter().find_map(|ext| self.file(&p.join(format!("index{ext}"))));
        }
        None
    }

    fn bare(&self, dir: &Path, spec: &str) -> Result<PathBuf, String> {
        let (name, sub) = split_bare(spec).ok_or_else(|| format!("invalid package specifier {spec:?}"))?;
        for ancestor in dir.ancestors() {
            let pkg = ancestor.join("node_modules").join(name);
            if !pkg.is_dir() {
                continue;
            }
            let pkg = pkg.canonicalize().map_err(|e| format!("{}: {e}", pkg.display()))?;
            let manifest = self.manifest(&pkg);
            if let Some(exports) = manifest.as_ref().and_then(|m| m.get("exports")) {
                let target = self.exports(exports, &sub).ok_or_else(|| format!("package {name:?} does not export {sub:?} (conditions {:?})", self.conditions))?;
                return self.file(&pkg.join(&target)).ok_or_else(|| format!("package {name:?} exports {sub:?} as {target:?}, which does not exist"));
            }
            let target = if sub == "." { pkg.clone() } else { pkg.join(&sub[2..]) };
            return self.file_or_dir(&target).ok_or_else(|| format!("cannot find {spec:?} in {}", pkg.display()));
        }
        Err(format!("cannot find package {name:?} from {} (is it installed?)", dir.display()))
    }

    fn package_imports(&self, dir: &Path, spec: &str) -> Result<PathBuf, String> {
        for d in dir.ancestors() {
            let Some(m) = self.manifest(d) else { continue };
            let imports = m.get("imports").ok_or_else(|| format!("{}: no \"imports\" for {spec:?}", d.join("package.json").display()))?;
            let target = self.subpath_map(imports, spec).ok_or_else(|| format!("{}: \"imports\" has no match for {spec:?}", d.join("package.json").display()))?;
            if target.starts_with("./") {
                return self.file(&d.join(&target)).ok_or_else(|| format!("{spec:?} maps to {target:?}, which does not exist"));
            }
            // A bare target ("#dep": "some-package") resolves as a dependency of this package.
            return self.bare(d, &target);
        }
        Err(format!("{spec:?}: no package.json above {}", dir.display()))
    }

    /// `exports` for subpath `sub` (`.` or `./x`): a target relative to the package.
    fn exports(&self, exports: &Json, sub: &str) -> Option<String> {
        let subpath_keys = matches!(exports, Json::Object(o) if o.first().is_some_and(|(k, _)| k.starts_with('.')));
        if subpath_keys {
            self.subpath_map(exports, sub)
        } else if sub == "." {
            self.target(exports, None)
        } else {
            None
        }
    }

    /// A map of subpaths (`exports`) or `#` names (`imports`): an exact key first, then the `*`
    /// pattern with the longest prefix.
    fn subpath_map(&self, map: &Json, key: &str) -> Option<String> {
        let Json::Object(entries) = map else { return None };
        if let Some((_, v)) = entries.iter().find(|(k, _)| k == key && !k.contains('*')) {
            return self.target(v, None);
        }
        let mut best: Option<(&str, &Json, &str)> = None;
        for (k, v) in entries {
            let Some((prefix, suffix)) = k.split_once('*') else { continue };
            if key.len() >= prefix.len() + suffix.len()
                && key.starts_with(prefix)
                && key.ends_with(suffix)
                && best.is_none_or(|(p, _, _)| prefix.len() > p.len())
            {
                best = Some((prefix, v, &key[prefix.len()..key.len() - suffix.len()]));
            }
        }
        best.and_then(|(_, v, star)| self.target(v, Some(star)))
    }

    /// A target: a path (with `*` replaced), the first array entry that resolves, or the first
    /// matching condition's target. `null` excludes.
    fn target(&self, t: &Json, star: Option<&str>) -> Option<String> {
        match t {
            Json::String(s) => Some(match star {
                Some(m) => s.replace('*', m),
                None => s.clone(),
            }),
            Json::Array(a) => a.iter().find_map(|x| self.target(x, star)),
            Json::Object(o) => o.iter().filter(|(k, _)| k == "default" || self.conditions.iter().any(|c| c == k)).find_map(|(_, v)| self.target(v, star)),
            _ => None,
        }
    }
}

/// `@scope/name/x` -> (`@scope/name`, `./x`); `name` -> (`name`, `.`).
fn split_bare(spec: &str) -> Option<(&str, String)> {
    let mut parts = spec.splitn(if spec.starts_with('@') { 3 } else { 2 }, '/');
    let first = parts.next()?;
    let name_len = if spec.starts_with('@') { first.len() + 1 + parts.next().filter(|s| !s.is_empty())?.len() } else { first.len() };
    if name_len == 0 {
        return None;
    }
    let rest = &spec[name_len..];
    Some((&spec[..name_len], if rest.is_empty() { ".".into() } else { format!(".{rest}") }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r() -> Resolver {
        Resolver::new(&["browser", "import", "module", "production", "default"], &[".mjs", ".js", ".json"], &["module", "main"])
    }

    fn json(s: &str) -> Json {
        serde_json::from_str(s).unwrap()
    }

    #[test]
    fn json_objects_keep_key_order() {
        assert_eq!(json(r#"{"b":1,"a":[true,null,"x"]}"#), Json::Object(vec![("b".into(), Json::Number(1.0)), ("a".into(), Json::Array(vec![Json::Bool(true), Json::Null, Json::String("x".into())]))]));
    }

    #[test]
    fn splits_bare_specifiers() {
        assert_eq!(split_bare("svelte"), Some(("svelte", ".".into())));
        assert_eq!(split_bare("svelte/internal/client"), Some(("svelte", "./internal/client".into())));
        assert_eq!(split_bare("@svelters/types/gtk"), Some(("@svelters/types", "./gtk".into())));
        assert_eq!(split_bare("@scope"), None);
    }

    #[test]
    fn exports_follow_condition_order_and_patterns() {
        let r = r();
        let e = json(
            r#"{
                ".": { "types": "./t.d.ts", "worker": "./w.js", "browser": "./b.js", "default": "./d.js" },
                "./internal/*": { "node": null, "default": "./src/internal/*/index.js" },
                "./internal/client/*": "./src/client/*.js",
                "./package.json": "./package.json",
                "./hidden": null
            }"#,
        );
        assert_eq!(r.exports(&e, ".").as_deref(), Some("./b.js"));
        assert_eq!(r.exports(&e, "./internal/flags").as_deref(), Some("./src/internal/flags/index.js"));
        assert_eq!(r.exports(&e, "./internal/client/x").as_deref(), Some("./src/client/x.js"), "the longest prefix wins");
        assert_eq!(r.exports(&e, "./hidden"), None);
        assert_eq!(r.exports(&e, "./nope"), None);
        // Condition order is the package's, not ours: `import` before `browser` here.
        let e = json(r#"{ "import": "./i.js", "browser": "./b.js" }"#);
        assert_eq!(r.exports(&e, ".").as_deref(), Some("./i.js"));
        assert_eq!(r.exports(&json(r#""./main.js""#), ".").as_deref(), Some("./main.js"));
        assert_eq!(r.exports(&json(r#"[{ "node": "./n.js" }, "./f.js"]"#), ".").as_deref(), Some("./f.js"));
    }

    #[test]
    fn resolves_on_disk() {
        let dir = std::env::temp_dir().join(format!("svelters-resolve-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let w = |rel: &str, text: &str| {
            let p = dir.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        };
        w("app/package.json", r##"{ "imports": { "#lib/*": "./src/lib/*.js" } }"##);
        w("app/src/a.js", "");
        w("app/src/lib/x.js", "");
        w("app/src/dir/index.js", "");
        w("app/node_modules/plain/package.json", r#"{ "main": "lib/main" }"#);
        w("app/node_modules/plain/lib/main.js", "");
        w("app/node_modules/plain/other.mjs", "");
        w("app/node_modules/@s/pkg/package.json", r#"{ "exports": { ".": { "import": "./esm.js", "default": "./cjs.js" }, "./sub": "./sub.js" } }"#);
        w("app/node_modules/@s/pkg/esm.js", "");
        w("app/node_modules/@s/pkg/sub.js", "");
        let r = r();
        let src = dir.join("app/src");
        let canon = |rel: &str| dir.join(rel).canonicalize().unwrap();
        assert_eq!(r.resolve(&src, "./a").unwrap(), canon("app/src/a.js"));
        assert_eq!(r.resolve(&src, "./dir").unwrap(), canon("app/src/dir/index.js"));
        assert_eq!(r.resolve(&src, "#lib/x").unwrap(), canon("app/src/lib/x.js"));
        assert_eq!(r.resolve(&src, "plain").unwrap(), canon("app/node_modules/plain/lib/main.js"));
        assert_eq!(r.resolve(&src, "plain/other").unwrap(), canon("app/node_modules/plain/other.mjs"));
        assert_eq!(r.resolve(&src, "@s/pkg").unwrap(), canon("app/node_modules/@s/pkg/esm.js"));
        assert_eq!(r.resolve(&src, "@s/pkg/sub").unwrap(), canon("app/node_modules/@s/pkg/sub.js"));
        assert!(r.resolve(&src, "@s/pkg/esm.js").unwrap_err().contains("does not export"));
        assert!(r.resolve(&src, "missing").unwrap_err().contains("cannot find package"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
