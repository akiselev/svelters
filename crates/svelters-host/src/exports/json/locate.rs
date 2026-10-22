//! SV48: the path of an error serde raised from content it buffered.
//!
//! serde reads internally tagged, adjacently tagged (content before the tag), untagged and flattened
//! types from a private buffer (`Content`), so their errors never pass through our deserializer and
//! arrive with the path to the buffered value only (`at s: invalid type: string "big", expected
//! f64`). The error itself is right: it has its kind and the offending detail (the unexpected value,
//! the field or variant name). This module finds the value it belongs to: it walks the value below
//! the known path, guided by `T`'s JSON Schema (the one the typings come from), and takes the first
//! value, in the value's own order, that the schema says is wrong in the way serde said. serde stops
//! at the first error in the same order, so it is the same value.
//!
//! The walk never decides whether a value is accepted (serde did), only where the error is. When
//! nothing matches, the error keeps the path it had.

use super::{DeError, Value, unexpected};
use serde_json::Value as J;
use std::cell::Cell;

/// What a serde error is about.
#[derive(Debug, Clone)]
pub(crate) enum Hint {
    None,
    /// `invalid_type`/`invalid_value`: the value as serde's `Unexpected` shows it.
    Unexpected(String),
    /// `invalid_length`.
    Length(usize),
    MissingField(String),
    UnknownField(String),
    UnknownVariant(String),
    /// serde's `custom`: an untagged enum no variant of which fits, `try_from`, ...
    Custom,
}

/// Values and schemas visited before giving up (the walk runs on errors only).
const BUDGET: u32 = 20_000;
/// Alternatives of one value (nested `oneOf`/`anyOf`).
const MAX_ALTS: usize = 64;
const MAX_DEPTH: usize = 64;

/// Whether `e`'s path is known to be complete without a schema: serde raised it while the
/// deserializer was on that value (`exact`), and the value there shows the error.
pub(super) fn settled(v: &Value, e: &DeError) -> bool {
    e.exact && !matches!(e.hint, Hint::MissingField(_)) && confirmed(v, None, e)
}

/// `e` with its full path, if it was raised from buffered content and the value shows where.
pub(super) fn locate(v: &Value, schema: Option<&J>, mut e: DeError) -> DeError {
    if matches!(e.hint, Hint::None) || (e.exact && confirmed(v, schema, &e)) {
        return e;
    }
    let found = [true, false].into_iter().find_map(|strict| walk(v, schema, &e, strict, false));
    if let Some(p) = found {
        e.path = p.into_iter().rev().collect();
    }
    e
}

/// Whether the value at `e`'s path shows the error. An `exact` error usually does; one serde
/// raised from content it buffered while still reading the object (an adjacently tagged enum whose
/// content comes before its tag) does not, and is looked for below.
fn confirmed(v: &Value, schema: Option<&J>, e: &DeError) -> bool {
    match &e.hint {
        // Nothing in the value to compare with.
        Hint::None | Hint::Custom => true,
        // The deserializer's own key errors end with the key.
        Hint::UnknownField(f) => e.path.first() == Some(f),
        _ => walk(v, schema, e, false, true).is_some(),
    }
}

/// The path of the first value at or below `e`'s path (only at it, if `shallow`) that the error is
/// about.
fn walk(v: &Value, schema: Option<&J>, e: &DeError, strict: bool, shallow: bool) -> Option<Vec<String>> {
    let any = J::Bool(true);
    let root = schema.unwrap_or(&any);
    let prefix: Vec<&str> = e.path.iter().rev().map(String::as_str).collect();
    let l = Locator { root, hint: &e.hint, budget: Cell::new(BUDGET), strict, shallow };
    let mut parts = Vec::new();
    l.expand(root, &mut parts, 0);
    l.find(v, &parts, &prefix, &mut Vec::new())
}

/// One schema a value must satisfy; a value's schema is a conjunction of these (`$ref`, `allOf`).
#[derive(Clone, Copy)]
struct Part<'s> {
    s: &'s J,
    /// Its `oneOf`/`anyOf` is already resolved to a branch in this alternative.
    chosen: bool,
}

struct Locator<'s, 'h> {
    root: &'s J,
    hint: &'h Hint,
    budget: Cell<u32>,
    /// The schema must confirm the error (first pass), or the value alone matches it (second pass:
    /// no schema, or one that does not describe what serde checks, like a custom `Deserialize`).
    strict: bool,
    /// Check the value at the path only.
    shallow: bool,
}

impl<'s> Locator<'s, '_> {
    fn spend(&self) -> bool {
        let b = self.budget.get();
        self.budget.set(b.saturating_sub(1));
        b > 0
    }

    fn resolve(&self, r: &str) -> Option<&'s J> {
        if r == "#" {
            return Some(self.root);
        }
        let name = r.strip_prefix("#/$defs/").or_else(|| r.strip_prefix("#/definitions/"))?;
        let name = name.replace("~1", "/").replace("~0", "~");
        self.root.get("$defs").or_else(|| self.root.get("definitions"))?.get(name.as_str())
    }

    /// `s` as conjuncts: itself, its `$ref` target and its `allOf` members.
    fn expand(&self, s: &'s J, out: &mut Vec<Part<'s>>, depth: usize) {
        if depth > MAX_DEPTH {
            return;
        }
        match s {
            J::Bool(true) => {}
            J::Object(o) => {
                out.push(Part { s, chosen: false });
                if let Some(t) = o.get("$ref").and_then(J::as_str).and_then(|r| self.resolve(r)) {
                    self.expand(t, out, depth + 1);
                }
                if let Some(J::Array(xs)) = o.get("allOf") {
                    for x in xs {
                        self.expand(x, out, depth + 1);
                    }
                }
            }
            _ => out.push(Part { s, chosen: false }),
        }
    }

    /// The alternatives `v` may be read as: one per combination of `oneOf`/`anyOf` branches that
    /// `v` plausibly is (its type, its tag constants and required keys fit), or every branch if none
    /// is (an unknown tag).
    fn alts(&self, v: &Value, parts: &[Part<'s>]) -> Vec<Vec<Part<'s>>> {
        let mut out = Vec::new();
        self.alts_into(v, parts.to_vec(), &mut out);
        out
    }

    fn alts_into(&self, v: &Value, mut parts: Vec<Part<'s>>, out: &mut Vec<Vec<Part<'s>>>) {
        if out.len() >= MAX_ALTS {
            return;
        }
        let Some(i) = parts.iter().position(|p| !p.chosen && choice(p.s).is_some()) else {
            out.push(parts);
            return;
        };
        let branches = choice(parts[i].s).unwrap_or(&[]);
        parts[i].chosen = true;
        let expanded: Vec<Vec<Part<'s>>> = branches
            .iter()
            .map(|b| {
                let mut p = parts.clone();
                self.expand(b, &mut p, 0);
                p
            })
            .collect();
        let fitting: Vec<Vec<Part<'s>>> = expanded.iter().filter(|p| fits(v, p)).cloned().collect();
        let pick = if fitting.is_empty() { expanded } else { fitting };
        if pick.is_empty() {
            out.push(parts);
        }
        for p in pick {
            self.alts_into(v, p, out);
        }
    }

    /// The schema of an object's entry `k`.
    fn prop(&self, alt: &[Part<'s>], k: &str) -> Vec<Part<'s>> {
        let mut out = Vec::new();
        for p in alt {
            if let Some(s) = p.s.get("properties").and_then(|ps| ps.get(k)) {
                self.expand(s, &mut out, 0);
            } else if let Some(s) = p.s.get("additionalProperties").filter(|s| s.is_object()) {
                self.expand(s, &mut out, 0);
            }
        }
        out
    }

    /// The schema of a list's element `i`.
    fn item(&self, alt: &[Part<'s>], i: usize) -> Vec<Part<'s>> {
        let mut out = Vec::new();
        for p in alt {
            if let Some(s) = p.s.get("prefixItems").and_then(|xs| xs.get(i)) {
                self.expand(s, &mut out, 0);
            } else if let Some(s) = p.s.get("items").filter(|s| s.is_object()) {
                self.expand(s, &mut out, 0);
            }
        }
        out
    }

    /// Whether `v` satisfies the schema (for an untagged enum no variant of which fits).
    fn valid(&self, v: &Value, parts: &[Part<'s>], depth: usize) -> bool {
        if !self.spend() || depth > MAX_DEPTH {
            // Out of budget: claim nothing.
            return true;
        }
        self.alts(v, parts).iter().any(|alt| {
            let here = alt.iter().all(|p| scalar_ok(v, p.s))
                && match v {
                    Value::Map(m) => alt.iter().all(|p| required(p.s).all(|k| has(m, k))) && m.iter().all(|(k, _)| !unknown_key(alt, k)),
                    Value::List(xs) => alt.iter().all(|p| len_ok(p.s, xs.len())),
                    _ => true,
                };
            here && match v {
                Value::Map(m) => m.iter().all(|(k, x)| self.valid(x, &self.prop(alt, k), depth + 1)),
                Value::List(xs) => xs.iter().enumerate().all(|(i, x)| self.valid(x, &self.item(alt, i), depth + 1)),
                _ => true,
            }
        })
    }

    /// The path of the first value at or below `prefix` that the error is about.
    fn find(&self, v: &Value, parts: &[Part<'s>], prefix: &[&str], path: &mut Vec<String>) -> Option<Vec<String>> {
        if !self.spend() || path.len() > MAX_DEPTH {
            return None;
        }
        // An untagged enum is about the innermost value that fits none of its variants: look below first.
        let custom = matches!(self.hint, Hint::Custom);
        for alt in self.alts(v, parts) {
            if prefix.is_empty()
                && !custom
                && let Some(p) = self.here(v, &alt, path)
            {
                return Some(p);
            }
            if prefix.is_empty() && self.shallow {
                continue;
            }
            let below = match v {
                Value::Map(m) => m.iter().find_map(|(k, x)| self.step(x, k, self.prop(&alt, k), prefix, path)),
                Value::List(xs) => xs.iter().enumerate().find_map(|(i, x)| self.step(x, &format!("[{i}]"), self.item(&alt, i), prefix, path)),
                _ => None,
            };
            if below.is_some() {
                return below;
            }
        }
        let untagged = custom && self.strict && !self.shallow && prefix.is_empty() && parts.iter().any(|p| choice(p.s).is_some());
        if untagged && !self.valid(v, parts, 0) {
            return Some(path.clone());
        }
        None
    }

    fn step(&self, x: &Value, seg: &str, parts: Vec<Part<'s>>, prefix: &[&str], path: &mut Vec<String>) -> Option<Vec<String>> {
        let rest = match prefix.split_first() {
            Some((first, rest)) if *first == seg => rest,
            Some(_) => return None,
            None => prefix,
        };
        path.push(seg.to_string());
        let found = self.find(x, &parts, rest, path);
        path.pop();
        found
    }

    /// Whether the error is about `v` itself (read as `alt`): its path if so.
    fn here(&self, v: &Value, alt: &[Part<'s>], path: &[String]) -> Option<Vec<String>> {
        let strict = self.strict;
        let at = |extra: Option<&str>| Some(path.iter().cloned().chain(extra.map(str::to_string)).collect());
        let scalar_wrong = || !alt.iter().all(|p| scalar_ok(v, p.s));
        match (self.hint, v) {
            (Hint::Unexpected(u), _) if unexpected(v).to_string() == *u && (!strict || scalar_wrong()) => at(None),
            (Hint::Length(n), Value::List(xs)) if xs.len() == *n && (!strict || !alt.iter().all(|p| len_ok(p.s, *n))) => at(None),
            (Hint::MissingField(f), Value::Map(m)) if !has(m, f) => {
                let named = if strict { alt.iter().any(|p| required(p.s).any(|k| k == f)) } else { alt.iter().any(|p| prop_names(p.s).any(|k| k == f)) };
                if named { at(None) } else { None }
            }
            (Hint::UnknownField(f), Value::Map(m)) if has(m, f) && (!strict || unknown_key(alt, f)) => at(Some(f)),
            (Hint::UnknownVariant(n), Value::Str(s)) if s == n && (!strict || scalar_wrong()) => at(None),
            // An externally tagged variant: `{ "name": ... }`.
            (Hint::UnknownVariant(n), Value::Map(m)) if m.len() == 1 && m[0].0 == *n && (!strict || unknown_key(alt, n)) => at(None),
            _ => None,
        }
    }
}

fn choice(s: &J) -> Option<&[J]> {
    s.get("oneOf").or_else(|| s.get("anyOf")).and_then(J::as_array).map(Vec::as_slice)
}

fn has(m: &[(String, Value)], k: &str) -> bool {
    m.iter().any(|(x, _)| x == k)
}

fn required(s: &J) -> impl Iterator<Item = &str> {
    s.get("required").and_then(J::as_array).into_iter().flatten().filter_map(J::as_str)
}

fn prop_names(s: &J) -> impl Iterator<Item = &str> {
    s.get("properties").and_then(J::as_object).into_iter().flat_map(|o| o.keys().map(String::as_str))
}

/// Whether `v` plausibly is this alternative: the shallow checks (type, constants, required keys,
/// tag constants) that pick a tagged enum's variant.
fn fits(v: &Value, alt: &[Part<'_>]) -> bool {
    alt.iter().all(|p| {
        scalar_ok(v, p.s)
            && match v {
                Value::Map(m) => {
                    required(p.s).all(|k| has(m, k))
                        && p.s.get("properties").and_then(J::as_object).is_none_or(|ps| {
                            ps.iter().all(|(k, ps)| match (m.iter().find(|(x, _)| x == k), ps.get("const")) {
                                (Some((_, x)), Some(c)) => eq(x, c),
                                _ => true,
                            })
                        })
                }
                _ => true,
            }
    })
}

/// An object key no conjunct declares while one forbids others (`deny_unknown_fields`).
fn unknown_key(alt: &[Part<'_>], k: &str) -> bool {
    let declared = alt.iter().any(|p| prop_names(p.s).any(|x| x == k) || p.s.get("patternProperties").is_some());
    let closed = alt.iter().any(|p| p.s.get("additionalProperties") == Some(&J::Bool(false)) || p.s.get("unevaluatedProperties") == Some(&J::Bool(false)));
    closed && !declared
}

fn number(v: &Value) -> Option<f64> {
    match v {
        Value::I64(i) => Some(*i as f64),
        Value::U64(u) => Some(*u as f64),
        Value::F64(f) | Value::DateTime(f) => Some(*f),
        _ => None,
    }
}

fn admits(t: &str, v: &Value) -> bool {
    match t {
        "null" => matches!(v, Value::Null),
        "boolean" => matches!(v, Value::Bool(_)),
        "integer" => matches!(v, Value::I64(_) | Value::U64(_)) || matches!(v, Value::F64(f) if f.is_finite() && f.fract() == 0.0),
        "number" => number(v).is_some(),
        "string" => matches!(v, Value::Str(_) | Value::Str16(_)),
        "array" => matches!(v, Value::List(_)),
        "object" => matches!(v, Value::Map(_)),
        _ => true,
    }
}

fn eq(v: &Value, c: &J) -> bool {
    match (v, c) {
        (Value::Null, J::Null) => true,
        (Value::Bool(a), J::Bool(b)) => a == b,
        (Value::Str(a), J::String(b)) => a == b,
        (_, J::Number(n)) => number(v).is_some() && number(v) == n.as_f64(),
        _ => false,
    }
}

/// The checks of a schema on `v` itself: `type`, `const`, `enum`, number bounds.
fn scalar_ok(v: &Value, s: &J) -> bool {
    let o = match s {
        J::Bool(b) => return *b,
        J::Object(o) => o,
        _ => return true,
    };
    match o.get("type") {
        Some(J::String(t)) if !admits(t, v) => return false,
        Some(J::Array(ts)) if !ts.iter().filter_map(J::as_str).any(|t| admits(t, v)) => return false,
        _ => {}
    }
    if o.get("const").is_some_and(|c| !eq(v, c)) {
        return false;
    }
    if let Some(J::Array(e)) = o.get("enum")
        && !e.iter().any(|c| eq(v, c))
    {
        return false;
    }
    if let Some(n) = number(v) {
        let bound = |k: &str| o.get(k).and_then(J::as_f64);
        let ok = bound("minimum").is_none_or(|b| n >= b)
            && bound("maximum").is_none_or(|b| n <= b)
            && bound("exclusiveMinimum").is_none_or(|b| n > b)
            && bound("exclusiveMaximum").is_none_or(|b| n < b);
        if !ok {
            return false;
        }
    }
    true
}

fn len_ok(s: &J, n: usize) -> bool {
    let get = |k: &str| s.get(k).and_then(J::as_u64).map(|x| x as usize);
    let closed = s.get("items") == Some(&J::Bool(false));
    let prefix = s.get("prefixItems").and_then(J::as_array).map(Vec::len);
    get("minItems").is_none_or(|m| n >= m) && get("maxItems").is_none_or(|m| n <= m) && !(closed && prefix.is_some_and(|p| n > p))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exports::json::DeError;
    use serde_json::json;

    fn err(hint: Hint, path: &[&str]) -> DeError {
        DeError { code: "type", path: path.iter().rev().map(|s| s.to_string()).collect(), msg: "m".into(), hint, exact: false }
    }

    fn at(v: J, schema: J, hint: Hint, path: &[&str]) -> String {
        let e = locate(&crate::exports::json::from_json(v), Some(&schema), err(hint, path));
        e.into_native().message
    }

    fn body() -> J {
        // An internally tagged enum (tag `kind`) holding another (tag `mode`), with closed variants.
        json!({
            "type": "object",
            "properties": { "body": { "$ref": "#/$defs/Body" } },
            "required": ["body"],
            "$defs": {
                "Body": { "oneOf": [
                    { "type": "object", "properties": { "kind": { "type": "string", "const": "extrude" }, "extent": { "$ref": "#/$defs/Extent" } },
                      "required": ["kind", "extent"], "additionalProperties": false },
                    { "type": "object", "properties": { "kind": { "type": "string", "const": "hole" }, "r": { "type": "number" } },
                      "required": ["kind", "r"], "additionalProperties": false }
                ]},
                "Extent": { "oneOf": [
                    { "type": "object", "properties": { "mode": { "type": "string", "const": "blind" }, "name": { "type": "string" }, "depth": { "type": "number", "format": "double" } },
                      "required": ["mode", "depth"] },
                    { "type": "object", "properties": { "mode": { "type": "string", "const": "through" } }, "required": ["mode"] }
                ]},
                "Num": { "anyOf": [{ "type": "number" }, { "type": "object", "properties": { "x": { "type": "number" } }, "required": ["x"] }] }
            }
        })
    }

    #[test]
    fn finds_errors_inside_tagged_content() {
        let v = json!({ "body": { "kind": "extrude", "extent": { "mode": "blind", "depth": "big" } } });
        assert_eq!(at(v, body(), Hint::Unexpected("string \"big\"".into()), &["body"]), "at body.extent.depth: m");
        let v = json!({ "body": { "kind": "extrude", "extent": { "mode": "blind" } } });
        assert_eq!(at(v, body(), Hint::MissingField("depth".into()), &["body"]), "at body.extent: m");
        let v = json!({ "body": { "kind": "hole", "r": 1, "extra": 2 } });
        assert_eq!(at(v, body(), Hint::UnknownField("extra".into()), &["body"]), "at body.extra: m");
        let v = json!({ "body": { "kind": "extrude", "extent": { "mode": "sideways" } } });
        assert_eq!(at(v, body(), Hint::UnknownVariant("sideways".into()), &["body"]), "at body.extent.mode: m");
        // The same string first, where the schema accepts it, is not the error.
        let v = json!({ "body": { "kind": "extrude", "extent": { "mode": "blind", "name": "big", "depth": "big" } } });
        assert_eq!(at(v, body(), Hint::Unexpected("string \"big\"".into()), &["body"]), "at body.extent.depth: m");
        // When the schema confirms nothing, the first value serde would have shown the same way.
        let v = json!({ "body": { "kind": "extrude", "extent": { "mode": "blind", "depth": 2, "note": "big" } } });
        assert_eq!(at(v, body(), Hint::Unexpected("string \"big\"".into()), &["body"]), "at body.extent.note: m");
    }

    #[test]
    fn finds_untagged_and_falls_back() {
        let mut s = body();
        s["properties"]["n"] = json!({ "$ref": "#/$defs/Num" });
        let v = json!({ "n": { "y": 1 } });
        assert_eq!(at(v, s.clone(), Hint::Custom, &[]), "at n: m");
        // No schema: the value alone.
        let v = json!({ "a": [1, { "b": "big" }] });
        assert_eq!(at(v.clone(), json!(true), Hint::Unexpected("string \"big\"".into()), &["a"]), "at a[1].b: m");
        // Nothing matches: the path stays.
        assert_eq!(at(v, json!(true), Hint::Unexpected("string \"small\"".into()), &["a"]), "at a: m");
    }
}
