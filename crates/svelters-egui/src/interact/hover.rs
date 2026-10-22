//! `Hoverable`: the pointer is over the host (`:hover`). Runs on disabled hosts too (R9).

use super::{InputCx, Intents, Unit};
use crate::provider::StateBits;

/// What [`Hoverable`] reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hover {
    Enter,
    Leave,
}

/// Owns `hovered`. Same-frame: the bit is this pass's hover, so `:hover` never lags (CS5e).
#[derive(Debug, Default)]
pub struct Hoverable {
    hovered: bool,
}

impl Hoverable {
    pub fn hovered(&self) -> bool {
        self.hovered
    }
}

impl Unit for Hoverable {
    type Params = ();
    type Intent = Hover;
    const OWNS: StateBits = StateBits::HOVERED;

    fn input(&mut self, _p: &(), cx: &mut InputCx<'_>, out: &mut Intents<Hover>) {
        // a disabled widget's response is not `hovered`; R9 keeps it hoverable (tooltips, reasons)
        let h = cx.resp.hovered || (!cx.enabled && cx.resp.contains_pointer);
        if h != self.hovered {
            self.hovered = h;
            out.push(if h { Hover::Enter } else { Hover::Leave });
        }
        cx.set(StateBits::HOVERED, h);
    }
}
