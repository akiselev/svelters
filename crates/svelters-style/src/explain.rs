//! SV88: why a node's property has its value. For one element and one property, the declaration
//! that won the cascade, the declarations it beat (with the step of the cascade each lost on), the
//! value after `var()` substitution, and each referenced custom property's resolution down to the
//! declaration that gives it its value.
//!
//! For tools (an inspector's "why does this look like this?"), never per frame: nothing here runs
//! unless [`Stylist::explain`] is called, and the restyle path keeps nothing extra for it. It
//! re-matches the element (and, for inheritance and `var()`, its ancestors) on each call.

use crate::cascade::{Origin, RuleId, SheetId, Stylist};
use crate::computed::ComputedStyle;
use crate::element::Element;
use crate::info::StyleSource;
use crate::sheet::{DeclBlock, Declaration};
use crate::values::ComputedValue;
use crate::{cascade, shorthand, var};

/// What [`Stylist::explain`] reads of an element besides what [`Element`] gives: its inline
/// `style=` block and its computed style as of the last restyle.
pub trait StyleLookup<E> {
    fn inline(&self, el: E) -> Option<&DeclBlock>;
    fn computed(&self, el: E) -> Option<&ComputedStyle>;
}

/// Where a declaration comes from. The theme stands where CSS has the user agent's sheet.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DeclOrigin {
    /// Theme and token sheets ([`Origin::Theme`]).
    Theme,
    /// Component and app sheets ([`Origin::Author`]).
    Author,
    /// The user's sheets ([`Origin::User`]).
    User,
    /// The element's own `style=`.
    Inline,
}

impl From<Origin> for DeclOrigin {
    fn from(o: Origin) -> Self {
        match o {
            Origin::Theme => DeclOrigin::Theme,
            Origin::Author => DeclOrigin::Author,
            Origin::User => DeclOrigin::User,
        }
    }
}

/// One declaration that applies to the element, with where it is.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub struct CascadeDecl {
    pub origin: DeclOrigin,
    /// The rule it is in; `None` for an inline declaration.
    pub rule: Option<RuleId>,
    /// The rule's sheet; `None` inline and for a generated utility rule.
    pub sheet: Option<SheetId>,
    /// The sheet's name, when the caller knows it (skelter's `set_sheet_info`); the engine leaves it `None`.
    pub sheet_name: Option<String>,
    /// The rule's selectors as CSS; `None` inline.
    pub selectors: Option<String>,
    /// The rule's `@layer` (as [`crate::Rule::layer_name`]); `None` unlayered and inline.
    pub layer: Option<String>,
    /// The highest specificity among the rule's selectors that matched; 0 inline.
    pub specificity: u32,
    pub important: bool,
    /// The declared name: the property itself, or a shorthand that sets it.
    pub name: String,
    /// The value as written.
    pub value: String,
    /// The declaration's position in its sheet's text (1-based line, UTF-16 column).
    pub line: u32,
    pub column: u32,
    /// The rule's position; equal to the declaration's inline.
    pub rule_line: u32,
    pub rule_column: u32,
    /// Where it is in a component's `<style>`, when the caller knows (dev builds); the engine leaves it `None`.
    pub source: Option<StyleSource>,
}

/// The step of the cascade a declaration lost on.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LossReason {
    /// The winner is `!important` and it isn't (or the reverse, where importance flips the order:
    /// a normal inline declaration loses to an `!important` theme one).
    Importance,
    /// Same importance, a stronger origin (theme < author < user < inline; for `!important`
    /// author < inline < theme < user).
    Origin,
    /// Same origin and importance, a later `@layer` (for `!important`, an earlier one).
    Layer,
    /// Same layer, a higher specificity.
    Specificity,
    /// Everything equal: the winner comes later (sheets in registration order, inline in written order).
    Order,
    /// It came above the winner but is `revert` or `revert-layer`, which defer to the cascade below.
    Rollback,
    /// It came above the winner but a `revert` or `revert-layer` above it excluded its origin or layer.
    RolledBack,
}

/// A declaration the winner beat.
#[derive(Clone, Debug, PartialEq)]
pub struct Beaten {
    pub decl: CascadeDecl,
    pub reason: LossReason,
}

/// One custom property a value uses through `var()`, resolved.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub struct VarResolution<E> {
    /// `--name`.
    pub name: String,
    /// The element whose declaration gives the value: the element that uses it or the ancestor it
    /// inherits from. `None` when no element declares it (the `var()` takes its fallback, or the
    /// value is invalid).
    pub declared_at: Option<E>,
    /// That declaration.
    pub decl: Option<CascadeDecl>,
    /// The custom property's computed value at the element that uses it (`var()`s substituted);
    /// `None` = guaranteed-invalid (unset, `initial`, or part of a cycle).
    pub value: Option<String>,
    /// The custom properties the declaration uses in turn, resolved at `declared_at`.
    pub uses: Vec<VarResolution<E>>,
    /// The resolution came back to a property already on the chain: `uses` stops here.
    pub cycle: bool,
}

/// What [`Stylist::explain`] says about one property of one element.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub struct Explanation<E> {
    /// The property asked about (a longhand, or a custom property `--name`).
    pub property: String,
    /// The declaration the property's value comes from on this element; `None` when none applies
    /// (or every one was rolled back): the value is inherited or the native default.
    pub winner: Option<CascadeDecl>,
    /// The other declarations of the property that apply to the element, highest first (the
    /// runner-up first), each with the step it lost on.
    pub beaten: Vec<Beaten>,
    /// The winner's value text after `var()` substitution (for a shorthand, the longhand's part);
    /// `None` with no winner, or when substitution failed (invalid at computed-value time: the
    /// property acts as `unset`). For a custom property, its computed value.
    pub specified: Option<String>,
    /// The computed value, as of the element's last restyle (`None`: not set, the native default).
    /// For a custom property see [`Explanation::specified`].
    pub computed: Option<ComputedValue>,
    /// The nearest ancestor whose declaration the value inherits, when there is no winner (or it
    /// says `inherit`/`unset`) and the property inherits.
    pub inherited_from: Option<E>,
    /// Each custom property the winner's value uses through `var()` (outside fallbacks), in order.
    pub vars: Vec<VarResolution<E>>,
}

impl<E> Explanation<E> {
    /// The same explanation with every element handle mapped (to a caller's node id).
    pub fn map<F>(self, f: &mut impl FnMut(E) -> F) -> Explanation<F> {
        Explanation {
            property: self.property,
            winner: self.winner,
            beaten: self.beaten,
            specified: self.specified,
            computed: self.computed,
            inherited_from: self.inherited_from.map(&mut *f),
            vars: self.vars.into_iter().map(|v| v.map(f)).collect(),
        }
    }

    /// Fills each declaration's [`CascadeDecl::sheet_name`] and [`CascadeDecl::source`] with what
    /// `f` returns for it.
    pub fn annotate(&mut self, f: &mut impl FnMut(&mut CascadeDecl)) {
        if let Some(w) = &mut self.winner {
            f(w);
        }
        for b in &mut self.beaten {
            f(&mut b.decl);
        }
        for v in &mut self.vars {
            v.annotate(f);
        }
    }
}

impl<E> VarResolution<E> {
    pub fn map<F>(self, f: &mut impl FnMut(E) -> F) -> VarResolution<F> {
        VarResolution {
            name: self.name,
            declared_at: self.declared_at.map(&mut *f),
            decl: self.decl,
            value: self.value,
            uses: self.uses.into_iter().map(|u| u.map(f)).collect(),
            cycle: self.cycle,
        }
    }

    fn annotate(&mut self, f: &mut impl FnMut(&mut CascadeDecl)) {
        if let Some(d) = &mut self.decl {
            f(d);
        }
        for u in &mut self.uses {
            u.annotate(f);
        }
    }
}

/// The cascade's sort key: origin and importance rank, layer, specificity, source order.
type Key<'k> = (u8, u32, u32, (u64, &'k str));

/// One applicable declaration of the property asked about, with its key and origin.
struct Applied<'s> {
    key: Key<'s>,
    decl: &'s Declaration,
    /// For `revert`: inline counts as author.
    origin: Origin,
    info: CascadeDecl,
}

/// What [`Stylist::explain`] finds on one element for one property.
struct Cascade<'s> {
    /// Lowest first.
    applied: Vec<Applied<'s>>,
    winner: Option<usize>,
    /// By index into `applied`: why each non-winner lost.
    reasons: Vec<Option<LossReason>>,
}

fn rollback(d: &Declaration) -> Option<bool> {
    let v = d.value.trim();
    if v.eq_ignore_ascii_case("revert") {
        Some(true)
    } else if v.eq_ignore_ascii_case("revert-layer") {
        Some(false)
    } else {
        None
    }
}

impl Stylist {
    /// SV88: why `el`'s `property` (a longhand or `--custom`) has its value: the winning
    /// declaration with its origin and rule, the declarations it beat with why each lost, the value
    /// after `var()` substitution, the ancestor it inherits from when nothing on `el` declares it, and
    /// each custom property it uses down to the declaration that gives it its value. A shorthand
    /// declaration counts for each of its longhands.
    ///
    /// Re-matches `el` (and its ancestors, as far as inheritance and `var()` chains go) on each
    /// call; `lookup` gives each element's inline block and its computed style. For tools only.
    pub fn explain<E: Element>(
        &self,
        el: E,
        property: &str,
        lookup: &dyn StyleLookup<E>,
    ) -> Explanation<E> {
        let mut caches = cascade::MatchCaches::new();
        let c = self.property_cascade(el, property, lookup, &mut caches);
        let custom = property.starts_with("--");
        let winner = c.winner.map(|w| &c.applied[w]);
        let computed_style = lookup.computed(el);
        let lookup_custom = |n: &str| computed_style.and_then(|s| s.custom(n)).map(str::to_string);

        let specified = if custom {
            computed_style
                .and_then(|s| s.custom(property))
                .map(str::to_string)
        } else {
            winner.and_then(|w| {
                let d = w.decl;
                let text = if d.has_var {
                    var::substitute(&d.value, &mut |n| lookup_custom(n)).ok()?
                } else {
                    d.value.to_string()
                };
                if &*d.name == property {
                    Some(text)
                } else {
                    shorthand::expand(&d.name, &text)
                        .ok()?
                        .into_iter()
                        .find(|(n, _)| *n == property)
                        .map(|(_, t)| t)
                }
            })
        };

        let inherits = custom || self.table.get(property).is_some_and(|p| p.is_inherited());
        let defers = |d: &Declaration| {
            let v = d.value.trim();
            v.eq_ignore_ascii_case("inherit") || (inherits && v.eq_ignore_ascii_case("unset"))
        };
        let inherited_from = if inherits && winner.is_none_or(|w| defers(w.decl)) {
            self.declaring_ancestor(el, property, lookup, &mut caches)
        } else {
            None
        };

        let vars = match winner {
            Some(w) if w.decl.has_var => {
                let mut stack = vec![(property.to_string(), el.key())];
                var::references(&w.decl.value)
                    .into_iter()
                    .map(|r| self.resolve_var(&r, el, el, lookup, &mut caches, &mut stack))
                    .collect()
            }
            _ => Vec::new(),
        };

        let mut beaten: Vec<Beaten> = c
            .reasons
            .iter()
            .enumerate()
            .filter_map(|(i, r)| {
                r.map(|reason| Beaten {
                    decl: c.applied[i].info.clone(),
                    reason,
                })
            })
            .collect();
        beaten.reverse();
        Explanation {
            property: property.to_string(),
            winner: winner.map(|w| w.info.clone()),
            beaten,
            specified,
            computed: if custom {
                None
            } else {
                computed_style.and_then(|s| s.get(property)).cloned()
            },
            inherited_from,
            vars,
        }
    }

    /// The declarations of `property` that apply to `el`, in cascade order, with the winner and why
    /// each other one lost. Keys as `Stylist::order_decls` (kept apart so the restyle path carries
    /// nothing for this); `revert` and `revert-layer` as `computed::rollback_wins`.
    fn property_cascade<'s, E: Element>(
        &'s self,
        el: E,
        property: &str,
        lookup: &'s dyn StyleLookup<E>,
        caches: &mut cascade::MatchCaches,
    ) -> Cascade<'s> {
        let sets = |d: &Declaration| {
            &*d.name == property
                || (!d.is_custom()
                    && shorthand::longhands_in(self.table, &d.name)
                        .is_some_and(|l| l.contains(&property)))
        };
        let mut applied: Vec<Applied<'s>> = Vec::new();
        for m in self.matched_rules(el, caches) {
            let e = &self.entries[m.rule.0 as usize];
            for d in &e.rule.block.decls {
                if !sets(d) || self.table.get(&d.name).is_some_and(|p| p.inline_only) {
                    continue;
                }
                let rank = e.origin.rank(d.important);
                let layer = if d.important {
                    u32::MAX - e.layer_rank
                } else {
                    e.layer_rank
                };
                let sheet = (e.sheet != cascade::UTILITY_SHEET).then_some(e.sheet);
                applied.push(Applied {
                    key: (
                        rank,
                        layer,
                        m.specificity,
                        (e.order, e.utility.as_deref().unwrap_or("")),
                    ),
                    decl: d,
                    origin: e.origin,
                    info: CascadeDecl {
                        origin: e.origin.into(),
                        rule: Some(m.rule),
                        sheet,
                        sheet_name: None,
                        selectors: Some(e.rule.selector_text()),
                        layer: e.rule.layer_name(),
                        specificity: m.specificity,
                        important: d.important,
                        name: d.name.to_string(),
                        value: d.value.to_string(),
                        line: d.line,
                        column: d.column,
                        rule_line: e.rule.line,
                        rule_column: e.rule.column,
                        source: None,
                    },
                });
            }
        }
        if let Some(block) = lookup.inline(el) {
            for (i, d) in block.decls.iter().enumerate().filter(|(_, d)| sets(d)) {
                let rank = if d.important {
                    cascade::INLINE_RANK.1
                } else {
                    cascade::INLINE_RANK.0
                };
                applied.push(Applied {
                    key: (rank, 0, 0, (i as u64, "")),
                    decl: d,
                    origin: Origin::Author,
                    info: CascadeDecl {
                        origin: DeclOrigin::Inline,
                        rule: None,
                        sheet: None,
                        sheet_name: None,
                        selectors: None,
                        layer: None,
                        specificity: 0,
                        important: d.important,
                        name: d.name.to_string(),
                        value: d.value.to_string(),
                        line: d.line,
                        column: d.column,
                        rule_line: d.line,
                        rule_column: d.column,
                        source: None,
                    },
                });
            }
        }
        applied.sort_by_key(|a| a.key);

        // from the top: rollbacks extend the exclusions until a declaration wins
        let mut reasons: Vec<Option<LossReason>> = vec![None; applied.len()];
        let (mut origins, mut levels): (Vec<Origin>, Vec<(u8, u32)>) = (Vec::new(), Vec::new());
        let mut winner: Option<usize> = None;
        for (i, a) in applied.iter().enumerate().rev() {
            if let Some(w) = winner {
                let wk: &Key<'_> = &applied[w].key;
                reasons[i] = Some(if a.key.0 != wk.0 {
                    if a.decl.important != applied[w].decl.important {
                        LossReason::Importance
                    } else {
                        LossReason::Origin
                    }
                } else if a.key.1 != wk.1 {
                    LossReason::Layer
                } else if a.key.2 != wk.2 {
                    LossReason::Specificity
                } else {
                    LossReason::Order
                });
                continue;
            }
            if origins.contains(&a.origin) || levels.contains(&(a.key.0, a.key.1)) {
                reasons[i] = Some(LossReason::RolledBack);
                continue;
            }
            match rollback(a.decl) {
                Some(true) => origins.push(a.origin),
                Some(false) => levels.push((a.key.0, a.key.1)),
                None => {
                    winner = Some(i);
                    continue;
                }
            }
            reasons[i] = Some(LossReason::Rollback);
        }
        Cascade {
            applied,
            winner,
            reasons,
        }
    }

    /// The nearest proper ancestor of `el` with a winning declaration of `property` that doesn't
    /// itself defer to its parent.
    fn declaring_ancestor<E: Element>(
        &self,
        el: E,
        property: &str,
        lookup: &dyn StyleLookup<E>,
        caches: &mut cascade::MatchCaches,
    ) -> Option<E> {
        let mut at = el.parent();
        while let Some(a) = at {
            if let Some((_, d)) = self.own_winner(a, property, lookup, caches) {
                let v = d.value.trim();
                if !(v.eq_ignore_ascii_case("inherit") || v.eq_ignore_ascii_case("unset")) {
                    return Some(a);
                }
            }
            at = a.parent();
        }
        None
    }

    fn own_winner<'s, E: Element>(
        &'s self,
        el: E,
        property: &str,
        lookup: &'s dyn StyleLookup<E>,
        caches: &mut cascade::MatchCaches,
    ) -> Option<(CascadeDecl, &'s Declaration)> {
        caches.clear();
        let c = self.property_cascade(el, property, lookup, caches);
        let w = c.winner?;
        let mut applied = c.applied;
        let a = applied.swap_remove(w);
        Some((a.info, a.decl))
    }

    /// `name` as `user` sees it: declared on `from` or the nearest ancestor that declares it, then
    /// what that declaration uses, resolved where it is declared. `stack` holds the (name, element)
    /// pairs being resolved, to stop at a cycle.
    fn resolve_var<E: Element>(
        &self,
        name: &str,
        user: E,
        from: E,
        lookup: &dyn StyleLookup<E>,
        caches: &mut cascade::MatchCaches,
        stack: &mut Vec<(String, std::num::NonZeroU64)>,
    ) -> VarResolution<E> {
        let value = lookup
            .computed(user)
            .and_then(|s| s.custom(name))
            .map(str::to_string);
        let mut out = VarResolution {
            name: name.to_string(),
            declared_at: None,
            decl: None,
            value,
            uses: Vec::new(),
            cycle: false,
        };
        let mut at = Some(from);
        while let Some(a) = at {
            if let Some((info, d)) = self.own_winner(a, name, lookup, caches) {
                let v = d.value.trim();
                let defers = ["inherit", "unset", "revert", "revert-layer"]
                    .iter()
                    .any(|k| v.eq_ignore_ascii_case(k));
                if !defers {
                    out.declared_at = Some(a);
                    out.decl = Some(info);
                    if d.has_var {
                        let key = (name.to_string(), a.key());
                        if stack.contains(&key) {
                            out.cycle = true;
                        } else {
                            stack.push(key);
                            out.uses = var::references(&d.value)
                                .into_iter()
                                .map(|r| self.resolve_var(&r, a, a, lookup, caches, stack))
                                .collect();
                            stack.pop();
                        }
                    }
                    return out;
                }
            }
            at = a.parent();
        }
        out
    }
}
