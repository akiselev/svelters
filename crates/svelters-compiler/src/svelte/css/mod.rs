//! Svelte's component CSS pipeline ported to Rust (Q3): parse (`phases/1-parse/read/style.js`),
//! analyze (`phases/2-analyze/css/css-analyze.js`), prune (`css-prune.js`), warn (`css-warn.js`)
//! and transform (`phases/3-transform/css/index.js`), Svelte 5.57.2. The output (scoped
//! stylesheet, `hasGlobal`, `css_unused_selector` warnings, CSS errors, the hash) is
//! byte-identical to `svelte/compiler`'s on the corpus of `tests/css_scope.rs`.
//!
//! Entry point: [`compile_css`], after `parse`, `ts::strip` and `analyze_component_scopes`.
//! Positions are byte offsets.

pub mod analyze;
pub mod ast;
pub mod errors;
pub mod magic_string;
pub mod parse;
pub mod prune;
pub mod template;
pub mod transform;
pub mod utils;
pub mod warn;

use crate::svelte::ast::{FragmentNode, Root};
use crate::svelte::errors::{CompileError, CompileWarning};
use crate::svelte::scope::analyze::ComponentScopeAnalysis;
use crate::svelte::scope::walk::{NodeKey, NodeRef};
use ast::{Node, Rule, StyleSheet};
use std::cell::Cell;
use std::collections::HashSet;

#[derive(Default)]
pub struct RuleMeta {
    pub parent_rule: Cell<Option<usize>>,
    pub has_local_selectors: Cell<bool>,
    pub has_global_selectors: Cell<bool>,
    pub is_global_block: Cell<bool>,
}

#[derive(Default)]
pub struct ComplexMeta {
    pub rule: Cell<Option<usize>>,
    pub is_global: Cell<bool>,
    pub used: Cell<bool>,
}

#[derive(Default)]
pub struct RelativeMeta {
    pub is_global: Cell<bool>,
    pub is_global_like: Cell<bool>,
    pub scoped: Cell<bool>,
}

/// The `metadata` of the CSS nodes, by id.
#[derive(Default)]
pub struct Meta {
    pub rules: Vec<RuleMeta>,
    pub complex: Vec<ComplexMeta>,
    pub rel: Vec<RelativeMeta>,
}

/// `analysis.css` plus the stylesheet and its metadata.
pub struct Css<'s> {
    pub sheet: &'s StyleSheet,
    /// rules by id
    pub rules: Vec<&'s Rule>,
    pub meta: Meta,
    pub keyframes: Vec<String>,
    pub has_global: bool,
}

impl<'s> Css<'s> {
    pub fn new(sheet: &'s StyleSheet) -> Css<'s> {
        let (nr, nc, nrel) = sheet.counts;
        let mut rules: Vec<Option<&'s Rule>> = vec![None; nr];
        fn collect<'s>(nodes: &'s [Node], out: &mut Vec<Option<&'s Rule>>) {
            for n in nodes {
                match n {
                    Node::Rule(r) => {
                        out[r.id] = Some(r);
                        collect(&r.block.children, out);
                    }
                    Node::Atrule(a) => {
                        if let Some(b) = &a.block {
                            collect(&b.children, out);
                        }
                    }
                    Node::Declaration(_) => {}
                }
            }
        }
        collect(&sheet.children, &mut rules);
        Css {
            sheet,
            rules: rules.into_iter().map(|r| r.expect("every rule id")).collect(),
            meta: Meta {
                rules: (0..nr).map(|_| RuleMeta::default()).collect(),
                complex: (0..nc).map(|_| ComplexMeta::default()).collect(),
                rel: (0..nrel).map(|_| RelativeMeta::default()).collect(),
            },
            keyframes: Vec::new(),
            has_global: false,
        }
    }
}

/// The `cssHash` compile option.
#[derive(Clone, Debug)]
pub enum CssHash {
    /// Svelte's default: `svelte-${hash(filename === '(unknown)' ? css : filename ?? css)}`
    /// (`filename` normalized to forward slashes, as `state.filename`).
    Default { filename: Option<String> },
    /// The class a custom `cssHash` function returned.
    Fixed(String),
}

#[derive(Clone, Debug)]
pub struct CssOptions {
    pub hash: CssHash,
    /// `css: 'injected'` (or a custom element): the stylesheet is minified unless `dev`
    pub injected: bool,
    pub dev: bool,
}

#[derive(Clone, Debug)]
pub struct CssOutput {
    /// `css.code`
    pub code: String,
    /// `css.hasGlobal`
    pub has_global: bool,
    /// `analysis.css.hash`, the scoping class
    pub hash: String,
    /// `analysis.inject_styles`: the stylesheet goes into the component's JS (upstream's
    /// `compile` then returns `css: null`); `code` is minified unless `dev`
    pub injected: bool,
    /// The `css_*` warnings, in Svelte's order
    pub warnings: Vec<CompileWarning>,
    /// Start offsets of the elements (RegularElement / SvelteElement) that get the scoping class
    /// (`metadata.scoped`)
    pub scoped_elements: Vec<u32>,
    /// DZ0: the runs of `code` that are untouched `<style>` text: `(offset in code, offset from
    /// the style content's start, length)`, bytes
    pub source_map: Vec<(u32, u32, u32)>,
    /// SVELTERS (presto `css_precompiled`): what pruning found, per rule.
    pub matches: SelectorMatches,
}

/// SVELTERS: which rules the pruning pass found might apply to which template elements, for
/// precompiled sheets' rule hints (`crate::scoped_css`).
#[derive(Clone, Debug, Default)]
pub struct SelectorMatches {
    /// Per rule id, its prelude's (start, end) source offsets.
    pub preludes: Vec<(u32, u32)>,
    /// (rule id, element start offset): a selector of the rule might apply to the element.
    pub pairs: Vec<(usize, u32)>,
    /// Rules with a selector that is entirely global.
    pub global_rules: Vec<usize>,
    /// Every element (RegularElement / SvelteElement) with what is static about it.
    pub elements: Vec<StaticElement>,
}

/// A template element's tag, and its class and id when they are static text.
#[derive(Clone, Debug)]
pub struct StaticElement {
    pub start: u32,
    pub tag: String,
    /// `None` when the classes can change (an expression, `class:`, a spread).
    pub classes: Option<Vec<String>>,
    /// `Err(())` when the id can change.
    pub id: Result<Option<String>, ()>,
}

/// `hash(str)` of `svelte/src/utils.js`: djb2 over UTF-16 units (without `\r`), base 36.
pub fn hash(s: &str) -> String {
    let units: Vec<u16> = s.encode_utf16().filter(|u| *u != u16::from(b'\r')).collect();
    let mut h: i32 = 5381;
    for u in units.iter().rev() {
        h = (h.wrapping_shl(5).wrapping_sub(h)) ^ i32::from(*u);
    }
    let mut n = h as u32;
    if n == 0 {
        return "0".into();
    }
    let digits = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut out = Vec::new();
    while n > 0 {
        out.push(digits[(n % 36) as usize]);
        n /= 36;
    }
    out.reverse();
    String::from_utf8(out).unwrap()
}

pub fn css_hash(opt: &CssHash, css: &str) -> String {
    match opt {
        CssHash::Fixed(h) => h.clone(),
        CssHash::Default { filename } => {
            let f = filename.as_ref().map(|f| f.replace('\\', "/"));
            match f.as_deref() {
                Some(f) if f != "(unknown)" => format!("svelte-{}", hash(f)),
                _ => format!("svelte-{}", hash(css)),
            }
        }
    }
}

/// `content.comment`: the HTML comment right before the top-level `<style>` (only whitespace
/// text in between), as `phases/1-parse/state/element.js` finds it.
fn style_comment(root: &Root, style_start: u32) -> Option<&str> {
    let before: Vec<&FragmentNode> = root.fragment.nodes.iter().filter(|n| crate::estree::Spanned::span(*n).end <= style_start).collect();
    for (i, n) in before.iter().enumerate().rev() {
        if i == before.len() - 1 && crate::estree::Spanned::span(*n).end != style_start {
            break;
        }
        match n {
            FragmentNode::Comment(c) => return Some(&c.data),
            FragmentNode::Text(t) if t.data.trim_matches(|c: char| crate::svelte::parse::utils::whitespace::is_whitespace(c as u32)).is_empty() => {}
            _ => break,
        }
    }
    None
}

/// Whether `extract_svelte_ignore(comment)` contains `css_unused_selector`.
fn ignores_unused_selector(comment: &str, runes: bool) -> bool {
    let is_ws = |c: char| crate::svelte::parse::utils::whitespace::is_whitespace(c as u32);
    // /^\s*svelte-ignore\s/
    let t = comment.trim_start_matches(is_ws);
    let Some(rest) = t.strip_prefix("svelte-ignore") else { return false };
    if !rest.starts_with(is_ws) {
        return false;
    }
    let rest = &rest[rest.chars().next().unwrap().len_utf8()..];
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '$' || c == '-';
    // the successive matches of /[\w$-]+/ (and, in runes mode, whether a `,` follows each)
    let mut tokens = Vec::new();
    let mut s = rest;
    while let Some(i) = s.find(word) {
        let tail = &s[i..];
        let len = tail.find(|c| !word(c)).unwrap_or(tail.len());
        let comma = tail[len..].starts_with(',');
        tokens.push((&tail[..len], comma));
        s = &tail[len..];
    }
    if runes {
        for (code, comma) in tokens {
            if code == "css_unused_selector" {
                return true;
            }
            if !comma {
                break;
            }
        }
        false
    } else {
        tokens.iter().any(|(code, _)| *code == "css_unused_selector" || code.replace('-', "_") == "css_unused_selector")
    }
}

/// Parses the component's top-level `<style>` with Svelte's CSS parser (a parse error is what
/// `parse` would throw).
pub fn parse_style(src: &str, root: &Root) -> Result<Option<StyleSheet>, CompileError> {
    let Some(block) = &root.css else { return Ok(None) };
    // the parser's template is the source without trailing white space (`template.trimEnd()`)
    let template = src.trim_end_matches(|c: char| crate::svelte::parse::utils::whitespace::is_whitespace(c as u32));
    parse::parse_stylesheet(template, block.start as usize, block.content.start as usize).map(Some)
}

/// Runs analyze, prune, warn and transform over the component's `<style>`. `Ok(None)`: no
/// `<style>`.
pub fn compile_css<'a>(src: &str, root: &'a Root, analysis: &ComponentScopeAnalysis<'a>, opts: &CssOptions) -> Result<Option<CssOutput>, CompileError> {
    let Some(sheet) = parse_style(src, root)? else { return Ok(None) };
    let block = root.css.as_ref().unwrap();
    let hash = css_hash(&opts.hash, &block.content.styles);

    let mut css = Css::new(&sheet);
    analyze::analyze_css(&mut css)?;

    let tpl = template::Template::build(root, analysis);
    let synth = prune::Synthetic::new();
    let pruned = prune::prune_matches(&css, &tpl, &synth);
    let matches = selector_matches(&css, &tpl, &pruned);
    let scoped: HashSet<NodeKey> = pruned.scoped;

    let mut warnings = Vec::new();
    let ignore = style_comment(root, block.start).is_some_and(|c| ignores_unused_selector(c, analysis.runes));
    if !ignore {
        warn::warn_unused(&css, src, &mut warnings);
    }

    // If a snippet is exported, a consumer could only import this named export and not the default
    // export (the component): `hasGlobal`, so that bundlers don't tree-shake the CSS.
    if let Some(module) = &root.module {
        for stmt in &module.content.body {
            let crate::estree::Statement::ExportNamedDeclaration(d) = stmt else { continue };
            if d.source.is_some() {
                continue;
            }
            for spec in &d.specifiers {
                let crate::estree::ModuleExportName::Identifier(local) = &spec.local else { continue };
                // SVELTERS: upstream finds top-level snippets in the module scope (`declare` on
                // porous scopes); the Rust scopes keep them in the template's, so a top-level
                // `{#snippet}` of that name counts too
                let is_snippet = match analysis.arena.get(analysis.module.scope, &local.name) {
                    Some(b) => matches!(analysis.arena.binding(b).initial, Some(crate::svelte::scope::Initial::SnippetBlock(_))),
                    None => root.fragment.nodes.iter().any(|n| matches!(n, FragmentNode::SnippetBlock(s) if s.expression.name == local.name)),
                };
                if is_snippet {
                    css.has_global = true;
                }
            }
        }
    }

    // `analysis.inject_styles`: `css: 'injected'` (the option or `<svelte:options css="injected">`)
    // or a custom element
    let inject = opts.injected
        || root.options.as_ref().is_some_and(|o| o.custom_element.is_some() || o.css.as_deref() == Some("injected"));
    let (code, source_map) = transform::render_stylesheet_mapped(src, &css, &hash, inject, opts.dev);
    let scoped_elements = tpl.elements.iter().filter(|e| scoped.contains(&e.key())).map(|e| e.span().start).collect();
    Ok(Some(CssOutput { code, has_global: css.has_global, hash, injected: inject, warnings, scoped_elements, source_map, matches }))
}

/// SVELTERS: [`SelectorMatches`] from the pruner's complex-selector matches.
fn selector_matches(css: &Css, tpl: &template::Template, pruned: &prune::Pruner) -> SelectorMatches {
    use crate::svelte::ast::{AttributeLike, AttributeValue, TextOrExpressionTag};
    let rule_of = |c: usize| css.meta.complex[c].rule.get();
    let preludes = css.rules.iter().map(|r| (r.prelude.start as u32, r.prelude.end as u32)).collect();
    let start_of: std::collections::HashMap<NodeKey, u32> = tpl.elements.iter().map(|e| (e.key(), e.span().start)).collect();
    let mut pairs: Vec<(usize, u32)> = pruned.matched.iter().filter_map(|(c, e)| Some((rule_of(*c)?, *start_of.get(e)?))).collect();
    pairs.sort_unstable();
    pairs.dedup();
    let mut global_rules: Vec<usize> = pruned.global.iter().filter_map(|c| rule_of(*c)).collect();
    global_rules.sort_unstable();
    global_rules.dedup();
    let text = |v: &AttributeValue| -> Option<String> {
        match v {
            AttributeValue::Sequence(parts) => parts.iter().map(|p| if let TextOrExpressionTag::Text(t) = p { Some(t.data.clone()) } else { None }).collect(),
            AttributeValue::True(_) => Some(String::new()),
            AttributeValue::Expression(_) => None,
        }
    };
    let elements = tpl
        .elements
        .iter()
        .filter_map(|&e| {
            let (tag, attrs) = template::element_parts(e)?;
            let mut classes = Some(Vec::new());
            let mut id = Ok(None);
            for a in attrs {
                match a {
                    AttributeLike::Attribute(a) if a.name == "class" => match text(&a.value) {
                        Some(t) => classes = classes.map(|mut c: Vec<String>| {
                            c.extend(t.split_ascii_whitespace().map(str::to_string));
                            c
                        }),
                        None => classes = None,
                    },
                    AttributeLike::Attribute(a) if a.name == "id" => id = text(&a.value).map(Some).ok_or(()),
                    AttributeLike::ClassDirective(_) => classes = None,
                    AttributeLike::SpreadAttribute(_) => {
                        classes = None;
                        id = Err(());
                    }
                    _ => {}
                }
            }
            // <svelte:element this={..}>: the tag is dynamic
            let tag = if matches!(e, NodeRef::SvelteElement(_)) { String::new() } else { tag.to_string() };
            Some(StaticElement { start: e.span().start, tag, classes, id })
        })
        .collect();
    SelectorMatches { preludes, pairs, global_rules, elements }
}

/// The whole pipeline from source: `parse` (`native`: the svelters native element syntax),
/// `lang="ts"` stripping, scope analysis (`runes`: the `runes` option, `None` to infer) and
/// [`compile_css`]. What `svelte/compiler`'s `compile(source, { css, cssHash, dev })` returns as
/// `css` and its `css_*` warnings, with no Node.
pub fn compile_component_css(src: &str, native: bool, runes: Option<bool>, opts: &CssOptions) -> Result<Option<CssOutput>, CompileError> {
    let mut root = crate::svelte::parse::parse(src, &crate::svelte::parse::ParseOptions { loose: false, native })?;
    crate::svelte::ts::strip(&mut root, src)?;
    let analysis = crate::svelte::scope::analyze::analyze_component_scopes(&root, runes)?;
    compile_css(src, &root, &analysis, opts)
}
