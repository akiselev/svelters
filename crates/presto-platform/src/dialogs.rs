//! Native file and message dialogs through rfd (feature `dialogs`): portal on Linux, `IFileDialog`
//! on Windows, `NSOpenPanel`/`NSSavePanel` on macOS. The calls block the calling (main) thread until
//! the user answers, as the OS dialogs are modal; winit keeps the window painting through the OS.

use std::path::PathBuf;

/// A file dialog request.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FileDialog {
    pub title: Option<String>,
    pub directory: Option<PathBuf>,
    pub file_name: Option<String>,
    /// (name, extensions without dots)
    pub filters: Vec<(String, Vec<String>)>,
}

impl FileDialog {
    fn build(&self) -> rfd::FileDialog {
        let mut d = rfd::FileDialog::new();
        if let Some(t) = &self.title {
            d = d.set_title(t);
        }
        if let Some(dir) = &self.directory {
            d = d.set_directory(dir);
        }
        if let Some(n) = &self.file_name {
            d = d.set_file_name(n);
        }
        for (name, exts) in &self.filters {
            d = d.add_filter(name, exts);
        }
        d
    }

    pub fn pick_file(&self) -> Option<PathBuf> {
        self.build().pick_file()
    }

    pub fn pick_files(&self) -> Option<Vec<PathBuf>> {
        self.build().pick_files()
    }

    pub fn pick_folder(&self) -> Option<PathBuf> {
        self.build().pick_folder()
    }

    pub fn save_file(&self) -> Option<PathBuf> {
        self.build().save_file()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MessageLevel {
    #[default]
    Info,
    Warning,
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MessageButtons {
    #[default]
    Ok,
    OkCancel,
    YesNo,
}

/// Shows a message box; `true` when the user chose OK or Yes.
pub fn message(level: MessageLevel, title: &str, text: &str, buttons: MessageButtons) -> bool {
    let r = rfd::MessageDialog::new()
        .set_level(match level {
            MessageLevel::Info => rfd::MessageLevel::Info,
            MessageLevel::Warning => rfd::MessageLevel::Warning,
            MessageLevel::Error => rfd::MessageLevel::Error,
        })
        .set_title(title)
        .set_description(text)
        .set_buttons(match buttons {
            MessageButtons::Ok => rfd::MessageButtons::Ok,
            MessageButtons::OkCancel => rfd::MessageButtons::OkCancel,
            MessageButtons::YesNo => rfd::MessageButtons::YesNo,
        })
        .show();
    matches!(r, rfd::MessageDialogResult::Ok | rfd::MessageDialogResult::Yes)
}
