//! [`ComputedStyle`]: the cascade's winners resolved to typed values (CSS.md §2.4 items 4–5).
//!
//! Custom properties inherit and resolve `var()` among themselves (a cycle makes every member
//! guaranteed-invalid). A standard property that isn't declared inherits when CSS inherits it and
//! is otherwise absent, meaning "the native default". A value that fails after `var()`
//! substitution is invalid at computed-value time and behaves as `unset`. Only properties this
//! engine applies are computed ([`Table::computes`]): `via: css` ones belong to the toolkit's own
//! CSS engine where there is one.

use crate::atom::Atom;
use crate::sheet::Declaration;
use crate::shorthand;
use crate::table::{Prop, Table};
use crate::values::{ComputedValue, Length, ValueCx, parse_value};
use crate::var::{references, substitute};
use std::collections::BTreeMap;
use std::sync::Arc;

/// A shorthand's longhands with their value text.
type Expansion = Vec<(&'static str, String)>;

/// Inputs that don't come from the tree.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ComputeContext {
    /// `font-size` of the root, for `rem`.
    pub root_font_size: f32,
    /// The font size when no ancestor sets one (the backend's body font).
    pub default_font_size: f32,
}

impl Default for ComputeContext {
    fn default() -> Self {
        ComputeContext { root_font_size: 16.0, default_font_size: 16.0 }
    }
}

/// The computed style of one element.
#[derive(Clone, Debug)]
pub struct ComputedStyle {
    table: &'static Table,
    /// By the index of the property in the table; `None` = not set (native default).
    values: Vec<Option<ComputedValue>>,
    custom: Arc<BTreeMap<Atom, Arc<str>>>,
}

impl PartialEq for ComputedStyle {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.table, other.table) && self.values == other.values && self.custom == other.custom
    }
}

impl ComputedStyle {
    /// A style with nothing set.
    pub fn initial(table: &'static Table) -> ComputedStyle {
        ComputedStyle { table, values: vec![None; table.props.len()], custom: Arc::default() }
    }

    pub fn table(&self) -> &'static Table {
        self.table
    }

    pub fn get(&self, name: &str) -> Option<&ComputedValue> {
        self.table.index_of(name).and_then(|i| self.values[i].as_ref())
    }

    /// By the property's index in [`Table::props`].
    pub fn get_index(&self, index: usize) -> Option<&ComputedValue> {
        self.values.get(index).and_then(Option::as_ref)
    }

    /// Every set property with its value, in table order.
    pub fn iter(&self) -> impl Iterator<Item = (&'static Prop, &ComputedValue)> + '_ {
        self.values.iter().enumerate().filter_map(|(i, v)| v.as_ref().map(|v| (&self.table.props[i], v)))
    }

    /// A custom property's computed value (token text with `var()` substituted).
    pub fn custom(&self, name: &str) -> Option<&str> {
        self.custom.get(name).map(|v| &**v)
    }

    pub fn customs(&self) -> impl Iterator<Item = (&str, &str)> + '_ {
        self.custom.iter().map(|(k, v)| (&**k, &**v))
    }

    /// The indexes of the properties whose value differs from `before` (what a setter backend
    /// pushes, and what it restores to the native default when the value went away).
    pub fn changed_since(&self, before: &ComputedStyle) -> Vec<usize> {
        (0..self.values.len()).filter(|&i| self.values[i] != before.values.get(i).cloned().flatten()).collect()
    }

    /// `font-size` in px, if the table has it and it is set.
    pub fn font_size(&self) -> Option<f32> {
        match self.get("font-size") {
            Some(ComputedValue::Length(l)) => Some(l.px),
            _ => None,
        }
    }
}

/// The cascade's last step: `decls` are the applicable declarations, lowest precedence first.
pub(crate) fn compute(table: &'static Table, decls: &[&Declaration], parent: Option<&ComputedStyle>, cx: &ComputeContext) -> ComputedStyle {
    // winners: a declaration of the property itself, or a shorthand that sets it (the longhand
    // named second)
    let mut winners: Vec<Option<(&Declaration, Option<&'static str>)>> = vec![None; table.props.len()];
    let mut custom_decls: BTreeMap<&str, &Declaration> = BTreeMap::new();
    for d in decls {
        if d.is_custom() {
            custom_decls.insert(&d.name, d);
            continue;
        }
        if let Some(longs) = shorthand::longhands_in(table, &d.name) {
            for l in longs {
                if let Some(i) = table.index_of(l) {
                    winners[i] = Some((d, Some(l)));
                }
            }
            continue;
        }
        if let Some(i) = table.index_of(&d.name) {
            winners[i] = Some((d, None));
        }
    }
    // a shorthand's expansion, once per declaration
    let mut expanded: Vec<(*const Declaration, Option<Expansion>)> = Vec::new();

    let parent_custom = parent.map(|p| p.custom.clone()).unwrap_or_default();
    let custom = if custom_decls.is_empty() { parent_custom } else { Arc::new(resolve_customs(&custom_decls, &parent_custom)) };

    let mut out = ComputedStyle { table, values: vec![None; table.props.len()], custom };
    let parent_value = |i: usize| parent.and_then(|p| p.values[i].clone());

    let parent_font = parent.and_then(ComputedStyle::font_size).unwrap_or(cx.default_font_size);
    let parent_weight = match parent.and_then(|p| p.get("font-weight")) {
        Some(ComputedValue::FontWeight(w)) => *w,
        _ => 400,
    };
    let mut vcx = ValueCx { table, em: parent_font, rem: cx.root_font_size, current_color: None, parent_weight, percent_base: None };

    // font-size first (em of everything else), then color (currentcolor), then the rest
    let font = table.index_of("font-size");
    let color = table.index_of("color");
    let order = font.into_iter().chain(color).chain((0..table.props.len()).filter(|i| Some(*i) != font && Some(*i) != color));
    for i in order {
        let prop = &table.props[i];
        if !table.computes(prop) {
            continue;
        }
        let v = match winners[i] {
            None => prop.is_inherited().then(|| parent_value(i)).flatten(),
            Some((d, long)) => {
                let full = if d.has_var { substitute(&d.value, &mut |n| out.custom.get(n).map(|v| v.to_string())).ok() } else { Some(d.value.to_string()) };
                let text = match long {
                    None => full,
                    Some(l) => {
                        let key = d as *const Declaration;
                        let k = match expanded.iter().position(|(p, _)| *p == key) {
                            Some(k) => k,
                            None => {
                                expanded.push((key, full.and_then(|t| shorthand::expand(&d.name, &t).ok())));
                                expanded.len() - 1
                            }
                        };
                        expanded[k].1.as_ref().and_then(|e| e.iter().find(|(n, _)| *n == l)).map(|(_, t)| t.clone())
                    }
                };
                match text.as_deref().map(|t| t.trim().to_ascii_lowercase()) {
                    Some(k) if k == "inherit" => parent_value(i),
                    Some(k) if k == "initial" => None,
                    // revert and revert-layer roll back to the previous origin/layer; with no
                    // user-agent sheet that is `unset`
                    Some(k) if k == "unset" || k == "revert" || k == "revert-layer" => prop.is_inherited().then(|| parent_value(i)).flatten(),
                    // `%` in font-size is of the parent's font size; everywhere else it stays a
                    // percentage for the layout
                    Some(_) => match parse_value(prop, text.as_deref().unwrap(), &if Some(i) == font { ValueCx { percent_base: Some(parent_font), ..vcx } } else { vcx }) {
                        Ok(v) => Some(v),
                        Err(_) => prop.is_inherited().then(|| parent_value(i)).flatten(),
                    },
                    None => prop.is_inherited().then(|| parent_value(i)).flatten(),
                }
            }
        };
        if Some(i) == font {
            let px = match &v {
                Some(ComputedValue::Length(Length { px, .. })) => *px,
                _ => parent_font,
            };
            vcx.em = px;
        }
        if Some(i) == color
            && let Some(ComputedValue::Color(c)) = &v
        {
            vcx.current_color = Some(*c);
        }
        out.values[i] = v;
    }
    // U2 E5: `transition` is assembled from its longhands, which the shorthand and the longhand
    // declarations cascaded into
    if let Some(t) = table.index_of("transition")
        && table.computes(&table.props[t])
        && table.index_of(shorthand::TRANSITION[0]).is_some()
    {
        let [p, d, e, l] = shorthand::TRANSITION.map(|n| table.index_of(n).and_then(|i| out.values[i].as_ref()));
        out.values[t] = crate::values::combine_transitions(p, d, e, l);
    }
    out
}

/// The element's custom properties: the parent's, overridden by the declared ones, with `var()`
/// resolved and cycles invalid.
fn resolve_customs(declared: &BTreeMap<&str, &Declaration>, parent: &BTreeMap<Atom, Arc<str>>) -> BTreeMap<Atom, Arc<str>> {
    #[derive(Clone)]
    enum S {
        Pending,
        Active,
        Done(Option<Arc<str>>),
    }
    let mut state: BTreeMap<&str, S> = declared.keys().map(|k| (*k, S::Pending)).collect();

    fn resolve<'d>(
        name: &'d str,
        declared: &BTreeMap<&'d str, &'d Declaration>,
        parent: &BTreeMap<Atom, Arc<str>>,
        state: &mut BTreeMap<&'d str, S>,
        stack: &mut Vec<&'d str>,
        cyclic: &mut Vec<&'d str>,
    ) -> Option<Arc<str>> {
        let Some(d) = declared.get(name).copied() else { return parent.get(name).cloned() };
        let key: &'d str = &d.name;
        match state.get(key) {
            Some(S::Done(v)) => return v.clone(),
            Some(S::Active) => {
                // every property on the stack from `key` up is in the cycle
                let from = stack.iter().position(|n| *n == key).unwrap_or(0);
                cyclic.extend_from_slice(&stack[from..]);
                return None;
            }
            _ => {}
        }
        let raw = d.value.trim();
        let lower = raw.to_ascii_lowercase();
        let v: Option<Arc<str>> = match lower.as_str() {
            "initial" => None,
            "inherit" | "unset" | "revert" | "revert-layer" => parent.get(key).cloned(),
            _ if !d.has_var => Some(Arc::from(raw)),
            _ => {
                state.insert(key, S::Active);
                stack.push(key);
                let refs = references(raw);
                let mut resolved: BTreeMap<String, Option<Arc<str>>> = BTreeMap::new();
                for r in refs {
                    if let Some(r_key) = declared.get_key_value(r.as_str()).map(|(k, _)| *k) {
                        let v = resolve(r_key, declared, parent, state, stack, cyclic);
                        resolved.insert(r, v);
                    }
                }
                stack.pop();
                let out = substitute(raw, &mut |n| match resolved.get(n) {
                    Some(v) => v.as_ref().map(|v| v.to_string()),
                    None => parent.get(n).map(|v| v.to_string()),
                });
                out.ok().map(Arc::from)
            }
        };
        let v = if cyclic.contains(&key) { None } else { v };
        state.insert(key, S::Done(v.clone()));
        v
    }

    let mut out = parent.clone();
    let mut cyclic = Vec::new();
    for name in declared.keys() {
        let mut stack = Vec::new();
        let v = resolve(name, declared, parent, &mut state, &mut stack, &mut cyclic);
        match v {
            Some(v) => {
                out.insert(Atom::new(name), v);
            }
            None => {
                out.remove(*name);
            }
        }
    }
    for name in cyclic {
        out.remove(name);
    }
    out
}
