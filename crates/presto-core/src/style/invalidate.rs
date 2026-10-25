//! Which nodes a change restyles, from the stylist's dependency maps (svelters-style's
//! [`Invalidation`](svelters_style::Invalidation)): a prop write restyles only when some attribute
//! selector names it, a state change only as far as the selectors testing that state reach, a class
//! or id change as far as the selectors naming it reach.

use crate::document::Document;
use crate::style::StyleInput;
use crate::tree::Dirty;
use svelters_style::{Reach, StateMask};

impl Document {
    /// Marks the nodes `reach` names from node `i` for restyle.
    pub(crate) fn restyle_reach(&mut self, i: usize, reach: Reach) {
        if reach.is_empty() {
            return;
        }
        if reach.contains(Reach::TREE) {
            let mut r = i;
            while let Some(p) = self.core[r].parent {
                r = self.ix(p);
            }
            self.core[r].dirty.insert(Dirty::STYLE | Dirty::STYLE_SUBTREE);
            return;
        }
        if reach.contains(Reach::SELF) {
            self.core[i].dirty.insert(Dirty::STYLE);
            self.mark_up(i, Dirty::DESC_STYLE);
        }
        if reach.contains(Reach::DESCENDANTS) {
            self.core[i].dirty.insert(Dirty::STYLE | Dirty::STYLE_SUBTREE);
            self.mark_up(i, Dirty::DESC_STYLE);
        }
        if reach.contains(Reach::LATER_SIBLINGS)
            && let Some(p) = self.core[i].parent
        {
            let pi = self.ix(p);
            let me = self.slots.id_at(i).expect("live");
            let later: Vec<usize> = self.children[pi].iter().skip_while(|&&c| c != me).skip(1).map(|&c| self.ix(c)).collect();
            for s in later {
                self.core[s].dirty.insert(Dirty::STYLE | Dirty::STYLE_SUBTREE);
            }
            self.core[pi].dirty.insert(Dirty::DESC_STYLE);
            self.mark_up(pi, Dirty::DESC_STYLE);
        }
        if reach.contains(Reach::ANCESTORS) {
            let mut a = i;
            while let Some(p) = self.core[a].parent {
                let pi = self.ix(p);
                let me = self.slots.id_at(a).expect("live");
                let earlier: Vec<usize> = self.children[pi].iter().take_while(|&&c| c != me).map(|&c| self.ix(c)).collect();
                for s in earlier {
                    self.core[s].dirty.insert(Dirty::STYLE);
                }
                self.core[pi].dirty.insert(Dirty::STYLE | Dirty::DESC_STYLE);
                a = pi;
            }
        }
    }

    /// A prop named `name` of node `i` changed value.
    pub(crate) fn restyle_attr(&mut self, i: usize, name: &str) {
        let reach = self.style.stylist.invalidation().attr_changed(name);
        self.restyle_reach(i, reach);
    }

    /// Node `i`'s state went from `before` to `after`.
    pub(crate) fn restyle_state(&mut self, i: usize, before: StateMask, after: StateMask) {
        let reach = self.style.stylist.invalidation().state_changed(before, after);
        self.restyle_reach(i, reach);
    }

    /// Node `i`'s style inputs changed from `old` to `new`.
    pub(crate) fn restyle_inputs(&mut self, i: usize, old: Option<&StyleInput>, new: &StyleInput) {
        let inv = self.style.stylist.invalidation();
        let mut reach = Reach::NONE;
        let old_classes: &[std::sync::Arc<str>] = old.map_or(&[], |o| &o.classes);
        for c in old_classes.iter().filter(|c| !new.classes.contains(c)).chain(new.classes.iter().filter(|c| !old_classes.contains(c))) {
            reach |= inv.class_changed(c);
        }
        let old_id = old.and_then(|o| o.id.as_deref());
        if old_id != new.id.as_deref() {
            for id in [old_id, new.id.as_deref()].into_iter().flatten() {
                reach |= inv.id_changed(id);
            }
        }
        if old.and_then(|o| o.inline.as_deref()) != new.inline.as_deref() {
            reach |= Reach::SELF;
        }
        self.restyle_reach(i, reach);
    }

    /// A child was inserted under or removed from node `pi` (the inserted subtree is styled anyway).
    pub(crate) fn restyle_structure(&mut self, pi: usize) {
        let reach = self.style.stylist.invalidation().structure_changed();
        self.restyle_reach(pi, reach);
    }
}
