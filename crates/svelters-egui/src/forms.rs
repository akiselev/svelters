//! FM1, the form widget gaps (plans/milestones/FM1-form-widgets.md): what `text-edit`'s `pattern`
//! (QW8) and a box's `button-order` (QW11) need.
//!
//! The props themselves are in the kind tables (`kinds/builtin.rs`, `kinds/inputs.rs`,
//! `kinds/containers.rs`, `kinds/views.rs`); `drag-value`'s steps and commits are `elements/inputs.rs`,
//! the table header's state `views/table.rs`, the keyboard context menu `menus.rs`.

use crate::scene::{Arena, W};
use egui::{Id, Key, Response, Ui};
use regex_lite::Regex;

/// A `text-edit` `pattern`: the whole text must match it (HTML's `pattern`: anchored).
#[derive(Clone, Debug)]
pub struct Pattern {
    src: String,
    re: Regex,
}

impl PartialEq for Pattern {
    fn eq(&self, other: &Self) -> bool {
        self.src == other.src
    }
}

impl Pattern {
    /// `None` for an empty pattern (no check); an error for one that does not compile.
    pub fn new(src: &str) -> Result<Option<Pattern>, String> {
        if src.is_empty() {
            return Ok(None);
        }
        let re = Regex::new(&format!("^(?:{src})$")).map_err(|e| format!("pattern {src:?}: {e}"))?;
        Ok(Some(Pattern { src: src.to_string(), re }))
    }

    pub fn source(&self) -> &str {
        &self.src
    }

    /// Whether `text` is acceptable: empty text always is (a required field is the app's check, as
    /// HTML's `required`), else the pattern matches all of it.
    pub fn accepts(&self, text: &str) -> bool {
        text.is_empty() || self.re.is_match(text)
    }
}

/// Whether `w` is a `text-edit` whose text fails its `pattern` (painted with the invalid outline).
pub(crate) fn text_rejected(w: &W) -> bool {
    matches!(w, W::TextEdit { text, pattern: Some(p), .. } if !p.accepts(text))
}

/// After `text-edit` node `i` ran (`resp`, its egui id `id`; `had_focus` before this pass,
/// `edited` when the user changed the text in it): the events it finishes with, `submit` and
/// `commit` (Qt `returnPressed`, `editingFinished`). Enter in a one-line field is both (also when
/// it went on to the default button, `focus::take_enter`); losing focus after an edit is a
/// `commit`. A text that fails `pattern` finishes nothing (and is AccessKit `invalid`).
pub(crate) fn text_finished(arena: &Arena, i: usize, ui: &Ui, resp: &Response, id: Id, had_focus: bool, edited: bool) -> Vec<&'static str> {
    let W::TextEdit { multiline, .. } = arena.w(i) else { return Vec::new() };
    let rejected = text_rejected(arena.w(i));
    if rejected {
        ui.ctx().accesskit_node_builder(resp.id, |b| b.set_invalid(egui::accesskit::Invalid::True));
    }
    let key = id.with("fm1-edited");
    let mut dirty = edited || (!resp.gained_focus() && ui.data(|d| d.get_temp::<bool>(key)).unwrap_or(false));
    // `take_enter` first: it clears the record whatever else happens
    let enter = (crate::focus::take_enter(ui, id) || (resp.lost_focus() && ui.input(|inp| inp.key_pressed(Key::Enter)))) && !*multiline;
    let lost = had_focus && !resp.has_focus();
    let mut out = Vec::new();
    if !rejected {
        if enter {
            out.extend(["submit", "commit"]);
            dirty = false;
        } else if lost && dirty {
            out.push("commit");
            dirty = false;
        }
    }
    ui.data_mut(|d| d.insert_temp(key, dirty));
    out
}

/// The `dialog-role`s of a button in a box with a `button-order` (Qt's `QDialogButtonBox::ButtonRole`, reduced).
/// `auto` is `accept` for a `default` button, `reject` for a `cancel` button, else `action`.
pub const BUTTON_ROLE: &[&str] = &["auto", "accept", "reject", "destructive", "apply", "reset", "help", "action"];

/// The layout `platform` stands for on this build's OS.
pub fn platform_layout() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        // GTK's order (Cancel before OK) for Linux and the other Unixes; KDE is chosen explicitly
        "gnome"
    }
}

/// The order of the roles in `layout`, left to right (Qt's `layouts[]` table, reduced; help and
/// reset are placed first by [`arrange`]).
fn order(layout: &str) -> &'static [&'static str] {
    match layout {
        // Help, Reset | Accept, Action, Reject, Destructive, Apply
        "windows" => &["help", "reset", "accept", "action", "reject", "destructive", "apply"],
        // Help, Reset | Destructive, Action, Reject, Accept
        "macos" => &["help", "reset", "destructive", "apply", "action", "reject", "accept"],
        // KDE: Help, Reset | Accept, Action, Apply, Destructive, Reject
        "kde" => &["help", "reset", "accept", "action", "apply", "destructive", "reject"],
        // GNOME: Help, Reset | Destructive, Action, Apply, Reject, Accept
        _ => &["help", "reset", "destructive", "action", "apply", "reject", "accept"],
    }
}

/// The resolved role of a button (`auto` from its `default`/`cancel` flags).
pub fn resolve_role(role: &str, default: bool, cancel: bool) -> &'static str {
    match role {
        "auto" | "" if default => "accept",
        "auto" | "" if cancel => "reject",
        "auto" | "" => "action",
        r => BUTTON_ROLE.iter().copied().find(|x| *x == r).unwrap_or("action"),
    }
}

/// The layout order of `roles` (children in tree order; `None` for a child that is not a button)
/// under `layout` (`platform` resolved): help and reset first, then the children that are not
/// buttons (a spacer), then the layout's order; stable within a role.
pub fn arrange(layout: &str, roles: &[Option<&str>]) -> Vec<usize> {
    let layout = if layout == "platform" { platform_layout() } else { layout };
    let ord = order(layout);
    let rank = |r: Option<&str>| match r {
        Some("help") => 0,
        Some("reset") => 1,
        None => 2,
        Some(r) => 3 + ord.iter().position(|x| *x == r).unwrap_or(0),
    };
    let mut ix: Vec<usize> = (0..roles.len()).collect();
    ix.sort_by_key(|&k| rank(roles[k]));
    ix
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fm1_pattern_is_anchored_and_empty_text_passes() {
        let p = Pattern::new("[0-9]+").unwrap().unwrap();
        assert!(p.accepts("123") && p.accepts("") && !p.accepts("12a") && !p.accepts("a12"));
        assert!(Pattern::new("").unwrap().is_none());
        assert!(Pattern::new("(").is_err());
        // alternation stays inside the anchors
        let p = Pattern::new("mm|in").unwrap().unwrap();
        assert!(p.accepts("mm") && !p.accepts("mmin"));
    }

    #[test]
    fn fm1_dialog_button_order_per_platform() {
        let roles = [Some("accept"), Some("reject"), Some("help"), Some("apply")];
        let names = |layout| arrange(layout, &roles).into_iter().map(|k| roles[k].unwrap()).collect::<Vec<_>>();
        assert_eq!(names("windows"), ["help", "accept", "reject", "apply"]);
        assert_eq!(names("macos"), ["help", "apply", "reject", "accept"]);
        assert_eq!(names("gnome"), ["help", "apply", "reject", "accept"]);
        assert_eq!(names("kde"), ["help", "accept", "apply", "reject"]);
        assert_eq!(resolve_role("auto", true, false), "accept");
        assert_eq!(resolve_role("auto", false, true), "reject");
        assert_eq!(resolve_role("", false, false), "action");
        assert_eq!(resolve_role("help", true, false), "help");
        // a spacer (not a button) sits after help, before the rest
        assert_eq!(arrange("windows", &[Some("reject"), None, Some("help"), Some("accept")]), [2, 1, 3, 0]);
    }
}
