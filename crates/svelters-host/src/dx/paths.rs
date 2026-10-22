//! DX1 (QD2): app identity and the standard per-app directories.

use std::path::{Path, PathBuf};

/// Who the app is (`QCoreApplication` organization and application names). Names the default
/// settings file, the UI-state directory and the per-app directories.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppId {
    pub qualifier: String,
    pub org: String,
    pub app: String,
}

impl AppId {
    pub fn new(qualifier: impl Into<String>, org: impl Into<String>, app: impl Into<String>) -> AppId {
        AppId { qualifier: qualifier.into(), org: org.into(), app: app.into() }
    }
}

/// The config, data, cache and state directories of one app. Nothing is created on disk here; the
/// defaults create the directories they write into.
#[derive(Clone, Debug)]
pub struct Paths {
    config: PathBuf,
    data: PathBuf,
    cache: PathBuf,
    state: PathBuf,
    runtime: Option<PathBuf>,
}

impl Paths {
    /// The platform's locations for `id`; `None` when the platform has no home directory.
    pub fn for_app(id: &AppId) -> Option<Paths> {
        let d = directories::ProjectDirs::from(&id.qualifier, &id.org, &id.app)?;
        Some(Paths {
            config: d.config_dir().to_path_buf(),
            data: d.data_dir().to_path_buf(),
            cache: d.cache_dir().to_path_buf(),
            // `state_dir` exists on Linux only; elsewhere state sits under the data directory.
            state: d.state_dir().map(Path::to_path_buf).unwrap_or_else(|| d.data_local_dir().join("state")),
            runtime: d.runtime_dir().map(Path::to_path_buf),
        })
    }

    /// All four directories under `root` (`config/`, `data/`, `cache/`, `state/`): tests, and apps
    /// that keep everything in one folder.
    pub fn under(root: impl AsRef<Path>) -> Paths {
        let r = root.as_ref();
        Paths { config: r.join("config"), data: r.join("data"), cache: r.join("cache"), state: r.join("state"), runtime: None }
    }

    pub fn config(&self) -> &Path {
        &self.config
    }
    pub fn data(&self) -> &Path {
        &self.data
    }
    pub fn cache(&self) -> &Path {
        &self.cache
    }
    pub fn state(&self) -> &Path {
        &self.state
    }
    pub fn runtime(&self) -> Option<&Path> {
        self.runtime.as_deref()
    }
}
