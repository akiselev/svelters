//! U1 (plans/research/UTILITY-CSS.md §4.1–§4.5, engine items E1 and E2): utility classes.
//!
//! [`Resolver`] turns a class name into a rule, using the generated vocabulary
//! ([`crate::utilities`], U0) and the active theme's token names ([`UtilityTokens`]):
//!
//! ```text
//! class    := [prefix ":"] (variant ":")* ["!"] ["-"] base ["/" modifier] ["!"]
//! base     := row-name ["-" value] | "[" property ":" value "]"
//! value    := keyword | token name | number | fraction | "[" arbitrary "]" | "(" [type ":"] "--" ident ")"
//! modifier := 0..100 | "[" arbitrary "]" | "(" "--" ident ")"        bg-accent/50, text-sm/6
//! ```
//!
//! - Values are our tokens only (UQ4/UQ5): `p-7` is `var(--sv-space-7)`, `bg-surface` is
//!   `var(--sv-surface)`; a scale's names come from the theme at run time, so an app's own tokens
//!   (`--sv-cad-panel`) are values too.
//! - Arbitrary values (`p-[13px]`, `text-[length:var(--x)]`; `_` is a space, `\_` an underscore)
//!   are checked against the property's type (UQ6); a colour literal is linted
//!   ([`IssueKind::ArbitraryColor`]).
//! - Variants stack left to right (UQ7): `hover:part-thumb:` is `.c:hover::part(thumb)`,
//!   `part-thumb:hover:` is `.c::part(thumb):hover`; media variants hoist outside the rule.
//! - Rows and variants whose engine item hasn't landed resolve to [`IssueKind::Pending`].
//!
//! The [`Stylist`]'s utility layer (E2, [`Stylist::ensure_utilities`]) generates the rules on first
//! use into the author layer `utilities`, below unlayered component CSS (UQ3), appended without a
//! rebuild and ordered canonically (variant rank, vocabulary row, class name), so the cascade
//! doesn't depend on the order classes were discovered in. The compiler's checker (U4) uses the
//! same [`Resolver`] and reports [`UtilityIssue`]s; it generates nothing.

use crate::atom::Atom;
use crate::cascade::{Entry, Origin, Stylist, UTILITY_SHEET, check_decl};
use crate::diag::Severity;
use crate::sheet::{Declaration, ParseOptions, Stylesheet};
use crate::table::{Table, ValueType};
use crate::utilities::*;
use crate::utility_table::*;
use crate::var::has_var;
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

/// The engine items of UTILITY-CSS §5 that have landed: rows and variants that require them are
/// live; others resolve to [`IssueKind::Pending`].
pub const LANDED: &[&str] = &["E3", "E4", "E5", "E7"];

/// The canonical order of the utility layer's rules sits above every sheet rule's sequence number.
const UTILITY_ORDER: u64 = 1 << 62;

/// The negative cache is cleared when it grows past this (dynamic strings that aren't utilities).
const NEGATIVE_CAP: usize = 16 * 1024;

fn landed(item: &str) -> bool {
    LANDED.contains(&item)
}

// ---- tokens ----

/// The token names a theme defines, with their types (`length`, `color`, `shadow`, `duration`,
/// `easing`, `font-family`): what scale values resolve against. Names are without the `--sv-`
/// prefix. The backend builds it from its theme (egui: the theme's token manifest); the default is
/// [`UtilityTokens::builtin`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UtilityTokens {
    types: HashMap<Box<str>, Box<str>>,
}

impl UtilityTokens {
    pub fn new() -> Self {
        Self::default()
    }

    /// The built-in theme's tokens, as the generator saw them (every scale's defaults).
    pub fn builtin() -> Self {
        let mut t = Self::new();
        for s in SCALES {
            for (_, tok) in s.defaults {
                t.insert(tok, s.token_type);
            }
        }
        t
    }

    /// Adds a token (`--sv-` prefix optional).
    pub fn insert(&mut self, name: &str, ty: &str) {
        let name = name.strip_prefix(CONFIG.token_prefix).unwrap_or(name);
        self.types.insert(name.into(), ty.into());
    }

    /// The type of the token `name` (without the prefix).
    pub fn token_type(&self, name: &str) -> Option<&str> {
        self.types.get(name).map(|t| &**t)
    }

    pub fn len(&self) -> usize {
        self.types.len()
    }

    pub fn is_empty(&self) -> bool {
        self.types.is_empty()
    }
}

impl<N: AsRef<str>, T: AsRef<str>> FromIterator<(N, T)> for UtilityTokens {
    fn from_iter<I: IntoIterator<Item = (N, T)>>(iter: I) -> Self {
        let mut t = Self::new();
        for (n, ty) in iter {
            t.insert(n.as_ref(), ty.as_ref());
        }
        t
    }
}

// ---- configuration ----

/// An embed's utility settings (UQ8; egui: `EmbedConfig::utilities`, U3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UtilityConfig {
    /// Off by default: existing semantic classes (`flex`, `hidden`, `border`) keep their meaning.
    pub enabled: bool,
    /// With `Some("tw")`, only `tw:`-prefixed classes are utilities.
    pub prefix: Option<String>,
    /// cadatomic's lint profile: a colour literal (`bg-[#f00]`) is an error, not a warning (UQ6).
    pub strict_tokens: bool,
    /// [`Stylist::collect_utilities`] drops unreferenced rules, oldest first, past this many.
    pub high_water: usize,
}

impl Default for UtilityConfig {
    fn default() -> Self {
        UtilityConfig { enabled: !CONFIG.opt_in, prefix: None, strict_tokens: false, high_water: 4096 }
    }
}

impl UtilityConfig {
    /// Utilities on, no prefix, default lints.
    pub fn enabled() -> Self {
        UtilityConfig { enabled: true, ..Default::default() }
    }
}

// ---- results ----

/// What is wrong with a utility-shaped class (the codes of UTILITY-CSS §4.7).
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum IssueKind {
    /// A known namespace with an unknown value (`bg-acent`, `p-x`): `utility_unknown`.
    UnknownValue,
    /// An unknown variant before a utility (`hovr:p-2`): `utility_unknown`.
    UnknownVariant,
    /// A Tailwind v4 utility or variant egui doesn't render: `utility_unsupported`.
    Unsupported { replacement: Option<&'static str> },
    /// Waits for an engine item (`group-hover:` needs E6, `@sm:` needs CS-CQ).
    Pending { requires: &'static str },
    /// An arbitrary value the property's type rejects (`w-[red]`).
    BadValue,
    /// A colour literal in `[...]` (not a token): `utility_arbitrary_color`. The rule is generated.
    ArbitraryColor,
    /// An accepted spelling with a canonical one (`aria-checked:` is `checked:`): `utility_alias`,
    /// informational. The rule is generated.
    Alias { canonical: &'static str },
}

impl IssueKind {
    /// The compiler diagnostic code (CX1) this kind reports as.
    pub fn code(&self) -> &'static str {
        match self {
            IssueKind::UnknownValue | IssueKind::UnknownVariant => "utility_unknown",
            IssueKind::Unsupported { .. } | IssueKind::Pending { .. } => "utility_unsupported",
            IssueKind::BadValue => "utility_bad_value",
            IssueKind::ArbitraryColor => "utility_arbitrary_color",
            IssueKind::Alias { .. } => "utility_alias",
        }
    }

    /// Whether the class still generates its rule (a lint, not a failure).
    pub fn is_note(&self) -> bool {
        matches!(self, IssueKind::ArbitraryColor | IssueKind::Alias { .. })
    }
}

/// A problem with one class.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UtilityIssue {
    pub class: Atom,
    pub kind: IssueKind,
    /// `Error` only for a colour literal under `strict_tokens`; the compiler maps kinds to its own
    /// severities (an alias is informational).
    pub severity: Severity,
    pub message: String,
    /// "Did you mean" replacements of the whole base (at most three, closest first), for
    /// [`IssueKind::UnknownValue`]: the compiler's fix-its (U4).
    pub suggestions: Vec<String>,
}

impl UtilityIssue {
    fn new(class: &str, kind: IssueKind, message: impl Into<String>) -> UtilityIssue {
        UtilityIssue { class: Atom::new(class), kind, severity: Severity::Warning, message: message.into(), suggestions: Vec::new() }
    }
}

impl std::fmt::Display for UtilityIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "`{}`: {}", self.class, self.message)
    }
}

/// A resolved utility: one style rule.
#[derive(Clone, Debug, PartialEq)]
pub struct Utility {
    pub class: Atom,
    /// The selector, with the class escaped and the variants applied (`.hover\:p-2:hover`).
    pub selector: String,
    /// The media queries of its media variants, outermost first.
    pub media: Vec<String>,
    /// (property, value) pairs; values are CSS text (`var(--sv-space-2)`).
    pub decls: Vec<(Atom, String)>,
    /// `p-4!`: every declaration is `!important`.
    pub important: bool,
    /// The media variants' rank: none 0, scheme/contrast/motion/platform 1, then breakpoints
    /// (`max-*` descending, then ascending).
    pub variant_rank: u16,
    /// The vocabulary row's rank (`u16::MAX` for an arbitrary property `[prop:value]`).
    pub row_rank: u16,
    /// Lints that don't stop the rule (an arbitrary colour, an alias).
    pub notes: Vec<UtilityIssue>,
}

impl Utility {
    /// The rule as CSS text, in the utility layer.
    pub fn css(&self) -> String {
        let mut s = String::with_capacity(64 + self.selector.len());
        let _ = write!(s, "@layer {} {{ ", CONFIG.layer);
        for m in &self.media {
            let _ = write!(s, "@media {m} {{ ");
        }
        let _ = write!(s, "{} {{ ", self.selector);
        for (p, v) in &self.decls {
            let _ = write!(s, "{p}: {v}{}; ", if self.important { " !important" } else { "" });
        }
        s.push('}');
        for _ in &self.media {
            s.push_str(" }");
        }
        s.push_str(" }");
        s
    }

    /// Its source-order key in the utility layer (after origin, layer and specificity; the class
    /// name breaks ties): independent of when it was generated.
    pub fn canonical_order(&self) -> u64 {
        UTILITY_ORDER | (self.variant_rank as u64) << 16 | self.row_rank as u64
    }
}

/// The verdict on one class.
#[derive(Clone, Debug, PartialEq)]
pub enum Resolved {
    /// Not utility-shaped (`row`, `my-widget`): a plain class, never reported.
    NotUtility,
    Utility(Utility),
    Invalid(UtilityIssue),
}

// ---- the resolver ----

/// Resolves class names against the vocabulary, a style table and the theme's tokens. Pure: the
/// host's stylist and the compiler's checker share it.
#[derive(Clone, Copy)]
pub struct Resolver<'a> {
    pub table: &'static Table,
    pub tokens: &'a UtilityTokens,
    /// Only `<prefix>:`-prefixed classes are utilities.
    pub prefix: Option<&'a str>,
    pub strict_tokens: bool,
}

struct Base {
    decls: Vec<(Atom, String)>,
    rank: u16,
    notes: Vec<UtilityIssue>,
}

/// `Err(None)`: no match here (try elsewhere); `Err(Some(_))`: a reportable problem.
type Try<T> = Result<T, Option<UtilityIssue>>;

impl<'a> Resolver<'a> {
    pub fn new(table: &'static Table, tokens: &'a UtilityTokens) -> Self {
        Resolver { table, tokens, prefix: None, strict_tokens: false }
    }

    pub fn resolve(&self, class: &str) -> Resolved {
        if class.is_empty() || class.len() > 512 || class.chars().any(|c| c.is_whitespace() || c.is_control() || matches!(c, '{' | '}' | ';')) {
            return Resolved::NotUtility;
        }
        let Some(mut parts) = split_top(class, b':') else { return Resolved::NotUtility };
        let mut base = parts.pop().unwrap_or_default();
        let mut important = false;
        if let Some(b) = base.strip_suffix('!') {
            (base, important) = (b, true);
        } else if let Some(b) = base.strip_prefix('!') {
            (base, important) = (b, true);
        }
        if base.is_empty() || parts.iter().any(|p| p.is_empty()) {
            return Resolved::NotUtility;
        }
        let mut variants = &parts[..];
        if let Some(p) = self.prefix {
            match variants.split_first() {
                Some((first, rest)) if *first == p => variants = rest,
                _ => return Resolved::NotUtility,
            }
        }
        let b = match self.base(class, base) {
            Ok(b) => b,
            Err(None) => return Resolved::NotUtility,
            Err(Some(i)) => return Resolved::Invalid(i),
        };
        let mut notes = b.notes;
        let sel = match self.variants(class, variants, &mut notes) {
            Ok(s) => s,
            Err(i) => return Resolved::Invalid(i),
        };
        Resolved::Utility(Utility {
            class: Atom::new(class),
            selector: sel.selector,
            media: sel.media,
            decls: b.decls,
            important,
            variant_rank: sel.rank,
            row_rank: b.rank,
            notes,
        })
    }

    fn base(&self, class: &str, base: &str) -> Try<Base> {
        let (negative, name) = match base.strip_prefix('-') {
            Some(r) if !r.is_empty() => (true, r),
            _ => (false, base),
        };
        if name.starts_with('[') {
            return if negative { Err(None) } else { self.arbitrary_property(class, name) };
        }
        // rows whose name the base starts with, longest name first (`border-x-2` before `border`)
        let mut cands: Vec<(&Row, Option<&str>)> = Vec::new();
        for r in ROWS {
            match r.kind {
                RowKind::Fixed { .. } if name == r.name => cands.push((r, None)),
                RowKind::Namespace { .. } if name == r.name => cands.push((r, Some(""))),
                RowKind::Namespace { .. } => {
                    if let Some(v) = name.strip_prefix(r.name).and_then(|v| v.strip_prefix('-')).filter(|v| !v.is_empty()) {
                        cands.push((r, Some(v)));
                    }
                }
                _ => {}
            }
        }
        // an untyped `[...]` value tries typed properties before raw ones (`font-[600]` is a
        // weight, `font-['Inter']` a family)
        let raw = |r: &Row, v: Option<&str>| match r.kind {
            RowKind::Namespace { props, .. } if v.is_some_and(|v| v.starts_with('[')) => props.iter().all(|p| self.table.get(p).is_none_or(|p| p.ty == ValueType::Raw)),
            _ => false,
        };
        cands.sort_by_key(|&(r, v)| (std::cmp::Reverse(r.name.len()), raw(r, v)));
        let mut first_err: Option<UtilityIssue> = None;
        let mut pending: Option<&'static str> = None;
        let mut valued = false;
        for modifier_pass in [false, true] {
            for &(r, v) in &cands {
                if r.pending {
                    pending = pending.or(r.requires);
                    continue;
                }
                let res = match (r.kind, v) {
                    (RowKind::Fixed { decls }, None) if !negative && !modifier_pass => {
                        return Ok(Base { decls: decls.iter().map(|(p, v)| (Atom::new(p), v.to_string())).collect(), rank: r.rank, notes: Vec::new() });
                    }
                    (RowKind::Namespace { .. }, Some(v)) if !modifier_pass => {
                        valued |= !v.is_empty();
                        self.row_value(class, r, v, negative, None)
                    }
                    (RowKind::Namespace { .. }, Some(v)) => match split_top(v, b'/') {
                        Some(p) if p.len() >= 2 => {
                            let m = p[p.len() - 1];
                            self.row_value(class, r, &v[..v.len() - m.len() - 1], negative, Some(m))
                        }
                        _ => Err(None),
                    },
                    _ => Err(None),
                };
                match res {
                    Ok(b) => return Ok(b),
                    Err(Some(e)) => first_err = first_err.or(Some(e)),
                    Err(None) => {}
                }
            }
        }
        if let Some(e) = first_err {
            return Err(Some(e));
        }
        if let Some(u) = UNSUPPORTED_UTILITIES.iter().filter(|u| prefix_match(name, u.name) && !u.allow.contains(&name)).max_by_key(|u| u.name.len()) {
            let mut msg = format!("`{}` is not supported on egui: {}", u.name, u.reason);
            if let Some(r) = u.replacement {
                let _ = write!(msg, " (use `{r}`)");
            }
            return Err(Some(UtilityIssue::new(class, IssueKind::Unsupported { replacement: u.replacement }, msg)));
        }
        if let Some(req) = pending {
            return Err(Some(UtilityIssue::new(class, IssueKind::Pending { requires: req }, format!("`{name}` needs engine item {req}, which hasn't landed"))));
        }
        if valued {
            // a semantic class that happens to start with a namespace (`my-widget`, `top-bar`) is a
            // plain class; only values shaped like this namespace's (a number, `[...]`, `(...)`, or
            // a near miss of a name: `bg-acent`) are reported
            let valued: Vec<(&Row, &str)> = cands.iter().filter_map(|&(r, v)| v.filter(|v| !v.is_empty()).map(|v| (r, v))).collect();
            let shaped = valued.iter().any(|(_, v)| v.starts_with(|c: char| c.is_ascii_digit() || c == '[' || c == '('));
            let suggestions = suggest(&valued, negative);
            if shaped || !suggestions.is_empty() {
                let what = if negative { "a negative value" } else { "a value" };
                let mut msg = format!("`{base}` is not {what} of `{}-*`", valued[0].0.name);
                if !suggestions.is_empty() {
                    let _ = write!(msg, "; did you mean {}?", suggestions.iter().map(|s| format!("`{s}`")).collect::<Vec<_>>().join(", "));
                }
                let mut i = UtilityIssue::new(class, IssueKind::UnknownValue, msg);
                i.suggestions = suggestions;
                return Err(Some(i));
            }
        }
        Err(None)
    }

    /// `name` is the value after `<row>-` (`""` for the bare row name).
    fn row_value(&self, class: &str, r: &Row, v: &str, negative: bool, modifier: Option<&str>) -> Try<Base> {
        let RowKind::Namespace { props, values, keywords, negative: neg_ok } = r.kind else { return Err(None) };
        if negative && !neg_ok {
            return Err(None);
        }
        let mut notes = Vec::new();
        let vals: Vec<String> = match keywords.iter().find(|k| k.name == v) {
            Some(_) if negative => return Err(None),
            Some(k) => k.values.iter().map(|s| s.to_string()).collect(),
            None => {
                let one = self.source_value(class, props, values, v, &mut notes)?;
                let one = if negative { format!("calc({one} * -1)") } else { one };
                vec![one; props.len()]
            }
        };
        let mut decls: Vec<(Atom, String)> = props.iter().zip(vals).map(|(p, v)| (Atom::new(p), v)).collect();
        if let Some(m) = modifier {
            self.modifier(props, &mut decls, m)?;
        }
        Ok(Base { decls, rank: r.rank, notes })
    }

    fn source_value(&self, class: &str, props: &[&'static str], values: &[ValueSource], v: &str, notes: &mut Vec<UtilityIssue>) -> Try<String> {
        let arbitrary = values.contains(&ValueSource::Arbitrary);
        if let Some(inner) = v.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
            if !arbitrary {
                return Err(None);
            }
            let (hint, text) = type_hint(inner);
            if hint.is_some_and(|h| !props.iter().all(|p| self.table.get(p).is_some_and(|p| hint_fits(h, p.ty)))) {
                return Err(None);
            }
            let text = decode(text).ok_or(None)?;
            self.check_value(class, props, &text, notes)?;
            return Ok(text);
        }
        if let Some(inner) = v.strip_prefix('(').and_then(|v| v.strip_suffix(')')) {
            if !arbitrary {
                return Err(None);
            }
            let (hint, var) = type_hint(inner);
            if hint.is_some_and(|h| !props.iter().all(|p| self.table.get(p).is_some_and(|p| hint_fits(h, p.ty)))) {
                return Err(None);
            }
            return custom_var(var).ok_or(None);
        }
        for src in values {
            let out = match *src {
                ValueSource::Scale(s) => self.scale_token(s, v).map(|t| format!("var({}{t})", CONFIG.token_prefix)),
                ValueSource::Fraction => v.split_once('/').and_then(|(a, b)| {
                    let (a, b) = (int(a)?, int(b)?);
                    (b > 0).then(|| format!("{}%", num(a as f64 * 100.0 / b as f64)))
                }),
                ValueSource::IntPx => int(v).map(|n| format!("{n}px")),
                ValueSource::Number => is_number(v).then(|| v.to_string()),
                ValueSource::Percent => int(v).filter(|n| *n <= 100).map(|n| num(n as f64 / 100.0)),
                ValueSource::Integer(t) => int(v).filter(|n| *n >= 1).map(|n| t.replace("{n}", &n.to_string())),
                ValueSource::Arbitrary => None,
            };
            if let Some(out) = out {
                return Ok(out);
            }
        }
        Err(None)
    }

    /// The token a scale's value name stands for in the active theme.
    pub fn scale_token(&self, scale: &str, v: &str) -> Option<String> {
        let s = SCALES.iter().find(|s| s.name == scale)?;
        if !is_token_name(v) && !v.is_empty() {
            return None;
        }
        if s.patterns.is_empty() {
            return (!v.is_empty() && self.tokens.token_type(v) == Some(s.token_type)).then(|| v.to_string());
        }
        for p in s.patterns {
            let tok = match p.name {
                Some(n) if n == v => p.glob.to_string(),
                Some(_) => continue,
                None if v.is_empty() => continue,
                None => p.glob.replacen('*', v, 1),
            };
            if s.exclude.iter().any(|g| glob_match(g, &tok)) {
                continue;
            }
            if self.tokens.token_type(&tok) == Some(s.token_type) {
                return Some(tok);
            }
        }
        None
    }

    /// Type-checks an arbitrary value against each property, and lints colour literals.
    fn check_value(&self, class: &str, props: &[&'static str], text: &str, notes: &mut Vec<UtilityIssue>) -> Try<()> {
        for p in props {
            let d = Declaration { name: Atom::new(p), value: text.into(), important: false, has_var: has_var(text), line: 1, column: 1 };
            if let Some(diag) = check_decl(self.table, &d, false).filter(|d| d.severity == Severity::Error) {
                return Err(Some(UtilityIssue::new(class, IssueKind::BadValue, diag.message)));
            }
        }
        let is_color = props.iter().any(|p| self.table.get(p).is_some_and(|p| p.ty == ValueType::Color));
        let keyword = matches!(text.to_ascii_lowercase().as_str(), "transparent" | "currentcolor" | "inherit" | "initial" | "unset");
        if is_color && !has_var(text) && !keyword && !notes.iter().any(|n| n.kind == IssueKind::ArbitraryColor) {
            let mut i = UtilityIssue::new(class, IssueKind::ArbitraryColor, format!("`{text}` is a colour literal: it ignores the theme and high contrast; use a token (`bg-<role>`)"));
            if self.strict_tokens {
                i.severity = Severity::Error;
            }
            notes.push(i);
        }
        Ok(())
    }

    /// `/50` on a colour (opacity through `color-mix()`, E4), `/6` on a font size (line height).
    fn modifier(&self, props: &[&'static str], decls: &mut Vec<(Atom, String)>, m: &str) -> Try<()> {
        let ty = |p: &str| self.table.get(p).map(|p| p.ty);
        if props.iter().all(|p| ty(p) == Some(ValueType::Color)) {
            let pct = if let Some(n) = int(m) {
                if n > 100 {
                    return Err(None);
                }
                format!("{n}%")
            } else if let Some(inner) = m.strip_prefix('[').and_then(|m| m.strip_suffix(']')) {
                if let Some(p) = inner.strip_suffix('%').filter(|p| is_number(p)) {
                    format!("{p}%")
                } else if is_number(inner) && inner.parse::<f64>().is_ok_and(|n| n <= 1.0) {
                    format!("{}%", num(inner.parse::<f64>().unwrap_or(1.0) * 100.0))
                } else {
                    return Err(None);
                }
            } else if let Some(inner) = m.strip_prefix('(').and_then(|m| m.strip_suffix(')')) {
                custom_var(inner).ok_or(None)?
            } else {
                return Err(None);
            };
            for (_, v) in decls.iter_mut() {
                *v = format!("color-mix(in oklab, {v} {pct}, transparent)");
            }
            return Ok(());
        }
        if props == ["font-size"] {
            let lh = if let Some(t) = self.scale_token("space", m) {
                format!("var({}{t})", CONFIG.token_prefix)
            } else if let Some(inner) = m.strip_prefix('[').and_then(|m| m.strip_suffix(']')) {
                decode(inner).ok_or(None)?
            } else if let Some(inner) = m.strip_prefix('(').and_then(|m| m.strip_suffix(')')) {
                custom_var(inner).ok_or(None)?
            } else {
                return Err(None);
            };
            decls.push((Atom::new("line-height"), lh));
            return Ok(());
        }
        Err(None)
    }

    /// `[prop:value]`: any property of the table, or a custom property.
    fn arbitrary_property(&self, class: &str, name: &str) -> Try<Base> {
        let inner = name.strip_prefix('[').and_then(|n| n.strip_suffix(']')).ok_or(None)?;
        let (prop, value) = inner.split_once(':').ok_or(None)?;
        let prop = if prop.starts_with("--") { prop.to_string() } else { prop.to_ascii_lowercase() };
        if !is_ident(&prop) {
            return Err(None);
        }
        let value = decode(value).ok_or(None)?;
        let mut notes = Vec::new();
        if !prop.starts_with("--") {
            if self.table.get(&prop).is_none() && crate::shorthand::longhands_in(self.table, &prop).is_none() {
                return Err(Some(UtilityIssue::new(
                    class,
                    IssueKind::Unsupported { replacement: None },
                    format!("`{prop}` is not {} {} style property", if self.table.toolkit.starts_with(['a', 'e', 'i', 'o', 'u']) { "an" } else { "a" }, self.table.toolkit),
                )));
            }
            let p: &'static str = self.table.get(&prop).map(|p| p.name).unwrap_or("");
            if p.is_empty() {
                // a shorthand: checked as a whole by `check_decl`
                let d = Declaration { name: Atom::new(&prop), value: value.as_str().into(), important: false, has_var: has_var(&value), line: 1, column: 1 };
                if let Some(diag) = check_decl(self.table, &d, false).filter(|d| d.severity == Severity::Error) {
                    return Err(Some(UtilityIssue::new(class, IssueKind::BadValue, diag.message)));
                }
            } else {
                self.check_value(class, &[p], &value, &mut notes)?;
            }
        }
        Ok(Base { decls: vec![(Atom::new(&prop), value)], rank: u16::MAX, notes })
    }

    fn variants(&self, class: &str, vs: &[&str], notes: &mut Vec<UtilityIssue>) -> Result<Sel, UtilityIssue> {
        let mut sel = String::from(".");
        let _ = cssparser::serialize_identifier(class, &mut sel);
        let mut media = Vec::new();
        let mut rank = 0u16;
        let unknown = |v: &str| UtilityIssue::new(class, IssueKind::UnknownVariant, format!("`{v}:` is not a variant"));
        for &v in vs {
            if let Some(s) = self.state(class, v, notes) {
                sel.push(':');
                sel.push_str(s);
                continue;
            }
            if let Some((_, pe)) = PSEUDO_ELEMENTS.iter().find(|(n, _)| *n == v) {
                sel.push_str(pe);
                continue;
            }
            if let Some((_, t)) = STRUCTURAL.iter().find(|(n, _)| *n == v) {
                sel = t.replace('&', &sel);
                continue;
            }
            if let Some(part) = v.strip_prefix(PART_PREFIX) {
                if !is_ident(part) {
                    return Err(unknown(v));
                }
                let _ = write!(sel, "::part({part})");
                continue;
            }
            if let Some(inner) = v.strip_prefix(STATE_PREFIX).and_then(bracket) {
                if inner.is_empty() || !inner.split('.').all(is_ident) {
                    return Err(unknown(v));
                }
                let _ = write!(sel, ":state({inner})");
                continue;
            }
            if let Some(rest) = v.strip_prefix(PROP_PREFIX) {
                let (name, value) = match bracket(rest) {
                    Some(inner) => match inner.split_once('=') {
                        Some((n, val)) => (n, decode(val.trim_matches(['"', '\''])).ok_or_else(|| unknown(v))?),
                        None => (inner, "true".to_string()),
                    },
                    None => (rest, "true".to_string()),
                };
                if !is_ident(name) {
                    return Err(unknown(v));
                }
                let _ = write!(sel, "[{name}=");
                let _ = cssparser::serialize_string(&value, &mut sel);
                sel.push(']');
                continue;
            }
            if let Some(c) = COMPOUND.iter().find(|c| v.strip_prefix(c.name).is_some_and(|r| r.starts_with('-'))) {
                if let Some(req) = c.requires.filter(|r| !landed(r)) {
                    return Err(UtilityIssue::new(class, IssueKind::Pending { requires: req }, format!("`{v}:` needs engine item {req}, which hasn't landed")));
                }
                let arg = &v[c.name.len() + 1..];
                if let Some(inner) = bracket(arg) {
                    let inner = decode(inner).ok_or_else(|| unknown(v))?;
                    let t = match c.name {
                        "not" => "&:not({x})",
                        "has" => "&:has({x})",
                        "in" => ":where({x}) &",
                        _ => return Err(unknown(v)),
                    };
                    sel = t.replace('&', &sel).replace("{x}", &inner);
                } else if let Some(s) = self.state(class, arg, notes) {
                    sel = c.selector.replace('&', &sel).replace("{state}", s);
                } else {
                    return Err(unknown(v));
                }
                continue;
            }
            if let Some(m) = MEDIA.iter().find(|m| m.name == v) {
                if m.pending {
                    let req = m.requires.unwrap_or("a media feature");
                    return Err(UtilityIssue::new(class, IssueKind::Pending { requires: req }, format!("`{v}:` needs engine item {req}, which hasn't landed")));
                }
                media.push(m.query.to_string());
                rank = rank.max(1);
                continue;
            }
            let nb = BREAKPOINTS.sizes.len() as u16;
            if let Some(i) = BREAKPOINTS.sizes.iter().position(|(n, _)| *n == v) {
                media.push(BREAKPOINTS.min.replace("{px}", &format!("{}px", num(BREAKPOINTS.sizes[i].1 as f64))));
                rank = rank.max(2 + nb + i as u16);
                continue;
            }
            if let Some(i) = v.strip_prefix(BREAKPOINTS.max_prefix).and_then(|n| BREAKPOINTS.sizes.iter().position(|(s, _)| *s == n)) {
                media.push(BREAKPOINTS.max.replace("{px}", &format!("{}px", num(BREAKPOINTS.sizes[i].1 as f64))));
                rank = rank.max(2 + (nb - 1 - i as u16));
                continue;
            }
            if v.starts_with('@')
                && let Some(req) = CONTAINERS.requires.filter(|r| !landed(r))
                && UNSUPPORTED_VARIANTS.iter().all(|u| u.name != v)
            {
                return Err(UtilityIssue::new(class, IssueKind::Pending { requires: req }, format!("`{v}:` needs {req} (container queries), which hasn't landed")));
            }
            if let Some(inner) = bracket(v)
                && inner.contains('&')
            {
                let inner = decode(inner).ok_or_else(|| unknown(v))?;
                sel = inner.replace('&', &sel);
                continue;
            }
            if let Some(u) = UNSUPPORTED_VARIANTS.iter().filter(|u| prefix_match(v, u.name)).max_by_key(|u| u.name.len()) {
                let mut msg = format!("`{v}:` is not supported on egui: {}", u.reason);
                if let Some(r) = u.replacement {
                    let _ = write!(msg, " (use `{r}`)");
                }
                return Err(UtilityIssue::new(class, IssueKind::Unsupported { replacement: u.replacement }, msg));
            }
            return Err(unknown(v));
        }
        Ok(Sel { selector: sel, media, rank })
    }

    /// A state variant's pseudo-class (aliases noted).
    fn state(&self, class: &str, v: &str, notes: &mut Vec<UtilityIssue>) -> Option<&'static str> {
        if let Some(s) = STATE_VARIANTS.iter().find(|s| s.name == v) {
            return Some(s.name);
        }
        let (_, to) = ALIASES.iter().find(|(a, _)| *a == v)?;
        let s = STATE_VARIANTS.iter().find(|s| s.name == *to)?;
        notes.push(UtilityIssue::new(class, IssueKind::Alias { canonical: s.name }, format!("`{v}:` is spelled `{}:` here", s.name)));
        Some(s.name)
    }
}

struct Sel {
    selector: String,
    media: Vec<String>,
    rank: u16,
}

// ---- lexical helpers ----

/// Splits at `sep` outside brackets and parentheses (`\` escapes the next byte); `None` when they
/// don't balance.
fn split_top(s: &str, sep: u8) -> Option<Vec<&str>> {
    let b = s.as_bytes();
    let (mut out, mut depth, mut start, mut i) = (Vec::new(), 0i32, 0usize, 0usize);
    while i < b.len() {
        match b[i] {
            b'\\' => {
                i += 2;
                continue;
            }
            b'[' | b'(' => depth += 1,
            b']' | b')' => {
                depth -= 1;
                if depth < 0 {
                    return None;
                }
            }
            c if c == sep && depth == 0 => {
                out.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    if depth != 0 || start > s.len() {
        return None;
    }
    out.push(&s[start..]);
    Some(out)
}

/// `[x]` → `x`.
fn bracket(s: &str) -> Option<&str> {
    s.strip_prefix('[')?.strip_suffix(']')
}

/// An arbitrary value's text: `_` is a space, `\_` an underscore. `None` when empty.
fn decode(s: &str) -> Option<String> {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'_') => {
                chars.next();
                out.push('_');
            }
            '_' => out.push(' '),
            c => out.push(c),
        }
    }
    let t = out.trim();
    (!t.is_empty()).then(|| t.to_string())
}

/// `(--x)` → `var(--x)`.
fn custom_var(s: &str) -> Option<String> {
    (s.starts_with("--") && s.len() > 2 && is_ident(s)).then(|| format!("var({s})"))
}

/// A Tailwind type hint before an arbitrary value (`length:var(--x)`).
fn type_hint(s: &str) -> (Option<&str>, &str) {
    const HINTS: &[&str] = &["color", "length", "number", "percentage", "shadow", "family-name"];
    match s.split_once(':') {
        Some((h, rest)) if HINTS.contains(&h) => (Some(h), rest),
        _ => (None, s),
    }
}

fn hint_fits(hint: &str, ty: ValueType) -> bool {
    use ValueType as T;
    match hint {
        "color" => ty == T::Color,
        "length" => matches!(ty, T::Length | T::Insets | T::Raw),
        "number" => matches!(ty, T::Number | T::Int | T::Raw | T::FontWeight),
        "percentage" => matches!(ty, T::Length | T::Insets | T::Number | T::Raw),
        "shadow" => ty == T::Shadow,
        "family-name" => ty == T::Raw,
        _ => false,
    }
}

/// `name` is `prefix` or starts with `prefix-`.
fn prefix_match(name: &str, prefix: &str) -> bool {
    name.strip_prefix(prefix).is_some_and(|r| r.is_empty() || r.starts_with('-'))
}

fn is_ident(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') && !s.as_bytes()[0].is_ascii_digit()
}

fn is_token_name(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// A non-negative integer without a sign or leading zeros.
fn int(s: &str) -> Option<u32> {
    (!s.is_empty() && s.len() <= 6 && s.bytes().all(|b| b.is_ascii_digit()) && (s == "0" || !s.starts_with('0'))).then(|| s.parse().ok()).flatten()
}

/// A non-negative decimal (`2`, `0.5`).
fn is_number(s: &str) -> bool {
    let (a, b) = s.split_once('.').unwrap_or((s, "0"));
    !a.is_empty() && !b.is_empty() && a.bytes().all(|c| c.is_ascii_digit()) && b.bytes().all(|c| c.is_ascii_digit())
}

/// A number as CSS text: at most 4 decimals, no trailing zeros.
fn num(v: f64) -> String {
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-0" { "0".into() } else { s.into() }
}

/// Near misses of a value among its rows' keywords and the built-in theme's scale names: the
/// whole base (`bg-accent` for `bg-acent`), closest first, at most three.
fn suggest(valued: &[(&Row, &str)], negative: bool) -> Vec<String> {
    let mut found: Vec<(usize, String)> = Vec::new();
    for &(r, v) in valued {
        let RowKind::Namespace { values, keywords, .. } = r.kind else { continue };
        let max = if v.chars().count() <= 3 { 1 } else { 2 };
        let names = keywords.iter().map(|k| k.name).chain(values.iter().filter_map(|s| match s {
            ValueSource::Scale(s) => SCALES.iter().find(|x| x.name == *s),
            _ => None,
        }).flat_map(|s| s.defaults.iter().map(|d| d.0)));
        for n in names.filter(|n| !n.is_empty()) {
            let d = distance(v, n);
            if d <= max {
                let s = format!("{}{}-{n}", if negative { "-" } else { "" }, r.name);
                if !found.iter().any(|(_, x)| *x == s) {
                    found.push((d, s));
                }
            }
        }
    }
    found.sort();
    found.into_iter().take(3).map(|(_, s)| s).collect()
}

/// Levenshtein distance.
fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cur = row[j + 1];
            row[j + 1] = (prev + usize::from(ca != *cb)).min(row[j] + 1).min(cur + 1);
            prev = cur;
        }
    }
    row[b.len()]
}

/// A glob with at most one `*`.
fn glob_match(glob: &str, s: &str) -> bool {
    match glob.split_once('*') {
        None => glob == s,
        Some((a, b)) => s.len() >= a.len() + b.len() && s.starts_with(a) && s.ends_with(b),
    }
}

// ---- the utility layer (E2) ----

struct Known {
    refs: u32,
    seq: u64,
}

/// What the utility layer has done so far ([`Stylist::utility_stats`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UtilityStats {
    /// Utilities generated (each once until dropped).
    pub generated: u64,
    /// Utilities with a live rule now.
    pub live: usize,
    /// Classes in the negative cache (not utilities, or invalid ones).
    pub negative: usize,
    /// Rules dropped by [`Stylist::collect_utilities`].
    pub dropped: u64,
    /// `ensure_utilities` calls that generated something.
    pub batches: u64,
}

/// The [`Stylist`]'s utility bookkeeping.
pub(crate) struct Layer {
    config: UtilityConfig,
    tokens: UtilityTokens,
    known: HashMap<Atom, Known>,
    negative: HashSet<Atom>,
    seq: u64,
    /// Rules marked dead since the last rebuild.
    pub(crate) dead: usize,
    stats: UtilityStats,
}

impl Default for Layer {
    fn default() -> Self {
        Layer { config: UtilityConfig::default(), tokens: UtilityTokens::builtin(), known: HashMap::new(), negative: HashSet::new(), seq: 0, dead: 0, stats: UtilityStats::default() }
    }
}

impl Stylist {
    /// Sets the utility settings (U3: from `EmbedConfig::utilities`). Turning utilities off or
    /// changing the prefix drops every generated rule.
    pub fn set_utilities(&mut self, config: UtilityConfig) {
        let old = &self.utilities.config;
        let reset = old.enabled != config.enabled || old.prefix != config.prefix;
        let relint = old.strict_tokens != config.strict_tokens;
        self.utilities.config = config;
        if reset {
            self.drop_all_utilities();
        } else if relint {
            self.utilities.negative.clear();
        }
    }

    pub fn utility_config(&self) -> &UtilityConfig {
        &self.utilities.config
    }

    /// Sets the theme's token names (U3: from the active theme; DS1 `set_theme`). Generated rules
    /// stay (they read `var()`s); classes rejected before are tried again.
    pub fn set_utility_tokens(&mut self, tokens: UtilityTokens) {
        if tokens != self.utilities.tokens {
            self.utilities.tokens = tokens;
            self.utilities.negative.clear();
        }
    }

    pub fn utility_tokens(&self) -> &UtilityTokens {
        &self.utilities.tokens
    }

    /// The resolver with this stylist's table, tokens and settings.
    pub fn utility_resolver(&self) -> Resolver<'_> {
        let c = &self.utilities.config;
        Resolver { table: self.table, tokens: &self.utilities.tokens, prefix: c.prefix.as_deref(), strict_tokens: c.strict_tokens }
    }

    /// Generates the rules of the classes that are utilities and have none yet, in one batch
    /// appended to the utility layer without a rebuild (E2). Classes seen before (generated or
    /// not) cost one hash lookup. Returns the problems found in classes seen for the first time
    /// (each reported once). Call it before using the classes' invalidation (`class_changed`):
    /// a new rule's selectors enter the maps here. Does nothing while utilities are off.
    pub fn ensure_utilities(&mut self, classes: &[Atom], opts: &ParseOptions<'_>) -> Vec<UtilityIssue> {
        let mut issues = Vec::new();
        if !self.utilities.config.enabled {
            return issues;
        }
        let mut rejected: Vec<Atom> = Vec::new();
        let mut fresh: Vec<(Utility, Stylesheet)> = Vec::new();
        {
            let res = self.utility_resolver();
            let mut seen: HashSet<&str> = HashSet::new();
            for c in classes {
                if self.utilities.known.contains_key(c) || self.utilities.negative.contains(c) || !seen.insert(c) {
                    continue;
                }
                match res.resolve(c) {
                    Resolved::NotUtility => rejected.push(c.clone()),
                    Resolved::Invalid(i) => {
                        rejected.push(c.clone());
                        issues.push(i);
                    }
                    Resolved::Utility(u) => {
                        let sheet = Stylesheet::parse(&u.css(), opts);
                        let bad = sheet.diagnostics.first().map(|d| d.message.clone()).or_else(|| {
                            sheet.rules.iter().flat_map(|r| r.block.decls.iter()).find_map(|d| check_decl(self.table, d, false).filter(|x| x.severity == Severity::Error).map(|x| x.message))
                        });
                        let bad = bad.or_else(|| sheet.rules.is_empty().then(|| "generates no rule".to_string()));
                        if let Some(msg) = bad {
                            rejected.push(c.clone());
                            issues.push(UtilityIssue::new(c, IssueKind::BadValue, msg));
                            continue;
                        }
                        issues.extend(u.notes.iter().cloned());
                        fresh.push((u, sheet));
                    }
                }
            }
        }
        let l = &mut self.utilities;
        if l.negative.len() + rejected.len() > NEGATIVE_CAP {
            l.negative.clear();
        }
        l.negative.extend(rejected);
        self.insert_utilities(fresh);
        issues
    }

    /// Adds utility rules the compiler resolved already (a precompiled sheet's, `binary`): no
    /// vocabulary lookup at run time. Classes that have a rule keep it. Does nothing while
    /// utilities are off. Returns how many were added.
    pub fn preload_utilities(&mut self, utilities: &[Utility], opts: &ParseOptions<'_>) -> usize {
        if !self.utilities.config.enabled {
            return 0;
        }
        let fresh: Vec<(Utility, Stylesheet)> = utilities
            .iter()
            .filter(|u| !self.utilities.known.contains_key(&u.class))
            .map(|u| (u.clone(), Stylesheet::parse(&u.css(), opts)))
            .filter(|(_, s)| s.diagnostics.is_empty() && !s.rules.is_empty())
            .collect();
        let n = fresh.len();
        for (u, _) in &fresh {
            self.utilities.negative.remove(&u.class);
        }
        self.insert_utilities(fresh);
        n
    }

    /// Appends generated utility rules to the utility layer (one batch).
    fn insert_utilities(&mut self, fresh: Vec<(Utility, Stylesheet)>) {
        if fresh.is_empty() {
            return;
        }
        self.utilities.stats.batches += 1;
        let mut need_rebuild = false;
        for (_, sheet) in &fresh {
            for name in &sheet.layers {
                if !self.layers.contains(name) {
                    self.layers.push(name.clone());
                    need_rebuild = true;
                }
            }
        }
        for (u, sheet) in fresh {
            let order = u.canonical_order();
            for rule in sheet.rules {
                let e = Entry { sheet: UTILITY_SHEET, origin: Origin::Author, rule, order, utility: Some(u.class.clone()), dead: false, layer_rank: 0, media_ok: true };
                if need_rebuild {
                    self.entries.push(e);
                } else {
                    self.append_entry(e);
                }
            }
            let l = &mut self.utilities;
            l.seq += 1;
            l.stats.generated += 1;
            l.known.insert(u.class, Known { refs: 0, seq: l.seq });
        }
        if need_rebuild {
            // the first utility rule declares the layer (unless the theme declared it): ranks change
            self.rebuild();
        }
    }

    /// Whether `class` has a generated utility rule.
    pub fn is_utility(&self, class: &str) -> bool {
        self.utilities.known.contains_key(class)
    }

    /// Counts nodes carrying these classes (U3: the classes a node gained). Non-utilities are
    /// ignored; call after [`Stylist::ensure_utilities`].
    pub fn retain_utilities(&mut self, classes: &[Atom]) {
        for c in classes {
            if let Some(k) = self.utilities.known.get_mut(c) {
                k.refs += 1;
            }
        }
    }

    /// The reverse of [`Stylist::retain_utilities`] (the classes a node lost, or a removed node's).
    pub fn release_utilities(&mut self, classes: &[Atom]) {
        for c in classes {
            if let Some(k) = self.utilities.known.get_mut(c) {
                k.refs = k.refs.saturating_sub(1);
            }
        }
    }

    /// Past the high-water mark, drops the rules of utilities no node references, oldest first,
    /// until the count is back at the mark (a dropped class is generated again on its next use).
    /// Returns how many were dropped; U3 calls it once per restyle pass, after retaining.
    pub fn collect_utilities(&mut self) -> usize {
        let l = &mut self.utilities;
        let over = l.known.len().saturating_sub(l.config.high_water);
        if over == 0 {
            return 0;
        }
        let mut idle: Vec<(u64, Atom)> = l.known.iter().filter(|(_, k)| k.refs == 0).map(|(c, k)| (k.seq, c.clone())).collect();
        idle.sort_unstable();
        idle.truncate(over);
        let drop: HashSet<Atom> = idle.into_iter().map(|(_, c)| c).collect();
        for c in &drop {
            l.known.remove(c);
        }
        l.stats.dropped += drop.len() as u64;
        let mut dead = 0;
        for e in &mut self.entries {
            if !e.dead && e.utility.as_ref().is_some_and(|c| drop.contains(c)) {
                e.dead = true;
                dead += 1;
            }
        }
        self.utilities.dead += dead;
        if self.utilities.dead * 2 > self.entries.len() {
            self.rebuild();
        }
        drop.len()
    }

    pub fn utility_stats(&self) -> UtilityStats {
        let l = &self.utilities;
        UtilityStats { live: l.known.len(), negative: l.negative.len(), ..l.stats }
    }

    fn drop_all_utilities(&mut self) {
        let l = &mut self.utilities;
        l.known.clear();
        l.negative.clear();
        if self.entries.iter().any(|e| e.utility.is_some()) {
            for e in &mut self.entries {
                e.dead |= e.utility.is_some();
            }
            self.rebuild();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(class: &str) -> Resolved {
        Resolver::new(&crate::tables::EGUI, &UtilityTokens::builtin()).resolve(class)
    }

    fn u(class: &str) -> Utility {
        match r(class) {
            Resolved::Utility(u) => u,
            other => panic!("{class}: {other:?}"),
        }
    }

    #[test]
    fn lexical_helpers() {
        assert_eq!(split_top("hover:bg-[a:b]:x", b':'), Some(vec!["hover", "bg-[a:b]", "x"]));
        assert_eq!(split_top("a\\:b:c", b':'), Some(vec!["a\\:b", "c"]));
        assert_eq!(split_top("a[b", b':'), None);
        assert_eq!(decode("calc(100%_-_8px)").as_deref(), Some("calc(100% - 8px)"));
        assert_eq!(decode("a\\_b").as_deref(), Some("a_b"));
        assert_eq!(num(100.0 / 3.0), "33.3333");
        assert_eq!(num(0.4), "0.4");
        assert_eq!(int("07"), None);
        assert!(glob_match("font-size-*", "font-size-sm") && !glob_match("font-size-*", "font-size"));
    }

    #[test]
    fn values() {
        let d = |c: &str| u(c).decls.iter().map(|(p, v)| format!("{p}: {v}")).collect::<Vec<_>>().join("; ");
        assert_eq!(d("p-7"), "padding: var(--sv-space-7)");
        assert_eq!(d("px-0-5"), "padding-left: var(--sv-space-0-5); padding-right: var(--sv-space-0-5)");
        assert_eq!(d("-mt-2"), "margin-top: calc(var(--sv-space-2) * -1)");
        assert_eq!(d("w-1/3"), "width: 33.3333%");
        assert_eq!(d("w-full"), "width: 100%");
        assert_eq!(d("h-row"), "height: var(--sv-row-h)");
        assert_eq!(d("border"), "border-width: var(--sv-stroke)");
        assert_eq!(d("border-x-2"), "border-left-width: 2px; border-right-width: 2px");
        assert_eq!(d("border-list-hover"), "border-color: var(--sv-list-hover)");
        assert_eq!(d("rounded"), "border-radius: var(--sv-radius)");
        assert_eq!(d("opacity-40"), "opacity: 0.4");
        assert_eq!(d("grid-cols-3"), "grid-template-columns: repeat(3, minmax(0, 1fr))");
        assert_eq!(d("text-sm"), "font-size: var(--sv-font-size-sm)");
        assert_eq!(d("text-fg"), "color: var(--sv-fg)");
        assert_eq!(d("text-center"), "text-align: center");
        assert_eq!(d("text-[13px]"), "font-size: 13px");
        assert_eq!(d("text-[length:var(--x)]"), "font-size: var(--x)");
        assert_eq!(d("text-(--x)"), "color: var(--x)");
        assert_eq!(d("text-sm/6"), "font-size: var(--sv-font-size-sm); line-height: var(--sv-space-6)");
        assert_eq!(d("bg-accent/50"), "background-color: color-mix(in oklab, var(--sv-accent) 50%, transparent)");
        assert_eq!(d("w-[calc(100%_-_8px)]"), "width: calc(100% - 8px)");
        assert_eq!(d("[scrollbar-gutter:stable]"), "scrollbar-gutter: stable");
        assert_eq!(d("[--my-gap:4px]"), "--my-gap: 4px");
        assert_eq!(d("duration-150"), "transition-duration: 150ms");
        assert_eq!(d("ease-out"), "transition-timing-function: var(--sv-ease-out)");
        assert_eq!(d("flex"), "display: flex");
        assert_eq!(d("flex-1"), "flex-grow: 1; flex-shrink: 1; flex-basis: 0%");
        assert!(u("p-4!").important && u("!p-4").important);
        for not in ["row", "my-widget", "text", "group", "p-", "", "a b", "x{y}"] {
            assert_eq!(r(not), Resolved::NotUtility, "{not}");
        }
    }

    #[test]
    fn issues() {
        let kind = |c: &str| match r(c) {
            Resolved::Invalid(i) => i.kind,
            other => panic!("{c}: {other:?}"),
        };
        assert_eq!(kind("bg-acent"), IssueKind::UnknownValue);
        assert_eq!(kind("-p-2"), IssueKind::UnknownValue);
        let Resolved::Invalid(i) = r("bg-acent") else { panic!() };
        assert_eq!(i.suggestions.first().map(|s| s.as_str()), Some("bg-accent"), "{i:?}");
        assert!(i.message.contains("did you mean `bg-accent`"), "{}", i.message);
        assert_eq!(kind("p-[x]"), IssueKind::BadValue);
        // semantic classes that start with a namespace stay plain classes
        for plain in ["my-widget", "top-bar", "text-input", "left-pane"] {
            assert_eq!(r(plain), Resolved::NotUtility, "{plain}");
        }
        assert_eq!(kind("w-[red]"), IssueKind::BadValue);
        assert_eq!(kind("ring-2"), IssueKind::Unsupported { replacement: Some("outline-*") });
        assert_eq!(kind("ring-offset-2"), IssueKind::Unsupported { replacement: Some("outline-offset-*") });
        assert_eq!(kind("hovr:p-2"), IssueKind::UnknownVariant);
        assert_eq!(kind("group-hover:p-2"), IssueKind::Pending { requires: "E6" });
        assert!(matches!(kind("visited:p-2"), IssueKind::Unsupported { .. }));
        assert!(matches!(kind("[transform:none]"), IssueKind::Unsupported { .. }));
        let c = u("bg-[#f00]");
        assert_eq!(c.notes[0].kind, IssueKind::ArbitraryColor);
        assert_eq!(c.notes[0].severity, Severity::Warning);
        let builtin = UtilityTokens::builtin();
        let strict = Resolver { strict_tokens: true, ..Resolver::new(&crate::tables::EGUI, &builtin) };
        let Resolved::Utility(c) = strict.resolve("bg-[#f00]") else { panic!() };
        assert_eq!(c.notes[0].severity, Severity::Error);
        assert_eq!(u("aria-checked:p-2").notes[0].kind, IssueKind::Alias { canonical: "checked" });
    }

    #[test]
    fn variants_stack_left_to_right() {
        let s = |c: &str| u(c).selector;
        assert_eq!(s("p-2"), ".p-2");
        assert_eq!(s("hover:p-2"), r".hover\:p-2:hover");
        assert_eq!(s("hover:part-thumb:bg-accent"), r".hover\:part-thumb\:bg-accent:hover::part(thumb)");
        assert_eq!(s("part-thumb:hover:bg-accent"), r".part-thumb\:hover\:bg-accent::part(thumb):hover");
        assert_eq!(s("not-hover:p-2"), r".not-hover\:p-2:not(:hover)");
        assert_eq!(s("in-focus-within:p-2"), r":where(*:focus-within) .in-focus-within\:p-2");
        assert_eq!(s("first:p-2"), r".first\:p-2:first-child");
        assert_eq!(s("prop-[orientation=vertical]:p-2"), r#".prop-\[orientation\=vertical\]\:p-2[orientation="vertical"]"#);
        assert_eq!(s("state-[sv.overflow-top]:p-2"), r".state-\[sv\.overflow-top\]\:p-2:state(sv.overflow-top)");
        assert_eq!(s("[&>row]:p-2"), r".\[\&\>row\]\:p-2>row");
        let d = u("dark:hover:bg-surface");
        assert_eq!((d.selector.as_str(), d.media.as_slice(), d.variant_rank), (r".dark\:hover\:bg-surface:hover", &["(prefers-color-scheme: dark)".to_string()][..], 1));
        assert_eq!(u("md:p-2").media, ["(width >= 960px)"]);
        assert!(u("max-md:p-2").variant_rank < u("sm:p-2").variant_rank);
        assert!(u("sm:p-2").variant_rank < u("md:p-2").variant_rank);
        assert_eq!(u("mac:p-2").media, ["(-sv-platform: macos)"]);
    }

    #[test]
    fn prefix() {
        let t = UtilityTokens::builtin();
        let res = Resolver { prefix: Some("tw"), ..Resolver::new(&crate::tables::EGUI, &t) };
        assert_eq!(res.resolve("p-4"), Resolved::NotUtility);
        let Resolved::Utility(u) = res.resolve("tw:hover:p-4") else { panic!() };
        assert_eq!(u.selector, r".tw\:hover\:p-4:hover");
    }
}
