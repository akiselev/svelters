//! DX1 (QD1): settings and persisted UI state. svelters owns no persistence: both bind to a store
//! the host supplies through these traits. The in-memory and simple-file defaults serve tests and
//! apps that bring no store of their own.

use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SettingsError {
    /// The key is locked; `by` names the layer that locks it.
    Locked { by: String },
    Invalid(String),
}

impl SettingsError {
    /// The `NativeError`-style code JS sees (`settings/locked`, `settings/invalid`).
    pub fn code(&self) -> &'static str {
        match self {
            SettingsError::Locked { .. } => "settings/locked",
            SettingsError::Invalid(_) => "settings/invalid",
        }
    }
}

impl fmt::Display for SettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SettingsError::Locked { by } => write!(f, "setting is locked by {by}"),
            SettingsError::Invalid(m) => write!(f, "invalid setting: {m}"),
        }
    }
}

impl std::error::Error for SettingsError {}

/// Where a key's value comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingSource {
    pub layer: String,
    pub locked: bool,
}

/// Keeps a [`SettingsBackend::subscribe`] callback registered until dropped.
pub struct Subscription(Option<Box<dyn FnOnce() + Send>>);

impl Subscription {
    pub fn new(on_drop: impl FnOnce() + Send + 'static) -> Subscription {
        Subscription(Some(Box::new(on_drop)))
    }
    pub fn none() -> Subscription {
        Subscription(None)
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        if let Some(f) = self.0.take() {
            f();
        }
    }
}

/// The settings store of the app (cadatomic: its layered store).
pub trait SettingsBackend: Send + Sync + 'static {
    fn get(&self, key: &str) -> Option<Value>;
    fn set(&self, key: &str, value: Value) -> Result<(), SettingsError>;
    fn source(&self, key: &str) -> Option<SettingSource>;
    /// Calls `on_change` with the changed keys, from any thread.
    fn subscribe(&self, on_change: Box<dyn Fn(&[String]) + Send + Sync>) -> Subscription;
}

/// Persisted UI state (column widths, splits, collapsed sections): not settings, never in a
/// document or command log. `root` names the root; the map is keyed by anchor.
pub trait UiStateBackend: Send + Sync + 'static {
    fn load(&self, root: &str) -> Map<String, Value>;
    fn save(&self, root: &str, state: Map<String, Value>);
}

type Listeners = Arc<Mutex<Vec<(u64, Arc<dyn Fn(&[String]) + Send + Sync>)>>>;

#[derive(Default)]
struct MemoryInner {
    values: BTreeMap<String, Value>,
    locked: BTreeMap<String, String>,
}

/// Settings in memory, with optional locks (a key locked by a named layer rejects `set`).
#[derive(Clone, Default)]
pub struct MemorySettings {
    inner: Arc<Mutex<MemoryInner>>,
    listeners: Listeners,
    next: Arc<Mutex<u64>>,
}

impl MemorySettings {
    pub fn new() -> MemorySettings {
        MemorySettings::default()
    }

    /// Sets `key` to `value` and locks it against `set`, as layer `by`.
    pub fn lock(&self, key: &str, value: Value, by: &str) {
        let mut i = self.inner.lock().unwrap();
        i.values.insert(key.to_string(), value);
        i.locked.insert(key.to_string(), by.to_string());
        drop(i);
        self.notify(key);
    }

    fn notify(&self, key: &str) {
        let ls: Vec<_> = self.listeners.lock().unwrap().iter().map(|(_, l)| l.clone()).collect();
        for l in ls {
            l(&[key.to_string()]);
        }
    }
}

impl SettingsBackend for MemorySettings {
    fn get(&self, key: &str) -> Option<Value> {
        self.inner.lock().unwrap().values.get(key).cloned()
    }

    fn set(&self, key: &str, value: Value) -> Result<(), SettingsError> {
        let mut i = self.inner.lock().unwrap();
        if let Some(by) = i.locked.get(key) {
            return Err(SettingsError::Locked { by: by.clone() });
        }
        i.values.insert(key.to_string(), value);
        drop(i);
        self.notify(key);
        Ok(())
    }

    fn source(&self, key: &str) -> Option<SettingSource> {
        let i = self.inner.lock().unwrap();
        if !i.values.contains_key(key) {
            return None;
        }
        match i.locked.get(key) {
            Some(by) => Some(SettingSource { layer: by.clone(), locked: true }),
            None => Some(SettingSource { layer: "user".into(), locked: false }),
        }
    }

    fn subscribe(&self, on_change: Box<dyn Fn(&[String]) + Send + Sync>) -> Subscription {
        let id = {
            let mut n = self.next.lock().unwrap();
            *n += 1;
            *n
        };
        self.listeners.lock().unwrap().push((id, Arc::from(on_change)));
        let ls = self.listeners.clone();
        Subscription::new(move || ls.lock().unwrap().retain(|(i, _)| *i != id))
    }
}

/// Settings in one JSON object file, written by atomic rename. The file is read once at `open`;
/// a missing or unreadable file starts empty.
pub struct FileSettings {
    mem: MemorySettings,
    path: PathBuf,
}

impl FileSettings {
    pub fn open(path: impl Into<PathBuf>) -> FileSettings {
        let path = path.into();
        let mem = MemorySettings::new();
        if let Some(Value::Object(m)) = read_json(&path) {
            mem.inner.lock().unwrap().values.extend(m);
        }
        FileSettings { mem, path }
    }
}

impl SettingsBackend for FileSettings {
    fn get(&self, key: &str) -> Option<Value> {
        self.mem.get(key)
    }

    fn set(&self, key: &str, value: Value) -> Result<(), SettingsError> {
        let old = self.mem.get(key);
        self.mem.set(key, value)?;
        let all: Map<String, Value> = self.mem.inner.lock().unwrap().values.clone().into_iter().collect();
        if let Err(e) = write_json_atomic(&self.path, &Value::Object(all)) {
            // Memory must not diverge from disk: put the old value back.
            let mut i = self.mem.inner.lock().unwrap();
            match old {
                Some(v) => i.values.insert(key.to_string(), v),
                None => i.values.remove(key),
            };
            return Err(SettingsError::Invalid(format!("writing {}: {e}", self.path.display())));
        }
        Ok(())
    }

    fn source(&self, key: &str) -> Option<SettingSource> {
        self.mem.source(key)
    }

    fn subscribe(&self, on_change: Box<dyn Fn(&[String]) + Send + Sync>) -> Subscription {
        self.mem.subscribe(on_change)
    }
}

/// UI state in memory.
#[derive(Clone, Default)]
pub struct MemoryUiState(Arc<Mutex<BTreeMap<String, Map<String, Value>>>>);

impl MemoryUiState {
    pub fn new() -> MemoryUiState {
        MemoryUiState::default()
    }
}

impl UiStateBackend for MemoryUiState {
    fn load(&self, root: &str) -> Map<String, Value> {
        self.0.lock().unwrap().get(root).cloned().unwrap_or_default()
    }
    fn save(&self, root: &str, state: Map<String, Value>) {
        self.0.lock().unwrap().insert(root.to_string(), state);
    }
}

/// UI state as one JSON file per root under a directory (`<dir>/<root>.json`, the root name
/// escaped to a safe file name).
pub struct FileUiState {
    dir: PathBuf,
}

impl FileUiState {
    pub fn new(dir: impl Into<PathBuf>) -> FileUiState {
        FileUiState { dir: dir.into() }
    }

    fn file(&self, root: &str) -> PathBuf {
        let mut name = String::new();
        for c in root.chars() {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                name.push(c);
            } else {
                name.push_str(&format!("%{:02X}", c as u32 & 0xFF));
            }
        }
        if name.is_empty() || name.starts_with('.') {
            name.insert(0, '_');
        }
        self.dir.join(format!("{name}.json"))
    }
}

impl UiStateBackend for FileUiState {
    fn load(&self, root: &str) -> Map<String, Value> {
        match read_json(&self.file(root)) {
            Some(Value::Object(m)) => m,
            _ => Map::new(),
        }
    }
    fn save(&self, root: &str, state: Map<String, Value>) {
        // A UI-state write must never take the app down; the state is a convenience.
        if let Err(e) = write_json_atomic(&self.file(root), &Value::Object(state)) {
            tracing::warn!(target: "svelters_host::dx", root, error = %e, "ui state not saved");
        }
    }
}

fn read_json(path: &Path) -> Option<Value> {
    let s = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&s).ok()
}

fn write_json_atomic(path: &Path, v: &Value) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(v).map_err(std::io::Error::other)?)?;
    std::fs::rename(&tmp, path)
}
