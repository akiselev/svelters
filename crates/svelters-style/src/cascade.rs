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
use crate::selector::{PseudoClass, PseudoElement, Selector};
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

/// The sheet id of generated utility rules (U1 E2): never registered or removed as a sheet.
pub(crate) const UTILITY_SHEET: SheetId = SheetId(u32::MAX);

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

pub(crate) struct Entry {
    pub(crate) sheet: SheetId,
    pub(crate) origin: Origin,
    pub(crate) rule: Rule,
    /// Global source order: the registration sequence for sheet rules; for generated utilities
    /// (U1 E2) a canonical key above every sheet rule ([`crate::utility::canonical_order`]).
    pub(crate) order: u64,
    /// A generated utility's class: the last tie-break of the canonical order (U1 E2).
    pub(crate) utility: Option<Atom>,
    /// A dropped utility rule (U1 E2 GC): never matched, compacted by the next rebuild.
    pub(crate) dead: bool,
    pub(crate) layer_rank: u32,
    pub(crate) media_ok: bool,
}

impl Entry {
    /// The cascade's source-order key after specificity (U1 E2: utilities in canonical order).
    fn order_key(&self) -> (u64, &str) {
        (self.order, self.utility.as_deref().unwrap_or(""))
    }
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
    pub(crate) table: &'static Table,
    pub(crate) entries: Vec<Entry>,
    next_sheet: u32,
    next_order: u32,
    /// Layer names in order of first appearance across the sheets.
    pub(crate) layers: Vec<Atom>,
    /// Each layer's rank, as of the last rebuild.
    pub(crate) layer_rank: HashMap<Atom, u32>,
    index: RuleIndex,
    pub(crate) media: MediaEnv,
    invalidation: Invalidation,
    /// Full rebuilds of the index so far (U1: utilities are appended without one).
    pub(crate) rebuilds: u64,
    /// U1 E2: the generated utility rules' bookkeeping.
    pub(crate) utilities: crate::utility::Layer,
}

/// Rule selectors bucketed by one compound's id, else a class, else its tag (Stylo's
/// `SelectorMap`), so an element only tries the rules that could match it. Entries are (rule,
/// selector index).
#[derive(Default)]
struct Buckets {
    ids: HashMap<Atom, Vec<(u32, u16)>>,
    classes: HashMap<Atom, Vec<(u32, u16)>>,
    tags: HashMap<Atom, Vec<(u32, u16)>>,
    universal: Vec<(u32, u16)>,
}

/// U2 E3: the rules of one part name: bucketed by their host compound, and the part-name sets
/// they require (a selector's names must all be among the element's part's: P10).
#[derive(Default)]
struct PartBucket {
    hosts: Buckets,
    /// The distinct name sets of the bucket's selectors (usually just `[name]`).
    name_sets: Vec<Box<[Atom]>>,
}

#[derive(Default)]
struct RuleIndex {
    /// Selectors matched against the element itself, by their subject compound.
    main: Buckets,
    /// U2 E3: selectors ending in `::part(…)`, matched only by [`Stylist::compute_pseudo`] and never
    /// candidates for the element itself; by the selector's first part name, then by the host
    /// compound.
    parts: HashMap<Atom, PartBucket>,
    /// Selectors ending in `::placeholder` / `::selection`, by the host compound.
    placeholder: Buckets,
    selection: Buckets,
}

impl Stylist {
    pub fn new(table: &'static Table) -> Stylist {
        Stylist {
            table,
            entries: Vec::new(),
            next_sheet: 0,
            next_order: 0,
            layers: Vec::new(),
            layer_rank: HashMap::new(),
            index: RuleIndex::default(),
            media: MediaEnv::default(),
            invalidation: Invalidation::default(),
            rebuilds: 0,
            utilities: crate::utility::Layer::default(),
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

    /// DZ0: the sheet rule `id` came from (`None` for a generated utility rule).
    pub fn rule_sheet(&self, id: RuleId) -> Option<SheetId> {
        let s = self.entries[id.0 as usize].sheet;
        (s != UTILITY_SHEET).then_some(s)
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
            let order = self.next_order as u64;
            self.next_order += 1;
            self.entries.push(Entry { sheet: id, origin, rule: rule.clone(), order, utility: None, dead: false, layer_rank: 0, media_ok: true });
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

    /// Re-ranks the layers and rebuilds the rule index and the invalidation maps from every entry
    /// (dropped utility rules are compacted away first).
    pub(crate) fn rebuild(&mut self) {
        self.rebuilds += 1;
        self.entries.retain(|e| !e.dead);
        self.utilities.dead = 0;
        self.layer_rank = layer_ranks(&self.layers);
        self.index = RuleIndex::default();
        self.invalidation = Invalidation::default();
        let env = self.media;
        for (i, e) in self.entries.iter_mut().enumerate() {
            e.layer_rank = match &e.rule.layer {
                Some(l) => self.layer_rank.get(l).copied().unwrap_or(u32::MAX),
                None => u32::MAX,
            };
            e.media_ok = e.rule.media.iter().all(|m| m.matches(&env));
            for (s, sel) in e.rule.selectors.slice().iter().enumerate() {
                self.index.insert(i as u32, s as u16, sel);
                self.invalidation.add_selector(sel);
            }
        }
    }

    /// U1 E2: appends one rule without a rebuild: its layer must already be ranked (else the
    /// caller rebuilds), and it goes into the index and the invalidation maps as `rebuild` puts it.
    pub(crate) fn append_entry(&mut self, mut e: Entry) {
        e.layer_rank = match &e.rule.layer {
            Some(l) => self.layer_rank.get(l).copied().unwrap_or(u32::MAX),
            None => u32::MAX,
        };
        e.media_ok = e.rule.media.iter().all(|m| m.matches(&self.media));
        let i = self.entries.len() as u32;
        for (s, sel) in e.rule.selectors.slice().iter().enumerate() {
            self.index.insert(i, s as u16, sel);
            self.invalidation.add_selector(sel);
        }
        self.entries.push(e);
    }

    /// Full rebuilds of the rule index so far (each `add_sheet`/`remove_sheet` makes one; the
    /// utility layer appends without one). For tests and stats.
    pub fn rebuild_count(&self) -> u64 {
        self.rebuilds
    }

    /// The rules that match `el`, in cascade order of their normal declarations (origin, layer,
    /// specificity, source order).
    pub fn matched_rules<E: Element>(&self, el: E, caches: &mut MatchCaches) -> Vec<MatchedRule> {
        let mut candidates: Vec<(u32, u16)> = Vec::new();
        self.index.main.collect(el, &mut candidates);
        candidates.sort_unstable();
        candidates.dedup();
        self.match_candidates(el, candidates, None, caches)
    }

    /// Whether any registered rule has a selector ending in a pseudo-element that `pseudo` matches
    /// (a widget skips computing a part's style when none does).
    pub fn has_pseudo_rules(&self, pseudo: &PseudoElement) -> bool {
        match pseudo {
            PseudoElement::Part(mine) => mine.iter().any(|n| self.index.parts.get(n).is_some_and(|b| b.name_sets.iter().any(|want| want.iter().all(|w| mine.contains(w))))),
            PseudoElement::Placeholder => !self.index.placeholder.is_empty(),
            PseudoElement::Selection => !self.index.selection.is_empty(),
        }
    }

    /// The candidate selectors (rule, selector index) for `el`'s pseudo-element `pseudo`, sorted:
    /// the part index's buckets for each of its names and `el`'s id, classes and tag (U2 E3).
    fn pseudo_candidates_of<E: Element>(&self, el: E, pseudo: &PseudoElement) -> Vec<(u32, u16)> {
        let mut out = Vec::new();
        match pseudo {
            PseudoElement::Part(names) => {
                for n in names.iter() {
                    if let Some(b) = self.index.parts.get(n) {
                        b.hosts.collect(el, &mut out);
                    }
                }
            }
            PseudoElement::Placeholder => self.index.placeholder.collect(el, &mut out),
            PseudoElement::Selection => self.index.selection.collect(el, &mut out),
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    /// The rules a computation of `el`'s pseudo-element `pseudo` tries before matching (U2 E3:
    /// only the buckets of its part names and of `el`'s id, classes and tag). For tests and stats.
    pub fn pseudo_candidates<E: Element>(&self, el: E, pseudo: &PseudoElement) -> Vec<RuleId> {
        let mut v: Vec<RuleId> = self.pseudo_candidates_of(el, pseudo).into_iter().map(|(r, _)| RuleId(r)).collect();
        v.dedup();
        v
    }

    /// `pseudo`: the pseudo-element being computed and its logical position (P1).
    fn match_candidates<E: Element>(&self, el: E, candidates: Vec<(u32, u16)>, pseudo: Option<(&PseudoElement, Option<(usize, usize)>)>, caches: &mut MatchCaches) -> Vec<MatchedRule> {
        let mut cx = MatchingContext::new(MatchingMode::Normal, None, &mut caches.0, QuirksMode::NoQuirks, NeedsSelectorFlags::No, MatchingForInvalidation::No);
        let node = Node(el);
        let mut out: Vec<MatchedRule> = Vec::new();
        for (r, s) in candidates {
            let e = &self.entries[r as usize];
            if !e.media_ok || e.dead {
                continue;
            }
            let sel = &e.rule.selectors.slice()[s as usize];
            // a pseudo-element's rule: what follows the pseudo-element is tested here, then the
            // host part (from `offset`) is matched against the element
            let offset = match pseudo {
                None => 0,
                Some((mine, at)) => match pseudo_offset(sel, mine, at, &node, &mut cx) {
                    Some(o) => o,
                    None => continue,
                },
            };
            if selectors::matching::matches_selector(sel, offset, None, &node, &mut cx) {
                let spec = sel.specificity();
                match out.last_mut() {
                    Some(m) if m.rule.0 == r => m.specificity = m.specificity.max(spec),
                    _ => out.push(MatchedRule { rule: RuleId(r), specificity: spec }),
                }
            }
        }
        out.sort_by_key(|m| {
            let e = &self.entries[m.rule.0 as usize];
            (e.origin, e.layer_rank, m.specificity, e.order_key())
        });
        out
    }

    /// The declarations that apply to `el`, lowest precedence first: the cascade's input. The last
    /// declaration of a property wins.
    pub fn cascaded<'s, E: Element>(&'s self, el: E, inline: Option<&'s DeclBlock>, caches: &mut MatchCaches) -> Vec<&'s Declaration> {
        let matched = self.matched_rules(el, caches);
        self.order_decls(&matched, inline)
    }

    /// DZ0b2: the declaration that wins each property `el` declares (rules and `inline`), keyed by
    /// property name; a shorthand is keyed by each of its longhands. Declarations of one property
    /// share the cascade order of [`Stylist::cascaded`], so the last one wins. For an inspector's
    /// "which declaration wins" and "is this inherited"; not per frame.
    pub fn winners<'s, E: Element>(&'s self, el: E, inline: Option<&'s DeclBlock>, caches: &mut MatchCaches) -> std::collections::BTreeMap<String, &'s Declaration> {
        let mut out = std::collections::BTreeMap::new();
        for d in self.cascaded(el, inline, caches) {
            match crate::shorthand::longhands_in(self.table, &d.name) {
                Some(longs) => {
                    for l in longs {
                        out.insert(l.to_string(), d);
                    }
                }
                None => {
                    out.insert(d.name.to_string(), d);
                }
            }
        }
        out
    }

    /// The rules whose selector ends in `pseudo` and whose host part matches `el`, in cascade
    /// order.
    pub fn matched_pseudo_rules<E: Element>(&self, el: E, pseudo: &PseudoElement, caches: &mut MatchCaches) -> Vec<MatchedRule> {
        self.matched_pseudo_rules_at(el, pseudo, None, caches)
    }

    /// [`Stylist::matched_pseudo_rules`] for a repeated part at logical position `at` (index,
    /// count; WIDGET-PARITY P1).
    pub fn matched_pseudo_rules_at<E: Element>(&self, el: E, pseudo: &PseudoElement, at: Option<(usize, usize)>, caches: &mut MatchCaches) -> Vec<MatchedRule> {
        let candidates = self.pseudo_candidates_of(el, pseudo);
        self.match_candidates(el, candidates, Some((pseudo, at)), caches)
    }

    /// Computes the style of `el`'s pseudo-element `pseudo` (`::part(name)`, `::placeholder`,
    /// `::selection`; ARCH-WIDGETS W3): the rules ending in that pseudo-element whose host part
    /// matches `el`, inheriting from `host`, the element's own computed style. With no such rule
    /// the result is `host`'s inherited properties only (as a child with no rules).
    pub fn compute_pseudo<E: Element>(&self, el: E, pseudo: &PseudoElement, host: &ComputedStyle, cx: &ComputeContext, caches: &mut MatchCaches) -> ComputedStyle {
        self.compute_pseudo_at(el, pseudo, None, host, cx, caches)
    }

    /// [`Stylist::compute_pseudo`] for a repeated part (a tab, a row, a tick) at logical position
    /// `at` = (index, count) in model order (WIDGET-PARITY P1): `::part(tab):first-child`,
    /// `:last-child`, `:only-child`, `:nth-child(An+B)` and `:nth-last-child(An+B)` match it. With
    /// `None` they never match.
    pub fn compute_pseudo_at<E: Element>(&self, el: E, pseudo: &PseudoElement, at: Option<(usize, usize)>, host: &ComputedStyle, cx: &ComputeContext, caches: &mut MatchCaches) -> ComputedStyle {
        let matched = self.matched_pseudo_rules_at(el, pseudo, at, caches);
        let decls = self.order_decls(&matched, None);
        compute(self.table, &decls, Some(host), cx)
    }

    fn order_decls<'s>(&'s self, matched: &[MatchedRule], inline: Option<&'s DeclBlock>) -> Vec<&'s Declaration> {
        // (origin and importance, layer, specificity, source order)
        type Key<'k> = (u8, u32, u32, (u64, &'k str));
        let mut keyed: Vec<(Key<'s>, &Declaration)> = Vec::new();
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
                keyed.push(((rank, layer, m.specificity, e.order_key()), d));
            }
        }
        if let Some(block) = inline {
            for (i, d) in block.decls.iter().enumerate() {
                keyed.push(((if d.important { 4 } else { 2 }, 0, 0, (i as u64, "")), d));
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
    /// Buckets a selector by its subject compound (or, for one ending in a pseudo-element, by the
    /// pseudo-element and then its host compound), so an element only tries rules that could match it.
    fn insert(&mut self, rule: u32, sel: u16, selector: &Selector) {
        let entry = (rule, sel);
        let mut it = selector.iter();
        let Some(pseudo) = selector.pseudo_element() else {
            self.main.insert(entry, it);
            return;
        };
        // the pseudo-element's compound (`::part(x):hover`), then the host compound
        for _ in it.by_ref() {}
        let host: &mut Buckets = match pseudo {
            PseudoElement::Part(names) => {
                let Some(first) = names.first() else { return };
                let b = self.parts.entry(first.clone()).or_default();
                if !b.name_sets.iter().any(|s| s == names) {
                    b.name_sets.push(names.clone());
                }
                &mut b.hosts
            }
            PseudoElement::Placeholder => &mut self.placeholder,
            PseudoElement::Selection => &mut self.selection,
        };
        if it.next_sequence().is_some() {
            host.insert(entry, it);
        } else {
            host.universal.push(entry);
        }
    }
}

impl Buckets {
    /// Files `entry` under `compound`'s id, else its first class, else its tag, else universal.
    fn insert<'a>(&mut self, entry: (u32, u16), compound: impl Iterator<Item = &'a Component<crate::selector::Impl>>) {
        let (mut id, mut class, mut tag) = (None, None, None);
        compound_keys(compound, &mut id, &mut class, &mut tag);
        match (id, class, tag) {
            (Some(a), _, _) => self.ids.entry(a).or_default().push(entry),
            (_, Some(a), _) => self.classes.entry(a).or_default().push(entry),
            (_, _, Some(a)) => self.tags.entry(a).or_default().push(entry),
            _ => self.universal.push(entry),
        }
    }

    /// Appends the entries `el` could match (its id's, classes', tag's and the universal ones),
    /// unsorted and possibly repeated.
    fn collect<E: Element>(&self, el: E, out: &mut Vec<(u32, u16)>) {
        if !self.ids.is_empty()
            && let Some(id) = el.id()
            && let Some(v) = self.ids.get(id)
        {
            out.extend_from_slice(v);
        }
        if !self.classes.is_empty() {
            el.for_each_class(&mut |c| {
                if let Some(v) = self.classes.get(c) {
                    out.extend_from_slice(v);
                }
            });
        }
        if !self.tags.is_empty() {
            if let Some(v) = self.tags.get(el.tag()) {
                out.extend_from_slice(v);
            }
            if let Some(v) = el.style_as().and_then(|t| self.tags.get(t)) {
                out.extend_from_slice(v);
            }
        }
        out.extend_from_slice(&self.universal);
    }

    fn is_empty(&self) -> bool {
        self.ids.is_empty() && self.classes.is_empty() && self.tags.is_empty() && self.universal.is_empty()
    }
}

/// The id, first class and tag a compound requires. `:is()`/`:where()` of a single selector (what
/// nesting's `&` becomes: `slider { &::part(x) }` is `:is(slider)::part(x)`) requires its subject
/// compound's, so they count too.
fn compound_keys<'a>(compound: impl Iterator<Item = &'a Component<crate::selector::Impl>>, id: &mut Option<Atom>, class: &mut Option<Atom>, tag: &mut Option<Atom>) {
    for c in compound {
        match c {
            Component::ID(a) => *id = Some(a.clone()),
            Component::Class(a) => *class = class.take().or(Some(a.clone())),
            Component::LocalName(l) => *tag = Some(l.name.clone()),
            Component::Is(list) | Component::Where(list) if list.len() == 1 => compound_keys(list.slice()[0].iter(), id, class, tag),
            _ => {}
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
pub(crate) fn check_decl(table: &'static Table, d: &Declaration, inline: bool) -> Option<Diagnostic> {
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

/// For a selector ending in a pseudo-element (`host::part(x):first-child`, stored as
/// `[::part(x), :first-child, >>pseudo, host…]`: compounds right to left, each in parse order):
/// whether the element's `mine` (at logical position `at`) passes the pseudo-element and what
/// follows it, and if so the offset of the host compound. What follows is tested here, not by
/// `selectors` (which matches nothing after a pseudo-element in its stateless mode): logical
/// pseudo-classes against `at` (P1), others against the host.
fn pseudo_offset<E: Element>(sel: &Selector, mine: &PseudoElement, at: Option<(usize, usize)>, node: &Node<E>, cx: &mut MatchingContext<'_, crate::selector::Impl>) -> Option<usize> {
    use selectors::Element as _;
    use selectors::parser::Combinator;
    let comps = sel.iter_raw_match_order().as_slice();
    let k = comps.iter().position(|c| matches!(c, Component::Combinator(_)))?;
    if !matches!(comps[k], Component::Combinator(Combinator::PseudoElement)) {
        return None;
    }
    let mut seen = false;
    for c in &comps[..k] {
        let ok = match c {
            Component::PseudoElement(p) => {
                seen = true;
                mine.matched_by(p)
            }
            Component::NonTSPseudoClass(PseudoClass::Logical(l)) => l.matches(at),
            Component::NonTSPseudoClass(pc) => node.match_non_ts_pseudo_class(pc, cx),
            _ => false,
        };
        if !ok {
            return None;
        }
    }
    seen.then_some(k + 1)
}
