//! DZR2: structural intents (insert, remove, move, wrap, unwrap) and the class and rule an
//! "override here" writes, each laid out like the text around it.
//!
//! Layout rules, all read from the file:
//! - **Block form**: a node alone on its line(s) gets lines of its own at its siblings' indent.
//!   A new or moved node goes on its own line before the sibling it precedes, or after the last
//!   one. Its continuation lines keep their indent relative to its first line.
//! - **Inline form**: a sibling sharing its line with others gets the new node on the same line,
//!   separated by the whitespace the siblings already use between them on one line (else one
//!   space).
//! - **Empty parents**: `<box />` becomes `<box>`, the child one step in, `</box>`; `<box></box>`
//!   gets the child on its own line one step in. The step is the template's own (a child's indent
//!   past its parent's), else the `<style>`'s, else two spaces.

use super::edit::{check_decl_value, check_name, check_property, editable_attr, editable_node, find_attr, indent_step, render, set_attr, Intent, Literal};
use super::{Branch, Design, DesignNode, Editability, NodeKind, Refusal, RefusalKind as K, Why};
use crate::diag::TextEdit;

/// Where a plan's target is.
pub(super) enum Target {
    /// A node or rule of the old text, mapped through the edits.
    Old(u32),
    /// `k` bytes into the text of the edit that inserts at old offset `at`.
    New { at: u32, k: u32 },
    /// The start of the file.
    Zero,
}

pub(super) fn plan(d: &Design, intent: &Intent) -> Result<(Vec<TextEdit>, Target), Refusal> {
    let src = d.source();
    Ok(match intent {
        Intent::Insert { parent, index, markup } => {
            let parent = container(d, *parent)?;
            let markup = check_markup(markup)?.0;
            let (e, k) = insert_at(d, parent, *index, markup, None)?;
            let at = e.start;
            (vec![e], Target::New { at, k })
        }
        Intent::Remove { node } => {
            let i = movable(d, *node)?;
            let n = &d.nodes[i];
            let target = match n.parent {
                Some(p) => Target::Old(d.nodes[p].start),
                None => Target::Zero,
            };
            (vec![removal(src, n)], target)
        }
        Intent::Move { node, parent, index } => {
            let i = movable(d, *node)?;
            let n = &d.nodes[i];
            let p = container(d, *parent)?;
            if let Some(p) = p
                && d.nodes[p].start >= n.start
                && d.nodes[p].start < n.end
            {
                return Err(Refusal::new(K::BadValue, format!("`{}` can't move into itself", n.name), Some((n.start, n.end))));
            }
            // already there: nothing to write
            if p == n.parent && n.branch == Branch::Body && siblings(d, p).iter().position(|&s| s == i) == Some(*index as usize) {
                return Ok((Vec::new(), Target::Old(*node)));
            }
            let own = lead(src, n.start).unwrap_or_else(|| line_indent(src, n.start));
            let text = reindent(&src[n.start as usize..n.end as usize], own, "");
            let (ins, k) = insert_at(d, p, *index, &text, Some(i))?;
            let at = ins.start;
            (vec![removal(src, n), ins], Target::New { at, k })
        }
        Intent::Wrap { first, last, tag, attrs } => wrap(d, *first, *last, tag, attrs)?,
        Intent::Unwrap { node } => unwrap(d, *node)?,
        Intent::AddClass { node, class } => {
            let n = editable_node(d, *node)?;
            if !is_ident(class) {
                return Err(Refusal::new(K::BadValue, format!("`{class}` is not a class name"), None));
            }
            let edits = match find_attr(n, "class") {
                None => set_attr(src, n, "class", &Literal::Str(class.clone()))?,
                Some(a) => {
                    editable_attr(a)?;
                    match &a.literal {
                        Some(Literal::Str(s)) if s.split_whitespace().any(|c| c == class) => Vec::new(),
                        Some(Literal::Str(s)) if !s.trim().is_empty() => set_attr(src, n, "class", &Literal::Str(format!("{} {class}", s.trim_end())))?,
                        Some(Literal::Str(_)) | Some(Literal::Bool(true)) => set_attr(src, n, "class", &Literal::Str(class.clone()))?,
                        _ => return Err(Refusal::new(K::NotEditable(Why::Expression), "`class` is not a string; edit it in code", Some((a.start, a.end)))),
                    }
                }
            };
            (edits, Target::Old(*node))
        }
        Intent::AddRule { selector, decls } => add_rule(d, selector, decls)?,
        _ => unreachable!("v1 intents are planned in edit.rs"),
    })
}

/// Checks a plan's result: the target is there and of the right kind, and the outline grew or
/// shrank by what the intent adds or takes (text nodes aside: they merge and split).
pub(super) fn verify(old: &Design, new: &Design, intent: &Intent, target: u32) -> Result<(), Refusal> {
    let reparse = |m: String| Refusal::new(K::Reparse, m, None);
    let count = |d: &Design| d.nodes.iter().filter(|n| n.kind != NodeKind::Text).count();
    let span_count = |d: &Design, n: &DesignNode| d.nodes.iter().filter(|x| x.kind != NodeKind::Text && x.start >= n.start && x.end <= n.end).count();
    let expect = |want: usize| if count(new) == want { Ok(()) } else { Err(reparse(format!("the outline has {} nodes, not {want}", count(new)))) };
    let at = || new.node_at(target).ok_or_else(|| reparse(format!("no node at the target {target}")));
    let like = |n: &DesignNode, kind: NodeKind, name: &str| {
        if n.kind == kind && n.name == name { Ok(()) } else { Err(reparse(format!("the target is `{}`, not `{name}`", n.name))) }
    };
    match intent {
        Intent::Insert { markup, .. } => {
            let (_, md) = check_markup(markup)?;
            let r = &md.nodes[md.roots[0]];
            like(at()?, r.kind, &r.name)?;
            expect(count(old) + count(&md))
        }
        Intent::Remove { node } => {
            let n = old.node_at(*node).expect("planned");
            if n.parent.is_some() {
                at()?;
            }
            expect(count(old) - span_count(old, n))
        }
        Intent::Move { node, .. } => {
            let n = old.node_at(*node).expect("planned");
            like(at()?, n.kind, &n.name)?;
            expect(count(old))
        }
        Intent::Wrap { first, last, tag, .. } => {
            let (f, l) = (old.node_at(*first).expect("planned"), old.node_at(*last).expect("planned"));
            let sibs = siblings_in(old, f.parent, f.branch);
            let pos = |n: &DesignNode| sibs.iter().position(|&s| old.nodes[s].start == n.start).expect("planned");
            let w = at()?;
            if !matches!(w.kind, NodeKind::Element | NodeKind::Component) || w.name != *tag {
                return Err(reparse(format!("the target is `{}`, not `{tag}`", w.name)));
            }
            if w.children.len() != pos(l) - pos(f) + 1 {
                return Err(reparse(format!("the wrapper has {} children, not {}", w.children.len(), pos(l) - pos(f) + 1)));
            }
            expect(count(old) + 1)
        }
        Intent::Unwrap { node } => {
            let n = old.node_at(*node).expect("planned");
            let c = &old.nodes[n.children[0]];
            like(at()?, c.kind, &c.name)?;
            expect(count(old) - 1)
        }
        Intent::AddClass { class, .. } => match find_attr(at()?, "class").and_then(|a| a.literal.as_ref()) {
            Some(Literal::Str(s)) if s.split_whitespace().any(|c| c == class) => expect(count(old)),
            other => Err(reparse(format!("`class` reads back as {other:?}"))),
        },
        Intent::AddRule { selector, decls } => {
            let r = new.rule_at(target).ok_or_else(|| reparse("no rule at the target".into()))?;
            if r.selector.trim() != selector.trim() || r.why.is_some() {
                return Err(reparse(format!("the rule at the target is `{}`", r.selector)));
            }
            for (p, v) in decls {
                if !r.decls.iter().any(|x| x.property == *p && x.value.trim() == v.trim()) {
                    return Err(reparse(format!("`{p}` does not read back")));
                }
            }
            if new.rules.len() != old.rules.len() + 1 {
                return Err(reparse("the rule count did not grow by one".into()));
            }
            Ok(())
        }
        _ => unreachable!("v1 intents are verified in edit.rs"),
    }
}

// ---- lines ----

/// The start of the line holding `off`.
fn ls(src: &str, off: u32) -> u32 {
    src[..off as usize].rfind('\n').map_or(0, |i| i as u32 + 1)
}

/// The end of the line holding `off` (its `\n`, or the end of the file).
fn le(src: &str, off: u32) -> u32 {
    src[off as usize..].find('\n').map_or(src.len() as u32, |i| off + i as u32)
}

/// The whitespace before `off` on its line, when only whitespace is.
fn lead(src: &str, off: u32) -> Option<&str> {
    let s = &src[ls(src, off) as usize..off as usize];
    s.chars().all(|c| c == ' ' || c == '\t').then_some(s)
}

/// The leading whitespace of the line holding `off`.
fn line_indent(src: &str, off: u32) -> &str {
    let line = &src[ls(src, off) as usize..];
    &line[..line.len() - line.trim_start_matches([' ', '\t']).len()]
}

/// True when only whitespace follows `off` on its line.
fn tail_blank(src: &str, off: u32) -> bool {
    src[off as usize..le(src, off) as usize].trim().is_empty()
}

/// `text`'s continuation lines with `from` taken off their start and `to` put on (blank lines
/// stay empty).
fn reindent(text: &str, from: &str, to: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for (k, line) in text.split('\n').enumerate() {
        if k == 0 {
            out.push_str(line);
            continue;
        }
        out.push('\n');
        let line = line.strip_prefix(from).unwrap_or(line);
        if !line.trim().is_empty() {
            out.push_str(to);
            out.push_str(line);
        }
    }
    out
}

/// The template's indent step: a child's indent past its parent's line, else the `<style>`'s.
fn markup_step(d: &Design) -> String {
    let src = d.source();
    for n in &d.nodes {
        let Some(p) = n.parent else { continue };
        let p = &d.nodes[p];
        if let Some(ci) = lead(src, n.start) {
            let pi = line_indent(src, p.start);
            if ls(src, n.start) != ls(src, p.start) && ci.len() > pi.len() && ci.starts_with(pi) {
                return ci[pi.len()..].to_string();
            }
        }
    }
    indent_step(src)
}

// ---- nodes ----

fn index_at(d: &Design, offset: u32) -> Result<usize, Refusal> {
    d.nodes.iter().position(|n| n.start == offset).ok_or_else(|| Refusal::new(K::NoNode, format!("no node starts at offset {offset}"), None))
}

/// A node that may be removed, moved or wrapped: editable or structural.
fn movable(d: &Design, offset: u32) -> Result<usize, Refusal> {
    let i = index_at(d, offset)?;
    let n = &d.nodes[i];
    if n.class == Editability::Opaque {
        let why = n.why.unwrap_or(Why::Special);
        return Err(Refusal::new(K::NotEditable(why), format!("`{}` is not part of the layout; edit it in code", n.name), Some((n.start, n.end))));
    }
    Ok(i)
}

/// A parent children may be inserted into or moved into: an element or component, editable or
/// structural only for a spread. `None`: the top level.
fn container(d: &Design, parent: Option<u32>) -> Result<Option<usize>, Refusal> {
    let Some(off) = parent else { return Ok(None) };
    let i = index_at(d, off)?;
    let n = &d.nodes[i];
    let ok = matches!(n.kind, NodeKind::Element | NodeKind::Component)
        && n.name_end.is_some()
        && (n.class == Editability::Editable || (n.class == Editability::Structural && n.why == Some(Why::Spread)));
    if ok {
        Ok(Some(i))
    } else {
        let why = n.why.unwrap_or(Why::Special);
        Err(Refusal::new(K::NotEditable(why), format!("`{}` can't take children from the designer; edit it in code", n.name), Some((n.start, n.end))))
    }
}

/// The body children of `parent` (or the top-level nodes).
fn siblings(d: &Design, parent: Option<usize>) -> Vec<usize> {
    siblings_in(d, parent, Branch::Body)
}

fn siblings_in(d: &Design, parent: Option<usize>, branch: Branch) -> Vec<usize> {
    match parent {
        Some(p) => d.nodes[p].children.iter().copied().filter(|&c| d.nodes[c].branch == branch).collect(),
        None => d.roots.clone(),
    }
}

/// `markup`, trimmed, when it is exactly one element or component; and its parse.
fn check_markup(markup: &str) -> Result<(&str, Design<'_>), Refusal> {
    let m = markup.trim();
    let bad = |why: &str| Refusal::new(K::BadValue, format!("the markup {why}"), None);
    let md = Design::parse(m).map_err(|r| Refusal::new(K::BadValue, format!("the markup does not parse: {}", r.message), None))?;
    match md.roots.as_slice() {
        [r] if md.nodes[*r].start == 0 && md.nodes[*r].end as usize == m.len() && md.style.is_none() => {
            if matches!(md.nodes[*r].kind, NodeKind::Element | NodeKind::Component) { Ok((m, md)) } else { Err(bad("is not an element or a component")) }
        }
        _ => Err(bad("is not exactly one element or component")),
    }
}

/// The whitespace the siblings use between them on one line, else one space.
fn inline_sep(d: &Design, sibs: &[usize]) -> String {
    let src = d.source();
    for w in sibs.windows(2) {
        let g = &src[d.nodes[w[0]].end as usize..d.nodes[w[1]].start as usize];
        if !g.is_empty() && g.trim().is_empty() && !g.contains('\n') {
            return g.to_string();
        }
    }
    " ".into()
}

/// The edit inserting `markup` (continuation lines indented from column 0) as child `index` of
/// `parent`, leaving out `exclude` (a node being moved); and where the markup starts in its text.
fn insert_at(d: &Design, parent: Option<usize>, index: u32, markup: &str, exclude: Option<usize>) -> Result<(TextEdit, u32), Refusal> {
    let src = d.source();
    let sibs: Vec<usize> = siblings(d, parent).into_iter().filter(|&s| Some(s) != exclude).collect();
    let index = index as usize;
    if index > sibs.len() {
        return Err(Refusal::new(K::BadValue, format!("index {index} is past the {} children", sibs.len()), None));
    }
    let ins = |at: u32, text: String, k: usize| Ok((TextEdit { start: at, end: at, text }, k as u32));
    if let Some(&s) = sibs.get(index) {
        let s = &d.nodes[s];
        if let Some(ind) = lead(src, s.start) {
            // on its own line, above the sibling
            return ins(ls(src, s.start), format!("{ind}{}\n", reindent(markup, "", ind)), ind.len());
        }
        let sep = inline_sep(d, &sibs);
        return ins(s.start, format!("{markup}{sep}"), 0);
    }
    if let Some(&l) = sibs.last() {
        let l = &d.nodes[l];
        if tail_blank(src, l.end) {
            let ind = lead(src, l.start).unwrap_or_else(|| line_indent(src, l.start));
            return ins(l.end, format!("\n{ind}{}", reindent(markup, "", ind)), 1 + ind.len());
        }
        let sep = inline_sep(d, &sibs);
        return ins(l.end, format!("{sep}{markup}"), sep.len());
    }
    let Some(p) = parent else {
        // an empty template: before the `<style>`, else at the end
        if let Some(st) = &d.style {
            return ins(st.start, format!("{markup}\n\n"), 0);
        }
        let pre = if src.is_empty() {
            ""
        } else if src.ends_with('\n') {
            "\n"
        } else {
            "\n\n"
        };
        return ins(src.len() as u32, format!("{pre}{markup}\n"), pre.len());
    };
    let p = &d.nodes[p];
    let ind = line_indent(src, p.start);
    let inner = format!("{ind}{}", markup_step(d));
    let m = reindent(markup, "", &inner);
    let tag = &src[p.start as usize..p.end as usize];
    if tag.ends_with("/>") {
        // `<box />` → `<box>`, the child, `</box>`
        let slash = p.end - 2;
        let before = &src[..slash as usize];
        let ws = (before.len() - before.trim_end().len()) as u32;
        let from = if src[(slash - ws) as usize..slash as usize].contains('\n') { slash } else { slash - ws };
        let text = format!(">\n{inner}{m}\n{ind}</{}>", p.name);
        return Ok((TextEdit { start: from, end: p.end, text }, 2 + inner.len() as u32));
    }
    let close = p.start + tag.rfind("</").ok_or_else(|| Refusal::new(K::Reparse, format!("`{}` has no closing tag", p.name), None))? as u32;
    let open_end = src[..close as usize].trim_end().len() as u32;
    Ok((TextEdit { start: open_end, end: close, text: format!("\n{inner}{m}\n{ind}") }, 1 + inner.len() as u32))
}

/// The edit deleting `n`: its whole line when it is alone on it, else it and the whitespace
/// before it (or after it, when it starts its line).
fn removal(src: &str, n: &DesignNode) -> TextEdit {
    let gone = |start: u32, end: u32| TextEdit { start, end, text: String::new() };
    match (lead(src, n.start), tail_blank(src, n.end)) {
        (Some(_), true) => {
            let (a, e) = (ls(src, n.start), le(src, n.end));
            if (e as usize) < src.len() {
                gone(a, e + 1)
            } else if a > 0 {
                gone(a - 1, e)
            } else {
                gone(a, e)
            }
        }
        (None, _) => {
            let before = &src[ls(src, n.start) as usize..n.start as usize];
            gone(n.start - (before.len() - before.trim_end().len()) as u32, n.end)
        }
        (Some(_), false) => {
            let after = &src[n.end as usize..le(src, n.end) as usize];
            gone(n.start, n.end + (after.len() - after.trim_start().len()) as u32)
        }
    }
}

fn is_tag(t: &str) -> bool {
    t.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) && t.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// A CSS class name (ASCII identifier).
fn is_ident(c: &str) -> bool {
    let body = c.strip_prefix('-').unwrap_or(c);
    body.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_') && body.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn wrap(d: &Design, first: u32, last: u32, tag: &str, attrs: &[(String, Literal)]) -> Result<(Vec<TextEdit>, Target), Refusal> {
    let src = d.source();
    let (fi, li) = (movable(d, first)?, movable(d, last)?);
    let (f, l) = (&d.nodes[fi], &d.nodes[li]);
    if f.parent != l.parent || f.branch != l.branch {
        return Err(Refusal::new(K::BadValue, "the nodes to wrap are not siblings in one branch", None));
    }
    let sibs = siblings_in(d, f.parent, f.branch);
    let (a, b) = (sibs.iter().position(|&s| s == fi).expect("a sibling"), sibs.iter().position(|&s| s == li).expect("a sibling"));
    if a > b {
        return Err(Refusal::new(K::BadValue, "`first` comes after `last`", None));
    }
    for &s in &sibs[a..=b] {
        movable(d, d.nodes[s].start)?;
    }
    if !is_tag(tag) {
        return Err(Refusal::new(K::BadValue, format!("`{tag}` is not a tag name"), None));
    }
    let mut open = format!("<{tag}");
    for (name, value) in attrs {
        check_name(name)?;
        open.push_str(&format!(" {name}={}", render(value, '"')?));
    }
    open.push('>');
    let close = format!("</{tag}>");
    match lead(src, f.start) {
        Some(ind) if tail_blank(src, l.end) => {
            // the lines from `first` to `last`, one step in, between the tags on lines of their own
            let step = markup_step(d);
            let (from, to) = (ls(src, f.start), le(src, l.end));
            let mut edits = vec![TextEdit { start: from, end: from, text: format!("{ind}{open}\n{step}") }];
            for (k, _) in src[from as usize..to as usize].match_indices('\n') {
                let at = from + k as u32 + 1;
                if !src[at as usize..le(src, at) as usize].trim().is_empty() {
                    edits.push(TextEdit { start: at, end: at, text: step.clone() });
                }
            }
            edits.push(TextEdit { start: to, end: to, text: format!("\n{ind}{close}") });
            Ok((edits, Target::New { at: from, k: ind.len() as u32 }))
        }
        _ => Ok((
            vec![TextEdit { start: f.start, end: f.start, text: open }, TextEdit { start: l.end, end: l.end, text: close }],
            Target::New { at: f.start, k: 0 },
        )),
    }
}

fn unwrap(d: &Design, node: u32) -> Result<(Vec<TextEdit>, Target), Refusal> {
    let src = d.source();
    let n = &d.nodes[index_at(d, node)?];
    if n.kind == NodeKind::Component {
        return Err(Refusal::new(K::BadValue, format!("`<{}>` is a component: its children are its content", n.name), Some((n.start, n.end))));
    }
    if n.kind != NodeKind::Element || n.class != Editability::Editable {
        let why = n.why.unwrap_or(Why::Special);
        return Err(Refusal::new(K::NotEditable(why), format!("`{}` can't be unwrapped; edit it in code", n.name), Some((n.start, n.end))));
    }
    let Some(&first) = n.children.first() else {
        return Err(Refusal::new(K::BadValue, format!("`<{}>` has no children to keep; remove it instead", n.name), Some((n.start, n.end))));
    };
    let after = n.attrs.iter().map(|a| a.end).chain(n.name_end).max().expect("an element has a name");
    let rest = &src[after as usize..];
    let open_end = after + (rest.len() - rest.trim_start().len()) as u32;
    if src.as_bytes().get(open_end as usize) != Some(&b'>') {
        return Err(Refusal::new(K::Reparse, format!("`<{}>`'s open tag does not end in `>`", n.name), None));
    }
    let open_end = open_end + 1;
    let close = n.start + src[n.start as usize..n.end as usize].rfind("</").expect("an element with children closes") as u32;
    let gone = |start: u32, end: u32| TextEdit { start, end, text: String::new() };
    let target = Target::Old(d.nodes[first].start);
    match (lead(src, n.start), lead(src, close)) {
        (Some(ind), Some(_)) if tail_blank(src, open_end) && tail_blank(src, n.end) => {
            let step = match lead(src, d.nodes[first].start) {
                Some(ci) if ci.len() > ind.len() && ci.starts_with(ind) => ci[ind.len()..].to_string(),
                _ => markup_step(d),
            };
            let r1 = (ls(src, n.start), le(src, open_end) + 1);
            let (cl, ce) = (ls(src, close), le(src, n.end));
            let r2 = if (ce as usize) < src.len() { (cl, ce + 1) } else { (cl, ce) };
            let mut edits = vec![gone(r1.0, r1.1)];
            let mut at = r1.1;
            while at < r2.0 {
                if src[at as usize..].starts_with(&step) {
                    edits.push(gone(at, at + step.len() as u32));
                }
                at = le(src, at) + 1;
            }
            edits.push(gone(r2.0, r2.1));
            Ok((edits, target))
        }
        _ => Ok((vec![gone(n.start, open_end), gone(close, n.end)], target)),
    }
}

fn add_rule(d: &Design, selector: &str, decls: &[(String, String)]) -> Result<(Vec<TextEdit>, Target), Refusal> {
    let src = d.source();
    let sel = selector.trim();
    if sel.is_empty() || sel.contains(['{', '}', ';', '\n', '<']) || sel.contains("/*") {
        return Err(Refusal::new(K::BadValue, format!("`{selector}` can't be written as a selector"), None));
    }
    // the selector the designer edits: the same check `Design::parse` gives each rule
    let probe = format!("<style>\n{sel} {{}}\n</style>\n");
    let pd = Design::parse(&probe).map_err(|r| Refusal::new(K::BadValue, format!("`{sel}` does not parse as a selector: {}", r.message), None))?;
    match pd.rules.as_slice() {
        [r] if r.why.is_none() => {}
        [r] => return Err(Refusal::new(K::NotEditable(r.why.unwrap_or(Why::Selector)), format!("`{sel}` is a selector the designer doesn't edit"), None)),
        _ => return Err(Refusal::new(K::BadValue, format!("`{sel}` is not one rule's selector"), None)),
    }
    for (p, v) in decls {
        check_property(p)?;
        check_decl_value(v)?;
    }
    // the declaration step of a multi-line rule, else the file's
    let ds = d
        .rules
        .iter()
        .filter(|r| r.why.is_none() && src[r.block_start as usize..r.block_end as usize].contains('\n'))
        .find_map(|r| {
            let ri = line_indent(src, r.start);
            let di = lead(src, r.decls.first()?.start)?;
            (di.len() > ri.len() && di.starts_with(ri)).then(|| di[ri.len()..].to_string())
        })
        .unwrap_or_else(|| indent_step(src));
    let body = |ri: &str| {
        let mut b = format!("{sel} {{\n");
        for (p, v) in decls {
            b.push_str(&format!("{ri}{ds}{p}: {};\n", v.trim()));
        }
        b.push_str(ri);
        b.push('}');
        b
    };
    let ins = |at: u32, end: u32, text: String, k: usize| Ok((vec![TextEdit { start: at, end, text }], Target::New { at, k: k as u32 }));
    let Some(st) = &d.style else {
        let pre = if src.is_empty() {
            ""
        } else if src.ends_with('\n') {
            "\n"
        } else {
            "\n\n"
        };
        let s = indent_step(src);
        return ins(src.len() as u32, src.len() as u32, format!("{pre}<style>\n{s}{}\n</style>\n", body(&s)), pre.len() + "<style>\n".len() + s.len());
    };
    if let Some(&(last_start, last_end)) = st.tops.last() {
        let ri = line_indent(src, last_start).to_string();
        let blank = st.tops.len() >= 2 && {
            let (_, prev_end) = st.tops[st.tops.len() - 2];
            src[prev_end as usize..last_start as usize].matches('\n').count() >= 2
        };
        let gap = if blank { "\n\n" } else { "\n" };
        return ins(last_end, last_end, format!("{gap}{ri}{}", body(&ri)), gap.len() + ri.len());
    }
    // a `<style>` with no rules: one step in
    let ri = indent_step(src);
    let content = &src[st.content_start as usize..st.content_end as usize];
    if content.trim().is_empty() {
        return ins(st.content_start, st.content_end, format!("\n{ri}{}\n", body(&ri)), 1 + ri.len());
    }
    let at = st.content_start + content.trim_end().len() as u32;
    ins(at, at, format!("\n{ri}{}", body(&ri)), 1 + ri.len())
}
