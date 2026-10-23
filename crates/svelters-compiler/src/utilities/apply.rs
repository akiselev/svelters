//! U4: `@apply` in `<style>` (UTILITY-CSS §4.6), expanded *before* scoping with the same resolver
//! the host generates utilities with.
//!
//! `@apply p-2 bg-surface hover:bg-list-hover;` becomes the utilities' declarations, in order:
//! a utility without variants gives its declarations, a state or part variant a nested rule
//! (`&:hover { … }`, `&::part(thumb) { … }`), a media variant a nested `@media … { & { … } }`.
//! `!` on a class, or a trailing `!important`, makes its declarations important. The result is
//! ordinary CSS in the component's own (unlayered) rules, so the cascade is that of hand-written
//! CSS. A name the resolver rejects is `utility_apply_unknown`, an error, with "did you mean".
//!
//! Only for a backend that runs the style engine (egui), and whether or not class utilities are
//! turned on: `@apply` is explicit, and its output needs no run-time support. The embed's prefix,
//! when set, applies (`@apply tw:p-2`).
//!
//! The expansion rewrites the source ([`Expansion::src`]); positions before the `<style>` content
//! are unchanged, and [`Expansion::to_orig`] maps a span of the rewritten text back (a span inside
//! an expansion maps to its whole `@apply` statement).

use super::{APPLY_UNKNOWN, Cx, Token, UtilityCheck, relative_selector};
use crate::diag::Diagnostics;
use crate::semantics::PlatformSemantics;
use crate::svelte::ast::Root;
use std::fmt::Write as _;
use svelters_style::{Resolved, Table};

/// The source with every `@apply` of the `<style>` expanded.
#[derive(Clone, Debug)]
pub struct Expansion {
    pub src: String,
    /// (original span, rewritten span) per `@apply` statement, in order.
    edits: Vec<((u32, u32), (u32, u32))>,
}

impl Expansion {
    /// An offset of the original source (outside every `@apply`) in the rewritten one.
    pub fn to_new(&self, p: u32) -> u32 {
        let mut d: i64 = 0;
        for &((os, oe), (ns, ne)) in &self.edits {
            if oe <= p {
                d += (ne - ns) as i64 - (oe - os) as i64;
            }
        }
        (p as i64 + d) as u32
    }

    /// A span of the rewritten source in the original one.
    pub fn to_orig(&self, span: (u32, u32)) -> (u32, u32) {
        let one = |p: u32, end: bool| {
            let mut d: i64 = 0;
            for &((os, oe), (ns, ne)) in &self.edits {
                if (!end && p >= ns && p < ne) || (end && p > ns && p <= ne) {
                    return if end { oe } else { os };
                }
                if ne <= p {
                    d += (ne - ns) as i64 - (oe - os) as i64;
                }
            }
            (p as i64 - d) as u32
        };
        let s = one(span.0, false);
        (s, one(span.1, true).max(s))
    }

    /// The rewritten `<style>` content.
    pub fn styles(&self, root: &Root) -> &str {
        let c = &root.css.as_ref().expect("an expansion has a <style>").content;
        &self.src[c.start as usize..self.to_new(c.end) as usize]
    }
}

/// Expands the `@apply` statements of the component's `<style>`: `None` without any (or on a
/// backend without the style engine).
pub(crate) fn expand(src: &str, root: &Root, sem: &dyn PlatformSemantics, opts: &UtilityCheck, diags: &mut Diagnostics) -> Option<Expansion> {
    if !sem.style_engine_in_backend() {
        return None;
    }
    let css = root.css.as_ref()?;
    let (start, end) = (css.content.start as usize, css.content.end as usize);
    let stmts = find(src, start, end);
    if stmts.is_empty() {
        return None;
    }
    let table = Table::for_toolkit(sem.backend())?;
    let tokens = opts.utility_tokens();
    let cx = Cx::new(table, &tokens, opts, src);
    let mut out = String::with_capacity(src.len() + stmts.len() * 64);
    let mut edits = Vec::new();
    let mut cur = 0;
    for (s, args, e) in stmts {
        out.push_str(&src[cur..s]);
        let ns = out.len();
        out.push_str(&expansion(&cx, src, args, diags));
        edits.push(((s as u32, e as u32), (ns as u32, out.len() as u32)));
        cur = e;
    }
    out.push_str(&src[cur..]);
    Some(Expansion { src: out, edits })
}

/// The expansion of one statement's arguments (`src[args.0..args.1]`).
fn expansion(cx: &Cx, src: &str, args: (usize, usize), diags: &mut Diagnostics) -> String {
    let text = &src[args.0..args.1];
    let mut words: Vec<(usize, &str)> = Vec::new();
    let mut at = None;
    for (k, c) in text.char_indices().chain(std::iter::once((text.len(), ' '))) {
        if c.is_whitespace() {
            if let Some(s) = at.take() {
                words.push((s, &text[s..k]));
            }
        } else if at.is_none() {
            at = Some(k);
        }
    }
    let important_all = words.last().is_some_and(|w| w.1 == "!important");
    if important_all {
        words.pop();
    }
    let mut pieces: Vec<String> = Vec::new();
    for (off, class) in words {
        let t = Token { class, span: ((args.0 + off) as u32, (args.0 + off + class.len()) as u32), editable: true };
        let u = match cx.resolver.resolve(class) {
            Resolved::Utility(u) => u,
            Resolved::Invalid(i) => {
                cx.report(&i, &t, Some(APPLY_UNKNOWN), diags);
                continue;
            }
            Resolved::NotUtility => {
                diags.error(APPLY_UNKNOWN, t.span, format!("`{class}` is not a utility class"));
                continue;
            }
        };
        for n in &u.notes {
            cx.report(n, &t, None, diags);
        }
        let important = if u.important || important_all { " !important" } else { "" };
        let mut decls = String::new();
        for (p, v) in &u.decls {
            if !decls.is_empty() {
                decls.push(' ');
            }
            let _ = write!(decls, "{p}: {v}{important};");
        }
        let rel = relative_selector(&u);
        let mut piece = if rel == "&" && u.media.is_empty() { decls } else { format!("{rel} {{ {decls} }}") };
        for m in u.media.iter().rev() {
            piece = format!("@media {m} {{ {piece} }}");
        }
        pieces.push(piece);
    }
    pieces.join(" ")
}

/// The `@apply` statements in `src[start..end]`: (statement start, argument range, statement end
/// after its `;`). Comments and strings are skipped.
fn find(src: &str, start: usize, end: usize) -> Vec<(usize, (usize, usize), usize)> {
    let b = &src.as_bytes()[..end];
    let mut out = Vec::new();
    let mut i = start;
    while i < end {
        match b[i] {
            b'/' if b.get(i + 1) == Some(&b'*') => {
                i = src[i + 2..end].find("*/").map_or(end, |k| i + 2 + k + 2);
                continue;
            }
            q @ (b'"' | b'\'') => {
                i += 1;
                while i < end && b[i] != q {
                    i += if b[i] == b'\\' { 2 } else { 1 };
                }
                i += 1;
                continue;
            }
            b'@' if src[i..end].starts_with("@apply") && b.get(i + 6).is_some_and(|c| c.is_ascii_whitespace()) => {
                let args = i + 6;
                let mut k = args;
                while k < end && !matches!(b[k], b';' | b'}') {
                    k += 1;
                }
                let stmt_end = if k < end && b[k] == b';' { k + 1 } else { k };
                out.push((i, (args, k), stmt_end));
                i = stmt_end;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_statements_outside_comments() {
        let src = ".a { /* @apply x; */ @apply p-1 hover:p-2; content: '@apply y;' } .b { @apply p-0 }";
        let f = find(src, 0, src.len());
        assert_eq!(f.len(), 2);
        assert_eq!(&src[f[0].1.0..f[0].1.1], " p-1 hover:p-2");
        assert_eq!(&src[f[0].0..f[0].2], "@apply p-1 hover:p-2;");
        assert_eq!(&src[f[1].1.0..f[1].1.1], " p-0 ");
    }

    #[test]
    fn spans_map_back() {
        let x = Expansion { src: String::new(), edits: vec![((10, 20), (10, 40))] };
        assert_eq!(x.to_new(5), 5);
        assert_eq!(x.to_new(25), 45);
        assert_eq!(x.to_orig((45, 47)), (25, 27));
        assert_eq!(x.to_orig((12, 30)), (10, 20));
        assert_eq!(x.to_orig((2, 4)), (2, 4));
    }
}
