//! DZ0/DZ2b for skelter: what a designer or inspector reads and forces on the document. The rules
//! that match a node (with their sheet's name and, in dev builds, where each rule and declaration is
//! in its component's `<style>`), state bits forced for matching (`:hover` shown without a pointer),
//! and every node drawn under a point. The same reads as svelters-egui's scene (`matched_rules`,
//! `force_state`, `hit_stack`); svelters-skelter's `designer` module puts them behind svelters-egui's
//! shapes. For tools, not per frame.

use super::SheetInfo;
use super::cascade::El;
use crate::document::Document;
use crate::tree::Dirty;
use skelter_base::geom::Point;
use skelter_base::ids::{NodeId, WindowId};
use std::sync::Arc;
use svelters_style::{ComputedStyle, DeclBlock, DeclInfo, Explanation, MatchCaches, MatchedRuleInfo, SheetId, SheetSource, StateMask, StyleLookup};

impl Document {
    /// Names sheet `id` (`MatchedRuleInfo::sheet`) and gives it its `<style>` source (dev builds:
    /// the component file and the compiler's map), for [`Document::matched_rules`]. Removing the
    /// sheet drops both.
    pub fn set_sheet_info(&mut self, id: SheetId, name: Option<String>, source: Option<SheetSource>) {
        self.style.sheet_info.insert(id, SheetInfo { name, source: source.map(Arc::new) });
    }

    /// The rules that match `node` now, in cascade order (normal declarations: origin, layer,
    /// specificity, source order), each with its declarations, which of them win the cascade, and
    /// (dev builds, [`Document::set_sheet_info`]) where each is in its component's `<style>`.
    /// Forced states count ([`Document::force_state`]). Empty for a node that is gone.
    pub fn matched_rules(&self, node: NodeId) -> Vec<MatchedRuleInfo> {
        let Some(i) = self.idx(node) else { return Vec::new() };
        let st = &self.style;
        let inline = st.nodes[i].inline.clone();
        let mut caches = MatchCaches::new();
        let winners = st.stylist.winners(El { doc: self, i }, inline.as_deref(), &mut caches);
        caches.clear();
        st.stylist
            .matched_rules(El { doc: self, i }, &mut caches)
            .into_iter()
            .map(|m| {
                let rule = st.stylist.rule(m.rule);
                let info = st.stylist.rule_sheet(m.rule).and_then(|id| st.sheet_info.get(&id));
                let at = |line, column| info.and_then(|x| x.source.as_ref()).and_then(|x| x.source_at(line, column));
                MatchedRuleInfo {
                    sheet: info.and_then(|x| x.name.clone()),
                    selectors: rule.selector_text(),
                    layer: rule.layer_name(),
                    specificity: m.specificity,
                    line: rule.line,
                    column: rule.column,
                    source: at(rule.line, rule.column),
                    decls: rule
                        .block
                        .decls
                        .iter()
                        .map(|d| DeclInfo {
                            name: d.name.to_string(),
                            value: d.value.to_string(),
                            important: d.important,
                            source: at(d.line, d.column),
                            winner: winners.values().any(|w| std::ptr::eq(*w, d)),
                        })
                        .collect(),
                }
            })
            .collect()
    }

    /// SV88: why `node`'s `property` (a longhand such as `background-color`, or a custom property
    /// `--name`) has its value: the winning declaration with its origin (theme, author, user or
    /// inline), rule, sheet name and (dev builds) `<style>` source; the declarations it beat, highest
    /// first, each with the cascade step it lost on; the value after `var()` substitution; the
    /// ancestor it inherits from when the node declares nothing; and each custom property it uses,
    /// down to the declaration that sets it. Forced states count. `None` for a node that is gone.
    /// Matches on each call and keeps nothing: for tools, not per frame.
    pub fn explain_style(&self, node: NodeId, property: &str) -> Option<Explanation<NodeId>> {
        let i = self.idx(node)?;
        let st = &self.style;
        let mut out = st.stylist.explain(El { doc: self, i }, property, &Lookup(self)).map(&mut |e: El<'_>| e.id());
        out.annotate(&mut |d| {
            let Some(info) = d.sheet.and_then(|id| st.sheet_info.get(&id)) else { return };
            d.sheet_name = info.name.clone();
            if let Some(src) = &info.source {
                d.source = src.source_at(d.line, d.column);
            }
        });
        Some(out)
    }

    /// Forces state bits on `node` for selector matching and painting until replaced;
    /// `StateMask::NONE` clears. A designer forces `:hover` or `:focus` to show and edit that state's
    /// rules. The bits are ORed over the node's own ([`Document::state`] doesn't show them) and never
    /// reach its events or accessibility; freeing the node drops them. False for a node that is gone.
    pub fn force_state(&mut self, node: NodeId, bits: StateMask) -> bool {
        let Some(i) = self.idx(node) else { return false };
        let old = self.style.nodes[i].forced;
        if old == bits {
            return true;
        }
        let own = self.state[i];
        self.style.nodes[i].forced = bits;
        self.stamp();
        self.restyle_state(i, StateMask(own.0 | old.0), StateMask(own.0 | bits.0));
        // through `mark`, so the ancestors' DESC_RECORD leads the frame's walk to the node
        self.mark(node, Dirty::PAINT);
        true
    }

    /// The bits forced on `node` ([`Document::force_state`]); `NONE` when none.
    pub fn forced_state(&self, node: NodeId) -> StateMask {
        self.idx(node).map_or(StateMask::NONE, |i| self.style.nodes[i].forced)
    }

    /// Every node `window`'s last frame drew under `p` (window points), topmost first: within the
    /// paint order, a node before the boxes it is drawn in. What a designer's pick cycles through
    /// (a click selects the first, a modifier click walks out). From the last frame's hit list, so
    /// `pointer-events: none` nodes are left out. Each node once.
    pub fn hit_stack(&self, window: WindowId, p: Point) -> Vec<NodeId> {
        let Some(list) = self.hit_list(window) else { return Vec::new() };
        let mut out: Vec<NodeId> = Vec::new();
        for e in list.entries.iter().rev() {
            if e.world.contains(p) && e.clip.is_none_or(|c| c.contains(p)) && self.idx(e.node).is_some() && !out.contains(&e.node) {
                out.push(e.node);
            }
        }
        out
    }
}

/// The document's per-node inputs for [`svelters_style::Stylist::explain`].
struct Lookup<'a>(&'a Document);

impl<'a> StyleLookup<El<'a>> for Lookup<'a> {
    fn inline(&self, el: El<'a>) -> Option<&DeclBlock> {
        self.0.style.nodes[el.i].inline.as_deref()
    }
    fn computed(&self, el: El<'a>) -> Option<&ComputedStyle> {
        self.0.style.nodes[el.i].css.as_deref()
    }
}
