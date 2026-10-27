//! W5 (plans/research/EGUI.md §8, user 2026-10-22): the X11/Wayland **primary selection** in the
//! text kinds, as GTK and Qt do on Linux: selecting text copies it to the primary selection, and a
//! middle click pastes the primary selection at the pointer. Linux only: elsewhere [`SUPPORTED`]
//! is false and nothing here runs.
//!
//! The selection itself is a [`PrimarySelection`] provider. FX3s folded it onto DX2's clipboard: any
//! [`ClipboardBackend`] is one ([`install_backend`], its [`ClipboardKind::Primary`]), and the eframe
//! runner installs the system one ([`use_system`]: [`crate::platform::SystemClipboard`], the
//! backend `Host::platform()` uses on the desktop; svelters-host's `platform::system_primary` since SV75). An embedder that owns its window calls
//! [`use_system`] or installs its own; tests install a fake. Without a provider the text kinds
//! behave as plain egui.
//!
//! egui 0.37 has PRIMARY of its own under egui-winit (`OutputCommand::TextSelectionSettled`,
//! `Event::MiddleClickPaste`): when the integration sent `MiddleClickPaste`, `TextEdit` has pasted
//! already and [`text_edit`] does not paste a second time.
//!
//! Text kinds call [`text_edit`] with their `TextEditOutput`: built-in `text-edit` does, and a
//! provider kind built on `egui::TextEdit` (`#[svelters::widget]`) can too.
//!
//! W1: no other text behaviour is changed here; selection and keyboard conventions stay egui's
//! until WG3's goldens measure the gap (W4).

use std::sync::{Arc, Mutex};
use svelters_host::platform::{ClipboardBackend, ClipboardKind, MimeData, TEXT_PLAIN};

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

/// A [`ClipboardBackend`]'s PRIMARY as the provider.
struct Backend(Arc<dyn ClipboardBackend>);

impl PrimarySelection for Backend {
    fn set(&self, text: &str) {
        let _ = self.0.write(ClipboardKind::Primary, MimeData::text(text));
    }
    fn get(&self) -> Option<String> {
        self.0.read(ClipboardKind::Primary, &[TEXT_PLAIN]).ok()?.plain_text().map(str::to_string)
    }
}

/// Installs `clipboard`'s [`ClipboardKind::Primary`] as the process's primary selection (DX2's
/// clipboard, e.g. `Host::platform().clipboard`).
pub fn install_backend(clipboard: Arc<dyn ClipboardBackend>) {
    if SUPPORTED {
        *PROVIDER.lock().unwrap() = Some(Arc::new(Backend(clipboard)));
    }
}

/// Installs the system primary selection (X11 or Wayland: [`crate::platform::SystemClipboard`])
/// unless a provider is installed already. False where it is not supported or not available (no
/// display).
pub fn use_system() -> bool {
    if !SUPPORTED {
        return false;
    }
    let mut p = PROVIDER.lock().unwrap();
    if p.is_some() {
        return true;
    }
    let Some(system) = svelters_host::platform::system_primary() else { return false };
    *p = Some(Arc::new(Backend(Arc::new(system))));
    true
}

fn last_id(id: egui::Id) -> egui::Id {
    id.with("svelters-primary")
}

/// Applies the primary selection to a text field after it ran: copies a new non-empty selection
/// (never from a password field), and on a middle click inserts the primary selection at the
/// pointer, moving the cursor after it (bounded by `char_limit` when it is > 0). True if the text
/// changed (the caller announces the edit). `id` is the `TextEdit`'s id.
pub fn text_edit(ui: &egui::Ui, out: &egui::text_edit::TextEditOutput, id: egui::Id, text: &mut String, password: bool, char_limit: usize) -> bool {
    let Some(p) = provider() else { return false };
    let resp = &out.response.response;
    let last = last_id(id);
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
    // egui-winit sent `MiddleClickPaste` with the press, and `TextEdit` pasted it
    if ui.ctx().data(|d| d.get_temp::<bool>(egui_pasted_id())).unwrap_or(false) {
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

fn egui_pasted_id() -> egui::Id {
    egui::Id::unique("svelters-egui/primary/egui-pasted")
}

/// Before a text kind runs: notes whether the integration sent `MiddleClickPaste` for the current
/// middle press (egui's `TextEdit` pastes it on the press; [`text_edit`] would paste again on the
/// release). A press resets it; the event sets it (with the press, or in a pass after it).
/// Idempotent within a pass.
pub(crate) fn note_egui_paste(ctx: &egui::Context) {
    let (press, egui) = ctx.input(|i| {
        let press = i.events.iter().any(|e| matches!(e, egui::Event::PointerButton { button: egui::PointerButton::Middle, pressed: true, .. }));
        (press, i.events.iter().any(|e| matches!(e, egui::Event::MiddleClickPaste { .. })))
    });
    if press || egui {
        ctx.data_mut(|d| d.insert_temp(egui_pasted_id(), egui));
    }
}
