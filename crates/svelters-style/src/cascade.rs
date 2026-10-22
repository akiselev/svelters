//! [`Stylist`]: the registered sheets of one backend, the rule index, matching and the cascade
//! (CSS.md §2.4).
//!
//! Cascade order, lowest first (CSS Cascade 5 §6, with the theme as the user origin):
//!
//! 1. origin and importance: theme < author rules < inline, then author `!important` < inline
//!    `!important` < theme `!important`;
//! 2. `@layer`: for normal declarations earlier layers lose and unlayered rules win; for
//!    `!important` the order reverses;
//! 3. specificity; 4. source order (sheets in registration order).

use crate::atom::Atom;
use crate::computed::{ComputeContext, ComputedStyle, compute};
use crate::diag::{Diagnostic, Severity};
use crate::element::{Element, Node};
use crate::invalidation::Invalidation;
use crate::media::MediaEnv;
use crate::selector::PseudoElement;
use crate::sheet::{DeclBlock, Declaration, Rule, Stylesheet};
use crate::table::{Table, ValueType};
use crate::values::{ValueCx, parse_value};
use selectors::context::{MatchingContext, MatchingForInvalidation, MatchingMode, NeedsSelectorFlags, QuirksMode, SelectorCaches};
use selectors::parser::Component;
use std::collections::HashMap;

/// Where a sheet comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Origin {
    /// Theme and token sheets (below every component sheet).
    Theme,
    /// Component sheets.
    Author,
}

/// A registered sheet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SheetId(u32);

/// A rule of the stylist, by index (stable until a sheet is removed).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RuleId(pub u32);

/// One rule that matched an element.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchedRule {
    pub rule: RuleId,
    /// The highest specificity among the rule's selectors that matched.
    pub specificity: u32,
}

struct Entry {
    sheet: SheetId,
    origin: Origin,
    rule: Rule,
    /// Global source order.
    order: u32,
    layer_rank: u32,
    media_ok: bool,
}

/// Caches reused across the elements of one restyle pass (`:nth-child` and `:has`). Clear them, or
/// make a new one, after the tree changes.
#[derive(Default)]
pub struct MatchCaches(SelectorCaches);

impl MatchCaches {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn clear(&mut self) {
        self.0 = SelectorCaches::default();
    }
}

/// The sheets and cascade of one backend's table.
pub struct Stylist {
    table: &'static Table,
    entries: Vec<Entry>,
    next_sheet: u32,
    next_order: u32,
    /// Layer names in order of first appearance across the sheets.
    layers: Vec<Atom>,
    index: RuleIndex,
    media: MediaEnv,
    invalidation: Invalidation,
}

#[derive(Default)]
struct RuleIndex {
    ids: HashMap<Atom, Vec<(u32, u16)>>,
    classes: HashMap<Atom, Vec<(u32, u16)>>,
    tags: HashMap<Atom, Vec<(u32, u16)>>,
    universal: Vec<(u32, u16)>,
    /// Selectors ending in a pseudo-element (`x::part(thumb)`), matched only by
    /// [`Stylist::compute_pseudo`]; never candidates for the element itself.
    pseudo: Vec<(u32, u16)>,
}

impl Stylist {
    pub fn new(table: &'static Table) -> Stylist {
        Stylist {
            table,
            entries: Vec::new(),
            next_sheet: 0,
            next_order: 0,
            layers: Vec::new(),
            index: RuleIndex::default(),
            media: MediaEnv::default(),
            invalidation: Invalidation::default(),
        }
    }

    pub fn table(&self) -> &'static Table {
        self.table
    }

    pub fn media_env(&self) -> &MediaEnv {
        &self.media
    }

    /// The dependency maps of every registered selector.
    pub fn invalidation(&self) -> &Invalidation {
        &self.invalidation
    }

    pub fn rule(&self, id: RuleId) -> &Rule {
        &self.entries[id.0 as usize].rule
    }

    pub fn rule_count(&self) -> usize {
        self.entries.len()
    }

    /// Registers a sheet after the ones already registered. Returns its id and the problems the
    /// backend's table finds in it (unknown properties, bad values); parse diagnostics stay on the
    /// [`Stylesheet`].
    pub fn add_sheet(&mut self, sheet: &Stylesheet, origin: Origin) -> (SheetId, Vec<Diagnostic>) {
        let id = SheetId(self.next_sheet);
        self.next_sheet += 1;
        let mut diags = Vec::new();
        for l in &sheet.layers {
            if !self.layers.contains(l) {
                self.layers.push(l.clone());
            }
        }
        for rule in &sheet.rules {
            for d in &rule.block.decls {
                if let Some(diag) = check_decl(self.table, d, false) {
                    diags.push(diag);
                }
            }
            let order = self.next_order;
            self.next_order += 1;
            self.entries.push(Entry { sheet: id, origin, rule: rule.clone(), order, layer_rank: 0, media_ok: true });
        }
        self.rebuild();
        (id, diags)
    }

    /// Unregisters a sheet. Rule ids after it shift.
    pub fn remove_sheet(&mut self, id: SheetId) {
        self.entries.retain(|e| e.sheet != id);
        self.rebuild();
    }

    /// Checks an inline block against the table, as `add_sheet` does for rules.
    pub fn check_inline(&self, block: &DeclBlock) -> Vec<Diagnostic> {
        block.decls.iter().filter_map(|d| check_decl(self.table, d, true)).collect()
    }

    /// Sets the media environment. Returns the rules whose `@media` result flipped: the nodes
    /// they match (now or before) need a restyle.
    pub fn set_media(&mut self, env: MediaEnv) -> Vec<RuleId> {
        self.media = env;
        let mut flipped = Vec::new();
        for (i, e) in self.entries.iter_mut().enumerate() {
            let ok = e.rule.media.iter().all(|m| m.matches(&env));
            if ok != e.media_ok {
                e.media_ok = ok;
                flipped.push(RuleId(i as u32));
            }
        }
        flipped
    }

    fn rebuild(&mut self) {
        let ranks = layer_ranks(&self.layers);
        self.index = RuleIndex::default();
        self.invalidation = Invalidation::default();
        let env = self.media;
        for (i, e) in self.entries.iter_mut().enumerate() {
            e.layer_rank = match &e.rule.layer {
                Some(l) => ranks.get(l).copied().unwrap_or(u32::MAX),
                None => u32::MAX,
            };
            e.media_ok = e.rule.media.iter().all(|m| m.matches(&env));
            for (s, sel) in e.rule.selectors.slice().iter().enumerate() {
                self.index.insert(i as u32, s as u16, sel);
                self.invalidation.add_selector(sel);
            }
        }
    }

    /// The rules that match `el`, in cascade order of their normal declarations (origin, layer,
    /// specificity, source order).
    pub fn matched_rules<E: Element>(&self, el: E, caches: &mut MatchCaches) -> Vec<MatchedRule> {
        let mut candidates: Vec<(u32, u16)> = Vec::new();
        if let Some(id) = el.id()
            && let Some(v) = self.index.ids.get(id)
        {
            candidates.extend_from_slice(v);
        }
        el.for_each_class(&mut |c| {
            if let Some(v) = self.index.classes.get(c) {
                candidates.extend_from_slice(v);
            }
        });
        if let Some(v) = self.index.tags.get(el.tag()) {
            candidates.extend_from_slice(v);
        }
        if let Some(v) = el.style_as().and_then(|t| self.index.tags.get(t)) {
            candidates.extend_from_slice(v);
        }
        candidates.extend_from_slice(&self.index.universal);
        candidates.sort_unstable();
        candidates.dedup();
        self.match_candidates(el, candidates, None, caches)
    }

    /// Whether any registered rule has a selector ending in a pseudo-element that `pseudo` matches
    /// (a widget skips computing a part's style when none does).
    pub fn has_pseudo_rules(&self, pseudo: &PseudoElement) -> bool {
        self.index.pseudo.iter().any(|&(r, s)| self.entries[r as usize].rule.selectors.slice()[s as usize].pseudo_element().is_some_and(|p| pseudo.matched_by(p)))
    }

    fn match_candidates<E: Element>(&self, el: E, candidates: Vec<(u32, u16)>, pseudo: Option<&PseudoElement>, caches: &mut MatchCaches) -> Vec<MatchedRule> {
        let wanted = |p: &PseudoElement| pseudo.is_some_and(|mine| mine.matched_by(p));
        let mut cx = MatchingContext::new(
            if pseudo.is_some() { MatchingMode::ForStatelessPseudoElement } else { MatchingMode::Normal },
            None,
            &mut caches.0,
            QuirksMode::NoQuirks,
            NeedsSelectorFlags::No,
            MatchingForInvalidation::No,
        );
        if pseudo.is_some() {
            cx.pseudo_element_matching_fn = Some(&wanted);
        }
        let node = Node(el);
        let mut out: Vec<MatchedRule> = Vec::new();
        for (r, s) in candidates {
            let e = &self.entries[r as usize];
            if !e.media_ok {
                continue;
            }
            let sel = &e.rule.selectors.slice()[s as usize];
            if let Some(mine) = pseudo
                && !sel.pseudo_element().is_some_and(|p| mine.matched_by(p))
            {
                continue;
            }
            if selectors::matching::matches_selector(sel, 0, None, &node, &mut cx) {
                let spec = sel.specificity();
                match out.last_mut() {
                    Some(m) if m.rule.0 == r => m.specificity = m.specificity.max(spec),
                    _ => out.push(MatchedRule { rule: RuleId(r), specificity: spec }),
                }
            }
        }
        out.sort_by_key(|m| {
            let e = &self.entries[m.rule.0 as usize];
            (e.origin, e.layer_rank, m.specificity, e.order)
        });
        out
    }

    /// The declarations that apply to `el`, lowest precedence first: the cascade's input. The last
    /// declaration of a property wins.
    pub fn cascaded<'s, E: Element>(&'s self, el: E, inline: Option<&'s DeclBlock>, caches: &mut MatchCaches) -> Vec<&'s Declaration> {
        let matched = self.matched_rules(el, caches);
        self.order_decls(&matched, inline)
    }

    /// The rules whose selector ends in `pseudo` and whose host part matches `el`, in cascade
    /// order.
    pub fn matched_pseudo_rules<E: Element>(&self, el: E, pseudo: &PseudoElement, caches: &mut MatchCaches) -> Vec<MatchedRule> {
        let candidates = self.index.pseudo.clone();
        self.match_candidates(el, candidates, Some(pseudo), caches)
    }

    /// Computes the style of `el`'s pseudo-element `pseudo` (`::part(name)`, `::placeholder`,
    /// `::selection`; ARCH-WIDGETS W3): the rules ending in that pseudo-element whose host part
    /// matches `el`, inheriting from `host`, the element's own computed style. With no such rule
    /// the result is `host`'s inherited properties only (as a child with no rules).
    pub fn compute_pseudo<E: Element>(&self, el: E, pseudo: &PseudoElement, host: &ComputedStyle, cx: &ComputeContext, caches: &mut MatchCaches) -> ComputedStyle {
        let matched = self.matched_pseudo_rules(el, pseudo, caches);
        let decls = self.order_decls(&matched, None);
        compute(self.table, &decls, Some(host), cx)
    }

    fn order_decls<'s>(&'s self, matched: &[MatchedRule], inline: Option<&'s DeclBlock>) -> Vec<&'s Declaration> {
        // (origin and importance, layer, specificity, source order)
        type Key = (u8, u32, u32, u32);
        let mut keyed: Vec<(Key, &Declaration)> = Vec::new();
        for m in matched {
            let e = &self.entries[m.rule.0 as usize];
            for d in &e.rule.block.decls {
                if self.table.get(&d.name).is_some_and(|p| p.inline_only) {
                    continue;
                }
                let rank = match (d.important, e.origin) {
                    (false, Origin::Theme) => 0,
                    (false, Origin::Author) => 1,
                    (true, Origin::Author) => 3,
                    (true, Origin::Theme) => 5,
                };
                let layer = if d.important { u32::MAX - e.layer_rank } else { e.layer_rank };
                keyed.push(((rank, layer, m.specificity, e.order), d));
            }
        }
        if let Some(block) = inline {
            for (i, d) in block.decls.iter().enumerate() {
                keyed.push(((if d.important { 4 } else { 2 }, 0, 0, i as u32), d));
            }
        }
        keyed.sort_by_key(|(k, _)| *k);
        keyed.into_iter().map(|(_, d)| d).collect()
    }

    /// Computes `el`'s style from the matched rules, its inline block and its parent's style.
    pub fn compute<E: Element>(
        &self,
        el: E,
        parent: Option<&ComputedStyle>,
        inline: Option<&DeclBlock>,
        cx: &ComputeContext,
        caches: &mut MatchCaches,
    ) -> ComputedStyle {
        let decls = self.cascaded(el, inline, caches);
        compute(self.table, &decls, parent, cx)
    }
}

impl RuleIndex {
    /// Buckets a selector by its subject compound's id, else a class, else its tag (Stylo's
    /// `SelectorMap`), so an element only tries rules that could match it.
    fn insert(&mut self, rule: u32, sel: u16, selector: &crate::selector::Selector) {
        if selector.has_pseudo_element() {
            self.pseudo.push((rule, sel));
            return;
        }
        let (mut id, mut class, mut tag) = (None, None, None);
        for c in selector.iter() {
            match c {
                Component::ID(a) => id = Some(a.clone()),
                Component::Class(a) => class = class.or(Some(a.clone())),
                Component::LocalName(l) => tag = Some(l.name.clone()),
                _ => {}
            }
        }
        let entry = (rule, sel);
        match (id, class, tag) {
            (Some(a), _, _) => self.ids.entry(a).or_default().push(entry),
            (_, Some(a), _) => self.classes.entry(a).or_default().push(entry),
            (_, _, Some(a)) => self.tags.entry(a).or_default().push(entry),
            _ => self.universal.push(entry),
        }
    }
}

/// Ranks layers so that, compared as numbers, earlier layers are lower, a layer's sublayers come
/// before its own rules, and unlayered rules (`u32::MAX`) are highest.
fn layer_ranks(layers: &[Atom]) -> HashMap<Atom, u32> {
    let pos = |name: &str| layers.iter().position(|l| **l == *name).unwrap_or(usize::MAX);
    let key = |name: &Atom| -> Vec<usize> {
        let mut k = Vec::new();
        let mut prefix = String::new();
        for part in name.split('.') {
            if !prefix.is_empty() {
                prefix.push('.');
            }
            prefix.push_str(part);
            k.push(pos(&prefix));
        }
        k.push(usize::MAX); // the layer's own rules, after its sublayers
        k
    };
    let mut sorted: Vec<&Atom> = layers.iter().collect();
    sorted.sort_by_key(|l| key(l));
    sorted.into_iter().enumerate().map(|(i, l)| (l.clone(), i as u32)).collect()
}

const CSS_WIDE: &[&str] = &["inherit", "initial", "unset", "revert", "revert-layer"];

/// The table's verdict on one declaration: unknown property (warning: another backend may have
/// it), a bad value (error), or a rule setting an inline-only property.
fn check_decl(table: &'static Table, d: &Declaration, inline: bool) -> Option<Diagnostic> {
    if d.is_custom() {
        return None;
    }
    let loc = cssparser::SourceLocation { line: d.line.saturating_sub(1), column: d.column };
    if crate::shorthand::longhands_in(table, &d.name).is_some() {
        if d.has_var {
            return None;
        }
        let cx = ValueCx { table, em: 16.0, rem: 16.0, current_color: Some(crate::Color::TRANSPARENT), parent_weight: 400, percent_base: None };
        let longs = match crate::shorthand::expand(&d.name, &d.value) {
            Ok(l) => l,
            Err(e) => return Some(Diagnostic::error(loc, e)),
        };
        for (l, text) in longs {
            let Some(prop) = table.get(l) else { continue };
            if prop.ty == ValueType::Raw || CSS_WIDE.contains(&text.to_ascii_lowercase().as_str()) {
                continue;
            }
            if let Err(e) = parse_value(prop, &text, &cx) {
                return Some(Diagnostic::error(loc, format!("{}: {e}", d.name)));
            }
        }
        return None;
    }
    let Some(prop) = table.get(&d.name) else {
        let article = if table.toolkit.starts_with(['a', 'e', 'i', 'o', 'u']) { "an" } else { "a" };
        let mut msg = format!("`{}` is not {article} {} style property", d.name, table.toolkit);
        let elsewhere: Vec<&str> = crate::tables::ALL.iter().filter(|t| t.get(&d.name).is_some()).map(|t| t.toolkit).collect();
        if !elsewhere.is_empty() {
            msg.push_str(&format!(" (it is one on {})", elsewhere.join(", ")));
        }
        return Some(Diagnostic::warning(loc, msg));
    };
    if prop.inline_only && !inline {
        return Some(Diagnostic::error(loc, format!("`{}` can only be set in style= on {}", d.name, table.toolkit)));
    }
    if d.has_var || prop.ty == ValueType::Raw || CSS_WIDE.contains(&d.value.to_ascii_lowercase().as_str()) {
        return None;
    }
    let cx = ValueCx { table, em: 16.0, rem: 16.0, current_color: Some(crate::Color::TRANSPARENT), parent_weight: 400, percent_base: None };
    match parse_value(prop, &d.value, &cx) {
        Ok(_) => None,
        Err(e) => Some(Diagnostic { severity: Severity::Error, ..Diagnostic::error(loc, e) }),
    }
}

/// Whether `el` matches any selector of `list` (queries, tests and MCP: CSS.md §2.9).
pub fn matches<E: Element>(list: &crate::selector::SelectorList, el: E, caches: &mut MatchCaches) -> bool {
    let mut cx = MatchingContext::new(
        MatchingMode::Normal,
        None,
        &mut caches.0,
        QuirksMode::NoQuirks,
        NeedsSelectorFlags::No,
        MatchingForInvalidation::No,
    );
    selectors::matching::matches_selector_list(list, &Node(el), &mut cx)
}

/// The nodes of `root`'s subtree (`root` included) that match `list`, in tree order.
pub fn select<E: Element>(list: &crate::selector::SelectorList, root: E, caches: &mut MatchCaches) -> Vec<E> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        if matches(list, n, caches) {
            out.push(n);
        }
        let mut kids = Vec::new();
        let mut c = n.first_child();
        while let Some(k) = c {
            kids.push(k);
            c = k.next_sibling();
        }
        stack.extend(kids.into_iter().rev());
    }
    out
}
