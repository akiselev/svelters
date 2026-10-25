//! `Roving` (ARCH-WIDGETS §1.2 catalogue, WIDGET-PARITY §2.3): the arrow keys move within a group
//! that is one Tab stop (radio buttons, `toggle-button` groups). While the host is focused, Up/Left go
//! to the previous member, Down/Right to the next (wrapping, as Qt and GTK do), and Home/End to the
//! first and last.
//!
//! The unit only reports the move. The kind turns it into a peer request
//! ([`PeerTarget::Rove`](crate::shim::core::PeerTarget::Rove)): skelter-core finds the members of the
//! node's [`PeerGroup`](crate::shim::core::PeerGroup), moves keyboard focus to the one the move lands
//! on ([`Rove::target`]) and tells it ([`PeerMsg::Roved`](crate::shim::core::PeerMsg::Roved)), and the
//! member checks itself.

use super::{FrameEvent, InputCx, Intents, Mods, StateBits, Unit};
use skelter_base::key::Key;

/// What a kind configures on its [`Roving`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RovingParams {
    /// Whether the group roves at all (a kind outside any group does not).
    pub on: bool,
}

/// What [`Roving`] reports: where focus goes within the group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rove {
    /// By `n` members (−1 previous, +1 next), wrapping.
    By(i32),
    First,
    Last,
}

impl Rove {
    /// The member index this move lands on, from `at` in a group of `len` (`len > 0`).
    pub fn target(self, at: usize, len: usize) -> usize {
        match self {
            Rove::First => 0,
            Rove::Last => len - 1,
            Rove::By(n) => (at as i64 + n as i64).rem_euclid(len as i64) as usize,
        }
    }
}

const KEYS: [(Key, Rove); 6] = [
    (Key::ArrowUp, Rove::By(-1)),
    (Key::ArrowLeft, Rove::By(-1)),
    (Key::ArrowDown, Rove::By(1)),
    (Key::ArrowRight, Rove::By(1)),
    (Key::Home, Rove::First),
    (Key::End, Rove::Last),
];

fn rove_of(key: Key) -> Option<Rove> {
    KEYS.iter().find(|(k, _)| *k == key).map(|(_, r)| *r)
}

#[derive(Debug, Default)]
pub struct Roving;

impl Unit for Roving {
    type Params = RovingParams;
    type Intent = Rove;
    const OWNS: StateBits = StateBits::NONE;

    fn input(&mut self, p: &RovingParams, cx: &mut InputCx<'_>, out: &mut Intents<Rove>) {
        if !p.on || !cx.enabled || !cx.focus.has {
            return;
        }
        for e in &cx.input.events {
            if let FrameEvent::Key { key, pressed: true, modifiers, .. } = e
                && modifiers.is_none()
                && let Some(r) = rove_of(*key)
            {
                out.push(r);
            }
        }
    }

    fn claims(&self, p: &RovingParams, key: Key, m: Mods) -> bool {
        p.on && m.is_none() && rove_of(key).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interact::{FocusCx, InputFrame, PartResponses, Policy, Resp};
    use smallvec::smallvec;

    #[test]
    fn rove_wraps() {
        assert_eq!(Rove::By(1).target(2, 3), 0);
        assert_eq!(Rove::By(-1).target(0, 3), 2);
        assert_eq!((Rove::First.target(1, 3), Rove::Last.target(1, 3)), (0, 2));
    }

    fn run(on: bool, focused: bool, enabled: bool, key: Key, modifiers: Mods) -> Vec<Rove> {
        let resp = Resp { has_focus: focused, enabled, ..Resp::default() };
        let input = InputFrame { modifiers, events: smallvec![FrameEvent::Key { key, pressed: true, repeat: false, modifiers }], ..InputFrame::default() };
        let policy = Policy::default();
        let mut state = StateBits::NONE;
        let mut focus = FocusCx { has: focused, key_pressed: true, ..FocusCx::default() };
        let parts = PartResponses::default();
        let mut cx = InputCx { resp: &resp, input: &input, policy: &policy, now: 0.0, state: &mut state, enabled, a11y: &[], focus: &mut focus, parts: &parts };
        let mut out = Intents::default();
        Roving.input(&RovingParams { on }, &mut cx, &mut out);
        out.0
    }

    #[test]
    fn arrows_home_and_end_rove_while_focused() {
        assert_eq!(run(true, true, true, Key::ArrowDown, Mods::NONE), [Rove::By(1)]);
        assert_eq!(run(true, true, true, Key::ArrowRight, Mods::NONE), [Rove::By(1)]);
        assert_eq!(run(true, true, true, Key::ArrowUp, Mods::NONE), [Rove::By(-1)]);
        assert_eq!(run(true, true, true, Key::ArrowLeft, Mods::NONE), [Rove::By(-1)]);
        assert_eq!(run(true, true, true, Key::Home, Mods::NONE), [Rove::First]);
        assert_eq!(run(true, true, true, Key::End, Mods::NONE), [Rove::Last]);
        assert!(run(true, true, true, Key::Space, Mods::NONE).is_empty());
    }

    #[test]
    fn no_rove_off_unfocused_disabled_or_modified() {
        assert!(run(false, true, true, Key::ArrowDown, Mods::NONE).is_empty());
        assert!(run(true, false, true, Key::ArrowDown, Mods::NONE).is_empty());
        assert!(run(true, true, false, Key::ArrowDown, Mods::NONE).is_empty());
        let shift = Mods { shift: true, ..Mods::NONE };
        assert!(run(true, true, true, Key::ArrowDown, shift).is_empty());
    }

    #[test]
    fn claims_the_arrows_when_on() {
        let p = RovingParams { on: true };
        assert!(Roving.claims(&p, Key::ArrowLeft, Mods::NONE) && Roving.claims(&p, Key::End, Mods::NONE));
        assert!(!Roving.claims(&p, Key::Tab, Mods::NONE));
        assert!(!Roving.claims(&p, Key::ArrowLeft, Mods { ctrl: true, command: true, ..Mods::NONE }));
        assert!(!Roving.claims(&RovingParams { on: false }, Key::ArrowLeft, Mods::NONE));
    }
}

/// A test stand-in for what skelter-core does with a peer group (the rules on
/// [`PeerGroup`](crate::shim::core::PeerGroup)): sibling nodes of one kind, keyboard focus among them,
/// and the peer requests applied in order. Tests only.
#[cfg(test)]
#[allow(dead_code)]
pub(crate) mod group {
    use crate::interact::{FocusReason, Policy, StateBits};
    use crate::shim::core::{Event, EventCx, Kind, PeerMsg, PeerRequest, PeerTarget, Value};
    use skelter_base::accesskit;
    use skelter_base::event::{KeyEvent, KeyLocation, KeyState, Modifiers, MouseButton, PointerEvent, PointerPhase, PointerType};
    use skelter_base::geom::{Point, Size, Vec2};
    use skelter_base::key::Key;
    use skelter_base::time::{Span, Time};
    use smallvec::SmallVec;
    use std::collections::VecDeque;

    /// Sibling nodes of kind `K` under one parent.
    pub struct Group<K: Kind> {
        pub nodes: Vec<K>,
        pub states: Vec<StateBits>,
        pub focused: Option<usize>,
        pub policy: Policy,
        pub size: Size,
        pub now: Time,
        /// `(member, signal, args)` in order.
        pub signals: Vec<(usize, &'static str, Vec<Value>)>,
        /// `(member, prop)` the kinds changed natively, in order.
        pub changed: Vec<(usize, &'static str)>,
    }

    impl<K: Kind> Group<K> {
        pub fn new(nodes: Vec<K>, size: Size) -> Group<K> {
            let n = nodes.len();
            Group { nodes, states: vec![StateBits::NONE; n], focused: None, policy: Policy::default(), size, now: Time(1_000_000_000), signals: Vec::new(), changed: Vec::new() }
        }

        pub fn set(&mut self, i: usize, prop: &str, v: Value) {
            let id = self.nodes[i].kind_spec().prop(prop).unwrap_or_else(|| panic!("no prop `{prop}`"));
            self.nodes[i].set_prop(id, &v).unwrap();
        }

        pub fn get(&self, i: usize, prop: &str) -> Value {
            let id = self.nodes[i].kind_spec().prop(prop).unwrap();
            self.nodes[i].get_prop(id).unwrap()
        }

        /// The values of a bool prop over all nodes.
        pub fn bools(&self, prop: &str) -> Vec<bool> {
            (0..self.nodes.len()).map(|i| self.get(i, prop).as_bool().unwrap()).collect()
        }

        /// The members of `of`'s peer group, in tree order (`of` alone without a group).
        pub fn members(&self, of: usize) -> Vec<usize> {
            let Some(g) = self.nodes[of].peer_group() else { return vec![of] };
            (0..self.nodes.len())
                .filter(|&i| {
                    let k = &self.nodes[i];
                    i == of
                        || (k.kind_spec().tag == g.tag
                            && k.peer_group().is_some()
                            && g.key.as_ref().is_none_or(|(p, v)| k.kind_spec().prop(p).and_then(|id| k.get_prop(id)).as_ref() == Some(v)))
                })
                .collect()
        }

        /// The group's Tab stop: the first member whose `stop` prop is true, else the first member.
        pub fn tab_stop(&self, of: usize) -> usize {
            let m = self.members(of);
            let Some(g) = self.nodes[of].peer_group() else { return of };
            m.iter().copied().find(|&i| self.get(i, g.stop).as_bool() == Some(true)).unwrap_or(m[0])
        }

        /// The signal names emitted so far by member `i`, cleared.
        pub fn take(&mut self) -> Vec<(usize, &'static str, Vec<Value>)> {
            std::mem::take(&mut self.signals)
        }

        fn deliver(&mut self, i: usize, ev: Option<&Event>, msg: Option<&PeerMsg>, queue: &mut VecDeque<(usize, PeerRequest)>) {
            let policy = self.policy;
            let focused = self.focused == Some(i);
            let spec = self.nodes[i].kind_spec();
            let mut cx = EventCx::new(self.now, self.size, &mut self.states[i], &policy);
            cx.focused = focused;
            if let Some(ev) = ev {
                self.nodes[i].event(&mut cx, ev);
            }
            if let Some(msg) = msg {
                self.nodes[i].peer(&mut cx, msg);
            }
            let (emitted, changed, peers, focus) = (std::mem::take(&mut cx.emitted), std::mem::take(&mut cx.changed), std::mem::take(&mut cx.peers), cx.focus_request);
            for e in emitted {
                self.signals.push((i, spec.signals[e.signal.0 as usize].name, e.args));
            }
            for p in changed {
                self.changed.push((i, spec.props[p.0 as usize].name));
            }
            queue.extend(peers.into_iter().map(|r| (i, r)));
            match focus {
                Some(true) if !focused => self.move_focus(i, FocusReason::Pointer, queue),
                Some(false) if focused => {
                    self.focused = None;
                    self.deliver(i, Some(&Event::Focus { gained: false, reason: FocusReason::Pointer }), None, queue);
                }
                _ => {}
            }
        }

        fn move_focus(&mut self, to: usize, reason: FocusReason, queue: &mut VecDeque<(usize, PeerRequest)>) {
            if self.focused == Some(to) {
                return;
            }
            if let Some(old) = self.focused.take() {
                self.deliver(old, Some(&Event::Focus { gained: false, reason }), None, queue);
            }
            self.focused = Some(to);
            self.deliver(to, Some(&Event::Focus { gained: true, reason }), None, queue);
        }

        fn drain(&mut self, mut queue: VecDeque<(usize, PeerRequest)>) {
            while let Some((from, req)) = queue.pop_front() {
                let members = self.members(from);
                match req.to {
                    PeerTarget::Others => {
                        for m in members.into_iter().filter(|&m| m != from) {
                            self.deliver(m, None, Some(&req.msg), &mut queue);
                        }
                    }
                    PeerTarget::Rove(r) => {
                        let at = members.iter().position(|&m| m == from).unwrap_or(0);
                        let to = members[r.target(at, members.len())];
                        if to != from {
                            self.move_focus(to, FocusReason::Keyboard, &mut queue);
                            self.deliver(to, None, Some(&req.msg), &mut queue);
                        }
                    }
                }
            }
        }

        /// Sends `ev` to node `i` and applies what follows (focus, peer requests).
        pub fn send(&mut self, i: usize, ev: Event) {
            let mut queue = VecDeque::new();
            self.deliver(i, Some(&ev), None, &mut queue);
            self.drain(queue);
        }

        /// Gives node `i` keyboard focus (Tab).
        pub fn focus(&mut self, i: usize) {
            let mut queue = VecDeque::new();
            self.move_focus(i, FocusReason::Keyboard, &mut queue);
            self.drain(queue);
        }

        fn pointer(phase: PointerPhase, p: Point, buttons: u32) -> Event {
            let button = matches!(phase, PointerPhase::Down | PointerPhase::Up).then_some(MouseButton::Primary);
            Event::Pointer(PointerEvent { phase, pointer: 0, kind: PointerType::Mouse, button, buttons, pos: p, modifiers: Modifiers::default(), pressure: 0.0, tilt: Vec2::ZERO, history: SmallVec::new() })
        }

        /// Hover, press and release node `i` at its centre, then leave it.
        pub fn click(&mut self, i: usize) {
            let c = Point::new(self.size.w / 2.0, self.size.h / 2.0);
            self.send(i, Event::Hover(true));
            self.send(i, Self::pointer(PointerPhase::Down, c, 1));
            self.now = self.now + Span::from_millis(50);
            self.send(i, Self::pointer(PointerPhase::Up, c, 0));
            self.send(i, Event::Hover(false));
            self.now = self.now + Span::from_millis(500);
        }

        /// A key press and release on the focused node.
        pub fn key(&mut self, key: Key) {
            let i = self.focused.expect("a focused node");
            let text = (key == Key::Space).then(|| " ".to_owned());
            let ev = |state| Event::Key(KeyEvent { key: Some(key), physical: 0, text: text.clone(), state, repeat: false, location: KeyLocation::Standard, modifiers: Modifiers::default() });
            self.send(i, ev(KeyState::Pressed));
            if let Some(i) = self.focused {
                self.send(i, ev(KeyState::Released));
            }
        }

        pub fn a11y_click(&mut self, i: usize) {
            self.send(i, Event::A11y(accesskit::ActionRequest { action: accesskit::Action::Click, target_tree: accesskit::TreeId::ROOT, target_node: accesskit::NodeId(i as u64 + 1), data: None }));
        }
    }
}
