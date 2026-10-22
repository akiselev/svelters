//! CS5g: compile-time checking of `<style>` for a backend that runs the CSS engine itself
//! (`style.engine: "backend"`, egui). Its rules are not lowered (D9, CS5e), so the compiler runs
//! the engine's own parser and table check (`svelters_style::Stylesheet::parse`, then
//! `Stylist::add_sheet` against the backend's generated table) on the component's CSS and maps the
//! engine's diagnostics to `.svelte` byte spans:
//!
//! - a bad value (or a rule setting an inline-only property): error `style_bad_value`;
//! - a property the backend's table does not know: warning `style_unknown_declaration` (another
//!   backend may have it; the engine ignores it);
//! - a syntax error (a rule the engine drops): error `style_syntax`; an at-rule the engine ignores:
//!   warning `style_unsupported_at_rule`.
//!
//! Svelte's own syntax is blanked first, keeping every position: `:global(x)` keeps `x`, a bare
//! `:global` goes (a `:global { … }` block becomes `* { … }`). `:state(ns.name)` accepts any name
//! (provider states register at run time). A parse diagnostic on a line the compiler's own `<style>`
//! parser already reported on is skipped.

use crate::diag::Diagnostics;
use crate::semantics::{PlatformSemantics, codes};
use crate::svelte::ast::Root;
use svelters_style::{Origin, ParseOptions, Severity, StateMask, StateNames, Stylesheet, Stylist, Table};

/// Any `:state(ns.name)` parses: provider states are registered at run time.
struct AnyState;

impl StateNames for AnyState {
    fn lookup(&self, _name: &str) -> Option<StateMask> {
        Some(StateMask(1 << 32))
    }
}

/// Checks the component's `<style>` with the engine and reports into `diags`.
pub(super) fn check(root: &Root, sem: &dyn PlatformSemantics, diags: &mut Diagnostics) {
    let Some(css) = &root.css else { return };
    let Some(table) = Table::for_toolkit(sem.backend()) else { return };
    let text = blank_svelte_syntax(&css.content.styles);
    let sheet = Stylesheet::parse(&text, &ParseOptions { states: Some(&AnyState) });
    let (_, table_diags) = Stylist::new(table).add_sheet(&sheet, Origin::Author);
    let src = &css.content.styles;
    let base = css.content.start;
    // lines the compiler's own `<style>` parser already reported on
    let reported: Vec<u32> = diags.list.iter().filter(|d| d.code.starts_with("style_") && d.start >= base && d.start <= css.content.end).map(|d| d.start).collect();
    for d in &sheet.diagnostics {
        let Some(span) = span_of(src, base, d.line, d.column) else { continue };
        let (ls, le) = line_bounds(src, base, span.0);
        if reported.iter().any(|&s| s >= ls && s <= le) {
            continue;
        }
        match d.severity {
            Severity::Error => diags.error(crate::style::SYNTAX, span, d.message.clone()),
            Severity::Warning => diags.warning(crate::style::UNSUPPORTED_AT_RULE, span, d.message.clone()),
        };
    }
    for d in &table_diags {
        let Some(span) = span_of(src, base, d.line, d.column) else { continue };
        match d.severity {
            Severity::Error => diags.native_error(codes::STYLE_BAD_VALUE, span, d.message.clone()),
            Severity::Warning => diags.native_warning(codes::STYLE_UNKNOWN_DECLARATION, span, d.message.clone()),
        };
    }
}

/// `:global(x)` → `        x `, `:global` → spaces (`*` when a `{` follows), same length.
fn blank_svelte_syntax(css: &str) -> String {
    const G: &str = ":global";
    let b = css.as_bytes();
    let mut out = b.to_vec();
    let mut i = 0;
    while let Some(k) = css[i..].find(G) {
        let at = i + k;
        let end = at + G.len();
        // not `:global-x` / `:globalx`
        if b.get(end).is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'-' || *c == b'_') {
            i = end;
            continue;
        }
        if b.get(end) == Some(&b'(') {
            // the matching `)`
            let mut depth = 0;
            let mut close = None;
            for (j, &c) in b.iter().enumerate().skip(end) {
                match c {
                    b'(' => depth += 1,
                    b')' => {
                        depth -= 1;
                        if depth == 0 {
                            close = Some(j);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let Some(close) = close else { break };
            out[at..=end].fill(b' ');
            out[close] = b' ';
            i = end + 1;
        } else {
            out[at..end].fill(b' ');
            let next = css[end..].trim_start().as_bytes().first().copied();
            let selector_before = css[..at].trim_end().as_bytes().last().is_some_and(|c| !matches!(c, b'{' | b'}' | b';' | b','));
            if next == Some(b'{') && !selector_before {
                out[at] = b'*';
            }
            i = end;
        }
    }
    // only ASCII bytes were replaced, at ASCII positions
    String::from_utf8(out).unwrap_or_else(|_| css.to_string())
}

/// The byte span in the `.svelte` file of the engine's (1-based line, 1-based UTF-16 column) in
/// the style text that starts at byte `base`: from there to the end of the declaration or line.
fn span_of(src: &str, base: u32, line: u32, column: u32) -> Option<(u32, u32)> {
    let line_start = if line <= 1 { 0 } else { src.match_indices('\n').nth(line as usize - 2)?.0 + 1 };
    let rest = &src[line_start..];
    let mut units = 0u32;
    let mut off = rest.len();
    for (k, ch) in rest.char_indices() {
        if units + 1 >= column.max(1) || ch == '\n' {
            off = k;
            break;
        }
        units += ch.len_utf16() as u32;
    }
    let start = line_start + off;
    let tail = &src[start..];
    let len = tail.find([';', '}', '{', '\n']).unwrap_or(tail.len());
    let len = tail[..len].trim_end().len().max(1).min(tail.len());
    Some((base + start as u32, base + (start + len) as u32))
}

/// The absolute byte range of the line holding absolute offset `at`.
fn line_bounds(src: &str, base: u32, at: u32) -> (u32, u32) {
    let rel = (at - base) as usize;
    let ls = src[..rel].rfind('\n').map_or(0, |k| k + 1);
    let le = src[rel..].find('\n').map_or(src.len(), |k| rel + k);
    (base + ls as u32, base + le as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_is_blanked_in_place() {
        let css = ".a :global(.b) { x: 1 } :global { .c { y: 2 } } .d:global(.e(f)) {} :global-x {}";
        let out = blank_svelte_syntax(css);
        assert_eq!(out.len(), css.len());
        assert_eq!(out, ".a         .b  { x: 1 } *       { .c { y: 2 } } .d        .e(f)  {} :global-x {}");
        assert_eq!(blank_svelte_syntax(".a :global { .b {} }"), ".a         { .b {} }");
    }

    #[test]
    fn spans_follow_lines_and_utf16_columns() {
        let src = "\n  .a { width: nope; }\n  .é { gap: 1x }";
        // line 2, column 8 = `width`
        assert_eq!(span_of(src, 100, 2, 8), Some((100 + 8, 100 + 19)));
        assert_eq!(&src[8..19], "width: nope");
        // line 3: `é` is one UTF-16 unit and two bytes
        let (s, e) = span_of(src, 0, 3, 8).unwrap();
        assert_eq!(&src[s as usize..e as usize], "gap: 1x");
    }
}
