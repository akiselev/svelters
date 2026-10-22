//! Q4: the release bundle, `.svb` (`plans/research/ASSETS.md` §4, `plans/milestones/Q4-bundle-format.md`).
//!
//! One container for an app (and later for extensions, AS3): every module's compiled JS, its
//! source map, optionally its source and QuickJS bytecode, the import table that replaces
//! resolution at startup, the assets the graph references, the package manifests, and a table of
//! versioned, optional sections for data added later (pre-parsed stylesheets, OW-CSS).
//!
//! ```text
//! "SVB1" | u32 LE index_len | index (UTF-8 JSON) | padding to 16 | blob area (each blob 16-aligned)
//! ```
//!
//! The index names blobs by number; `blobs[n] = [offset, len]` is relative to the start of the blob
//! area. Blobs are content-addressed: identical bytes are stored once. Format 1 stores every blob
//! raw (`codec: "none"`); `deflate` is reserved.
//!
//! Compatibility: `format` changes only for changes old readers cannot ignore. New index keys and
//! new sections are additive. A section marked `required` that a reader does not know makes the
//! bundle unreadable for it; any other unknown section is skipped.

mod provider;

pub use provider::SvbProvider;
/// The builder (feature `source`): an app's sources to a `.svb`.
#[cfg(feature = "source")]
pub use crate::source::build::{BuildOptions, build};

use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::Arc;

use crate::AssetBytes;

/// The four bytes every bundle starts with.
pub const MAGIC: &[u8; 4] = b"SVB1";
/// The index format this reader and writer speak.
pub const FORMAT: u64 = 1;
/// Section names this reader understands. A bundle with a `required` section not listed here is
/// refused. (None yet: OW-CSS's parsed stylesheets would be the first.)
pub const KNOWN_SECTIONS: &[&str] = &[];
/// The engine tag of bundle bytecode (`ModuleCode::Cached`).
pub const ENGINE: &str = "quickjs-ng";
const ALIGN: usize = 16;

/// What a bundle holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BundleKind {
    App,
    Extension,
}

impl BundleKind {
    fn as_str(self) -> &'static str {
        match self {
            BundleKind::App => "app",
            BundleKind::Extension => "extension",
        }
    }
}

/// What a file entry is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileRole {
    /// A module's source (`.svelte`, `.svelte.js`, `.js`), for live compile and editors.
    Source,
    /// A `package.json`.
    Manifest,
    /// Served through `asset://<root>/<path>`.
    Asset,
}

impl FileRole {
    fn as_str(self) -> &'static str {
        match self {
            FileRole::Source => "source",
            FileRole::Manifest => "manifest",
            FileRole::Asset => "asset",
        }
    }

    fn parse(s: &str) -> Result<FileRole, String> {
        Ok(match s {
            "source" => FileRole::Source,
            "manifest" => FileRole::Manifest,
            "asset" => FileRole::Asset,
            _ => return Err(format!("unknown file role {s:?}")),
        })
    }
}

/// Which tools and engine wrote a bundle.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Stamp {
    /// The svelters-compiler (workspace) version that compiled the JS.
    pub compiler: String,
    /// `@svelters/native-runtime`'s version.
    pub runtime: Option<String>,
    /// The engine stamp of the bytecode (`svelters_host::bytecode::stamp()`); `None` when the bundle
    /// has none. Bytecode is used only by an engine with exactly this stamp.
    pub engine_build: Option<String>,
}

/// One file (an asset, a manifest, a source).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileEntry {
    pub blob: usize,
    /// The content hash (FNV-1a 64, hex).
    pub hash: String,
    pub role: FileRole,
    pub mime: Option<String>,
}

/// One module.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ModuleEntry {
    /// The module's source file (a key of `files`), when sources are shipped.
    pub source: Option<String>,
    /// The hash of the source the JS was compiled from (FNV-1a 64, hex); `None` for generated modules.
    pub source_hash: Option<String>,
    pub js: usize,
    pub map: Option<usize>,
    pub bytecode: Option<usize>,
    /// Specifier -> module id, as resolved at build time. Host modules (`svelters:core`,
    /// `svelters:app/*`, …) are not listed: the host serves them before a provider is asked.
    pub imports: BTreeMap<String, String>,
}

/// A versioned entry of the section table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    pub version: u32,
    /// A reader that does not know the section must refuse the bundle.
    pub required: bool,
    pub blob: usize,
}

/// A bundle's index.
#[derive(Clone, Debug, PartialEq)]
pub struct Index {
    pub kind: BundleKind,
    /// The asset root its files are served under (`app`, `ext.<id>`); module ids start with it.
    pub root: String,
    /// The package's version.
    pub version: Option<String>,
    /// A hash of the index (written by [`SvbWriter::finish`]): names this build.
    pub build: String,
    pub stamp: Stamp,
    /// The entry module's id.
    pub entry: String,
    pub files: BTreeMap<String, FileEntry>,
    pub modules: BTreeMap<String, ModuleEntry>,
    pub sections: BTreeMap<String, Section>,
    /// Embedder data, opaque.
    pub meta: Value,
}

#[derive(Clone)]
enum Data {
    Static(&'static [u8]),
    Shared(Arc<[u8]>),
}

impl Data {
    fn bytes(&self) -> &[u8] {
        match self {
            Data::Static(b) => b,
            Data::Shared(b) => b,
        }
    }
}

/// A read `.svb`: the index and the bytes it points into. Cheap to share (`Arc<Svb>`).
pub struct Svb {
    data: Data,
    index: Index,
    blobs: Vec<(usize, usize)>,
}

impl std::fmt::Debug for Svb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Svb").field("root", &self.index.root).field("build", &self.index.build).field("modules", &self.index.modules.len()).finish()
    }
}

pub(crate) fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// The content hash used in the index.
pub fn content_hash(bytes: &[u8]) -> String {
    format!("{:016x}", fnv1a(bytes))
}

fn align(n: usize) -> usize {
    n.div_ceil(ALIGN) * ALIGN
}

impl Svb {
    /// A bundle compiled into the binary (`include_bytes!`): blobs are served zero-copy.
    pub fn from_static(bytes: &'static [u8]) -> Result<Svb, String> {
        Svb::parse(Data::Static(bytes))
    }

    /// A bundle in memory.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Svb, String> {
        Svb::parse(Data::Shared(bytes.into()))
    }

    /// A bundle file, read whole. (Memory-mapping is a later step, ASSETS.md §4.3.)
    pub fn open(path: &Path) -> Result<Svb, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Svb::from_bytes(bytes).map_err(|e| format!("{}: {e}", path.display()))
    }

    fn parse(data: Data) -> Result<Svb, String> {
        let b = data.bytes();
        if b.len() < 8 || &b[..4] != MAGIC {
            return Err("not a svelters bundle (.svb)".into());
        }
        let index_len = u32::from_le_bytes(b[4..8].try_into().unwrap()) as usize;
        let index_end = 8usize.checked_add(index_len).filter(|e| *e <= b.len()).ok_or("bundle truncated (index)")?;
        let json: Value = serde_json::from_slice(&b[8..index_end]).map_err(|e| format!("bundle index: {e}"))?;
        let area = align(index_end);
        let blobs = json
            .get("blobs")
            .and_then(Value::as_array)
            .ok_or("bundle index: no blob table")?
            .iter()
            .map(|v| {
                let pair = v.as_array().filter(|a| a.len() == 2).ok_or("bundle index: bad blob entry")?;
                let off = pair[0].as_u64().ok_or("bundle index: bad blob offset")? as usize;
                let len = pair[1].as_u64().ok_or("bundle index: bad blob length")? as usize;
                let start = area.checked_add(off).ok_or("bundle blob out of range")?;
                start.checked_add(len).filter(|e| *e <= b.len()).ok_or("bundle truncated (blob out of range)")?;
                Ok((start, len))
            })
            .collect::<Result<Vec<_>, &str>>()?;
        let index = Index::from_json(&json, blobs.len())?;
        Ok(Svb { data, index, blobs })
    }

    pub fn index(&self) -> &Index {
        &self.index
    }

    /// Blob `n`'s bytes.
    pub fn blob(&self, n: usize) -> &[u8] {
        let (start, len) = self.blobs[n];
        &self.data.bytes()[start..start + len]
    }

    fn blob_bytes(&self, n: usize) -> AssetBytes {
        let (start, len) = self.blobs[n];
        match &self.data {
            Data::Static(b) => AssetBytes::Static(&b[start..start + len]),
            Data::Shared(b) => AssetBytes::Shared(b[start..start + len].into()),
        }
    }

    /// File `path` (relative to the bundle's root).
    pub fn file(&self, path: &str) -> Option<&[u8]> {
        self.index.files.get(path).map(|f| self.blob(f.blob))
    }

    /// File `path` as asset bytes (zero-copy for a static bundle).
    pub fn file_bytes(&self, path: &str) -> Option<AssetBytes> {
        self.index.files.get(path).map(|f| self.blob_bytes(f.blob))
    }

    /// Module `id`'s JavaScript.
    pub fn module_js(&self, id: &str) -> Result<String, String> {
        let m = self.index.modules.get(id).ok_or_else(|| format!("{id}: not in the bundle"))?;
        String::from_utf8(self.blob(m.js).to_vec()).map_err(|_| format!("{id}: JS is not UTF-8"))
    }

    /// A section's version and bytes. Unknown sections are kept but not interpreted.
    pub fn section(&self, name: &str) -> Option<(u32, &[u8])> {
        self.index.sections.get(name).map(|s| (s.version, self.blob(s.blob)))
    }

    /// A copy of this bundle with each listed module's bytecode set, stamped with the engine that
    /// wrote it (`precompile`, ASSETS.md §4.2). Modules not listed lose any bytecode they had.
    pub fn with_bytecode(&self, bytecode: BTreeMap<String, Vec<u8>>, engine_build: String) -> Result<Vec<u8>, String> {
        let mut w = SvbWriter::from_svb(self);
        for m in w.index.modules.values_mut() {
            m.bytecode = None;
        }
        for (id, bytes) in bytecode {
            let blob = w.blob(&bytes);
            w.index.modules.get_mut(&id).ok_or_else(|| format!("{id}: not in the bundle"))?.bytecode = Some(blob);
        }
        w.index.stamp.engine_build = Some(engine_build);
        Ok(w.finish())
    }
}

/// Builds a `.svb`.
pub struct SvbWriter {
    pub index: Index,
    blobs: Vec<Vec<u8>>,
    by_hash: HashMap<u64, Vec<usize>>,
}

impl SvbWriter {
    pub fn new(kind: BundleKind, root: &str, entry: &str) -> SvbWriter {
        SvbWriter {
            index: Index {
                kind,
                root: root.to_string(),
                version: None,
                build: String::new(),
                stamp: Stamp { compiler: env!("CARGO_PKG_VERSION").to_string(), ..Stamp::default() },
                entry: entry.to_string(),
                files: BTreeMap::new(),
                modules: BTreeMap::new(),
                sections: BTreeMap::new(),
                meta: Value::Object(Map::new()),
            },
            blobs: Vec::new(),
            by_hash: HashMap::new(),
        }
    }

    /// A writer holding everything `svb` holds (to change and write again).
    pub fn from_svb(svb: &Svb) -> SvbWriter {
        let mut w = SvbWriter { index: svb.index.clone(), blobs: Vec::new(), by_hash: HashMap::new() };
        for n in 0..svb.blobs.len() {
            w.blobs.push(svb.blob(n).to_vec());
            w.by_hash.entry(fnv1a(svb.blob(n))).or_default().push(n);
        }
        w
    }

    /// Stores `bytes` once and returns its blob number.
    pub fn blob(&mut self, bytes: &[u8]) -> usize {
        let h = fnv1a(bytes);
        if let Some(n) = self.by_hash.get(&h).and_then(|ns| ns.iter().copied().find(|n| self.blobs[*n] == bytes)) {
            return n;
        }
        self.blobs.push(bytes.to_vec());
        let n = self.blobs.len() - 1;
        self.by_hash.entry(h).or_default().push(n);
        n
    }

    pub fn file(&mut self, path: &str, bytes: &[u8], role: FileRole, mime: Option<&str>) {
        let blob = self.blob(bytes);
        self.index.files.insert(path.to_string(), FileEntry { blob, hash: content_hash(bytes), role, mime: mime.map(str::to_string) });
    }

    pub fn module(&mut self, id: &str, entry: ModuleEntry) {
        self.index.modules.insert(id.to_string(), entry);
    }

    /// Adds a section (OW-CSS's parsed stylesheets, for example). `required` makes readers that do
    /// not know `name` refuse the bundle; leave it off for data that is only an optimisation.
    pub fn section(&mut self, name: &str, version: u32, required: bool, bytes: &[u8]) {
        let blob = self.blob(bytes);
        self.index.sections.insert(name.to_string(), Section { version, required, blob });
    }

    /// The bundle's bytes. Blobs nothing refers to are dropped.
    pub fn finish(mut self) -> Vec<u8> {
        // Keep referenced blobs only, renumbered in first-use order (stable output).
        let mut used: Vec<usize> = Vec::new();
        let mut remap = HashMap::new();
        let mut take = |n: &mut usize| {
            let next = remap.len();
            *n = *remap.entry(*n).or_insert_with(|| {
                used.push(*n);
                next
            });
        };
        for f in self.index.files.values_mut() {
            take(&mut f.blob);
        }
        for m in self.index.modules.values_mut() {
            take(&mut m.js);
            m.map.as_mut().map(&mut take);
            m.bytecode.as_mut().map(&mut take);
        }
        for s in self.index.sections.values_mut() {
            take(&mut s.blob);
        }
        let mut table = Vec::new();
        let mut off = 0usize;
        for &n in &used {
            table.push(json!([off, self.blobs[n].len()]));
            off = align(off + self.blobs[n].len());
        }
        self.index.build = String::new();
        let mut index = self.index.to_json(&table);
        let build = content_hash(index.to_string().as_bytes());
        index["build"] = Value::String(build);
        let index = index.to_string();
        let mut out = Vec::with_capacity(align(8 + index.len()) + off);
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&u32::try_from(index.len()).expect("index under 4 GiB").to_le_bytes());
        out.extend_from_slice(index.as_bytes());
        out.resize(align(out.len()), 0);
        let area = out.len();
        for &n in &used {
            out.resize(area + align(out.len() - area), 0);
            out.extend_from_slice(&self.blobs[n]);
        }
        out
    }
}

impl Index {
    fn to_json(&self, blobs: &[Value]) -> Value {
        let files: Map<String, Value> = self
            .files
            .iter()
            .map(|(k, f)| {
                let mut o = json!({ "blob": f.blob, "hash": f.hash, "codec": "none", "role": f.role.as_str() });
                if let Some(m) = &f.mime {
                    o["mime"] = json!(m);
                }
                (k.clone(), o)
            })
            .collect();
        let modules: Map<String, Value> = self
            .modules
            .iter()
            .map(|(k, m)| {
                let mut o = json!({ "js": m.js, "imports": m.imports });
                for (key, v) in [("map", m.map), ("bytecode", m.bytecode)] {
                    if let Some(v) = v {
                        o[key] = json!(v);
                    }
                }
                if let Some(s) = &m.source {
                    o["source"] = json!(s);
                }
                if let Some(s) = &m.source_hash {
                    o["source_hash"] = json!(s);
                }
                (k.clone(), o)
            })
            .collect();
        let sections: Map<String, Value> =
            self.sections.iter().map(|(k, s)| (k.clone(), json!({ "version": s.version, "required": s.required, "blob": s.blob }))).collect();
        json!({
            "format": FORMAT,
            "kind": self.kind.as_str(),
            "root": self.root,
            "version": self.version,
            "build": self.build,
            "stamp": {
                "compiler": self.stamp.compiler,
                "runtime": self.stamp.runtime,
                "engine": ENGINE,
                "engine_build": self.stamp.engine_build,
            },
            "entry": self.entry,
            "files": files,
            "modules": modules,
            "sections": sections,
            "meta": self.meta,
            "blobs": blobs,
        })
    }

    fn from_json(v: &Value, nblobs: usize) -> Result<Index, String> {
        let err = |what: &str| format!("bundle index: {what}");
        let format = v.get("format").and_then(Value::as_u64).ok_or_else(|| err("no format"))?;
        if format != FORMAT {
            return Err(format!("bundle format {format} is not supported (this build reads format {FORMAT})"));
        }
        let str_of = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
        let blob = |v: &Value, k: &str| -> Result<Option<usize>, String> {
            match v.get(k) {
                None | Some(Value::Null) => Ok(None),
                Some(b) => b.as_u64().map(|b| b as usize).filter(|b| *b < nblobs).map(Some).ok_or_else(|| err(&format!("bad blob reference {k}"))),
            }
        };
        let kind = match v.get("kind").and_then(Value::as_str) {
            Some("app") => BundleKind::App,
            Some("extension") => BundleKind::Extension,
            k => return Err(err(&format!("unknown kind {k:?}"))),
        };
        let stamp = v.get("stamp").ok_or_else(|| err("no stamp"))?;
        let engine = stamp.get("engine").and_then(Value::as_str);
        let stamp = Stamp {
            compiler: str_of(stamp, "compiler").unwrap_or_default(),
            runtime: str_of(stamp, "runtime"),
            // Bytecode for another engine is as good as none.
            engine_build: str_of(stamp, "engine_build").filter(|_| engine == Some(ENGINE)),
        };
        let obj = |k: &str| v.get(k).and_then(Value::as_object).cloned().unwrap_or_default();
        let mut files = BTreeMap::new();
        for (k, f) in obj("files") {
            if let Some(c) = f.get("codec").and_then(Value::as_str).filter(|c| *c != "none") {
                return Err(err(&format!("{k}: codec {c:?} is not supported")));
            }
            let role = FileRole::parse(f.get("role").and_then(Value::as_str).unwrap_or(""))?;
            files.insert(k, FileEntry { blob: blob(&f, "blob")?.ok_or_else(|| err("file without blob"))?, hash: str_of(&f, "hash").unwrap_or_default(), role, mime: str_of(&f, "mime") });
        }
        let mut modules = BTreeMap::new();
        for (k, m) in obj("modules") {
            let imports = m
                .get("imports")
                .and_then(Value::as_object)
                .map(|o| o.iter().filter_map(|(s, id)| Some((s.clone(), id.as_str()?.to_string()))).collect())
                .unwrap_or_default();
            modules.insert(
                k.clone(),
                ModuleEntry {
                    source: str_of(&m, "source"),
                    source_hash: str_of(&m, "source_hash"),
                    js: blob(&m, "js")?.ok_or_else(|| err(&format!("{k}: no js")))?,
                    map: blob(&m, "map")?,
                    bytecode: blob(&m, "bytecode")?,
                    imports,
                },
            );
        }
        let mut sections = BTreeMap::new();
        for (k, s) in obj("sections") {
            let required = s.get("required").and_then(Value::as_bool).unwrap_or(false);
            if required && !KNOWN_SECTIONS.contains(&k.as_str()) {
                return Err(format!("bundle section {k:?} is required but not supported by this build"));
            }
            let version = s.get("version").and_then(Value::as_u64).and_then(|v| u32::try_from(v).ok()).ok_or_else(|| err(&format!("section {k}: no version")))?;
            sections.insert(k, Section { version, required, blob: blob(&s, "blob")?.ok_or_else(|| err("section without blob"))? });
        }
        let entry = str_of(v, "entry").ok_or_else(|| err("no entry"))?;
        if !modules.contains_key(&entry) {
            return Err(err(&format!("entry {entry:?} is not a module")));
        }
        Ok(Index {
            kind,
            root: str_of(v, "root").ok_or_else(|| err("no root"))?,
            version: str_of(v, "version"),
            build: str_of(v, "build").unwrap_or_default(),
            stamp,
            entry,
            files,
            modules,
            sections,
            meta: v.get("meta").cloned().unwrap_or(Value::Null),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> SvbWriter {
        let mut w = SvbWriter::new(BundleKind::App, "app", "app/main.js");
        let js = w.blob(b"import './a.js';");
        let a = w.blob(b"export {}");
        let map = w.blob(b"{\"version\":3}");
        w.module("app/main.js", ModuleEntry { js, map: Some(map), imports: [("./a.js".to_string(), "app/a.js".to_string())].into(), ..ModuleEntry::default() });
        w.module("app/a.js", ModuleEntry { js: a, ..ModuleEntry::default() });
        w.file("icon.svg", b"<svg/>", FileRole::Asset, Some("image/svg+xml"));
        w.file("copy.svg", b"<svg/>", FileRole::Asset, None);
        w.blob(b"unreferenced");
        w
    }

    #[test]
    fn round_trips_with_aligned_deduplicated_blobs() {
        let bytes = sample().finish();
        let svb = Svb::from_bytes(bytes.clone()).unwrap();
        let ix = svb.index();
        assert_eq!(ix.entry, "app/main.js");
        assert_eq!(svb.module_js("app/main.js").unwrap(), "import './a.js';");
        assert_eq!(ix.modules["app/main.js"].imports["./a.js"], "app/a.js");
        assert_eq!(svb.blob(ix.modules["app/main.js"].map.unwrap()), b"{\"version\":3}");
        assert_eq!(svb.file("icon.svg").unwrap(), b"<svg/>");
        assert_eq!(ix.files["icon.svg"].blob, ix.files["copy.svg"].blob, "identical files share a blob");
        assert_eq!(ix.files["icon.svg"].mime.as_deref(), Some("image/svg+xml"));
        assert_eq!(svb.blobs.len(), 4, "the unreferenced blob is dropped");
        assert!(svb.blobs.iter().all(|(start, _)| start % ALIGN == 0));
        assert_eq!(ix.build.len(), 16);
        // Deterministic: the same input gives the same bytes and build id.
        assert_eq!(sample().finish(), bytes);
        // A static bundle reads the same.
        let leaked: &'static [u8] = Box::leak(bytes.into_boxed_slice());
        assert!(matches!(Svb::from_static(leaked).unwrap().file_bytes("icon.svg"), Some(AssetBytes::Static(b"<svg/>"))));
    }

    #[test]
    fn sections_are_versioned_and_unknown_required_ones_refused() {
        let mut w = sample();
        w.section("css.parsed", 3, false, b"sheets");
        let svb = Svb::from_bytes(w.finish()).unwrap();
        assert_eq!(svb.section("css.parsed"), Some((3, &b"sheets"[..])));
        assert_eq!(svb.section("other"), None);

        let mut w = sample();
        w.section("future.must-know", 1, true, b"x");
        assert!(Svb::from_bytes(w.finish()).unwrap_err().contains("required but not supported"));
    }

    #[test]
    fn bad_input_is_an_error() {
        assert!(Svb::from_bytes(b"PK\x03\x04".to_vec()).unwrap_err().contains("not a svelters bundle"));
        let bytes = sample().finish();
        assert!(Svb::from_bytes(bytes[..bytes.len() - 3].to_vec()).unwrap_err().contains("truncated"));
        assert!(Svb::from_bytes(bytes[..20].to_vec()).unwrap_err().contains("truncated"));
        let future = String::from_utf8_lossy(&bytes).replacen("\"format\":1", "\"format\":2", 1).into_bytes();
        assert!(Svb::from_bytes(future).unwrap_err().contains("format 2"));
    }

    #[test]
    fn with_bytecode_stamps_the_engine() {
        let svb = Svb::from_bytes(sample().finish()).unwrap();
        assert_eq!(svb.index().stamp.engine_build, None);
        let out = svb.with_bytecode([("app/a.js".to_string(), vec![1, 2, 3])].into(), "quickjs-ng test".into()).unwrap();
        let svb = Svb::from_bytes(out).unwrap();
        assert_eq!(svb.index().stamp.engine_build.as_deref(), Some("quickjs-ng test"));
        assert_eq!(svb.blob(svb.index().modules["app/a.js"].bytecode.unwrap()), [1, 2, 3]);
        assert_eq!(svb.index().modules["app/main.js"].bytecode, None);
        assert!(svb.with_bytecode([("nope".to_string(), vec![])].into(), String::new()).is_err());
    }
}
