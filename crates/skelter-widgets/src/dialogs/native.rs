//! Native file and message dialogs (svelters-egui `dialogs.rs`, WG4b), as futures, so a dialog never
//! blocks a frame. lane SVELTERS-SKELTER exposes them to Svelte as the Promise-returning exports of
//! `svelters:app/egui` (`openFile`, `openFiles`, `saveFile`, `pickFolder`, `messageBox`); dropping
//! the future (an aborted `{ signal }`) drops the answer while the OS dialog stays up until closed.
//!
//! The dialogs come from a [`DialogBackend`]: rfd with feature `dialogs` (rfd has no toolkit
//! dependency), or one installed with [`set_dialog_backend`] (tests, headless runs, an app's own
//! dialogs). Without either, every call fails with [`DialogError::Unsupported`].

use std::cell::RefCell;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::rc::Rc;

/// A file type choice of a file dialog.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FileFilter {
    /// What the dialog shows for it (`STEP files`).
    pub name: String,
    /// Extensions without the dot (`["step", "stp"]`).
    pub extensions: Vec<String>,
}

/// Options of the file dialogs; every field is optional.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FileDialogOptions {
    pub title: Option<String>,
    /// The folder it starts in.
    pub directory: Option<String>,
    /// The file name it proposes (save dialogs).
    pub file_name: Option<String>,
    /// The file types it offers, in order.
    pub filters: Option<Vec<FileFilter>>,
}

/// The icon of a message box.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MessageLevel {
    #[default]
    Info,
    Warning,
    Error,
}

/// The buttons of a message box (without `labels`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MessageButtons {
    #[default]
    Ok,
    OkCancel,
    YesNo,
    YesNoCancel,
}

/// Options of a message box.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MessageBoxOptions {
    pub title: Option<String>,
    /// The message.
    pub description: Option<String>,
    pub level: Option<MessageLevel>,
    pub buttons: Option<MessageButtons>,
    /// Own captions for one to three buttons instead of `buttons` (ok; ok, cancel; yes, no,
    /// cancel). The answer is then the caption clicked.
    pub labels: Option<Vec<String>>,
}

/// What a file dialog is asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileMode {
    /// One existing file.
    Open,
    /// Several existing files.
    OpenMany,
    /// A file name to write.
    Save,
    /// A folder.
    Folder,
}

/// A dialog's answer, later.
pub type DialogFuture<T> = Pin<Box<dyn Future<Output = T>>>;

/// Shows the native dialogs (on the thread that calls it: the JS thread).
pub trait DialogBackend {
    /// A file dialog: the chosen paths, `None` when cancelled.
    fn files(&self, mode: FileMode, options: FileDialogOptions) -> DialogFuture<Option<Vec<PathBuf>>>;
    /// A message box: `ok`, `cancel`, `yes`, `no`, or the caption of a `labels` button.
    fn message(&self, options: MessageBoxOptions) -> DialogFuture<String>;
}

thread_local! {
    static BACKEND: RefCell<Option<Rc<dyn DialogBackend>>> = const { RefCell::new(None) };
}

/// Installs `backend` for the dialogs started on this thread from now on; `None` goes back to the
/// default.
pub fn set_dialog_backend(backend: Option<Rc<dyn DialogBackend>>) {
    BACKEND.with(|b| *b.borrow_mut() = backend);
}

/// Why a dialog call failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DialogError {
    /// No native dialogs in this build (feature `dialogs` off) and no [`DialogBackend`] installed.
    Unsupported,
}

impl std::fmt::Display for DialogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DialogError::Unsupported => f.write_str("native dialogs are not available (skelter-widgets without the `dialogs` feature)"),
        }
    }
}

impl std::error::Error for DialogError {}

fn backend() -> Result<Rc<dyn DialogBackend>, DialogError> {
    if let Some(b) = BACKEND.with(|b| b.borrow().clone()) {
        return Ok(b);
    }
    #[cfg(feature = "dialogs")]
    {
        Ok(Rc::new(rfd_backend::Rfd))
    }
    #[cfg(not(feature = "dialogs"))]
    {
        Err(DialogError::Unsupported)
    }
}

fn path_string(p: PathBuf) -> String {
    p.to_string_lossy().into_owned()
}

async fn one(mode: FileMode, options: Option<FileDialogOptions>) -> Result<Option<String>, DialogError> {
    let paths = backend()?.files(mode, options.unwrap_or_default()).await;
    Ok(paths.and_then(|v| v.into_iter().next()).map(path_string))
}

/// An open dialog for one file: its path, or `None` when cancelled.
pub async fn open_file(opts: Option<FileDialogOptions>) -> Result<Option<String>, DialogError> {
    one(FileMode::Open, opts).await
}

/// An open dialog for several files: their paths, or `None` when cancelled.
pub async fn open_files(opts: Option<FileDialogOptions>) -> Result<Option<Vec<String>>, DialogError> {
    let paths = backend()?.files(FileMode::OpenMany, opts.unwrap_or_default()).await;
    Ok(paths.map(|v| v.into_iter().map(path_string).collect()))
}

/// A save dialog: the chosen path, or `None` when cancelled.
pub async fn save_file(opts: Option<FileDialogOptions>) -> Result<Option<String>, DialogError> {
    one(FileMode::Save, opts).await
}

/// A folder dialog: the folder's path, or `None` when cancelled.
pub async fn pick_folder(opts: Option<FileDialogOptions>) -> Result<Option<String>, DialogError> {
    one(FileMode::Folder, opts).await
}

/// A message box: the button clicked (`ok`, `cancel`, `yes`, `no`, or a `labels` caption).
pub async fn message_box(opts: MessageBoxOptions) -> Result<String, DialogError> {
    Ok(backend()?.message(opts).await)
}

#[cfg(feature = "dialogs")]
mod rfd_backend {
    use super::*;

    /// The native dialogs (XDG portal or zenity on Linux, AppKit, Win32) through rfd's async API.
    pub(super) struct Rfd;

    impl DialogBackend for Rfd {
        fn files(&self, mode: FileMode, o: FileDialogOptions) -> DialogFuture<Option<Vec<PathBuf>>> {
            let mut d = rfd::AsyncFileDialog::new();
            if let Some(t) = o.title {
                d = d.set_title(t);
            }
            if let Some(dir) = o.directory {
                d = d.set_directory(dir);
            }
            if let Some(name) = o.file_name {
                d = d.set_file_name(name);
            }
            for f in o.filters.unwrap_or_default() {
                d = d.add_filter(f.name, &f.extensions);
            }
            let path = |h: rfd::FileHandle| h.path().to_path_buf();
            match mode {
                FileMode::Open => Box::pin(async move { d.pick_file().await.map(|h| vec![path(h)]) }),
                FileMode::OpenMany => Box::pin(async move { d.pick_files().await.map(|v| v.into_iter().map(path).collect()) }),
                FileMode::Save => Box::pin(async move { d.save_file().await.map(|h| vec![path(h)]) }),
                FileMode::Folder => Box::pin(async move { d.pick_folder().await.map(|h| vec![path(h)]) }),
            }
        }

        fn message(&self, o: MessageBoxOptions) -> DialogFuture<String> {
            let mut d = rfd::AsyncMessageDialog::new().set_level(match o.level.unwrap_or_default() {
                MessageLevel::Info => rfd::MessageLevel::Info,
                MessageLevel::Warning => rfd::MessageLevel::Warning,
                MessageLevel::Error => rfd::MessageLevel::Error,
            });
            if let Some(t) = o.title {
                d = d.set_title(t);
            }
            if let Some(t) = o.description {
                d = d.set_description(t);
            }
            let labels = o.labels.unwrap_or_default();
            let buttons = match (labels.as_slice(), o.buttons.unwrap_or_default()) {
                ([a], _) => rfd::MessageButtons::OkCustom(a.clone()),
                ([a, b], _) => rfd::MessageButtons::OkCancelCustom(a.clone(), b.clone()),
                ([a, b, c, ..], _) => rfd::MessageButtons::YesNoCancelCustom(a.clone(), b.clone(), c.clone()),
                (_, MessageButtons::Ok) => rfd::MessageButtons::Ok,
                (_, MessageButtons::OkCancel) => rfd::MessageButtons::OkCancel,
                (_, MessageButtons::YesNo) => rfd::MessageButtons::YesNo,
                (_, MessageButtons::YesNoCancel) => rfd::MessageButtons::YesNoCancel,
            };
            let d = d.set_buttons(buttons);
            Box::pin(async move {
                match d.show().await {
                    rfd::MessageDialogResult::Ok => "ok".into(),
                    rfd::MessageDialogResult::Cancel => "cancel".into(),
                    rfd::MessageDialogResult::Yes => "yes".into(),
                    rfd::MessageDialogResult::No => "no".into(),
                    rfd::MessageDialogResult::Custom(s) => s,
                }
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::task::{Context, Poll, Waker};

    fn ready<T>(f: impl Future<Output = T>) -> T {
        let mut f = std::pin::pin!(f);
        match f.as_mut().poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(v) => v,
            Poll::Pending => panic!("the test backend answers at once"),
        }
    }

    struct Canned;
    impl DialogBackend for Canned {
        fn files(&self, mode: FileMode, o: FileDialogOptions) -> DialogFuture<Option<Vec<PathBuf>>> {
            let r = match mode {
                FileMode::OpenMany => Some(vec![PathBuf::from("/a.step"), PathBuf::from("/b.step")]),
                FileMode::Save => o.file_name.map(|n| vec![PathBuf::from("/out").join(n)]),
                _ => None,
            };
            Box::pin(async move { r })
        }
        fn message(&self, o: MessageBoxOptions) -> DialogFuture<String> {
            let a = o.labels.and_then(|l| l.into_iter().last()).unwrap_or_else(|| "ok".into());
            Box::pin(async move { a })
        }
    }

    #[test]
    fn an_installed_backend_answers_and_cancel_is_none() {
        set_dialog_backend(Some(Rc::new(Canned)));
        assert_eq!(ready(open_files(None)), Ok(Some(vec!["/a.step".to_string(), "/b.step".to_string()])));
        assert_eq!(ready(open_file(None)), Ok(None), "cancelled");
        let o = FileDialogOptions { file_name: Some("x.kicad_pcb".into()), ..Default::default() };
        // the backend joins with the OS separator (`\` on Windows)
        let saved = PathBuf::from("/out").join("x.kicad_pcb").to_string_lossy().into_owned();
        assert_eq!(ready(save_file(Some(o))), Ok(Some(saved)));
        let m = MessageBoxOptions { labels: Some(vec!["Save".into(), "Discard".into()]), ..Default::default() };
        assert_eq!(ready(message_box(m)), Ok("Discard".to_string()));
        set_dialog_backend(None);
    }

    #[cfg(not(feature = "dialogs"))]
    #[test]
    fn without_a_backend_dialogs_are_unsupported() {
        set_dialog_backend(None);
        assert_eq!(ready(pick_folder(None)), Err(DialogError::Unsupported));
    }
}
