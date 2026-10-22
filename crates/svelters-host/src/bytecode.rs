//! Q2: the on-disk QuickJS bytecode cache (ASSETS.md AS-O2: compiled JS ships, bytecode is cached at
//! first launch).
//!
//! A module the provider hands over as JavaScript ([`svelters_modules::ModuleCode::Source`]) is
//! compiled by QuickJS once and its bytecode written here; the next launch reads the bytecode instead
//! of parsing the JavaScript again. The cache sits in a directory the host supplies (DX1's
//! `Paths::cache()/bytecode` by default, [`crate::Host::set_bytecode_cache`]).
//!
//! Layout: `<root>/<stamp hash>/STAMP` (the full [`stamp`] text) and `<root>/<stamp hash>/<key>.qbc`,
//! one file per module.
//! - **Engine stamp.** Bytecode is only valid for the QuickJS-NG build that wrote it, so every entry
//!   lives under the hash of [`stamp`]: the QuickJS-NG version (which moves with its bytecode format),
//!   the write flags, the target and profile, and this crate's version. A different engine reads a
//!   different directory; directories of other stamps not opened for [`STALE_STAMP`] are removed.
//! - **Key.** `fnv1a64(stamp, module name, JavaScript text)`: a changed source (or a source compiled
//!   differently, e.g. with HMR) is a different key. The name is part of the key because QuickJS
//!   stores it in the bytecode.
//! - **Safe invalidation.** Each file carries a header (magic, key, name, the JavaScript's hash, the
//!   payload's length and checksum) that is checked before the bytes reach `JS_ReadObject`, so a
//!   truncated, corrupt or foreign file is a miss, not a crash; a miss recompiles and rewrites the
//!   entry. Writes go to a temporary file renamed into place, so readers never see half a file, and
//!   two processes writing the same entry both succeed. Entries not written for [`STALE_ENTRY`] are
//!   removed when the cache is opened (an entry in use is rewritten once that long, no more).
//! - **Trust.** The checksum detects damage, not tampering: the directory is the app's own cache,
//!   written by this process, and is as trusted as the app's binary (QUICKJS-RUNTIME §3.5: never
//!   read bytecode from users). Bytecode that comes with an extension bundle is not this cache.
// ZU3: the module loader's bytecode cache: file I/O while loading modules (startup, reload), never per frame.
#![allow(clippy::disallowed_methods)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

const MAGIC: &[u8; 8] = b"SVQBC\x01\0\0";
/// Other stamps' directories not opened for this long are removed.
pub const STALE_STAMP: Duration = Duration::from_secs(7 * 24 * 3600);
/// Entries not written for this long are removed.
pub const STALE_ENTRY: Duration = Duration::from_secs(30 * 24 * 3600);
/// Leftover temporary files (a writer that died) older than this are removed.
const STALE_TMP: Duration = Duration::from_secs(3600);

/// The engine stamp: everything the bytecode's validity depends on.
pub fn stamp() -> String {
    // Safety: `JS_GetVersion` returns a static NUL-terminated string.
    let version = unsafe { std::ffi::CStr::from_ptr(rquickjs::qjs::JS_GetVersion()) }.to_string_lossy().into_owned();
    format!(
        "quickjs-ng {version}; write-flags {:#x}; {}-{}; {}-bit {}-endian; {}; svelters-host {}",
        write_options().to_flag(),
        std::env::consts::ARCH,
        std::env::consts::OS,
        usize::BITS,
        if cfg!(target_endian = "little") { "little" } else { "big" },
        if cfg!(debug_assertions) { "debug" } else { "release" },
        env!("CARGO_PKG_VERSION"),
    )
}

/// How modules are written: native endianness, source and debug info kept (stack traces,
/// `Function.prototype.toString`).
pub(crate) fn write_options() -> rquickjs::WriteOptions {
    rquickjs::WriteOptions::default()
}

fn fnv1a(h: &mut u64, bytes: &[u8]) {
    for &b in bytes {
        *h ^= b as u64;
        *h = h.wrapping_mul(0x0100_0000_01b3);
    }
}

fn hash(parts: &[&[u8]]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325;
    for p in parts {
        fnv1a(&mut h, &(p.len() as u64).to_le_bytes());
        fnv1a(&mut h, p);
    }
    h
}

/// The bytecode cache of one engine stamp (see the module docs).
#[derive(Debug)]
pub struct BytecodeCache {
    dir: PathBuf,
    stamp: String,
    hits: AtomicU64,
    writes: AtomicU64,
}

static TMP_SEQ: AtomicU64 = AtomicU64::new(0);

impl BytecodeCache {
    /// Opens (creating it) the cache under `root` for this engine's [`stamp`], and prunes other
    /// stamps' stale directories and stale entries.
    pub fn open(root: impl AsRef<Path>) -> std::io::Result<BytecodeCache> {
        Self::open_stamped(root.as_ref(), stamp())
    }

    pub(crate) fn open_stamped(root: &Path, stamp: String) -> std::io::Result<BytecodeCache> {
        let dir = root.join(format!("{:016x}", hash(&[stamp.as_bytes()])));
        std::fs::create_dir_all(&dir)?;
        // rewritten on every open: its mtime says when this stamp was last used
        std::fs::write(dir.join("STAMP"), &stamp)?;
        let cache = BytecodeCache { dir, stamp, hits: AtomicU64::new(0), writes: AtomicU64::new(0) };
        cache.prune(root);
        Ok(cache)
    }

    /// The directory of this stamp's entries.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn stamp(&self) -> &str {
        &self.stamp
    }

    /// Entries read back valid by [`BytecodeCache::get`] since the cache was opened.
    pub fn hits(&self) -> u64 {
        self.hits.load(Ordering::Relaxed)
    }

    /// Entries written by [`BytecodeCache::put`] since the cache was opened.
    pub fn writes(&self) -> u64 {
        self.writes.load(Ordering::Relaxed)
    }

    fn key(&self, name: &str, js: &str) -> u64 {
        hash(&[self.stamp.as_bytes(), name.as_bytes(), js.as_bytes()])
    }

    fn path(&self, key: u64) -> PathBuf {
        self.dir.join(format!("{key:016x}.qbc"))
    }

    /// The bytecode of module `name` compiled from `js`, if a valid entry exists.
    pub fn get(&self, name: &str, js: &str) -> Option<Vec<u8>> {
        let key = self.key(name, js);
        let file = std::fs::read(self.path(key)).ok()?;
        let bytes = decode(&file, key, name, js)?.to_vec();
        self.hits.fetch_add(1, Ordering::Relaxed);
        Some(bytes)
    }

    /// Stores the bytecode of module `name` compiled from `js` (atomically: temporary file, rename).
    pub fn put(&self, name: &str, js: &str, bytes: &[u8]) -> std::io::Result<()> {
        let key = self.key(name, js);
        let tmp = self.dir.join(format!("{key:016x}.tmp.{}.{}", std::process::id(), TMP_SEQ.fetch_add(1, Ordering::Relaxed)));
        std::fs::write(&tmp, encode(key, name, js, bytes))?;
        std::fs::rename(&tmp, self.path(key)).inspect_err(|_| {
            let _ = std::fs::remove_file(&tmp);
        })?;
        self.writes.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// Drops the entry of module `name` compiled from `js` (the engine rejected it).
    pub fn remove(&self, name: &str, js: &str) {
        let _ = std::fs::remove_file(self.path(self.key(name, js)));
    }

    fn prune(&self, root: &Path) {
        let now = SystemTime::now();
        let age = |p: &Path| p.metadata().and_then(|m| m.modified()).ok().and_then(|t| now.duration_since(t).ok()).unwrap_or_default();
        // other stamps: only directories this cache made (a hash name and a STAMP file)
        for e in std::fs::read_dir(root).into_iter().flatten().flatten() {
            let p = e.path();
            let ours = p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.len() == 16 && n.bytes().all(|b| b.is_ascii_hexdigit()));
            if p != self.dir && ours && p.join("STAMP").is_file() && age(&p.join("STAMP")) > STALE_STAMP {
                let _ = std::fs::remove_dir_all(&p);
            }
        }
        for e in std::fs::read_dir(&self.dir).into_iter().flatten().flatten() {
            let p = e.path();
            let name = e.file_name();
            let name = name.to_string_lossy();
            let stale = if name.ends_with(".qbc") {
                age(&p) > STALE_ENTRY
            } else {
                name.contains(".tmp.") && age(&p) > STALE_TMP
            };
            if stale {
                let _ = std::fs::remove_file(&p);
            }
        }
    }
}

/// `MAGIC, key, fnv(js), len(payload), fnv(payload), len(name), name, payload` (little-endian).
fn encode(key: u64, name: &str, js: &str, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(48 + name.len() + payload.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&key.to_le_bytes());
    out.extend_from_slice(&hash(&[js.as_bytes()]).to_le_bytes());
    out.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    out.extend_from_slice(&hash(&[payload]).to_le_bytes());
    out.extend_from_slice(&(name.len() as u64).to_le_bytes());
    out.extend_from_slice(name.as_bytes());
    out.extend_from_slice(payload);
    out
}

/// The payload of a file written by [`encode`] for exactly this key, name and JavaScript.
fn decode<'a>(file: &'a [u8], key: u64, name: &str, js: &str) -> Option<&'a [u8]> {
    let u64_at = |i: usize| file.get(i..i + 8).map(|b| u64::from_le_bytes(b.try_into().unwrap()));
    if file.get(..8)? != MAGIC || u64_at(8)? != key || u64_at(16)? != hash(&[js.as_bytes()]) {
        return None;
    }
    let (len, sum, name_len) = (usize::try_from(u64_at(24)?).ok()?, u64_at(32)?, usize::try_from(u64_at(40)?).ok()?);
    let name_end = 48usize.checked_add(name_len)?;
    if file.get(48..name_end)? != name.as_bytes() || file.len() != name_end.checked_add(len)? {
        return None;
    }
    let payload = &file[name_end..];
    (hash(&[payload]) == sum).then_some(payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("svelters-q2-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn stamp_names_the_engine() {
        let s = stamp();
        assert!(s.starts_with("quickjs-ng 0."), "{s}");
        assert!(s.contains(std::env::consts::ARCH), "{s}");
    }

    #[test]
    fn round_trip_and_keys() {
        let root = tmp("rt");
        let c = BytecodeCache::open(&root).unwrap();
        assert_eq!(c.get("/a.js", "1"), None);
        c.put("/a.js", "1", b"bytes").unwrap();
        assert_eq!(c.get("/a.js", "1").as_deref(), Some(&b"bytes"[..]));
        // another source, another name: misses
        assert_eq!(c.get("/a.js", "2"), None);
        assert_eq!(c.get("/b.js", "1"), None);
        // another stamp reads another directory
        let other = BytecodeCache::open_stamped(&root, "quickjs-ng 9.9.9".into()).unwrap();
        assert_ne!(other.dir(), c.dir());
        assert_eq!(other.get("/a.js", "1"), None);
        c.remove("/a.js", "1");
        assert_eq!(c.get("/a.js", "1"), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn damaged_entries_are_misses() {
        let root = tmp("dmg");
        let c = BytecodeCache::open(&root).unwrap();
        c.put("/a.js", "src", b"0123456789").unwrap();
        let file = c.path(c.key("/a.js", "src"));
        let good = std::fs::read(&file).unwrap();
        // truncated
        std::fs::write(&file, &good[..good.len() - 1]).unwrap();
        assert_eq!(c.get("/a.js", "src"), None);
        // a flipped payload byte
        let mut bad = good.clone();
        *bad.last_mut().unwrap() ^= 1;
        std::fs::write(&file, &bad).unwrap();
        assert_eq!(c.get("/a.js", "src"), None);
        // garbage, empty
        std::fs::write(&file, b"not bytecode").unwrap();
        assert_eq!(c.get("/a.js", "src"), None);
        std::fs::write(&file, b"").unwrap();
        assert_eq!(c.get("/a.js", "src"), None);
        // an entry renamed onto another key
        c.put("/b.js", "src", b"0123456789").unwrap();
        std::fs::copy(c.path(c.key("/b.js", "src")), &file).unwrap();
        assert_eq!(c.get("/a.js", "src"), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn prunes_stale_stamps_and_entries() {
        let root = tmp("prune");
        let old = BytecodeCache::open_stamped(&root, "quickjs-ng 0.0.1".into()).unwrap();
        old.put("/a.js", "1", b"x").unwrap();
        let keep = BytecodeCache::open_stamped(&root, "quickjs-ng 0.0.2".into()).unwrap();
        let unrelated = root.join("notes");
        std::fs::create_dir_all(&unrelated).unwrap();
        let back = |p: &Path, d: Duration| std::fs::File::options().write(true).open(p).unwrap().set_modified(SystemTime::now() - d).unwrap();
        back(&old.dir().join("STAMP"), STALE_STAMP + Duration::from_secs(60));
        keep.put("/k.js", "1", b"x").unwrap();
        keep.put("/s.js", "1", b"x").unwrap();
        back(&keep.path(keep.key("/s.js", "1")), STALE_ENTRY + Duration::from_secs(60));
        let keep = BytecodeCache::open_stamped(&root, "quickjs-ng 0.0.2".into()).unwrap();
        assert!(!old.dir().exists(), "a stale stamp's directory is removed");
        assert!(unrelated.exists(), "directories the cache did not make stay");
        assert!(keep.get("/k.js", "1").is_some());
        assert!(keep.get("/s.js", "1").is_none(), "a stale entry is removed");
        let _ = std::fs::remove_dir_all(&root);
    }
}
