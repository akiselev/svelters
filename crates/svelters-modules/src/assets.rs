//! AS1: assets, the files an app imports that are not code (`plans/research/ASSETS.md` §1–§3).
//!
//! An asset is named by an [`AssetRef`], `asset://<root>/<logical path>`: a mounted root (`app`
//! for the primary app) and a `/`-separated path inside it. The ref names a logical path, not
//! content, so it stays the same when the file changes; the content version lives here
//! ([`AssetInfo::version`]) and a change fires [`Assets::on_change`], so a backend repaints what
//! shows the asset without any JS running (§3).
//!
//! [`Assets`] is the one registry a host owns (`Arc`, `Send + Sync`, no toolkit types): the module
//! loader registers the files that imports name, the host's `svelters:assets` ops and the backend's
//! loaders (egui's `asset://` loader) read through it, and the reloader reports changed files to it.

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock, Weak};

/// The scheme of an asset ref.
pub const SCHEME: &str = "asset://";
/// The native module of asset handles and reads (`asset`, `text`, `bytes`, the `Asset` class).
pub const MODULE_ID: &str = "svelters:assets";
/// Its source. It calls the host's `assets` op (`svelters-host`).
pub const MODULE_JS: &str = include_str!("assets.js");
/// The hidden own property of an `Asset` handle that holds its ref string. The host's value
/// converter takes it wherever a handle reaches native code (a prop, an export argument, `Json<T>`).
pub const HANDLE_KEY: &str = "__svelters_asset";

/// The root of the primary app.
pub const APP_ROOT: &str = "app";

/// `asset://<root>/<path>`: a validated, normalised asset name. Crosses every boundary as its
/// string ([`AssetRef::as_uri`]).
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AssetRef(Arc<str>);

impl AssetRef {
    /// The ref of `path` in `root`. The path is normalised (`.` folded, `\` as `/`); `..` that
    /// would leave the root, an empty path and an invalid root name are errors.
    pub fn new(root: &str, path: &str) -> Result<AssetRef, AssetError> {
        check_root(root)?;
        let mut parts: Vec<&str> = Vec::new();
        for p in path.split(['/', '\\']) {
            match p {
                "" | "." => {}
                ".." => {
                    if parts.pop().is_none() {
                        return Err(AssetError::new(format!("asset path {path:?} leaves its root")));
                    }
                }
                p => parts.push(p),
            }
        }
        if parts.is_empty() {
            return Err(AssetError::new(format!("asset path {path:?} is empty")));
        }
        if parts.iter().any(|p| p.contains(['?', '#']) || p.chars().any(char::is_control)) {
            return Err(AssetError::new(format!("asset path {path:?}: `?`, `#` and control characters are not allowed")));
        }
        Ok(AssetRef(format!("{SCHEME}{root}/{}", parts.join("/")).into()))
    }

    /// Parses `asset://<root>/<path>`.
    pub fn parse(uri: &str) -> Result<AssetRef, AssetError> {
        let rest = uri.strip_prefix(SCHEME).ok_or_else(|| AssetError::new(format!("{uri:?} is not an asset ref (asset://<root>/<path>)")))?;
        let (root, path) = rest.split_once('/').ok_or_else(|| AssetError::new(format!("{uri:?}: no path after the root")))?;
        let r = AssetRef::new(root, path)?;
        if &*r.0 != uri {
            return Err(AssetError::new(format!("{uri:?} is not normalised (expected {:?})", &*r.0)));
        }
        Ok(r)
    }

    pub fn root(&self) -> &str {
        let rest = &self.0[SCHEME.len()..];
        &rest[..rest.find('/').unwrap_or(rest.len())]
    }

    pub fn path(&self) -> &str {
        let rest = &self.0[SCHEME.len()..];
        rest.split_once('/').map_or("", |(_, p)| p)
    }

    /// The ref as a uri (what JS sees, what an egui `Image` takes).
    pub fn as_uri(&self) -> &str {
        &self.0
    }

    /// The MIME type from the path's extension.
    pub fn mime(&self) -> &'static str {
        mime_of(self.path())
    }
}

impl fmt::Debug for AssetRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&*self.0, f)
    }
}

impl fmt::Display for AssetRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::str::FromStr for AssetRef {
    type Err = AssetError;
    fn from_str(s: &str) -> Result<AssetRef, AssetError> {
        AssetRef::parse(s)
    }
}

impl TryFrom<String> for AssetRef {
    type Error = AssetError;
    fn try_from(s: String) -> Result<AssetRef, AssetError> {
        AssetRef::parse(&s)
    }
}

impl From<AssetRef> for String {
    fn from(r: AssetRef) -> String {
        r.0.to_string()
    }
}

impl AsRef<str> for AssetRef {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for AssetRef {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for AssetRef {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<AssetRef, D::Error> {
        let s = String::deserialize(d)?;
        AssetRef::parse(&s).map_err(serde::de::Error::custom)
    }
}

/// `{ "type": "string", "format": "svelters-asset" }`: `Json<T>` typings give `Asset`.
#[cfg(feature = "serde")]
impl schemars::JsonSchema for AssetRef {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "AssetRef".into()
    }
    fn inline_schema() -> bool {
        true
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({ "type": "string", "format": "svelters-asset" })
    }
}

fn check_root(root: &str) -> Result<(), AssetError> {
    let ok = !root.is_empty() && root.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'));
    if !ok {
        return Err(AssetError::new(format!("asset root {root:?}: expected [a-z0-9._-]+")));
    }
    Ok(())
}

/// The MIME type of a file name, from its extension (ASSETS.md §1.1's table).
pub fn mime_of(name: &str) -> &'static str {
    let ext = name.rsplit_once('.').map_or("", |(_, e)| e).to_ascii_lowercase();
    match ext.as_str() {
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "woff2" => "font/woff2",
        "txt" => "text/plain",
        "json" => "application/json",
        _ => "application/octet-stream",
    }
}

/// An asset failed to resolve or read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetError(pub String);

impl AssetError {
    fn new(m: impl Into<String>) -> AssetError {
        AssetError(m.into())
    }
}

impl fmt::Display for AssetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for AssetError {}

/// An asset's bytes: borrowed from a static table (zero-copy) or shared.
#[derive(Clone, Debug)]
pub enum AssetBytes {
    Static(&'static [u8]),
    Shared(Arc<[u8]>),
}

impl std::ops::Deref for AssetBytes {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        match self {
            AssetBytes::Static(b) => b,
            AssetBytes::Shared(b) => b,
        }
    }
}

/// What a root serves from.
#[derive(Clone, Debug)]
pub enum AssetSource {
    /// A package directory (dev): files are read when asked and watched.
    Dir(PathBuf),
    /// A fixed table of `(path, bytes)` (tests, tiny embeds).
    Static(&'static [(&'static str, &'static [u8])]),
    /// Q4: a release bundle's files (`.svb`), by their path in the bundle.
    Bundle(Arc<crate::svb::Svb>),
}

/// What [`Assets::info`] knows about an asset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetInfo {
    pub mime: &'static str,
    pub len: u64,
    /// A hash of the content (not cryptographic).
    pub hash: u64,
    /// 0 until the asset first changes; bumped by every change [`Assets::changed`] reports.
    pub version: u64,
}

type Sink = Arc<dyn Fn(&[AssetRef]) + Send + Sync>;

/// Keeps an [`Assets::on_change`] callback registered; dropping it unregisters.
pub struct Subscription {
    id: u64,
    sinks: Weak<Mutex<Vec<(u64, Sink)>>>,
}

impl Drop for Subscription {
    fn drop(&mut self) {
        if let Some(s) = self.sinks.upgrade() {
            s.lock().unwrap().retain(|(i, _)| *i != self.id);
        }
    }
}

impl Subscription {
    /// Keeps the callback for the registry's lifetime.
    pub fn forget(self) {
        std::mem::forget(self);
    }
}

struct Root {
    src: AssetSource,
    /// `Dir` roots: the canonical directory, for confinement.
    canon: Option<PathBuf>,
}

#[derive(Default)]
struct State {
    versions: HashMap<AssetRef, u64>,
    /// Files outside a root's directory that the loader registered (dependencies' assets).
    registered: HashMap<AssetRef, PathBuf>,
    /// Canonical files read or registered, by the ref they serve (the watch list).
    files: HashMap<PathBuf, AssetRef>,
}

/// The asset registry (see the module docs).
pub struct Assets {
    roots: RwLock<BTreeMap<String, Root>>,
    state: Mutex<State>,
    sinks: Arc<Mutex<Vec<(u64, Sink)>>>,
    next: AtomicU64,
}

impl fmt::Debug for Assets {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Assets").field("roots", &self.roots.read().unwrap().keys().collect::<Vec<_>>()).finish()
    }
}

impl Assets {
    pub fn new() -> Arc<Assets> {
        Arc::new(Assets { roots: RwLock::default(), state: Mutex::default(), sinks: Arc::default(), next: AtomicU64::new(1) })
    }

    /// A registry with `app` mounted at `dir`.
    pub fn for_app(dir: &Path) -> Arc<Assets> {
        let a = Assets::new();
        a.mount(APP_ROOT, AssetSource::Dir(dir.to_path_buf())).expect("a fresh registry");
        a
    }

    /// Mounts `root`. A root mounts once (unmount it first); `svelters` is reserved.
    pub fn mount(&self, root: &str, src: AssetSource) -> Result<(), AssetError> {
        check_root(root)?;
        if root == "svelters" {
            return Err(AssetError::new("asset root `svelters` is reserved"));
        }
        let mut roots = self.roots.write().unwrap();
        if roots.contains_key(root) {
            return Err(AssetError::new(format!("asset root `{root}` is already mounted")));
        }
        let canon = match &src {
            AssetSource::Dir(d) => Some(d.canonicalize().map_err(|e| AssetError::new(format!("{}: {e}", d.display())))?),
            AssetSource::Static(_) | AssetSource::Bundle(_) => None,
        };
        roots.insert(root.to_string(), Root { src, canon });
        Ok(())
    }

    pub fn unmount(&self, root: &str) {
        self.roots.write().unwrap().remove(root);
        let mut st = self.state.lock().unwrap();
        st.registered.retain(|r, _| r.root() != root);
        st.files.retain(|_, r| r.root() != root);
    }

    pub fn is_mounted(&self, root: &str) -> bool {
        self.roots.read().unwrap().contains_key(root)
    }

    /// The ref of `file`, an import the module loader resolved, served from `root` (a `Dir` root).
    /// A file inside the root's directory gets its path there; a file elsewhere (a dependency) gets
    /// `node_modules/<package name>/<path in package>`, from the nearest package.json above it, and
    /// is recorded so the ref serves that file.
    pub fn register(&self, root: &str, file: &Path) -> Result<AssetRef, AssetError> {
        let file = file.canonicalize().map_err(|e| AssetError::new(format!("{}: {e}", file.display())))?;
        let dir = {
            let roots = self.roots.read().unwrap();
            let r = roots.get(root).ok_or_else(|| AssetError::new(format!("asset root `{root}` is not mounted")))?;
            r.canon.clone().ok_or_else(|| AssetError::new(format!("asset root `{root}` is not a directory")))?
        };
        let r = match file.strip_prefix(&dir) {
            Ok(rel) => AssetRef::new(root, &slashed(rel))?,
            Err(_) => {
                let (pkg_dir, name) = file
                    .ancestors()
                    .skip(1)
                    .find_map(|d| package_name(d).map(|n| (d.to_path_buf(), n)))
                    .ok_or_else(|| AssetError::new(format!("{}: outside asset root `{root}` and in no package", file.display())))?;
                let rel = file.strip_prefix(&pkg_dir).expect("an ancestor");
                let r = AssetRef::new(root, &format!("node_modules/{name}/{}", slashed(rel)))?;
                self.state.lock().unwrap().registered.insert(r.clone(), file.clone());
                r
            }
        };
        self.state.lock().unwrap().files.insert(file, r.clone());
        Ok(r)
    }

    /// The file behind a ref of a `Dir` root (confined to the root, symlinks resolved), or a
    /// registered dependency file. `None` for static roots and missing files.
    pub fn file_path(&self, r: &AssetRef) -> Option<PathBuf> {
        if let Some(p) = self.state.lock().unwrap().registered.get(r) {
            return Some(p.clone());
        }
        let roots = self.roots.read().unwrap();
        let dir = roots.get(r.root())?.canon.as_ref()?;
        let p = dir.join(r.path()).canonicalize().ok()?;
        p.starts_with(dir).then_some(p)
    }

    /// The asset's bytes.
    pub fn bytes(&self, r: &AssetRef) -> Result<AssetBytes, AssetError> {
        let missing = || AssetError::new(format!("{r}: no such asset"));
        let is_static = {
            let roots = self.roots.read().unwrap();
            let root = roots.get(r.root()).ok_or_else(|| AssetError::new(format!("{r}: asset root `{}` is not mounted", r.root())))?;
            match &root.src {
                AssetSource::Static(table) => Some(AssetBytes::Static(table.iter().find(|(p, _)| *p == r.path()).map(|(_, b)| *b).ok_or_else(missing)?)),
                AssetSource::Bundle(svb) => Some(svb.file_bytes(r.path()).ok_or_else(missing)?),
                AssetSource::Dir(_) => None,
            }
        };
        if let Some(b) = is_static {
            return Ok(b);
        }
        let p = self.file_path(r).ok_or_else(missing)?;
        let bytes = std::fs::read(&p).map_err(|e| AssetError::new(format!("{r}: {}: {e}", p.display())))?;
        self.state.lock().unwrap().files.insert(p, r.clone());
        Ok(AssetBytes::Shared(bytes.into()))
    }

    /// The asset's MIME type, size, content hash and version (reads the content).
    pub fn info(&self, r: &AssetRef) -> Result<AssetInfo, AssetError> {
        let b = self.bytes(r)?;
        let mut h = std::hash::DefaultHasher::new();
        b.hash(&mut h);
        Ok(AssetInfo { mime: r.mime(), len: b.len() as u64, hash: h.finish(), version: self.version(r) })
    }

    /// The asset's version: 0 until it first changes.
    pub fn version(&self, r: &AssetRef) -> u64 {
        self.state.lock().unwrap().versions.get(r).copied().unwrap_or(0)
    }

    /// Calls `f` with the refs of every change [`Assets::changed`] reports, on the thread that
    /// reports it (the host's checkpoint).
    pub fn on_change(&self, f: impl Fn(&[AssetRef]) + Send + Sync + 'static) -> Subscription {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        self.sinks.lock().unwrap().push((id, Arc::new(f)));
        Subscription { id, sinks: Arc::downgrade(&self.sinks) }
    }

    /// The files served so far from directories (imports, reads): what the reloader watches.
    pub fn watch_files(&self) -> Vec<PathBuf> {
        let mut v: Vec<PathBuf> = self.state.lock().unwrap().files.keys().cloned().collect();
        v.sort();
        v
    }

    /// True when `path` is a served asset file.
    pub fn serves(&self, path: &Path) -> bool {
        self.state.lock().unwrap().files.contains_key(&canonical(path))
    }

    /// Changed files (watcher events): bumps the version of each asset they serve, fires the
    /// [`Assets::on_change`] callbacks once with all of them, and returns them.
    pub fn changed(&self, paths: &[PathBuf]) -> Vec<AssetRef> {
        let refs: Vec<AssetRef> = {
            let mut st = self.state.lock().unwrap();
            let mut refs: Vec<AssetRef> = paths.iter().filter_map(|p| st.files.get(&canonical(p)).cloned()).collect();
            refs.sort();
            refs.dedup();
            for r in &refs {
                *st.versions.entry(r.clone()).or_insert(0) += 1;
            }
            refs
        };
        if !refs.is_empty() {
            self.notify(&refs);
        }
        refs
    }

    /// Fires the [`Assets::on_change`] callbacks for `refs` (an overlay edit, a bundle swap).
    pub fn notify(&self, refs: &[AssetRef]) {
        let sinks: Vec<Sink> = self.sinks.lock().unwrap().iter().map(|(_, s)| s.clone()).collect();
        for s in sinks {
            s(refs);
        }
    }
}

fn canonical(p: &Path) -> PathBuf {
    p.canonicalize().unwrap_or_else(|_| crate::normalize(&std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf())))
}

fn slashed(rel: &Path) -> String {
    rel.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/")
}

/// The `name` of `dir/package.json`, when there is one.
pub(crate) fn package_name(dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join("package.json")).ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    json.get("name")?.as_str().map(str::to_string)
}

/// `data:<mime>;base64,<bytes>` (`?inline` imports).
pub fn data_url(mime: &str, bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(mime.len() + 13 + bytes.len().div_ceil(3) * 4);
    s.push_str("data:");
    s.push_str(mime);
    s.push_str(";base64,");
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= c.len() {
                s.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refs_validate_and_normalise() {
        let r = AssetRef::new("app", "./src//icons/../icon.svg").unwrap();
        assert_eq!(r.as_uri(), "asset://app/src/icon.svg");
        assert_eq!((r.root(), r.path(), r.mime()), ("app", "src/icon.svg", "image/svg+xml"));
        assert_eq!(AssetRef::parse("asset://ext.acme-x/a/b.png").unwrap().path(), "a/b.png");
        assert!(AssetRef::new("app", "../x.svg").is_err());
        assert!(AssetRef::new("App", "x.svg").is_err());
        assert!(AssetRef::new("app", "x.svg?v=2").is_err());
        assert!(AssetRef::parse("asset://app/./x.svg").is_err());
        assert!(AssetRef::parse("file:///x.svg").is_err());
        assert_eq!(std::path::Path::new(r.as_uri()).extension().unwrap(), "svg");
    }

    #[test]
    fn base64_data_urls() {
        assert_eq!(data_url("text/plain", b""), "data:text/plain;base64,");
        assert_eq!(data_url("text/plain", b"f"), "data:text/plain;base64,Zg==");
        assert_eq!(data_url("text/plain", b"fo"), "data:text/plain;base64,Zm8=");
        assert_eq!(data_url("text/plain", b"foobar"), "data:text/plain;base64,Zm9vYmFy");
    }

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("svelters-assets-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("src")).unwrap();
        d
    }

    #[test]
    fn dir_roots_serve_watch_and_version() {
        let d = tmp("dir");
        std::fs::write(d.join("src/i.svg"), "<svg/>").unwrap();
        let a = Assets::for_app(&d);
        let r = a.register(APP_ROOT, &d.join("src/i.svg")).unwrap();
        assert_eq!(r.as_uri(), "asset://app/src/i.svg");
        assert_eq!(&*a.bytes(&r).unwrap(), b"<svg/>");
        assert!(a.serves(&d.join("src/i.svg")));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let s2 = seen.clone();
        let sub = a.on_change(move |refs| s2.lock().unwrap().extend(refs.iter().cloned()));
        std::fs::write(d.join("src/i.svg"), "<svg width='2'/>").unwrap();
        assert_eq!(a.changed(&[d.join("src/i.svg"), d.join("src/other.js")]), vec![r.clone()]);
        assert_eq!(a.info(&r).unwrap().version, 1);
        assert_eq!(*seen.lock().unwrap(), vec![r.clone()]);
        drop(sub);
        a.changed(&[d.join("src/i.svg")]);
        assert_eq!(seen.lock().unwrap().len(), 1);
        // confinement: a ref never leaves its root
        std::fs::write(d.join("../svelters-assets-outside.txt"), "x").unwrap();
        assert!(a.bytes(&AssetRef::new(APP_ROOT, "nope.svg").unwrap()).is_err());
        assert!(a.mount(APP_ROOT, AssetSource::Static(&[])).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn dependency_files_get_package_paths() {
        let d = tmp("dep");
        let pkg = d.join("node_modules/@acme/icons");
        std::fs::create_dir_all(pkg.join("svg")).unwrap();
        std::fs::write(pkg.join("package.json"), r#"{"name":"@acme/icons"}"#).unwrap();
        std::fs::write(pkg.join("svg/gear.svg"), "<svg/>").unwrap();
        let app = tmp("dep-app");
        let a = Assets::for_app(&app);
        let r = a.register(APP_ROOT, &pkg.join("svg/gear.svg")).unwrap();
        assert_eq!(r.as_uri(), "asset://app/node_modules/@acme/icons/svg/gear.svg");
        assert_eq!(&*a.bytes(&r).unwrap(), b"<svg/>");
        let _ = std::fs::remove_dir_all(&d);
        let _ = std::fs::remove_dir_all(&app);
    }

    #[test]
    fn static_roots() {
        static TABLE: &[(&str, &[u8])] = &[("i.svg", b"<svg/>")];
        let a = Assets::new();
        a.mount("cad", AssetSource::Static(TABLE)).unwrap();
        let r = AssetRef::new("cad", "i.svg").unwrap();
        assert!(matches!(a.bytes(&r).unwrap(), AssetBytes::Static(_)));
        assert_eq!(a.info(&r).unwrap().len, 6);
        assert!(a.file_path(&r).is_none());
        assert!(a.bytes(&AssetRef::new("cad", "j.svg").unwrap()).is_err());
        assert!(a.mount("svelters", AssetSource::Static(TABLE)).is_err());
    }
}
