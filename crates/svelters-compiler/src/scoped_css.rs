//! Q3w: the component's `<style>` scoped by the Rust port of Svelte's CSS pipeline
//! (`crate::svelte::css`, Q3) as a [`compile`](crate::compile::compile) output
//! (`CompileOptions::scoped_css`, `svelters-compile --css`). This is what the Vite plugin used to
//! get from stock `svelte/compiler` (`css: 'external'`): the scoped stylesheet, its hash and
//! `hasGlobal`. `css_*` errors and `css_unused_selector` warnings go into the compile's
//! diagnostics.
//!
//! svelters' `@native <toolkit> { ... }` blocks (X6, raw toolkit CSS) are not Svelte CSS: they are
//! blanked (same byte length, newlines kept) before the port sees the sheet, as the Vite plugin
//! lifts them out (`extractNative`) before scoping. A sheet without them is scoped byte-identically
//! to svelte@5.57.2.

use crate::diag::Diagnostics;
use crate::svelte::ast::Root;
use crate::svelte::css::{CssHash, CssOptions, compile_css};
use crate::svelte::scope::analyze::ComponentScopeAnalysis;
use serde::Serialize;

/// The scoped `<style>` of a component (`CompileResult::css`).
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopedCss {
    /// Svelte's `css.code` (`css: 'external'`; minified only when `injected` and not `dev`).
    pub code: String,
    /// The scoping class (`svelte-<hash>`): `CompileOptions::css_scope` when given, else Svelte's
    /// default `cssHash` of the filename.
    pub hash: String,
    /// Svelte's `css.hasGlobal`.
    pub has_global: bool,
    /// `<svelte:options css="injected">` or a custom element: upstream would put the sheet in the
    /// component's JS and return `css: null`. svelters never injects; the caller decides.
    pub injected: bool,
    /// Byte offsets (start) of the elements upstream gives the scoping class
    /// (`metadata.scoped`). The codegen does not use them yet (it scopes every native element).
    pub scoped_elements: Vec<u32>,
    /// DZ0: which bytes of `code` are untouched `<style>` text: `(offset in code, offset from the
    /// start of the `<style>` content, length)` runs, in order. Offsets relative to the style
    /// content stay right while the template above it is edited; the designer adds the content
    /// start of the file as it is now. [`ScopedCss::source_offset`] reads it. DZ0b2: in the JSON
    /// output (`--json`) as `map`, the [`ScopedCss::source_map_text`] text.
    #[serde(rename = "map", serialize_with = "map_text")]
    pub source_map: Vec<(u32, u32, u32)>,
    /// presto `css_precompiled` (release builds): the sheet as `svelters_style::binary` bytes,
    /// with rule hints per element and the resolved utility rules; `None` in dev, on other
    /// backends, or when the engine's parser reports something about the sheet (the text path
    /// then carries it, with its diagnostics).
    #[serde(skip)]
    pub precompiled: Option<Vec<u8>>,
}

/// What [`scoped_css`] needs to precompile the sheet.
pub(crate) struct Precompile<'r> {
    /// Resolves the static utility classes (U4's resolver), when utilities are checked.
    pub utilities: Option<svelters_style::Resolver<'r>>,
}

impl ScopedCss {
    /// DZ0: the `<style>`-relative offset of byte `at` of `code`, if it is untouched style text.
    pub fn source_offset(&self, at: u32) -> Option<u32> {
        style_offset(&self.source_map, at)
    }

    /// [`ScopedCss::precompiled`] as base64 (`registerStyles(id, { bin })`).
    pub fn precompiled_base64(&self) -> Option<String> {
        self.precompiled.as_deref().map(svelters_style::binary::to_base64)
    }

    /// DZ0: [`ScopedCss::source_map`] as text: `out,src,len` runs joined by `;` (what the loader
    /// hands the runtime's `registerStyles`).
    pub fn source_map_text(&self) -> String {
        runs_text(&self.source_map)
    }
}

fn runs_text(map: &[(u32, u32, u32)]) -> String {
    map.iter().map(|(o, s, l)| format!("{o},{s},{l}")).collect::<Vec<_>>().join(";")
}

fn map_text<S: serde::Serializer>(map: &[(u32, u32, u32)], s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&runs_text(map))
}

/// DZ0: the source offset of output byte `at` in a `(out, src, len)` run map, if `at` is in a run.
pub fn style_offset(map: &[(u32, u32, u32)], at: u32) -> Option<u32> {
    let k = map.partition_point(|&(o, _, _)| o <= at).checked_sub(1)?;
    let (o, s, l) = map[k];
    (at < o + l).then_some(s + (at - o))
}

/// DZ0: parses [`ScopedCss::source_map_text`]; `None` if malformed.
pub fn parse_source_map(text: &str) -> Option<Vec<(u32, u32, u32)>> {
    if text.is_empty() {
        return Some(Vec::new());
    }
    text.split(';')
        .map(|run| {
            let mut it = run.split(',').map(|x| x.parse::<u32>().ok());
            Some((it.next()??, it.next()??, it.next()??))
        })
        .collect()
}

/// presto `css_precompiled`: the scoped sheet `code` parsed by the engine and written as bytes, with
/// per element the rules Svelte's pruning says might apply (`candidates`) and those that match it
/// in any context (`always`). A rule maps to the Svelte rule whose selector list holds its start
/// (through the sheet's source map); a rule that maps to none, or that has an entirely global
/// selector, goes to the global rules every element tests. `None` when the engine reports anything
/// about the sheet (an unknown `:state()`, ...).
fn precompiled(
    code: &str,
    hash: &str,
    source_map: &[(u32, u32, u32)],
    content_start: u32,
    matches: &crate::svelte::css::SelectorMatches,
    element_start: impl Fn(u32) -> u32,
    p: &Precompile<'_>,
) -> Option<Vec<u8>> {
    use svelters_style::binary::{ElementHints, Precompiled, compound_matches, encode};
    let sheet = svelters_style::Stylesheet::parse(code, &svelters_style::ParseOptions::default());
    if !sheet.diagnostics.is_empty() {
        return None;
    }
    let line_starts: Vec<usize> = std::iter::once(0).chain(code.match_indices('\n').map(|(i, _)| i + 1)).collect();
    // cssparser's columns count UTF-16 units from 1
    let offset_of = |line: u32, column: u32| -> Option<u32> {
        let start = *line_starts.get(line.checked_sub(1)? as usize)?;
        let mut units = 1;
        for (i, c) in code[start..].char_indices() {
            if units >= column {
                return Some((start + i) as u32);
            }
            units += c.len_utf16() as u32;
        }
        None
    };
    let svelte_rule = |r: &svelters_style::Rule| -> Option<usize> {
        let at = content_start + style_offset(source_map, offset_of(r.line, r.column)?)?;
        matches.preludes.iter().enumerate().filter(|(_, (s, e))| *s <= at && at < *e).min_by_key(|(_, (s, e))| e - s).map(|(i, _)| i)
    };
    let owners: Vec<Option<usize>> = sheet.rules.iter().map(svelte_rule).collect();
    let global_rules: Vec<u32> =
        owners.iter().enumerate().filter(|(_, o)| o.is_none_or(|r| matches.global_rules.binary_search(&r).is_ok())).map(|(i, _)| i as u32).collect();
    let mut hints = Vec::new();
    let mut classes_seen = std::collections::BTreeSet::new();
    for el in &matches.elements {
        let candidates: Vec<u32> = owners
            .iter()
            .enumerate()
            .filter(|(i, o)| o.is_some_and(|r| matches.pairs.binary_search(&(r, el.start)).is_ok()) && global_rules.binary_search(&(*i as u32)).is_err())
            .map(|(i, _)| i as u32)
            .collect();
        let mut always = Vec::new();
        if let (Some(classes), Ok(id), false) = (&el.classes, &el.id, el.tag.is_empty()) {
            let mut all: Vec<&str> = classes.iter().map(String::as_str).collect();
            // every native element carries the scope class
            all.push(hash);
            for &i in &candidates {
                let rule = &sheet.rules[i as usize];
                if rule.media.is_empty() && rule.selector_text().split(", ").any(|s| compound_matches(s, &el.tag, &all, id.as_deref()) == Some(true)) {
                    always.push(i);
                }
            }
            classes_seen.extend(classes.iter().cloned());
        }
        hints.push(ElementHints { element: element_start(el.start), candidates, always });
    }
    let utilities = match &p.utilities {
        Some(res) => classes_seen
            .iter()
            .filter_map(|c| match res.resolve(c) {
                svelters_style::Resolved::Utility(u) => Some(u),
                _ => None,
            })
            .collect(),
        None => Vec::new(),
    };
    Some(encode(&Precompiled { sheet, hints, global_rules, utilities }))
}

/// Runs the Q3 port over the component's `<style>`: `None` when there is none or when the CSS
/// step failed (its error is in `diags`).
///
/// With `apply` (U4), the source with `<style>`'s `@apply` expanded is scoped, and the spans of its
/// diagnostics map back to the original.
#[allow(clippy::too_many_arguments)]
pub(crate) fn scoped_css<'a>(
    src: &str,
    root: &'a Root,
    analysis: &ComponentScopeAnalysis<'a>,
    filename: &str,
    hash: Option<&str>,
    dev: bool,
    apply: Option<&crate::utilities::apply::Expansion>,
    precompile: Option<&Precompile<'_>>,
    diags: &mut Diagnostics,
) -> Option<ScopedCss> {
    let block = root.css.as_ref()?;
    let map = |s: (u32, u32)| apply.map_or(s, |x| x.to_orig(s));
    let src = apply.map_or(src, |x| x.src.as_str());
    let content_end = apply.map_or(block.content.end, |x| x.to_new(block.content.end));
    let blanked = blank_native_blocks(src, block.content.start as usize, content_end as usize);
    let src = blanked.as_deref().unwrap_or(src);
    let hash = match hash {
        Some(h) => CssHash::Fixed(h.to_string()),
        None => CssHash::Default { filename: Some(filename.to_string()) },
    };
    match compile_css(src, root, analysis, &CssOptions { hash, injected: false, dev }) {
        Ok(Some(out)) => {
            for w in &out.warnings {
                diags.warning(w.code, map((w.start, w.end)), w.message.clone());
            }
            // with `@apply` expanded the runs are in the expanded text: keep the ones that map
            // back to the original unchanged in length
            let source_map = match apply {
                None => out.source_map,
                Some(x) => {
                    let start = block.content.start;
                    let new_start = x.to_new(start);
                    out.source_map
                        .into_iter()
                        .filter_map(|(o, s, l)| {
                            let (a, b) = x.to_orig((new_start + s, new_start + s + l));
                            (b - a == l && a >= start).then_some((o, a - start, l))
                        })
                        .collect()
                }
            };
            let element_start = |s: u32| map((s, s)).0;
            let precompiled = precompile.and_then(|p| precompiled(&out.code, &out.hash, &source_map, block.content.start, &out.matches, element_start, p));
            Some(ScopedCss { code: out.code, hash: out.hash, has_global: out.has_global, injected: out.injected, scoped_elements: out.scoped_elements, source_map, precompiled })
        }
        Ok(None) => None,
        Err(e) => {
            diags.error(e.code, map((e.start, e.end)), e.message);
            None
        }
    }
}

/// `src` with the `@native <name> { ... }` blocks of `src[start..end]` replaced by spaces
/// (newlines kept), or `None` when there are none. Finds them as `@svelters/style`'s
/// `extractNative` does (skips matches inside comments, honours quotes and comments in the body);
/// an unclosed block is left alone (the CSS parser reports it).
pub fn blank_native_blocks(src: &str, start: usize, end: usize) -> Option<String> {
    let css = &src[start..end];
    let b = css.as_bytes();
    let mut ranges = Vec::new();
    let mut i = 0;
    let mut from = 0;
    while let Some(off) = css[from..].find("@native") {
        let at = from + off;
        from = at + 1;
        // /@native\s+([a-z]+)\s*\{/
        let mut k = at + "@native".len();
        let ws = k;
        while k < b.len() && b[k].is_ascii_whitespace() {
            k += 1;
        }
        let name = k;
        while k < b.len() && b[k].is_ascii_lowercase() {
            k += 1;
        }
        if ws == name || name == k {
            continue;
        }
        while k < b.len() && b[k].is_ascii_whitespace() {
            k += 1;
        }
        if b.get(k) != Some(&b'{') {
            continue;
        }
        k += 1;
        // a match inside a comment is skipped
        if let Some(open) = css[..at].rfind("/*").filter(|o| *o >= i)
            && css[open..].find("*/").is_some_and(|c| open + c > at)
        {
            i = k;
            from = k;
            continue;
        }
        let mut depth = 1;
        let mut quote = 0u8;
        while k < b.len() && depth > 0 {
            let ch = b[k];
            if quote != 0 {
                if ch == b'\\' {
                    k += 1;
                } else if ch == quote {
                    quote = 0;
                }
            } else if ch == b'"' || ch == b'\'' {
                quote = ch;
            } else if ch == b'/' && b.get(k + 1) == Some(&b'*') {
                k = css[k + 2..].find("*/").map_or(b.len(), |e| k + 2 + e + 1);
            } else if ch == b'{' {
                depth += 1;
            } else if ch == b'}' {
                depth -= 1;
            }
            k += 1;
        }
        if depth > 0 {
            break;
        }
        ranges.push((at, k.min(b.len())));
        i = k;
        from = k;
    }
    if ranges.is_empty() {
        return None;
    }
    let mut out = src.as_bytes().to_vec();
    for (a, z) in ranges {
        for byte in &mut out[start + a..start + z] {
            if *byte != b'\n' && *byte != b'\r' {
                *byte = b' ';
            }
        }
    }
    // every byte of a blanked range became ASCII, so the rest stays valid UTF-8
    Some(String::from_utf8(out).expect("blanking keeps UTF-8"))
}

#[cfg(test)]
mod tests {
    use super::blank_native_blocks;

    #[test]
    fn blanks_native_blocks_only() {
        let src = "<p/>\n<style>\n.a { color: red; }\n@native gtk {\n  button { min-height: 24px; }\n}\n/* @native qt { } */\n@native qt { .Q { a: \"}\" } }\n</style>";
        let start = src.find("<style>").unwrap() + 7;
        let end = src.find("</style>").unwrap();
        let out = blank_native_blocks(src, start, end).unwrap();
        assert_eq!(out.len(), src.len());
        assert_eq!(out.matches('\n').count(), src.matches('\n').count());
        assert!(out.contains(".a { color: red; }"));
        assert!(out.contains("/* @native qt { } */"));
        assert!(!out.contains("button"));
        assert!(!out.contains(".Q"));
        assert!(blank_native_blocks("<style>.a{}</style>", 7, 11).is_none());
    }
}
