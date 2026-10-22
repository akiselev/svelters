//! DX1: settings binding, persisted UI state, standard paths, JS logging.
//!
//! svelters owns no persistence. The app hands the host a [`Dx`]: an [`AppId`] with its [`Paths`], a
//! [`SettingsBackend`] and a [`UiStateBackend`] (in-memory or simple-file defaults ship here;
//! cadatomic supplies its own over its layered store and UI-layout file). JS reaches them through
//! the modules `svelters:settings`, `svelters:paths` and `svelters:log`, which call the host's
//! `__svelters_host.dx` op. Without a `Dx` the modules load and every call fails with
//! `settings/unavailable`.

mod paths;
mod settings;

pub use paths::{AppId, Paths};
pub use settings::{
    FileSettings, FileUiState, MemorySettings, MemoryUiState, SettingSource, SettingsBackend, SettingsError, Subscription,
    UiStateBackend,
};

use serde_json::{Value, json};
use std::sync::Arc;
use svelters_modules::NativeModules;

/// What the app supplies for DX1.
#[derive(Clone)]
pub struct Dx {
    pub app: AppId,
    pub paths: Paths,
    pub settings: Arc<dyn SettingsBackend>,
    pub ui_state: Arc<dyn UiStateBackend>,
}

impl Dx {
    /// In-memory settings and UI state, all directories under `root`: for tests.
    pub fn in_memory(app: AppId, root: impl AsRef<std::path::Path>) -> Dx {
        Dx { app, paths: Paths::under(root), settings: Arc::new(MemorySettings::new()), ui_state: Arc::new(MemoryUiState::new()) }
    }

    /// The simple-file defaults: `settings.json` under the config directory and one UI-state file
    /// per root under `<state>/ui`. `None` when the platform has no home directory.
    pub fn files(app: AppId) -> Option<Dx> {
        let paths = Paths::for_app(&app)?;
        let settings = Arc::new(FileSettings::open(paths.config().join("settings.json")));
        let ui_state = Arc::new(FileUiState::new(paths.state().join("ui")));
        Some(Dx { app, paths, settings, ui_state })
    }

    /// One `__svelters_host.dx(op, a, b)` call; `Err((code, message))` becomes a JS error whose
    /// message starts with `code: `.
    pub(crate) fn op(&self, op: &str, a: &str, b: &str) -> Result<String, (&'static str, String)> {
        let bad = |m: String| ("settings/invalid", m);
        match op {
            "settings.get" => Ok(match self.settings.get(a) {
                Some(v) => v.to_string(),
                None => String::new(),
            }),
            "settings.source" => Ok(match self.settings.source(a) {
                Some(s) => json!({ "layer": s.layer, "locked": s.locked }).to_string(),
                None => String::new(),
            }),
            "settings.set" => {
                let v: Value = serde_json::from_str(b).map_err(|e| bad(e.to_string()))?;
                self.settings.set(a, v).map(|()| String::new()).map_err(|e| (e.code(), e.to_string()))
            }
            "ui.load" => Ok(Value::Object(self.ui_state.load(a)).to_string()),
            "ui.save" => match serde_json::from_str(b).map_err(|e| bad(e.to_string()))? {
                Value::Object(m) => {
                    self.ui_state.save(a, m);
                    Ok(String::new())
                }
                _ => Err(bad("ui state must be an object".into())),
            },
            "paths" => {
                let p = &self.paths;
                Ok(json!({
                    "config": p.config(), "data": p.data(), "cache": p.cache(), "state": p.state(), "runtime": p.runtime(),
                    "app": { "qualifier": self.app.qualifier, "org": self.app.org, "name": self.app.app },
                })
                .to_string())
            }
            _ => Err(("settings/invalid", format!("unknown dx op {op}"))),
        }
    }
}

/// The error for a `dx` op with no [`Dx`] installed.
pub(crate) fn unavailable() -> (&'static str, String) {
    ("settings/unavailable", "no settings backend: the host has no Dx (Host::set_dx)".into())
}

pub const SETTINGS_ID: &str = "svelters:settings";
pub const PATHS_ID: &str = "svelters:paths";
pub const LOG_ID: &str = "svelters:log";

/// `svelters:settings`, `svelters:paths` and `svelters:log` as native modules.
pub(crate) struct DxModules;

impl NativeModules for DxModules {
    fn ids(&self) -> Vec<String> {
        vec![SETTINGS_ID.into(), PATHS_ID.into(), LOG_ID.into()]
    }

    fn source(&self, id: &str) -> Option<String> {
        match id {
            SETTINGS_ID => Some(include_str!("settings.js").to_string()),
            PATHS_ID => Some(include_str!("paths.js").to_string()),
            LOG_ID => Some(include_str!("log.js").to_string()),
            _ => None,
        }
    }
}
