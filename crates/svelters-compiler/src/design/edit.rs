//! Intents to edits: one attribute or declaration at a time, written in the file's own style.

use super::{apply, Design, DesignAttr, DesignNode, DesignRule, Editability, Refusal, RefusalKind as K, Why};
use crate::diag::TextEdit;

/// A literal attribute value.
#[derive(Clone, Debug, PartialEq)]
pub enum Literal {
    Str(String),
    Num(f64),
    Bool(bool),
}

/// One designer edit. Nodes are named by their start offset, rules by their selector's.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Intent {
    /// Sets an attribute (or a `style:` directive) to a literal, adding it after the last one if
    /// absent.
    SetAttr { node: u32, name: String, value: Literal },
    /// Removes an attribute.
    ClearAttr { node: u32, name: String },
    /// Sets a declaration in a rule, adding it after the last one if absent.
    SetDecl { rule: u32, property: String, value: String },
    /// Removes every declaration of the property from a rule.
    ClearDecl { rule: u32, property: String },
}

/// What an intent changes.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    /// Sorted, not overlapping, in the old file's offsets; empty when the file already says it.
    pub edits: Vec<TextEdit>,
    /// The file after the edits.
    pub text: String,
    /// Where the target (node or rule) starts in the new file.
    pub target: u32,
}

/// Maps an offset of the old text through `edits` (sorted, not overlapping). `None` when the
/// edits replace the byte at `offset`; an insertion exactly at `offset` goes before it.
pub fn rebase(offset: u32, edits: &[TextEdit]) -> Option<u32> {
    let mut shift: i64 = 0;
    for e in edits {
        if e.end <= offset && (e.start < offset || e.start == e.end) {
            shift += e.text.len() as i64 - (e.end - e.start) as i64;
        } else if e.start <= offset && offset < e.end {
            return None;
        }
    }
    Some((offset as i64 + shift) as u32)
}

pub(super) fn plan(d: &Design, intent: &Intent) -> Result<Plan, Refusal> {
    let src = d.source();
    let (edits, target) = match intent {
        Intent::SetAttr { node, name, value } => {
            let n = editable_node(d, *node)?;
            check_name(name)?;
            (set_attr(src, n, name, value)?, *node)
        }
        Intent::ClearAttr { node, name } => {
            let n = editable_node(d, *node)?;
            let a = find_attr(n, name).ok_or_else(|| missing_attr(n, name))?;
            editable_attr(a)?;
            let from = prev_end(n, a);
            (vec![TextEdit { start: from, end: a.end, text: String::new() }], *node)
        }
        Intent::SetDecl { rule, property, value } => {
            let r = editable_rule(d, *rule)?;
            check_property(property)?;
            check_decl_value(value)?;
            (set_decl(src, r, property, value), *rule)
        }
        Intent::ClearDecl { rule, property } => {
            let r = editable_rule(d, *rule)?;
            let hits: Vec<_> = r.decls.iter().filter(|x| x.property == *property).collect();
            if hits.is_empty() {
                return Err(Refusal::new(K::Missing, format!("`{}` has no `{property}` declaration", r.selector), Some((r.start, r.end))));
            }
            let edits = hits
                .iter()
                .map(|x| {
                    let mut end = x.end as usize;
                    let rest = &src[end..r.block_end as usize];
                    let ws = rest.len() - rest.trim_start().len();
                    if rest[ws..].starts_with(';') {
                        end += ws + 1;
                    }
                    let before = &src[r.block_start as usize + 1..x.start as usize];
                    let from = x.start as usize - (before.len() - before.trim_end().len());
                    TextEdit { start: from as u32, end: end as u32, text: String::new() }
                })
                .collect();
            (edits, *rule)
        }
    };
    let edits: Vec<TextEdit> = edits.into_iter().filter(|e| src[e.start as usize..e.end as usize] != e.text).collect();
    let text = apply(src, &edits);
    let target = rebase(target, &edits).ok_or_else(|| Refusal::new(K::Reparse, "the edit replaced its own target", None))?;
    verify(&text, intent, target)?;
    Ok(Plan { edits, text, target })
}

/// Parses the result and checks the target is still there and says what the intent asked.
fn verify(text: &str, intent: &Intent, target: u32) -> Result<(), Refusal> {
    let reparse = |m: String| Refusal::new(K::Reparse, m, None);
    let d = Design::parse(text).map_err(|r| reparse(format!("the edited file does not parse: {}", r.message)))?;
    match intent {
        Intent::SetAttr { name, value, .. } => {
            let n = d.node_at(target).ok_or_else(|| reparse("the node moved".into()))?;
            match find_attr(n, name).and_then(|a| a.literal.as_ref()) {
                Some(l) if same(l, value) => Ok(()),
                other => Err(reparse(format!("`{name}` reads back as {other:?}"))),
            }
        }
        Intent::ClearAttr { name, .. } => {
            let n = d.node_at(target).ok_or_else(|| reparse("the node moved".into()))?;
            if find_attr(n, name).is_some() { Err(reparse(format!("`{name}` is still there"))) } else { Ok(()) }
        }
        Intent::SetDecl { property, value, .. } => {
            let r = d.rule_at(target).ok_or_else(|| reparse("the rule moved".into()))?;
            match r.decls.iter().rev().find(|x| x.property == *property) {
                Some(x) if x.value.trim() == value.trim() => Ok(()),
                other => Err(reparse(format!("`{property}` reads back as {:?}", other.map(|x| &x.value)))),
            }
        }
        Intent::ClearDecl { property, .. } => {
            let r = d.rule_at(target).ok_or_else(|| reparse("the rule moved".into()))?;
            if r.decls.iter().any(|x| x.property == *property) { Err(reparse(format!("`{property}` is still there"))) } else { Ok(()) }
        }
    }
}

fn same(a: &Literal, b: &Literal) -> bool {
    match (a, b) {
        (Literal::Num(x), Literal::Num(y)) => x == y || (x - y).abs() <= f64::EPSILON * x.abs().max(1.0),
        _ => a == b,
    }
}

fn editable_node<'d>(d: &'d Design, offset: u32) -> Result<&'d DesignNode, Refusal> {
    let n = d.node_at(offset).ok_or_else(|| Refusal::new(K::NoNode, format!("no node starts at offset {offset}"), None))?;
    if !matches!(n.kind, super::NodeKind::Element | super::NodeKind::Component) || n.name_end.is_none() {
        return Err(Refusal::new(K::NotEditable(n.why.unwrap_or(Why::Special)), format!("`{}` has no attributes to edit", n.name), Some((n.start, n.end))));
    }
    if n.class != Editability::Editable {
        let why = n.why.unwrap_or(Why::Special);
        let what = match why {
            Why::Spread => "a spread may override any of its attributes",
            Why::Dynamic => "its kind is computed",
            _ => "it is not editable",
        };
        return Err(Refusal::new(K::NotEditable(why), format!("`<{}>`: {what}; edit it in code", n.name), Some((n.start, n.end))));
    }
    Ok(n)
}

fn editable_attr(a: &DesignAttr) -> Result<(), Refusal> {
    match a.why {
        None => Ok(()),
        Some(why) => {
            let what = match why {
                Why::Binding => "a binding",
                Why::Interpolated => "an interpolated string",
                Why::Directive => "a directive",
                _ => "an expression",
            };
            Err(Refusal::new(K::NotEditable(why), format!("`{}` is {what}; edit it in code", a.name), Some((a.start, a.end))))
        }
    }
}

fn editable_rule<'d>(d: &'d Design, offset: u32) -> Result<&'d DesignRule, Refusal> {
    let r = d.rule_at(offset).ok_or_else(|| Refusal::new(K::NoRule, format!("no rule's selector starts at offset {offset}"), None))?;
    match r.why {
        None => Ok(r),
        Some(why) => {
            let what = if why == Why::AtRule { "inside an at-rule or another rule" } else { "a selector the designer doesn't edit" };
            Err(Refusal::new(K::NotEditable(why), format!("`{}` is {what}; edit it in code", r.selector), Some((r.start, r.end))))
        }
    }
}

fn find_attr<'n>(n: &'n DesignNode, name: &str) -> Option<&'n DesignAttr> {
    n.attrs.iter().find(|a| a.name == name)
}

fn missing_attr(n: &DesignNode, name: &str) -> Refusal {
    Refusal::new(K::Missing, format!("`<{}>` has no `{name}`", n.name), Some((n.start, n.end)))
}

/// Where the separator before `a` starts: the end of the attribute before it, or of the name.
fn prev_end(n: &DesignNode, a: &DesignAttr) -> u32 {
    n.attrs.iter().filter(|x| x.end <= a.start).map(|x| x.end).max().unwrap_or(n.name_end.unwrap_or(a.start))
}

fn check_name(name: &str) -> Result<(), Refusal> {
    let body = name.strip_prefix("style:").unwrap_or(name);
    let ok = !body.is_empty()
        && body.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '-' || c == '_')
        && body.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if ok && !name.contains("::") { Ok(()) } else { Err(Refusal::new(K::BadValue, format!("`{name}` is not an attribute name the designer writes"), None)) }
}

fn check_property(p: &str) -> Result<(), Refusal> {
    let ok = !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') && !p.starts_with(|c: char| c.is_ascii_digit());
    if ok { Ok(()) } else { Err(Refusal::new(K::BadValue, format!("`{p}` is not a property name"), None)) }
}

fn check_decl_value(v: &str) -> Result<(), Refusal> {
    if v.trim().is_empty() || v.contains([';', '{', '}']) || v.contains("/*") || v.contains('\n') {
        Err(Refusal::new(K::BadValue, format!("`{v}` can't be written as one declaration value"), None))
    } else {
        Ok(())
    }
}

/// `12`, `0.5`, `-4`.
fn num(n: f64) -> Result<String, Refusal> {
    if !n.is_finite() {
        return Err(Refusal::new(K::BadValue, format!("{n} is not a finite number"), None));
    }
    Ok(if n == n.trunc() && n.abs() < 1e15 { format!("{}", n as i64) } else { format!("{n}") })
}

/// The value as source: `"text"` (with `quote`), `{12}`, `{true}`.
fn render(value: &Literal, quote: char) -> Result<String, Refusal> {
    Ok(match value {
        Literal::Num(n) => format!("{{{}}}", num(*n)?),
        Literal::Bool(b) => format!("{{{b}}}"),
        Literal::Str(s) => {
            // `{` starts an expression and `&` an entity in attribute text: write a JS string then
            let other = if quote == '"' { '\'' } else { '"' };
            if !s.contains(['{', '}', '&']) && !s.contains(quote) {
                format!("{quote}{s}{quote}")
            } else if !s.contains(['{', '}', '&']) && !s.contains(other) {
                format!("{other}{s}{other}")
            } else {
                format!("{{{}}}", serde_json::to_string(s).expect("a string serializes"))
            }
        }
    })
}

fn set_attr(src: &str, n: &DesignNode, name: &str, value: &Literal) -> Result<Vec<TextEdit>, Refusal> {
    if let Some(a) = find_attr(n, name) {
        editable_attr(a)?;
        let text = &src[a.start as usize..a.end as usize];
        let Some(eq) = text.find('=') else {
            // bare `disabled`
            return Ok(match value {
                Literal::Bool(true) => Vec::new(),
                v => vec![TextEdit { start: a.end, end: a.end, text: format!("={}", render(v, '"')?) }],
            });
        };
        let rest = &text[eq + 1..];
        let lead = rest.len() - rest.trim_start().len();
        let start = a.start + (eq + 1 + lead) as u32;
        let quote = rest.trim_start().chars().next().filter(|c| matches!(c, '"' | '\'')).unwrap_or('"');
        return Ok(vec![TextEdit { start, end: a.end, text: render(value, quote)? }]);
    }
    let attr = format!("{name}={}", render(value, house_quote(src, n))?);
    let Some(last) = n.attrs.iter().max_by_key(|a| a.end) else {
        let at = n.name_end.expect("checked by editable_node");
        return Ok(vec![TextEdit { start: at, end: at, text: format!(" {attr}") }]);
    };
    // the separator the tag already uses between attributes
    let first = n.attrs.iter().min_by_key(|a| a.start).expect("there is a last");
    let sep = if n.attrs.len() >= 2 {
        let before_last = n.attrs.iter().filter(|a| a.end <= last.start).map(|a| a.end).max().expect("two attributes");
        &src[before_last as usize..last.start as usize]
    } else {
        &src[n.name_end.expect("checked") as usize..first.start as usize]
    };
    let sep = if sep.trim().is_empty() && !sep.is_empty() { sep.to_string() } else { " ".to_string() };
    Ok(vec![TextEdit { start: last.end, end: last.end, text: format!("{sep}{attr}") }])
}

/// The quote the tag's quoted attributes use, `"` when none is quoted.
fn house_quote(src: &str, n: &DesignNode) -> char {
    for a in &n.attrs {
        let t = &src[a.start as usize..a.end as usize];
        if let Some(eq) = t.find('=')
            && let Some(q @ ('"' | '\'')) = t[eq + 1..].trim_start().chars().next()
        {
            return q;
        }
    }
    '"'
}

fn set_decl(src: &str, r: &DesignRule, property: &str, value: &str) -> Vec<TextEdit> {
    let value = value.trim();
    if let Some(x) = r.decls.iter().rev().find(|x| x.property == property) {
        return vec![TextEdit { start: x.value_start, end: x.end, text: value.to_string() }];
    }
    let decl = format!("{property}: {value}");
    let Some(last) = r.decls.last() else {
        // an empty block: one declaration on its own line, indented one step from the rule
        let line_start = src[..r.start as usize].rfind('\n').map_or(0, |i| i + 1);
        let indent: String = src[line_start..r.start as usize].chars().take_while(|c| c.is_whitespace()).collect();
        let step = indent_step(src);
        return vec![TextEdit { start: r.block_start + 1, end: r.block_end - 1, text: format!("\n{indent}{step}{decl};\n{indent}") }];
    };
    // after the last declaration (and its `;`), with the separator before it
    let before = &src[r.block_start as usize + 1..last.start as usize];
    let sep = &before[before.trim_end().len()..];
    let sep = if sep.is_empty() { " " } else { sep };
    let rest = &src[last.end as usize..r.block_end as usize];
    let ws = rest.len() - rest.trim_start().len();
    if rest[ws..].starts_with(';') {
        let at = last.end + (ws + 1) as u32;
        vec![TextEdit { start: at, end: at, text: format!("{sep}{decl};") }]
    } else {
        vec![TextEdit { start: last.end, end: last.end, text: format!(";{sep}{decl}") }]
    }
}

/// The file's indentation step: from another rule's first declaration, else two spaces.
fn indent_step(src: &str) -> String {
    let mut lines = src.lines();
    while let Some(l) = lines.next() {
        if l.trim_end().ends_with('{') {
            let outer: String = l.chars().take_while(|c| c.is_whitespace()).collect();
            if let Some(next) = lines.next() {
                let inner: String = next.chars().take_while(|c| c.is_whitespace()).collect();
                if inner.len() > outer.len() && inner.starts_with(&outer) && !next.trim().is_empty() {
                    return inner[outer.len()..].to_string();
                }
            }
        }
    }
    "  ".into()
}
