//! `RovingList` (OW3; named so beside OW1a2's group [`Roving`](super::Roving), which moves focus
//! across sibling nodes; this one moves a highlighted index inside one widget): one current item in a group (menu rows, a menu bar, a select's options, radio groups,
//! toolbars), moved by the arrow keys of its orientation, Home and End; wraps if asked; skips
//! disabled items (ARIA APG roving tabindex). The group is one tab stop; the widget moves its
//! highlight or focus to [`RovingList::current`]. Writer lane: OW3.

use super::{InputCx, Intents, Unit};
use crate::provider::StateBits;
use egui::Key;

/// The arrow keys a group moves with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Orientation {
    /// Up and Down (menus, lists).
    #[default]
    Vertical,
    /// Left and Right (menu bars, toolbars, tab strips).
    Horizontal,
}

/// What [`RovingList`] is given each pass.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RovingListParams {
    /// Per item, whether it can be current (enabled and not a separator).
    pub enabled: Vec<bool>,
    pub orientation: Orientation,
    /// Past the last item to the first, and back.
    pub wrap: bool,
}

/// What [`RovingList`] reports: the current item moved to `to`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Move {
    pub to: usize,
}

/// One current index, moved by the keyboard (and set by the widget: hover, a pick).
#[derive(Clone, Debug, Default)]
pub struct RovingList {
    current: Option<usize>,
}

impl RovingList {
    pub fn current(&self) -> Option<usize> {
        self.current
    }
    /// Sets the current item (the pointer hovered it, the widget opened at an item); `None`
    /// clears it.
    pub fn set(&mut self, i: Option<usize>) {
        self.current = i;
    }

    /// The first (`forward`) or last item that can be current.
    pub fn edge(enabled: &[bool], forward: bool) -> Option<usize> {
        if forward { enabled.iter().position(|e| *e) } else { enabled.iter().rposition(|e| *e) }
    }

    /// The item `step` (±1) from `from` that can be current, wrapping if asked; from nothing, the
    /// first or last one.
    pub fn next(enabled: &[bool], from: Option<usize>, step: i32, wrap: bool) -> Option<usize> {
        let n = enabled.len() as i32;
        let Some(from) = from.filter(|f| (*f as i32) < n) else { return RovingList::edge(enabled, step > 0) };
        let mut i = from as i32;
        for _ in 0..n {
            i += step;
            if i < 0 || i >= n {
                if !wrap {
                    return Some(from);
                }
                i = i.rem_euclid(n);
            }
            if enabled[i as usize] {
                return Some(i as usize);
            }
        }
        Some(from)
    }

    /// Applies one key: the item it moves to, if the key is one of the group's.
    pub fn key(&mut self, p: &RovingListParams, key: Key) -> Option<usize> {
        let (back, fwd) = match p.orientation {
            Orientation::Vertical => (Key::ArrowUp, Key::ArrowDown),
            Orientation::Horizontal => (Key::ArrowLeft, Key::ArrowRight),
        };
        let to = match key {
            k if k == back => RovingList::next(&p.enabled, self.current, -1, p.wrap),
            k if k == fwd => RovingList::next(&p.enabled, self.current, 1, p.wrap),
            Key::Home => RovingList::edge(&p.enabled, true),
            Key::End => RovingList::edge(&p.enabled, false),
            _ => return None,
        }?;
        self.current = Some(to);
        Some(to)
    }
}

impl Unit for RovingList {
    type Params = RovingListParams;
    type Intent = Move;
    const OWNS: StateBits = StateBits::NONE;

    fn input(&mut self, p: &RovingListParams, cx: &mut InputCx<'_>, out: &mut Intents<Move>) {
        if !cx.enabled || !cx.resp.has_focus {
            return;
        }
        for e in &cx.input.events {
            if let egui::Event::Key { key, pressed: true, modifiers, .. } = e
                && !modifiers.any()
                && let Some(to) = self.key(p, *key)
            {
                out.push(Move { to });
            }
        }
    }

    fn claims(&self, p: &RovingListParams, key: Key, m: egui::Modifiers) -> bool {
        let axis = match p.orientation {
            Orientation::Vertical => matches!(key, Key::ArrowUp | Key::ArrowDown),
            Orientation::Horizontal => matches!(key, Key::ArrowLeft | Key::ArrowRight),
        };
        !m.any() && (axis || matches!(key, Key::Home | Key::End))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_skip_disabled_and_wrap() {
        let p = RovingListParams { enabled: vec![false, true, false, true, true], orientation: Orientation::Vertical, wrap: true };
        let mut r = RovingList::default();
        assert_eq!(r.key(&p, Key::ArrowDown), Some(1), "from nothing: the first enabled");
        assert_eq!(r.key(&p, Key::ArrowDown), Some(3), "skips the disabled one");
        assert_eq!(r.key(&p, Key::ArrowDown), Some(4));
        assert_eq!(r.key(&p, Key::ArrowDown), Some(1), "wraps");
        assert_eq!(r.key(&p, Key::ArrowUp), Some(4));
        assert_eq!(r.key(&p, Key::Home), Some(1));
        assert_eq!(r.key(&p, Key::End), Some(4));
        assert_eq!(r.key(&p, Key::ArrowLeft), None, "not this orientation's key");
        let q = RovingListParams { wrap: false, ..p };
        assert_eq!(r.key(&q, Key::ArrowDown), Some(4), "stays at the end without wrap");
        assert_eq!(RovingList::next(&[false, false], None, 1, true), None);
    }
}
