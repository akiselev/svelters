//! `RovingList` (named so beside the group [`Roving`](super::Roving), which moves focus across
//! sibling nodes; this one moves a highlighted index inside one kind): one current item in a group
//! (menu rows, a menu bar, a select's options, radio groups, toolbars), moved by the arrow keys of
//! its orientation, Home and End; wraps if asked; skips disabled items (ARIA APG roving tabindex).
//! The group is one tab stop; the kind moves its highlight or focus to [`RovingList::current`].

use super::{FrameEvent, InputCx, Intents, Mods, StateBits, Unit};
use presto_base::key::Key;

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

/// One current index, moved by the keyboard (and set by the kind: hover, a pick).
#[derive(Clone, Debug, Default)]
pub struct RovingList {
    current: Option<usize>,
}

impl RovingList {
    pub fn current(&self) -> Option<usize> {
        self.current
    }
    /// Sets the current item (the pointer hovered it, the kind opened at an item); `None` clears
    /// it.
    pub fn set(&mut self, i: Option<usize>) {
        self.current = i;
    }

    /// The first (`forward`) or last item that can be current.
    pub fn edge(enabled: &[bool], forward: bool) -> Option<usize> {
        RovingList::edge_by(enabled.len(), forward, |i| enabled[i])
    }

    /// The item `step` (±1) from `from` that can be current, wrapping if asked; from nothing, the
    /// first or last one.
    pub fn next(enabled: &[bool], from: Option<usize>, step: i32, wrap: bool) -> Option<usize> {
        RovingList::next_by(enabled.len(), from, step, wrap, |i| enabled[i])
    }

    /// [`edge`](Self::edge) over `len` items, `enabled(i)` asked lazily (lists of 100k rows: no
    /// per-key allocation).
    pub fn edge_by(len: usize, forward: bool, enabled: impl Fn(usize) -> bool) -> Option<usize> {
        if forward { (0..len).find(|&i| enabled(i)) } else { (0..len).rev().find(|&i| enabled(i)) }
    }

    /// [`next`](Self::next) over `len` items, `enabled(i)` asked lazily.
    pub fn next_by(len: usize, from: Option<usize>, step: i32, wrap: bool, enabled: impl Fn(usize) -> bool) -> Option<usize> {
        let n = len as i64;
        let Some(from) = from.filter(|f| (*f as i64) < n) else { return RovingList::edge_by(len, step > 0, enabled) };
        let mut i = from as i64;
        for _ in 0..n {
            i += step as i64;
            if i < 0 || i >= n {
                if !wrap {
                    return Some(from);
                }
                i = i.rem_euclid(n);
            }
            if enabled(i as usize) {
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
            if let FrameEvent::Key { key, pressed: true, modifiers, .. } = e
                && !modifiers.any()
                && let Some(to) = self.key(p, *key)
            {
                out.push(Move { to });
            }
        }
    }

    fn claims(&self, p: &RovingListParams, key: Key, m: Mods) -> bool {
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
    use crate::interact::Sense;
    use crate::interact::tracker::testing::Rig;

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

    #[test]
    fn the_lazy_form_agrees_with_the_slice_form() {
        let sets: [&[bool]; 4] = [&[false, true, false, true, true], &[true; 6], &[false, false], &[]];
        for en in sets {
            for wrap in [false, true] {
                for &step in &[-1, 1] {
                    for from in [None, Some(0), Some(2), Some(4), Some(9)] {
                        assert_eq!(RovingList::next(en, from, step, wrap), RovingList::next_by(en.len(), from, step, wrap, |i| en[i]), "{en:?} {from:?} {step} {wrap}");
                    }
                }
            }
            for fwd in [false, true] {
                assert_eq!(RovingList::edge(en, fwd), RovingList::edge_by(en.len(), fwd, |i| en[i]));
            }
        }
    }

    #[test]
    fn keys_move_only_while_focused_and_unmodified() {
        let p = RovingListParams { enabled: vec![true, true, true], orientation: Orientation::Horizontal, wrap: false };
        let mut rig = Rig::new(Sense::FOCUSABLE);
        let mut r = RovingList::default();
        let mut out = Intents::default();
        rig.key(Key::ArrowRight, |cx| r.input(&p, cx, &mut out));
        assert!(out.is_empty(), "not focused");
        rig.focus(true, |_| {});
        rig.key(Key::ArrowRight, |cx| r.input(&p, cx, &mut out));
        rig.key(Key::ArrowRight, |cx| r.input(&p, cx, &mut out));
        rig.key_with(Key::ArrowRight, presto_base::event::Modifiers::SHIFT, None, |cx| r.input(&p, cx, &mut out));
        assert_eq!(out.0, vec![Move { to: 0 }, Move { to: 1 }]);
        assert!(r.claims(&p, Key::Home, Mods::NONE) && !r.claims(&p, Key::ArrowUp, Mods::NONE));
    }
}
