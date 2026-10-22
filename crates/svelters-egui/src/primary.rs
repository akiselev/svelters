//! W5 (plans/research/EGUI.md §8, user 2026-10-22): the X11/Wayland **primary selection** in the
//! text kinds, as GTK and Qt do on Linux: selecting text copies it to the primary selection, and a
//! middle click pastes the primary selection at the pointer. Linux only: elsewhere [`SUPPORTED`]
//! is false and nothing here runs.
//!
//! The selection itself is a [`PrimarySelection`] provider. The eframe runner installs the system
//! one ([`use_system`], arboard's `LinuxClipboardKind::Primary`); an embedder that owns its window
//! calls [`use_system`] or [`install`]s its own; tests install a fake. Without a provider the text
//! kinds behave as plain egui.
//!
//! Text kinds call [`text_edit`] with their `TextEditOutput`: built-in `text-edit` does, and a
//! provider kind built on `egui::TextEdit` (`#[svelters::widget]`) can too.
//!
//! W1: no other text behaviour is changed here; selection and keyboard conventions stay egui's
//! until WG3's goldens measure the gap (W4).

use std::sync::{Arc, Mutex};

/// True where the primary selection exists (Linux).
pub const SUPPORTED: bool = cfg!(target_os = "linux");

/// The primary selection: what select-to-copy writes and middle-click paste reads.
pub trait PrimarySelection: Send + Sync {
    /// Makes `text` the primary selection.
    fn set(&self, text: &str);
    /// The primary selection, if there is one.
    fn get(&self) -> Option<String>;
}

static PROVIDER: Mutex<Option<Arc<dyn PrimarySelection>>> = Mutex::new(None);

/// Installs `p` as the process's primary selection (replacing any other). Ignored where the
/// primary selection is not supported.
pub fn install(p: impl PrimarySelection + 'static) {
    if SUPPORTED {
        *PROVIDER.lock().unwrap() = Some(Arc::new(p));
    }
}

/// Removes the provider: the text kinds stop using the primary selection.
pub fn uninstall() {
    *PROVIDER.lock().unwrap() = None;
}

/// The installed provider.
pub fn provider() -> Option<Arc<dyn PrimarySelection>> {
    if !SUPPORTED {
        return None;
    }
    PROVIDER.lock().unwrap().clone()
}

/// Installs the system primary selection (X11 or Wayland, through arboard) unless a provider is
/// installed already. False where it is not supported or not available (no display).
pub fn use_system() -> bool {
    #[cfg(target_os = "linux")]
    {
        let mut p = PROVIDER.lock().unwrap();
        if p.is_some() {
            return true;
        }
        match system::System::new() {
            Some(s) => {
                *p = Some(Arc::new(s));
                true
            }
            None => false,
        }
    }
    #[cfg(not(target_os = "linux"))]
    false
}

#[cfg(target_os = "linux")]
mod system {
    use arboard::{Clipboard, GetExtLinux, LinuxClipboardKind, SetExtLinux};
    use std::sync::Mutex;

    pub(super) struct System(Mutex<Clipboard>);

    impl System {
        pub(super) fn new() -> Option<System> {
            Clipboard::new().ok().map(|c| System(Mutex::new(c)))
        }
    }

    impl super::PrimarySelection for System {
        fn set(&self, text: &str) {
            let _ = self.0.lock().unwrap().set().clipboard(LinuxClipboardKind::Primary).text(text.to_string());
        }
        fn get(&self) -> Option<String> {
            self.0.lock().unwrap().get().clipboard(LinuxClipboardKind::Primary).text().ok()
        }
    }
}

/// Applies the primary selection to a text field after it ran: copies a new non-empty selection
/// (never from a password field), and on a middle click inserts the primary selection at the
/// pointer, moving the cursor after it (bounded by `char_limit` when it is > 0). True if the text
/// changed (the caller announces the edit). `id` is the `TextEdit`'s id.
pub fn text_edit(ui: &egui::Ui, out: &egui::text_edit::TextEditOutput, id: egui::Id, text: &mut String, password: bool, char_limit: usize) -> bool {
    let Some(p) = provider() else { return false };
    let resp = &out.response.response;
    let last = id.with("svelters-primary");
    // select-to-copy: a selection that differs from the last one copied
    if !password
        && let Some(range) = out.cursor_range
        && !range.is_empty()
    {
        let sel = range.slice_str(text);
        if ui.data(|d| d.get_temp::<String>(last)).as_deref() != Some(sel) {
            p.set(sel);
            let sel = sel.to_string();
            ui.data_mut(|d| d.insert_temp(last, sel));
        }
    }
    // middle-click paste at the pointer
    if !resp.clicked_by(egui::PointerButton::Middle) || !resp.enabled() {
        return false;
    }
    let (Some(pos), Some(mut paste)) = (resp.interact_pointer_pos(), p.get()) else { return false };
    let at = out.galley.cursor_from_pos(pos - out.galley_pos).index.0.min(text.chars().count());
    if char_limit > 0 {
        let room = char_limit.saturating_sub(text.chars().count());
        paste = paste.chars().take(room).collect();
    }
    if paste.is_empty() {
        return false;
    }
    let byte = text.char_indices().nth(at).map_or(text.len(), |(b, _)| b);
    text.insert_str(byte, &paste);
    let mut state = out.state.clone();
    state.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(at + paste.chars().count()))));
    state.store(ui.ctx(), id);
    resp.request_focus();
    ui.ctx().request_repaint();
    true
}
