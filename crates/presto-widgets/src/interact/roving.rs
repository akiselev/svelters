//! `Roving` (ARCH-WIDGETS §1.2 catalogue, WIDGET-PARITY §2.3): the arrow keys move within a group
//! that is one Tab stop (radio buttons, `toggle-button radio group`). While the host is focused,
//! Up/Left go to the previous member, Down/Right to the next (wrapping, as Qt and GTK do), and
//! Home/End to the first and last. The unit only reports the move; the kind finds the members,
//! moves focus there and checks it.

use super::{InputCx, Intents, Mods, StateBits, Unit};
use presto_base::key::Key;

/// What a kind configures on its [`Roving`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RovingParams {
    /// Whether the group roves at all (a lone member, or a disabled host, does not).
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
        for (k, r) in KEYS {
            if cx.input.key_pressed(k) && cx.input.modifiers.is_none() {
                out.push(r);
            }
        }
    }

    fn claims(&self, p: &RovingParams, key: Key, m: Mods) -> bool {
        p.on && m.is_none() && KEYS.iter().any(|(k, _)| *k == key)
    }
}

#[cfg(test)]
mod tests {
    use super::Rove;

    #[test]
    fn rove_wraps() {
        assert_eq!(Rove::By(1).target(2, 3), 0);
        assert_eq!(Rove::By(-1).target(0, 3), 2);
        assert_eq!((Rove::First.target(1, 3), Rove::Last.target(1, 3)), (0, 2));
    }
}
