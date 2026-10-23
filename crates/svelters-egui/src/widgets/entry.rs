//! OW1b: the one-line text entry of the numeric widgets' value boxes (`drag-value` after a click,
//! `slider`'s value box). Interim: a private `String` buffer with a caret and a select-all
//! selection, typing, Backspace/Delete, Left/Right/Home/End (Shift extends), Ctrl/Cmd+A, and the
//! clipboard events. OW4's `TextInput` unit on TC0 (`quire-text`) replaces it (word motions, IME
//! preedit, undo, AccessKit text runs); the API the widgets use stays [`Entry::begin`],
//! [`Entry::input`], [`Entry::paint`].

use crate::paint::{self, PaintCx, StyleView};
use egui::{Event, Key, Rect};

/// What one pass of input did to the entry.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EntryOut {
    /// The text changed.
    pub edited: bool,
    /// Enter: apply.
    pub enter: bool,
    /// Escape: drop the edit.
    pub escape: bool,
}

#[derive(Debug, Default)]
pub struct Entry {
    pub editing: bool,
    pub text: String,
    /// Byte offsets: the caret and the selection's other end.
    caret: usize,
    anchor: usize,
    /// The user changed the text since [`Entry::begin`].
    pub dirty: bool,
}

fn prev(s: &str, i: usize) -> usize {
    s[..i].char_indices().next_back().map_or(0, |(k, _)| k)
}

fn next(s: &str, i: usize) -> usize {
    s[i..].chars().next().map_or(i, |c| i + c.len_utf8())
}

impl Entry {
    /// Starts editing `text`, all of it selected (as a spin box's text on focus).
    pub fn begin(&mut self, text: String) {
        self.anchor = 0;
        self.caret = text.len();
        self.text = text;
        self.editing = true;
        self.dirty = false;
    }

    /// Stops editing.
    pub fn end(&mut self) {
        self.editing = false;
        self.dirty = false;
    }

    /// Replaces the text (a step while editing), all selected.
    pub fn reset(&mut self, text: String) {
        let dirty = self.dirty;
        self.begin(text);
        self.dirty = dirty;
    }

    fn sel(&self) -> (usize, usize) {
        (self.caret.min(self.anchor), self.caret.max(self.anchor))
    }

    fn selected(&self) -> &str {
        let (a, b) = self.sel();
        &self.text[a..b]
    }

    fn insert(&mut self, t: &str) {
        let (a, b) = self.sel();
        let t: String = t.chars().filter(|c| !c.is_control()).collect();
        self.text.replace_range(a..b, &t);
        self.caret = a + t.len();
        self.anchor = self.caret;
    }

    fn delete(&mut self, forward: bool) {
        let (a, b) = self.sel();
        if a != b {
            self.insert("");
        } else if forward && b < self.text.len() {
            let n = next(&self.text, b);
            self.text.replace_range(b..n, "");
        } else if !forward && a > 0 {
            let p = prev(&self.text, a);
            self.text.replace_range(p..a, "");
            self.caret = p;
            self.anchor = p;
        }
    }

    fn motion(&mut self, to: usize, extend: bool) {
        self.caret = to;
        if !extend {
            self.anchor = to;
        }
    }

    /// Applies this pass's keyboard events (the caller checks that the box has focus). Copy and
    /// Cut put the selection on egui's clipboard (`ctx`).
    pub fn input(&mut self, ctx: &egui::Context, events: &[Event]) -> EntryOut {
        let mut out = EntryOut::default();
        let before = self.text.clone();
        for e in events {
            match e {
                Event::Text(t) | Event::Paste(t) => self.insert(t),
                Event::Copy => {
                    let s = self.selected().to_string();
                    if !s.is_empty() {
                        ctx.copy_text(s);
                    }
                }
                Event::Cut => {
                    let s = self.selected().to_string();
                    if !s.is_empty() {
                        ctx.copy_text(s);
                        self.insert("");
                    }
                }
                Event::Key { key, pressed: true, modifiers: m, .. } => {
                    let cmd = m.command || m.ctrl;
                    match key {
                        Key::A if cmd => {
                            self.anchor = 0;
                            self.caret = self.text.len();
                        }
                        Key::Backspace => self.delete(false),
                        Key::Delete => self.delete(true),
                        Key::ArrowLeft => {
                            let (a, _) = self.sel();
                            let to = if a != self.caret.max(self.anchor) && !m.shift { a } else { prev(&self.text, self.caret) };
                            self.motion(to, m.shift);
                        }
                        Key::ArrowRight => {
                            let (_, b) = self.sel();
                            let to = if b != self.caret.min(self.anchor) && !m.shift { b } else { next(&self.text, self.caret) };
                            self.motion(to, m.shift);
                        }
                        Key::Home => self.motion(0, m.shift),
                        Key::End => self.motion(self.text.len(), m.shift),
                        Key::Enter => out.enter = true,
                        Key::Escape => out.escape = true,
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        out.edited = self.text != before;
        self.dirty |= out.edited;
        out
    }

    /// Paints the text in `content` (scrolled to keep the caret in view), the selection and the
    /// caret (`caret` colour, 1 physical px; no blink yet).
    pub fn paint(&self, cx: &mut PaintCx, s: &StyleView, content: Rect, align: paint::Align, focused: bool) {
        let ctx = cx.painter.ctx().clone();
        let g = paint::text_galley(&ctx, s, &self.text, None);
        let chars = |b: usize| self.text[..b].chars().count();
        let caret_x = g.pos_from_cursor(egui::text::CCursor::new(chars(self.caret))).min.x;
        let free = content.width() - g.size().x;
        let mut x0 = match align {
            paint::Align::Start => content.left(),
            paint::Align::Center => content.center().x - g.size().x / 2.0,
            paint::Align::End => content.right() - g.size().x,
        };
        if free < 0.0 {
            // keep the caret inside
            x0 = (content.right() - caret_x - 1.0).min(content.left()).max(content.right() - g.size().x - 1.0);
        }
        let y0 = content.center().y - g.size().y / 2.0;
        let clip = cx.painter.clip_rect().intersect(content.expand(1.0));
        let p = cx.painter.with_clip_rect(clip);
        let (a, b) = self.sel();
        if focused && a != b {
            let xa = g.pos_from_cursor(egui::text::CCursor::new(chars(a))).min.x;
            let xb = g.pos_from_cursor(egui::text::CCursor::new(chars(b))).min.x;
            let r = Rect::from_min_max(egui::pos2(x0 + xa, y0), egui::pos2(x0 + xb, y0 + g.size().y));
            let sel = crate::paint::ua_token(s.is_dark(), "selection").unwrap_or(egui::Color32::from_rgb(38, 79, 120));
            p.rect_filled(r, 0.0, sel);
        }
        let color = s.color("color").unwrap_or(egui::Color32::PLACEHOLDER);
        p.galley(egui::pos2(x0, y0), g.clone(), color);
        if focused {
            let w = cx.snap_width(1.0);
            let x = ((x0 + caret_x) * cx.ppp).round() / cx.ppp;
            let caret = crate::paint::ua_token(s.is_dark(), "caret").unwrap_or(color);
            p.rect_filled(Rect::from_min_size(egui::pos2(x, y0), egui::vec2(w, g.size().y)), 0.0, caret);
        }
    }

    /// Where the caret is, for the IME and `scroll_to`.
    pub fn caret_rect(&self, ctx: &egui::Context, s: &StyleView, content: Rect) -> Rect {
        let g = paint::text_galley(ctx, s, &self.text, None);
        let c = g.pos_from_cursor(egui::text::CCursor::new(self.text[..self.caret].chars().count()));
        Rect::from_min_size(egui::pos2(content.left() + c.min.x, content.center().y - g.size().y / 2.0), egui::vec2(1.0, g.size().y))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(k: Key, m: egui::Modifiers) -> Event {
        Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: m }
    }

    #[test]
    fn ow1b_entry_types_over_the_selection_and_edits() {
        let ctx = egui::Context::default();
        let mut e = Entry::default();
        e.begin("12.5".into());
        let o = e.input(&ctx, &[Event::Text("7".into())]);
        assert!(o.edited && e.text == "7" && e.dirty);
        e.input(&ctx, &[Event::Text("é".into()), key(Key::ArrowLeft, egui::Modifiers::NONE), key(Key::Backspace, egui::Modifiers::NONE)]);
        assert_eq!(e.text, "é");
        e.input(&ctx, &[key(Key::A, egui::Modifiers::COMMAND), key(Key::Delete, egui::Modifiers::NONE)]);
        assert_eq!(e.text, "");
        let o = e.input(&ctx, &[key(Key::Enter, egui::Modifiers::NONE)]);
        assert!(o.enter && !o.edited);
    }
}
