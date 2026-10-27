//! DX2: the clipboard and the shell (open a URL or file with its handler, reveal a file in the
//! file manager), reachable from JS as `svelters:clipboard` and `svelters:shell`
//! (`@svelters/native-runtime`) and from Rust as [`Platform`] (`HostConfig::platform`,
//! [`crate::Host::platform`]).
//!
//! - [`MimeData`]: typed data, an ordered list of `(type, bytes)` entries. One type for the clipboard
//!   and drag and drop: a WG4b drag (`drag-type`, `drag-data`) is one entry
//!   ([`MimeData::single`]), so a Rust panel that copies, drags or pastes handles both the same way.
//! - [`ClipboardBackend`]: read, write and list formats of the clipboard or the primary selection.
//!   [`MemoryClipboard`] keeps everything in process (the default: headless hosts and tests never
//!   touch the system clipboard); `SystemClipboard` (arboard, feature `system-clipboard`) is the
//!   desktop one, with the Linux primary selection (`system_primary`).
//! - [`ShellBackend`]: open a URL, open a file with the system handler, reveal a file.
//!   [`NoShell`] (the default) fails every call, [`RecordingShell`] records them (tests),
//!   [`SystemShell`] does them.
//!
//! The backends are `Send + Sync` and shared ([`Arc`]): every host of a process (the roots of an
//! embedder, its Rust panels, a code editor) uses the same clipboard, so a copy in one pastes in
//! another. A host caller (EV3's paste command, a Rust panel) reads the same backend with
//! [`crate::Host::platform`].
//!
//! Type patterns (`read`'s `accept`) follow WG4b's `drop-types`: an exact type, `*` (any type), or
//! `prefix/*` (any type starting with `prefix/`).
//!
//! `Platform::system` (feature `system-clipboard`; also `platform::system()`) is what a desktop app
//! installs: `SystemClipboard` and [`SystemShell`]. svelters-egui's runners install it; an embedder
//! (cadatomic, a svelters-skelter app) sets it on its own `HostConfig`:
//!
//! ```ignore
//! let mut host = HostConfig::default();
//! host.platform = svelters_host::platform::Platform::system();
//! ```
//!
//! Writer lane: DX2 (plans/milestones/DX2-clipboard-files.md); SV75 moved the system clipboard
//! here from svelters-egui.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

#[cfg(feature = "system-clipboard")]
mod system_clipboard;
#[cfg(feature = "system-clipboard")]
pub use system_clipboard::{system_primary, SystemClipboard, PRIMARY_SUPPORTED};

/// The plain-text type, the fallback every clipboard understands.
pub const TEXT_PLAIN: &str = "text/plain";
/// The HTML type.
pub const TEXT_HTML: &str = "text/html";

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Typed data for the clipboard and for drag and drop: `(type, bytes)` entries in the writer's order
/// of preference, at most one per type. Types are MIME-like strings (`text/plain`,
/// `application/x-cadatomic-refs+json`, `cad/part`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MimeData {
    entries: Vec<(String, Vec<u8>)>,
}

impl MimeData {
    pub fn new() -> MimeData {
        MimeData::default()
    }

    /// One entry: what a WG4b drag carries (`drag-type`, `drag-data`).
    pub fn single(ty: impl Into<String>, data: impl Into<Vec<u8>>) -> MimeData {
        MimeData::new().with(ty, data)
    }

    /// Plain text only.
    pub fn text(text: impl Into<String>) -> MimeData {
        MimeData::single(TEXT_PLAIN, text.into())
    }

    /// Adds (or replaces) the entry of type `ty`, builder style.
    pub fn with(mut self, ty: impl Into<String>, data: impl Into<Vec<u8>>) -> MimeData {
        self.set(ty, data);
        self
    }

    /// Sets the entry of type `ty`: a new type goes last, an existing one keeps its place.
    pub fn set(&mut self, ty: impl Into<String>, data: impl Into<Vec<u8>>) {
        let (ty, data) = (ty.into(), data.into());
        match self.entries.iter_mut().find(|(t, _)| *t == ty) {
            Some(e) => e.1 = data,
            None => self.entries.push((ty, data)),
        }
    }

    /// The bytes of type `ty`.
    pub fn get(&self, ty: &str) -> Option<&[u8]> {
        self.entries.iter().find(|(t, _)| t == ty).map(|(_, d)| d.as_slice())
    }

    /// The data of type `ty` as UTF-8 text (None if absent or not UTF-8).
    pub fn get_str(&self, ty: &str) -> Option<&str> {
        self.get(ty).and_then(|d| std::str::from_utf8(d).ok())
    }

    /// The `text/plain` entry.
    pub fn plain_text(&self) -> Option<&str> {
        self.get_str(TEXT_PLAIN)
    }

    /// The types, in order.
    pub fn types(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|(t, _)| t.as_str())
    }

    pub fn entries(&self) -> &[(String, Vec<u8>)] {
        &self.entries
    }

    pub fn into_entries(self) -> Vec<(String, Vec<u8>)> {
        self.entries
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// The entries whose type matches one of `accept` (an empty `accept`: all), in this data's
    /// order. See [`type_matches`].
    pub fn filter(&self, accept: &[&str]) -> MimeData {
        if accept.is_empty() {
            return self.clone();
        }
        MimeData { entries: self.entries.iter().filter(|(t, _)| accept.iter().any(|p| type_matches(p, t))).cloned().collect() }
    }

    /// The first entry, by `accept`'s order of preference: the first pattern that matches any type
    /// wins, and among its matches the data's own order. An empty `accept` takes the first entry.
    pub fn best(&self, accept: &[&str]) -> Option<(&str, &[u8])> {
        let pick = |p: &str| self.entries.iter().find(|(t, _)| type_matches(p, t));
        let e = if accept.is_empty() { self.entries.first() } else { accept.iter().find_map(|p| pick(p)) };
        e.map(|(t, d)| (t.as_str(), d.as_slice()))
    }
}

impl FromIterator<(String, Vec<u8>)> for MimeData {
    fn from_iter<I: IntoIterator<Item = (String, Vec<u8>)>>(it: I) -> MimeData {
        let mut m = MimeData::new();
        for (t, d) in it {
            m.set(t, d);
        }
        m
    }
}

/// Whether the type pattern `pattern` matches `ty`: the same rules as WG4b's `drop-types` (`*`, an
/// exact type, or `prefix/*`).
pub fn type_matches(pattern: &str, ty: &str) -> bool {
    pattern == "*" || pattern == ty || pattern.strip_suffix('*').is_some_and(|prefix| prefix.ends_with('/') && ty.starts_with(prefix))
}

/// Whether data of type `ty` is text: `text/*`, JSON (`application/json`, `…+json`), XML, JavaScript
/// and `text/uri-list`. JS gets text types as strings and the rest as `Uint8Array`s.
pub fn is_text_type(ty: &str) -> bool {
    let base = ty.split(';').next().unwrap_or(ty).trim();
    base.starts_with("text/")
        || base.ends_with("+json")
        || base.ends_with("+xml")
        || matches!(base, "application/json" | "application/xml" | "application/javascript" | "application/ecmascript")
}

/// Which clipboard: the regular one or the primary selection (Linux, X11 and Wayland).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ClipboardKind {
    #[default]
    Clipboard,
    Primary,
}

impl ClipboardKind {
    /// `"clipboard"` or `"primary"` (JS's `kind` option).
    pub fn parse(s: &str) -> Option<ClipboardKind> {
        match s {
            "clipboard" | "" => Some(ClipboardKind::Clipboard),
            "primary" => Some(ClipboardKind::Primary),
            _ => None,
        }
    }
}

/// A clipboard failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClipError {
    /// This backend has no such clipboard (the primary selection outside Linux) or no clipboard at all.
    Unsupported(String),
    /// The system clipboard failed (busy, no display, a format it cannot convert).
    Failed(String),
}

impl fmt::Display for ClipError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClipError::Unsupported(m) => write!(f, "clipboard unsupported: {m}"),
            ClipError::Failed(m) => write!(f, "clipboard failed: {m}"),
        }
    }
}

impl std::error::Error for ClipError {}

/// A clipboard: the system's, an in-process one, or a test double.
pub trait ClipboardBackend: Send + Sync + 'static {
    /// The entries whose types match `accept` (empty: all), as [`MimeData::filter`]. An empty
    /// clipboard, or no matching type, is an empty `MimeData`, not an error.
    fn read(&self, kind: ClipboardKind, accept: &[&str]) -> Result<MimeData, ClipError>;
    /// Replaces the contents with `data`.
    fn write(&self, kind: ClipboardKind, data: MimeData) -> Result<(), ClipError>;
    /// The types on the clipboard now.
    fn formats(&self, kind: ClipboardKind) -> Result<Vec<String>, ClipError> {
        Ok(self.read(kind, &[])?.types().map(str::to_string).collect())
    }
}

/// A clipboard in process memory, with a primary selection. The default of every host, so tests
/// and headless runs never touch the system clipboard; also a test sink (`contents`).
#[derive(Default)]
pub struct MemoryClipboard {
    clipboard: Mutex<MimeData>,
    primary: Mutex<MimeData>,
    writes: Mutex<u64>,
}

impl MemoryClipboard {
    pub fn new() -> MemoryClipboard {
        MemoryClipboard::default()
    }

    fn slot(&self, kind: ClipboardKind) -> &Mutex<MimeData> {
        match kind {
            ClipboardKind::Clipboard => &self.clipboard,
            ClipboardKind::Primary => &self.primary,
        }
    }

    /// What `kind` holds now.
    pub fn contents(&self, kind: ClipboardKind) -> MimeData {
        lock(self.slot(kind)).clone()
    }

    /// Number of writes so far (both kinds).
    pub fn writes(&self) -> u64 {
        *lock(&self.writes)
    }
}

impl ClipboardBackend for MemoryClipboard {
    fn read(&self, kind: ClipboardKind, accept: &[&str]) -> Result<MimeData, ClipError> {
        Ok(lock(self.slot(kind)).filter(accept))
    }

    fn write(&self, kind: ClipboardKind, data: MimeData) -> Result<(), ClipError> {
        *lock(self.slot(kind)) = data;
        *lock(&self.writes) += 1;
        Ok(())
    }
}

/// A shell failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShellError {
    /// No shell backend is installed ([`NoShell`]), or this platform has no way to do it.
    Unsupported(String),
    /// The path does not exist.
    NotFound(PathBuf),
    /// The argument is not usable (an empty URL, a relative path).
    Invalid(String),
    /// The handler could not be started.
    Failed(String),
}

impl fmt::Display for ShellError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShellError::Unsupported(m) => write!(f, "shell unsupported: {m}"),
            ShellError::NotFound(p) => write!(f, "no such file or directory: {}", p.display()),
            ShellError::Invalid(m) => write!(f, "invalid shell argument: {m}"),
            ShellError::Failed(m) => write!(f, "shell failed: {m}"),
        }
    }
}

impl std::error::Error for ShellError {}

/// Opening URLs and files with the system's handlers.
pub trait ShellBackend: Send + Sync + 'static {
    /// Opens `url` with its handler (the browser for `https:`, the mail client for `mailto:`).
    fn open_url(&self, url: &str) -> Result<(), ShellError>;
    /// Opens the file or directory `path` (absolute) with the system's default application.
    fn open_path(&self, path: &Path) -> Result<(), ShellError>;
    /// Shows `path` (absolute) selected in the file manager.
    fn reveal(&self, path: &Path) -> Result<(), ShellError>;
}

/// The default shell: every call fails with [`ShellError::Unsupported`]. A desktop runner installs
/// [`SystemShell`] (`Platform::system`, feature `system-clipboard`).
#[derive(Clone, Copy, Debug, Default)]
pub struct NoShell;

impl ShellBackend for NoShell {
    fn open_url(&self, _: &str) -> Result<(), ShellError> {
        Err(ShellError::Unsupported("no shell backend is installed (HostConfig::platform)".into()))
    }
    fn open_path(&self, _: &Path) -> Result<(), ShellError> {
        self.open_url("")
    }
    fn reveal(&self, _: &Path) -> Result<(), ShellError> {
        self.open_url("")
    }
}

/// A recorded shell call ([`RecordingShell`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShellCall {
    OpenUrl(String),
    OpenPath(PathBuf),
    Reveal(PathBuf),
}

/// A test double: records every call and launches nothing. Paths are checked as [`SystemShell`]
/// does (absolute; existing unless `check_exists` is off).
pub struct RecordingShell {
    calls: Mutex<Vec<ShellCall>>,
    check_exists: bool,
}

impl Default for RecordingShell {
    fn default() -> Self {
        RecordingShell { calls: Mutex::new(Vec::new()), check_exists: true }
    }
}

impl RecordingShell {
    pub fn new() -> RecordingShell {
        RecordingShell::default()
    }

    /// Accepts paths that do not exist.
    pub fn without_exists_check() -> RecordingShell {
        RecordingShell { check_exists: false, ..RecordingShell::default() }
    }

    pub fn calls(&self) -> Vec<ShellCall> {
        lock(&self.calls).clone()
    }
}

impl ShellBackend for RecordingShell {
    fn open_url(&self, url: &str) -> Result<(), ShellError> {
        check_url(url)?;
        lock(&self.calls).push(ShellCall::OpenUrl(url.to_string()));
        Ok(())
    }
    fn open_path(&self, path: &Path) -> Result<(), ShellError> {
        check_path(path, self.check_exists)?;
        lock(&self.calls).push(ShellCall::OpenPath(path.to_path_buf()));
        Ok(())
    }
    fn reveal(&self, path: &Path) -> Result<(), ShellError> {
        check_path(path, self.check_exists)?;
        lock(&self.calls).push(ShellCall::Reveal(path.to_path_buf()));
        Ok(())
    }
}

fn check_url(url: &str) -> Result<(), ShellError> {
    let scheme = url.split_once(':').map(|(s, _)| s).unwrap_or("");
    let ok = !scheme.is_empty() && scheme.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    if ok { Ok(()) } else { Err(ShellError::Invalid(format!("not a URL with a scheme: {url:?}"))) }
}

// ZU3: debt: one stat on the UI thread per shell open (a user action).
#[allow(clippy::disallowed_methods)]
fn check_path(path: &Path, exists: bool) -> Result<(), ShellError> {
    if !path.is_absolute() {
        return Err(ShellError::Invalid(format!("not an absolute path: {}", path.display())));
    }
    if exists && !path.exists() {
        return Err(ShellError::NotFound(path.to_path_buf()));
    }
    Ok(())
}

/// The system's handlers: `open` (`xdg-open`, `open`, `ShellExecute`; WSL aware) for URLs and
/// files; for reveal, `org.freedesktop.FileManager1.ShowItems` over D-Bus on Linux and the BSDs
/// (falling back to opening the parent directory), `open -R` on macOS, `explorer /select,` on
/// Windows. Nothing blocks: handlers are started detached.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemShell;

impl ShellBackend for SystemShell {
    fn open_url(&self, url: &str) -> Result<(), ShellError> {
        check_url(url)?;
        open::that_detached(url).map_err(|e| ShellError::Failed(format!("{url}: {e}")))
    }

    fn open_path(&self, path: &Path) -> Result<(), ShellError> {
        check_path(path, true)?;
        open::that_detached(path).map_err(|e| ShellError::Failed(format!("{}: {e}", path.display())))
    }

    fn reveal(&self, path: &Path) -> Result<(), ShellError> {
        check_path(path, true)?;
        reveal_system(path)
    }
}

// ZU3: a detached launch, never waited on, once per user action (like open::that_detached above).
#[allow(clippy::disallowed_methods)]
#[cfg(target_os = "macos")]
fn reveal_system(path: &Path) -> Result<(), ShellError> {
    std::process::Command::new("open").arg("-R").arg(path).spawn().map(drop).map_err(|e| ShellError::Failed(format!("open -R: {e}")))
}

// ZU3: a detached launch, never waited on, once per user action (like open::that_detached above).
#[allow(clippy::disallowed_methods)]
#[cfg(windows)]
fn reveal_system(path: &Path) -> Result<(), ShellError> {
    use std::os::windows::process::CommandExt;
    // explorer parses its own command line: `/select,"<path>"` as one raw argument.
    std::process::Command::new("explorer")
        .raw_arg(format!("/select,\"{}\"", path.display()))
        .spawn()
        .map(drop)
        .map_err(|e| ShellError::Failed(format!("explorer /select: {e}")))
}

// ZU3: dbus-send runs on its own svelters-reveal thread.
#[allow(clippy::disallowed_methods)]
#[cfg(not(any(target_os = "macos", windows)))]
fn reveal_system(path: &Path) -> Result<(), ShellError> {
    // ShowItems on a thread: dbus-send waits for the reply, and a desktop without FileManager1
    // (or without D-Bus) gets the parent directory opened instead.
    let uri = file_uri(path);
    let parent = path.parent().unwrap_or(path).to_path_buf();
    std::thread::Builder::new()
        .name("svelters-reveal".into())
        .spawn(move || {
            let shown = std::process::Command::new("dbus-send")
                .args(["--session", "--print-reply", "--dest=org.freedesktop.FileManager1", "--type=method_call", "/org/freedesktop/FileManager1", "org.freedesktop.FileManager1.ShowItems"])
                .arg(format!("array:string:{uri}"))
                .arg("string:")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .is_ok_and(|s| s.success());
            if !shown {
                let _ = open::that_detached(&parent);
            }
        })
        .map(drop)
        .map_err(|e| ShellError::Failed(format!("reveal: {e}")))
}

/// `file://` URI of an absolute path, percent-encoding everything but unreserved characters and `/`.
/// A `,` is encoded too: dbus-send splits array arguments on it.
pub fn file_uri(path: &Path) -> String {
    let s = path.to_string_lossy();
    let mut out = String::from("file://");
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => out.push(*b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// The clipboard and shell a host uses (`HostConfig::platform`). Cheap to clone: the backends are
/// shared. The default is [`Platform::headless`].
#[derive(Clone)]
pub struct Platform {
    pub clipboard: Arc<dyn ClipboardBackend>,
    pub shell: Arc<dyn ShellBackend>,
    /// Still [`Platform::headless`]: a runner may replace it with the system platform.
    headless: bool,
}

impl Platform {
    pub fn new(clipboard: Arc<dyn ClipboardBackend>, shell: Arc<dyn ShellBackend>) -> Platform {
        Platform { clipboard, shell, headless: false }
    }

    /// An in-process [`MemoryClipboard`] and [`NoShell`]: nothing reaches the system.
    pub fn headless() -> Platform {
        Platform { clipboard: Arc::new(MemoryClipboard::new()), shell: Arc::new(NoShell), headless: true }
    }

    /// The desktop platform: [`SystemClipboard`] (arboard) and [`SystemShell`].
    #[cfg(feature = "system-clipboard")]
    pub fn system() -> Platform {
        Platform::new(Arc::new(SystemClipboard::new()), Arc::new(SystemShell))
    }

    /// Whether this is an unchanged [`Platform::headless`] (a desktop runner then installs the
    /// system platform).
    pub fn is_headless(&self) -> bool {
        self.headless
    }

    pub fn with_clipboard(mut self, c: Arc<dyn ClipboardBackend>) -> Platform {
        self.clipboard = c;
        self.headless = false;
        self
    }

    pub fn with_shell(mut self, s: Arc<dyn ShellBackend>) -> Platform {
        self.shell = s;
        self.headless = false;
        self
    }
}

/// The desktop platform ([`Platform::system`]).
#[cfg(feature = "system-clipboard")]
pub fn system() -> Platform {
    Platform::system()
}

impl Default for Platform {
    fn default() -> Self {
        Platform::headless()
    }
}

impl fmt::Debug for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Platform { .. }")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mime_data_order_and_patterns() {
        let d = MimeData::new().with("text/plain", "a\tb").with("text/html", "<table/>").with("application/x-cad+json", "[1]");
        assert_eq!(d.types().collect::<Vec<_>>(), ["text/plain", "text/html", "application/x-cad+json"]);
        assert_eq!(d.best(&["application/*", "text/plain"]).unwrap().0, "application/x-cad+json");
        assert_eq!(d.best(&["image/png", "text/*"]).unwrap().0, "text/plain");
        assert_eq!(d.best(&[]).unwrap().0, "text/plain");
        assert!(d.best(&["image/png"]).is_none());
        assert_eq!(d.filter(&["text/*"]).len(), 2);
        // `set` keeps the place of an existing type.
        let d = d.with("text/plain", "x");
        assert_eq!(d.types().next(), Some("text/plain"));
        assert_eq!(d.plain_text(), Some("x"));
        assert!(!type_matches("cad*", "cad/part"));
        assert!(type_matches("cad/*", "cad/part"));
    }

    #[test]
    fn text_types() {
        for t in ["text/plain", "text/html; charset=utf-8", "application/json", "application/x-cadatomic-refs+json", "image/svg+xml"] {
            assert!(is_text_type(t), "{t}");
        }
        for t in ["image/png", "application/octet-stream", "cad/part"] {
            assert!(!is_text_type(t), "{t}");
        }
    }

    #[test]
    fn memory_clipboard_kinds() {
        let c = MemoryClipboard::new();
        c.write(ClipboardKind::Clipboard, MimeData::text("a")).unwrap();
        c.write(ClipboardKind::Primary, MimeData::text("b")).unwrap();
        assert_eq!(c.read(ClipboardKind::Clipboard, &[]).unwrap().plain_text(), Some("a"));
        assert_eq!(c.read(ClipboardKind::Primary, &["text/plain"]).unwrap().plain_text(), Some("b"));
        assert!(c.read(ClipboardKind::Clipboard, &["image/png"]).unwrap().is_empty());
        assert_eq!(c.formats(ClipboardKind::Clipboard).unwrap(), ["text/plain"]);
        assert_eq!(c.writes(), 2);
    }

    #[test]
    fn shell_checks_and_uri() {
        let s = RecordingShell::new();
        assert!(matches!(s.open_url("no scheme"), Err(ShellError::Invalid(_))));
        assert!(matches!(s.open_path(Path::new("rel/x")), Err(ShellError::Invalid(_))));
        let missing = std::env::temp_dir().join("svelters-dx2-missing-file");
        assert!(matches!(s.reveal(&missing), Err(ShellError::NotFound(_))));
        s.open_url("https://example.org/a b").unwrap();
        assert_eq!(s.calls(), [ShellCall::OpenUrl("https://example.org/a b".into())]);
        assert!(NoShell.open_url("https://x").is_err());
        assert_eq!(file_uri(Path::new("/a b/c,d.txt")), "file:///a%20b/c%2Cd.txt");
    }
}
