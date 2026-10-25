//! The XDG desktop portal (ashpd on zbus with async-io, no async runtime): the colour scheme, the accent
//! colour and file dialogs.
//!
//! The colour scheme is watched on a helper thread and reaches every window as `WindowEvent::ThemeChanged`.
//! The dialog and accent functions block on D-Bus (dialogs until the user answers), so call them from a
//! worker thread, never from the event loop.

use crate::app::Internal;
use ashpd::desktop::file_chooser::{FileFilter, SelectedFiles};
use ashpd::desktop::settings::{ColorScheme, Settings};
use futures_lite::StreamExt;
use futures_lite::future::block_on;
use skelter_base::color::Color;
use skelter_base::event::Theme;
use std::path::PathBuf;
use std::sync::mpsc::Sender;

#[derive(Debug)]
pub enum PortalError {
    /// No portal on the session bus, or the call failed.
    Portal(String),
    /// The user dismissed the dialog.
    Cancelled,
}

impl std::fmt::Display for PortalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PortalError::Portal(e) => write!(f, "desktop portal: {e}"),
            PortalError::Cancelled => f.write_str("cancelled"),
        }
    }
}

impl std::error::Error for PortalError {}

fn portal_err(e: ashpd::Error) -> PortalError {
    match e {
        ashpd::Error::Response(ashpd::desktop::ResponseError::Cancelled) => PortalError::Cancelled,
        e => PortalError::Portal(e.to_string()),
    }
}

pub fn theme_of(scheme: ColorScheme) -> Option<Theme> {
    match scheme {
        ColorScheme::PreferDark => Some(Theme::Dark),
        ColorScheme::PreferLight => Some(Theme::Light),
        ColorScheme::NoPreference => None,
    }
}

/// Watches the colour scheme on a helper thread: sends the current one, then every change. A missing portal
/// ends the thread quietly (the app keeps its default theme).
pub(crate) fn watch_theme(tx: Sender<Internal>, notify: Box<dyn Fn() + Send + Sync>) {
    let spawned = std::thread::Builder::new().name("skelter-portal".into()).spawn(move || {
        block_on(async move {
            let Ok(settings) = Settings::new().await else { return };
            let send = |scheme: ColorScheme| {
                if let Some(t) = theme_of(scheme) {
                    if tx.send(Internal::Theme(t)).is_ok() {
                        notify();
                    }
                }
            };
            if let Ok(s) = settings.color_scheme().await {
                send(s);
            }
            let Ok(mut changes) = settings.receive_color_scheme_changed().await else { return };
            while let Some(s) = changes.next().await {
                send(s);
            }
        })
    });
    drop(spawned);
}

/// The desktop's accent colour (blocking).
pub fn accent_color() -> Result<Color, PortalError> {
    block_on(async {
        let settings = Settings::new().await.map_err(portal_err)?;
        let c = settings.accent_color().await.map_err(portal_err)?;
        let ch = |v: f64| v.clamp(0.0, 1.0) as f32;
        Ok(Color::rgba(ch(c.red()), ch(c.green()), ch(c.blue()), 1.0))
    })
}

/// A file dialog's options.
#[derive(Clone, Debug, Default)]
pub struct FileDialog {
    pub title: String,
    pub accept_label: Option<String>,
    /// Name → glob patterns (`*.kicad_pcb`).
    pub filters: Vec<(String, Vec<String>)>,
    pub directory: Option<PathBuf>,
    /// Save dialogs: the proposed file name.
    pub file_name: Option<String>,
}

fn filters(d: &FileDialog) -> Vec<FileFilter> {
    d.filters.iter().map(|(name, globs)| globs.iter().fold(FileFilter::new(name), |f, g| f.glob(g))).collect()
}

fn paths(files: &SelectedFiles) -> Vec<PathBuf> {
    let list: String = files.uris().iter().map(|u| format!("{u}\n")).collect();
    crate::data::parse_uri_list(&list)
}

/// Asks the user for files to open (blocking until they answer).
pub fn open_files(d: &FileDialog, multiple: bool, directories: bool) -> Result<Vec<PathBuf>, PortalError> {
    block_on(async {
        let mut req = SelectedFiles::open_file()
            .title(d.title.as_str())
            .accept_label(d.accept_label.as_deref())
            .modal(true)
            .multiple(multiple)
            .directory(directories)
            .filters(filters(d));
        if let Some(dir) = &d.directory {
            req = req.current_folder(dir).map_err(portal_err)?;
        }
        let files = req.send().await.map_err(portal_err)?.response().map_err(portal_err)?;
        Ok(paths(&files))
    })
}

/// Asks the user where to save (blocking until they answer).
pub fn save_file(d: &FileDialog) -> Result<PathBuf, PortalError> {
    block_on(async {
        let mut req = SelectedFiles::save_file()
            .title(d.title.as_str())
            .accept_label(d.accept_label.as_deref())
            .modal(true)
            .current_name(d.file_name.as_deref())
            .filters(filters(d));
        if let Some(dir) = &d.directory {
            req = req.current_folder(dir).map_err(portal_err)?;
        }
        let files = req.send().await.map_err(portal_err)?.response().map_err(portal_err)?;
        paths(&files).into_iter().next().ok_or(PortalError::Cancelled)
    })
}
