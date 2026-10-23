//! U4 (plans/research/UTILITY-CSS.md §4.1, §4.4, §4.6, §4.7): compile-time checking of utility
//! classes, and `@apply` ([`apply`]).
//!
//! Utilities are generated at run time by the host's style engine (`svelters_style::utility`, U1);
//! the compiler only checks them, with the same [`Resolver`] and generated vocabulary, so the
//! compiler and the runtime agree on what a class means. The check is opt-in like the runtime
//! ([`CompileOptions::utilities`](crate::compile::CompileOptions::utilities)) and runs only for a
//! backend that runs the style engine itself (egui).
//!
//! **What is checked**: every *complete* class token the compiler can see on a native element:
//! the static `class="..."` text, `class:<name>` directives, string literals and template-literal
//! text inside `class={...}` (clsx arrays and object keys, conditionals, `cn(...)` calls) and, with
//! [`UtilityCheck::scan_script`], string literals in `<script>` (as Tailwind's scanner reads whole
//! files). A token an interpolation touches (`p-{n}`) is not complete; when the text before the
//! interpolation is a utility namespace it gets `utility_dynamic_value`.
//!
//! **Codes** (`crate::codes`):
//! - the resolver's verdict: `utility_unknown` (a near-miss value or an unknown variant; "did you
//!   mean" fix-its), `utility_unsupported` (a Tailwind name egui doesn't render, or one waiting for
//!   an engine item; the replacement as a fix-it), `utility_bad_value` (an arbitrary value the
//!   property's type rejects: an error, as at run time the rule is dropped), `utility_arbitrary_color`
//!   (a colour literal; an error under `strict_tokens`) and `utility_alias` (an accepted spelling; a
//!   note with the canonical one);
//! - per kind, from the generated `KINDS` and the egui style table: `utility_variant_never_matches`
//!   (a state the kind never sets, `focus-visible` counting as `focus`; a part it lacks; a provider
//!   state it doesn't provide) and `utility_not_applicable` (a property the kind doesn't take);
//! - `utility_conflict`: two utilities of one static list with the same properties and variants
//!   (the loser is removed by the fix-it);
//! - `utility_shadowed_class`: a class selector in the component's `<style>` that is also a
//!   utility;
//! - `utility_apply_unknown` (an error) for `@apply` ([`apply`]).
//!
//! A class that isn't utility-shaped is a plain class and never reported. A bare name that
//! Tailwind has but egui doesn't render (`block`, `fill`, `content`) is left alone too unless the
//! embed sets a prefix, since without one it is as likely a semantic class.
//!
//! Every fix-it is verified: the rewritten class must resolve to a utility, so applying it clears
//! the diagnostic (the `cx1_check` rule).

pub mod apply;

use crate::diag::{Diagnostic, Diagnostics, FixIt, TextEdit};
use crate::estree::{ArrayElement, Expression, LiteralValue, LogicalOperator, ObjectMember, PropertyKey, Spanned, TemplateLiteral};
use crate::semantics::PlatformSemantics;
use crate::svelte::ast::{AttributeLike, AttributeValue, Fragment, FragmentNode, Root, TextOrExpressionTag};
use crate::svelte::scope::walk::NodeRef;
use svelters_style::utilities::{ALIASES, BREAKPOINTS, COMPOUND, KINDS, MEDIA, PROVIDER_STATES, PSEUDO_ELEMENTS, ROWS, STATE_VARIANTS, STRUCTURAL};
use svelters_style::utility_table::{KindStyle, RowKind};
use svelters_style::{IssueKind, Resolved, Resolver, Severity, Table, Utility, UtilityIssue, UtilityTokens};

pub const UNKNOWN: &str = "utility_unknown";
pub const UNSUPPORTED: &str = "utility_unsupported";
pub const BAD_VALUE: &str = "utility_bad_value";
pub const ARBITRARY_COLOR: &str = "utility_arbitrary_color";
pub const ALIAS: &str = "utility_alias";
pub const VARIANT_NEVER_MATCHES: &str = "utility_variant_never_matches";
pub const NOT_APPLICABLE: &str = "utility_not_applicable";
pub const CONFLICT: &str = "utility_conflict";
pub const DYNAMIC_VALUE: &str = "utility_dynamic_value";
pub const SHADOWED_CLASS: &str = "utility_shadowed_class";
pub const APPLY_UNKNOWN: &str = "utility_apply_unknown";

/// Part names the egui widgets style by name without declaring them as `#[part]` yet (the scroll
/// bars, the lists' rows; U0 note 3): accepted on every kind.
const UNDECLARED_PARTS: &[&str] = &["scrollbar-x", "scrollbar-y", "thumb-x", "thumb-y", "corner", "row"];

/// The embed's utility settings, as the checker needs them (egui: `EmbedConfig::utilities`, U3).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UtilityCheck {
    /// With `Some("tw")`, only `tw:`-prefixed classes are utilities.
    pub prefix: Option<String>,
    /// cadatomic's lint profile: a colour literal (`bg-[#f00]`) is an error.
    pub strict_tokens: bool,
    /// Also check string literals in `<script>` (`utilities.scan: "script"`).
    pub scan_script: bool,
    /// The app theme's own tokens beyond the built-in theme's, as (name, type) (`cad-panel`,
    /// `color`), so `bg-cad-panel` is a value.
    pub tokens: Vec<(String, String)>,
}

impl UtilityCheck {
    /// The token set the resolver uses: the built-in theme's plus [`UtilityCheck::tokens`].
    pub fn utility_tokens(&self) -> UtilityTokens {
        let mut t = UtilityTokens::builtin();
        for (n, ty) in &self.tokens {
            t.insert(n, ty);
        }
        t
    }
}

/// Checks the utility classes of the component and reports into `diags`.
pub(crate) fn check(root: &Root, src: &str, sem: &dyn PlatformSemantics, opts: &UtilityCheck, diags: &mut Diagnostics) {
    if !sem.style_engine_in_backend() {
        return;
    }
    let Some(table) = Table::for_toolkit(sem.backend()) else { return };
    let tokens = opts.utility_tokens();
    let cx = Cx::new(table, &tokens, opts, src);
    cx.fragment(&root.fragment, diags);
    if opts.scan_script {
        for s in [root.instance.as_ref(), root.module.as_ref()].into_iter().flatten() {
            cx.script(NodeRef::Program(&s.content), diags);
        }
    }
    shadowed(root, src, &cx, diags);
}

pub(crate) struct Cx<'a> {
    pub(crate) resolver: Resolver<'a>,
    table: &'static Table,
    prefix: Option<&'a str>,
    src: &'a str,
}

/// One complete class token and where it is written.
#[derive(Clone, Copy)]
pub(crate) struct Token<'s> {
    pub(crate) class: &'s str,
    pub(crate) span: (u32, u32),
    /// The text can be rewritten in place (attribute text, a string literal); an identifier key of
    /// a clsx object can't take every class name.
    pub(crate) editable: bool,
}

impl<'a> Cx<'a> {
    pub(crate) fn new(table: &'static Table, tokens: &'a UtilityTokens, opts: &'a UtilityCheck, src: &'a str) -> Cx<'a> {
        let mut resolver = Resolver::new(table, tokens);
        resolver.prefix = opts.prefix.as_deref();
        resolver.strict_tokens = opts.strict_tokens;
        Cx { resolver, table, prefix: opts.prefix.as_deref(), src }
    }

    fn fragment(&self, f: &Fragment, diags: &mut Diagnostics) {
        for n in &f.nodes {
            self.node(n, diags);
        }
    }

    fn node(&self, n: &FragmentNode, diags: &mut Diagnostics) {
        use FragmentNode::*;
        match n {
            RegularElement(e) => {
                self.element(&e.name, &e.attributes, diags);
                self.fragment(&e.fragment, diags);
            }
            Component(e) | TitleElement(e) | SlotElement(e) | SvelteBody(e) | SvelteDocument(e) | SvelteFragment(e) | SvelteBoundary(e)
            | SvelteHead(e) | SvelteOptionsRaw(e) | SvelteSelf(e) | SvelteWindow(e) => self.fragment(&e.fragment, diags),
            SvelteComponent(e) => self.fragment(&e.fragment, diags),
            // the tag is dynamic: no per-kind checks
            SvelteElement(e) => {
                self.element("", &e.attributes, diags);
                self.fragment(&e.fragment, diags);
            }
            EachBlock(b) => {
                self.fragment(&b.body, diags);
                if let Some(f) = &b.fallback {
                    self.fragment(f, diags);
                }
            }
            IfBlock(b) => {
                self.fragment(&b.consequent, diags);
                if let Some(f) = &b.alternate {
                    self.fragment(f, diags);
                }
            }
            AwaitBlock(b) => {
                for f in [&b.pending, &b.then, &b.catch].into_iter().flatten() {
                    self.fragment(f, diags);
                }
            }
            KeyBlock(b) => self.fragment(&b.fragment, diags),
            SnippetBlock(b) => self.fragment(&b.body, diags),
            _ => {}
        }
    }

    fn element(&self, tag: &str, attrs: &[AttributeLike], diags: &mut Diagnostics) {
        let kind = KINDS.iter().find(|k| k.tag == tag);
        for a in attrs {
            match a {
                AttributeLike::Attribute(at) if at.name == "class" => match &at.value {
                    AttributeValue::Sequence(parts) => {
                        let mut tokens = Vec::new();
                        for (i, p) in parts.iter().enumerate() {
                            let TextOrExpressionTag::Text(t) = p else { continue };
                            // the raw text is what the source holds; a value with entities is skipped
                            if t.raw != t.data {
                                continue;
                            }
                            let next = match parts.get(i + 1) {
                                Some(TextOrExpressionTag::ExpressionTag(e)) => Some(e.end),
                                _ => None,
                            };
                            self.text_tokens(&t.raw, t.start, i > 0, next, &mut tokens, diags);
                        }
                        self.list(&tokens, kind, diags);
                    }
                    AttributeValue::Expression(crate::svelte::ast::ExpressionTagNode::ExpressionTag(e)) => self.clsx(&e.expression, kind, diags),
                    AttributeValue::True(_) => {}
                },
                AttributeLike::ClassDirective(d) => {
                    let start = d.start + "class:".len() as u32;
                    let end = start + d.name.len() as u32;
                    if self.src.get(start as usize..end as usize) != Some(d.name.as_str()) {
                        continue;
                    }
                    self.token(&Token { class: &d.name, span: (start, end), editable: true }, kind, diags);
                }
                _ => {}
            }
        }
    }

    /// The whitespace-separated tokens of `raw` (at byte `base`). `expr_before`: an interpolation
    /// ends right before the text; `expr_after`: one starts right after it (ending at that offset),
    /// so the first or last word is incomplete.
    fn text_tokens<'s>(&self, raw: &'s str, base: u32, expr_before: bool, expr_after: Option<u32>, out: &mut Vec<Token<'s>>, diags: &mut Diagnostics) {
        let mut words = Vec::new();
        let mut at = None;
        for (k, c) in raw.char_indices() {
            if c.is_whitespace() {
                if let Some(s) = at.take() {
                    words.push((s, k));
                }
            } else if at.is_none() {
                at = Some(k);
            }
        }
        if let Some(s) = at {
            words.push((s, raw.len()));
        }
        for (s, e) in words {
            if let Some(expr_end) = expr_after.filter(|_| e == raw.len()) {
                // `p-{n}`, `w-[{px}px]`: the text before an interpolation
                self.dynamic(&raw[s..e], (base + s as u32, expr_end), diags);
                continue;
            }
            if expr_before && s == 0 {
                continue;
            }
            out.push(Token { class: &raw[s..e], span: (base + s as u32, base + e as u32), editable: true });
        }
    }

    /// `utility_dynamic_value` when `partial` (a token's text before an interpolation) is a
    /// utility namespace.
    fn dynamic(&self, partial: &str, span: (u32, u32), diags: &mut Diagnostics) {
        let mut rest = partial;
        if let Some(p) = self.prefix {
            match rest.strip_prefix(p).and_then(|r| r.strip_prefix(':')) {
                Some(r) => rest = r,
                None => return,
            }
        }
        let base = rest.rsplit(':').next().unwrap_or(rest);
        let base = base.trim_start_matches(['!', '-']);
        let row = ROWS.iter().find(|r| {
            matches!(r.kind, RowKind::Namespace { .. })
                && base.strip_prefix(r.name).is_some_and(|v| v == "-" || v.starts_with("-[") || v.starts_with("-("))
        });
        let Some(row) = row else { return };
        let RowKind::Namespace { props, .. } = row.kind else { return };
        let prop = props.first().copied().unwrap_or("");
        diags
            .warning(DYNAMIC_VALUE, span, format!("`{partial}…` builds a utility class from a value: every distinct value generates a style rule at run time"))
            .help(format!("bind the value as a style instead: `style:{prop}={{…}}`"));
    }

    /// The tokens of a `class={...}` expression (clsx semantics) and of each string in it.
    fn clsx(&self, e: &Expression, kind: Option<&KindStyle>, diags: &mut Diagnostics) {
        match e {
            Expression::Literal(l) => {
                if let LiteralValue::String(_) = l.value {
                    let mut tokens = Vec::new();
                    if let Some(text) = literal_text(&l.raw) {
                        self.text_tokens(text, l.start + 1, false, None, &mut tokens, diags);
                    }
                    self.list(&tokens, kind, diags);
                }
            }
            Expression::TemplateLiteral(t) => {
                let tokens = self.template_tokens(t, diags);
                self.list(&tokens, kind, diags);
            }
            Expression::ArrayExpression(a) => {
                for el in a.elements.iter().flatten() {
                    if let ArrayElement::Expression(x) = el {
                        self.clsx(x, kind, diags);
                    }
                }
            }
            Expression::ObjectExpression(o) => {
                for m in &o.properties {
                    let ObjectMember::Property(p) = m else { continue };
                    if p.computed {
                        continue;
                    }
                    let PropertyKey::Expression(k) = &p.key else { continue };
                    match &**k {
                        Expression::Identifier(id) => {
                            let t = Token { class: &id.name, span: (id.start, id.end), editable: false };
                            self.token(&t, kind, diags);
                        }
                        k => self.clsx(k, kind, diags),
                    }
                }
            }
            Expression::ConditionalExpression(c) => {
                self.clsx(&c.consequent, kind, diags);
                self.clsx(&c.alternate, kind, diags);
            }
            Expression::LogicalExpression(l) => {
                if l.operator != LogicalOperator::And {
                    self.clsx(&l.left, kind, diags);
                }
                self.clsx(&l.right, kind, diags);
            }
            Expression::ParenthesizedExpression(p) => self.clsx(&p.expression, kind, diags),
            // `clsx(...)`, `cn(...)`
            Expression::CallExpression(c) if matches!(&*c.callee, Expression::Identifier(_)) => {
                for a in &c.arguments {
                    if let ArrayElement::Expression(x) = a {
                        self.clsx(x, kind, diags);
                    }
                }
            }
            _ => {}
        }
    }

    fn template_tokens<'s>(&self, t: &'s TemplateLiteral, diags: &mut Diagnostics) -> Vec<Token<'s>> {
        let mut tokens = Vec::new();
        for (i, q) in t.quasis.iter().enumerate() {
            if q.value.cooked.as_deref() != Some(q.value.raw.as_str()) {
                continue;
            }
            let next = t.expressions.get(i).filter(|_| !q.tail).map(|e| e.span().end + 1);
            self.text_tokens(&q.value.raw, q.start, i > 0, next, &mut tokens, diags);
        }
        tokens
    }

    /// `<script>` strings (`scan_script`): the resolver's verdict only (no element, no list).
    fn script(&self, n: NodeRef<'_>, diags: &mut Diagnostics) {
        match n {
            NodeRef::ImportDeclaration(_) | NodeRef::ExportAllDeclaration(_) => return,
            NodeRef::Literal(l) => {
                if let LiteralValue::String(_) = l.value
                    && let Some(text) = literal_text(&l.raw)
                {
                    let mut tokens = Vec::new();
                    self.text_tokens(text, l.start + 1, false, None, &mut tokens, &mut Diagnostics::default());
                    for t in &tokens {
                        self.token(t, None, diags);
                    }
                }
                return;
            }
            NodeRef::TemplateLiteral(t) => {
                for t in &self.template_tokens(t, &mut Diagnostics::default()) {
                    self.token(t, None, diags);
                }
            }
            _ => {}
        }
        for c in n.children() {
            self.script(c, diags);
        }
    }

    /// One static list: each token, then the conflicts between them.
    fn list(&self, tokens: &[Token], kind: Option<&KindStyle>, diags: &mut Diagnostics) {
        let mut resolved: Vec<(Token, Utility)> = Vec::new();
        for t in tokens {
            if let Some(u) = self.token(t, kind, diags) {
                resolved.push((*t, u));
            }
        }
        self.conflicts(&resolved, diags);
    }

    /// Checks one token (on an element of kind `kind` when known); the resolved utility, when it is one.
    pub(crate) fn token(&self, t: &Token, kind: Option<&KindStyle>, diags: &mut Diagnostics) -> Option<Utility> {
        match self.resolver.resolve(t.class) {
            Resolved::NotUtility => None,
            Resolved::Invalid(i) => {
                let bare = !t.class.contains([':', '-', '[', '(', '/']);
                if bare && self.prefix.is_none() && matches!(i.kind, IssueKind::Unsupported { .. }) {
                    // `block`, `fill`, `content`: as likely a semantic class
                    return None;
                }
                self.report(&i, t, None, diags);
                None
            }
            Resolved::Utility(u) => {
                for n in &u.notes {
                    self.report(n, t, None, diags);
                }
                self.part_aliases(t, diags);
                if let Some(k) = kind {
                    self.per_kind(t, &u, k, diags);
                }
                Some(u)
            }
        }
    }

    /// Reports a resolver issue for `t` with its fix-its. `code` overrides the code (`@apply`).
    pub(crate) fn report(&self, i: &UtilityIssue, t: &Token, code: Option<&'static str>, diags: &mut Diagnostics) {
        let fixes = if t.editable { self.fixes(i, t) } else { Vec::new() };
        let d: &mut Diagnostic = match (code, &i.kind) {
            (Some(c), _) => diags.error(c, t.span, i.message.clone()),
            (None, IssueKind::UnknownValue | IssueKind::UnknownVariant) => diags.warning(UNKNOWN, t.span, i.message.clone()),
            (None, IssueKind::Unsupported { .. } | IssueKind::Pending { .. }) => diags.warning(UNSUPPORTED, t.span, i.message.clone()),
            (None, IssueKind::BadValue) => diags.error(BAD_VALUE, t.span, format!("`{}`: {}", i.class, i.message)),
            (None, IssueKind::ArbitraryColor) if i.severity == Severity::Error => diags.error(ARBITRARY_COLOR, t.span, i.message.clone()),
            (None, IssueKind::ArbitraryColor) => diags.warning(ARBITRARY_COLOR, t.span, i.message.clone()),
            (None, IssueKind::Alias { .. }) => diags.note(ALIAS, t.span, i.message.clone()),
            (None, _) => diags.warning(UNKNOWN, t.span, i.message.clone()),
        };
        if let IssueKind::Unsupported { replacement: Some(r) } = &i.kind {
            d.help(format!("use {}", if r.contains(' ') { r.to_string() } else { format!("`{r}`") }));
        }
        for f in fixes {
            d.fix(f);
        }
    }

    /// The fix-its of a resolver issue, each verified to resolve.
    fn fixes(&self, i: &UtilityIssue, t: &Token) -> Vec<FixIt> {
        let segs = segments(t.class);
        let Some(&(bs, be)) = segs.last() else { return Vec::new() };
        // the base without a leading or trailing `!`
        let base = &t.class[bs..be];
        let (bs, be) = (bs + usize::from(base.starts_with('!')), be - usize::from(base.ends_with('!') && base.len() > 1));
        let mut out = Vec::new();
        let mut offer = |range: (usize, usize), with: &str, label: String| {
            let new = format!("{}{with}{}", &t.class[..range.0], &t.class[range.1..]);
            if out.len() < 3 && matches!(self.resolver.resolve(&new), Resolved::Utility(_)) {
                let at = t.span.0 + range.0 as u32;
                out.push(FixIt::new(label, vec![TextEdit { start: at, end: t.span.0 + range.1 as u32, text: with.to_string() }]));
            }
        };
        match &i.kind {
            IssueKind::UnknownValue => {
                for s in &i.suggestions {
                    offer((bs, be), s, format!("replace with `{s}`"));
                }
            }
            IssueKind::UnknownVariant => {
                let vs = self.variant_ranges(t.class, &segs);
                for (r, v) in vs {
                    if self.variant_known(v) {
                        continue;
                    }
                    for s in crate::semantics::closest(v, variant_names().iter().map(|s| s.as_str())) {
                        offer(r, &s, format!("replace `{v}:` with `{s}:`"));
                    }
                    break;
                }
            }
            IssueKind::Unsupported { replacement: Some(rep) } if !rep.contains([' ', '<']) => {
                if let Some(v) = rep.strip_suffix(':') {
                    // a variant: the unsupported one is the first that isn't known
                    if let Some((r, old)) = self.variant_ranges(t.class, &segs).into_iter().find(|(_, v)| !self.variant_known(v)) {
                        offer(r, v, format!("replace `{old}:` with `{v}:`"));
                    }
                } else if let Some(stem) = rep.strip_suffix("-*") {
                    // `ring-2` → `outline-2`: the unsupported name's stem, keeping the value
                    let core = &t.class[bs..be];
                    if let Some(u) = svelters_style::utilities::UNSUPPORTED_UTILITIES
                        .iter()
                        .filter(|u| core.strip_prefix(u.name).is_some_and(|r| r.starts_with('-')) && u.replacement == Some(rep))
                        .max_by_key(|u| u.name.len())
                    {
                        offer((bs, bs + u.name.len()), stem, format!("replace `{}` with `{stem}`", u.name));
                    }
                } else {
                    offer((bs, be), rep, format!("replace with `{rep}`"));
                }
            }
            IssueKind::Alias { canonical } => {
                if let Some((r, old)) = self.variant_ranges(t.class, &segs).into_iter().find(|(_, v)| ALIASES.iter().any(|a| a.0 == *v && a.1 == *canonical)) {
                    offer(r, canonical, format!("replace `{old}:` with `{canonical}:`"));
                }
            }
            _ => {}
        }
        out
    }

    /// The variant segments of a class (byte ranges and text), the prefix left out.
    fn variant_ranges<'c>(&self, class: &'c str, segs: &[(usize, usize)]) -> Vec<((usize, usize), &'c str)> {
        let mut vs: Vec<((usize, usize), &str)> = segs[..segs.len().saturating_sub(1)].iter().map(|&(a, b)| ((a, b), &class[a..b])).collect();
        if let (Some(p), Some(first)) = (self.prefix, vs.first())
            && first.1 == p
        {
            vs.remove(0);
        }
        vs
    }

    /// Whether `v` alone is a variant the resolver takes (`p-0` as the probe utility).
    fn variant_known(&self, v: &str) -> bool {
        let probe = match self.prefix {
            Some(p) => format!("{p}:{v}:p-0"),
            None => format!("{v}:p-0"),
        };
        matches!(self.resolver.resolve(&probe), Resolved::Utility(_))
    }

    /// `[&::part(x)]:` is accepted; its canonical spelling is `part-x:` (`utility_alias`).
    fn part_aliases(&self, t: &Token, diags: &mut Diagnostics) {
        let segs = segments(t.class);
        for (r, v) in self.variant_ranges(t.class, &segs) {
            let Some(name) = v.strip_prefix("[&::part(").and_then(|r| r.strip_suffix(")]")) else { continue };
            if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') {
                continue;
            }
            let canonical = format!("part-{name}");
            let d = diags.note(ALIAS, t.span, format!("`{v}:` is spelled `{canonical}:` here"));
            if t.editable {
                d.fix(FixIt::new(format!("replace with `{canonical}:`"), vec![TextEdit { start: t.span.0 + r.0 as u32, end: t.span.0 + r.1 as u32, text: canonical }]));
            }
        }
    }

    /// `utility_variant_never_matches` and `utility_not_applicable` for a built-in kind.
    fn per_kind(&self, t: &Token, u: &Utility, k: &KindStyle, diags: &mut Diagnostics) {
        let segs = segments(t.class);
        // whose state a state variant tests: the element's, until the subject moves to another
        // element (`[&>row]:`, `has-`, `in-`); a paint part reads its host's state (DOC2)
        let mut own = true;
        let mut part = false;
        for (r, v) in self.variant_ranges(t.class, &segs) {
            if let Some(name) = v.strip_prefix("part-").or_else(|| v.strip_prefix("[&::part(").and_then(|r| r.strip_suffix(")]"))) {
                part = true;
                match k.parts.iter().find(|p| p.0 == name) {
                    // a node part has states of its own (none are declared yet)
                    Some(p) if p.1 => own = false,
                    Some(_) => {}
                    None if UNDECLARED_PARTS.contains(&name) => own = false,
                    None => {
                        let names: Vec<&str> = k.parts.iter().map(|p| p.0).collect();
                        let msg = if names.is_empty() {
                            format!("`<{}>` has no parts, so `{v}:` never matches", k.tag)
                        } else {
                            format!("`<{}>` has no part `{name}` (its parts: {}), so `{v}:` never matches", k.tag, names.join(", "))
                        };
                        let d = diags.warning(VARIANT_NEVER_MATCHES, t.span, msg);
                        if t.editable && v.starts_with("part-") {
                            for s in crate::semantics::closest(name, names.iter().copied()) {
                                let at = t.span.0 + r.0 as u32 + "part-".len() as u32;
                                d.fix(FixIt::new(format!("replace with `part-{s}:`"), vec![TextEdit { start: at, end: t.span.0 + r.1 as u32, text: s }]));
                            }
                        }
                        return;
                    }
                }
                continue;
            }
            if PSEUDO_ELEMENTS.iter().any(|p| p.0 == v) {
                part = true;
                own = false;
                continue;
            }
            if v.starts_with('[') || COMPOUND.iter().any(|c| c.name != "not" && v.strip_prefix(c.name).is_some_and(|r| r.starts_with('-'))) {
                own = false;
                continue;
            }
            if let Some(state) = v.strip_prefix("state-[").and_then(|r| r.strip_suffix(']')) {
                // svelters' own provider states are known per kind; an app's register at run time
                if own && state.starts_with("sv.") {
                    let msg = if !PROVIDER_STATES.contains(&state) {
                        Some(format!("`{state}` is not a provider state of svelters (known: {})", PROVIDER_STATES.join(", ")))
                    } else if !k.provider_states.contains(&state) {
                        Some(format!("`<{}>` never sets `{state}`, so `{v}:` never matches", k.tag))
                    } else {
                        None
                    };
                    if let Some(m) = msg {
                        diags.warning(VARIANT_NEVER_MATCHES, t.span, m);
                    }
                }
                continue;
            }
            if !own || k.states.is_empty() {
                continue;
            }
            let state = STATE_VARIANTS.iter().find(|s| s.name == v).map(|s| s.name).or_else(|| ALIASES.iter().find(|a| a.0 == v).map(|a| a.1));
            let Some(state) = state else { continue };
            let looked_up = if state == "focus-visible" { "focus" } else { state };
            if !k.states.contains(&looked_up) {
                diags.warning(VARIANT_NEVER_MATCHES, t.span, format!("`<{}>` never has the `{state}` state, so `{v}:` never matches", k.tag));
            }
        }
        if part || !own {
            return;
        }
        for (p, _) in &u.decls {
            if let Some(prop) = self.table.get(p)
                && !prop.applies_to(k.tag)
            {
                diags.warning(NOT_APPLICABLE, t.span, format!("`{p}` doesn't apply to `<{}>` on {}, so `{}` has no effect", k.tag, self.table.toolkit, t.class));
                break;
            }
        }
    }

    /// `utility_conflict`: in one static list, two utilities with the same variants and the same
    /// properties. The one later in the canonical order wins (vocabulary row, then class name);
    /// the other is reported, with a fix-it removing it.
    fn conflicts(&self, list: &[(Token, Utility)], diags: &mut Diagnostics) {
        fn props(u: &Utility) -> Vec<&str> {
            let mut p: Vec<&str> = u.decls.iter().map(|d| &*d.0).collect();
            p.sort_unstable();
            p
        }
        let mut reported = vec![false; list.len()];
        for i in 0..list.len() {
            for j in i + 1..list.len() {
                let ((ta, a), (tb, b)) = (&list[i], &list[j]);
                if a.important != b.important || a.media != b.media || a.class == b.class || props(a) != props(b) {
                    continue;
                }
                // the same variants: the selectors are equal once each class is taken out
                if relative_selector(a) != relative_selector(b) {
                    continue;
                }
                let a_wins = (a.canonical_order(), &*a.class) > (b.canonical_order(), &*b.class);
                let (loser, lt, winner) = if a_wins { (j, tb, ta) } else { (i, ta, tb) };
                if reported[loser] {
                    continue;
                }
                reported[loser] = true;
                let d = diags.warning(CONFLICT, lt.span, format!("`{}` and `{}` set the same properties; `{}` wins, so `{}` never applies", ta.class, tb.class, winner.class, lt.class));
                if lt.editable {
                    d.fix(FixIt::new(format!("remove `{}`", lt.class), vec![self.removal(lt.span)]));
                }
            }
        }
    }

    /// Deletes the token at `span` and the white space after it (else before it).
    fn removal(&self, span: (u32, u32)) -> TextEdit {
        let b = self.src.as_bytes();
        let (mut s, mut e) = (span.0 as usize, span.1 as usize);
        let after = b[e..].iter().take_while(|c| c.is_ascii_whitespace() && **c != b'\n').count();
        if after > 0 && b.get(e + after).is_some_and(|c| !matches!(c, b'"' | b'\'' | b'`')) {
            e += after;
        } else {
            s -= b[..s].iter().rev().take_while(|c| c.is_ascii_whitespace() && **c != b'\n').count();
        }
        TextEdit { start: s as u32, end: e as u32, text: String::new() }
    }
}

/// A utility's selector relative to its class: `.hover\:p-2:hover` → `&:hover`.
pub(crate) fn relative_selector(u: &Utility) -> String {
    let mut esc = String::from(".");
    let _ = cssparser::serialize_identifier(&u.class, &mut esc);
    u.selector.replace(&esc, "&")
}

/// The `:`-separated segments of a class outside brackets (byte ranges); the last is the base.
pub(crate) fn segments(class: &str) -> Vec<(usize, usize)> {
    let b = class.as_bytes();
    let (mut out, mut depth, mut start, mut i) = (Vec::new(), 0i32, 0usize, 0usize);
    while i < b.len() {
        match b[i] {
            b'\\' => i += 1,
            b'[' | b'(' => depth += 1,
            b']' | b')' => depth -= 1,
            b':' if depth == 0 => {
                out.push((start, i));
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    out.push((start, class.len()));
    out
}

/// Every named variant, for "did you mean".
fn variant_names() -> &'static [String] {
    static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    NAMES.get_or_init(|| {
        let mut v: Vec<String> = STATE_VARIANTS.iter().map(|s| s.name.to_string()).collect();
        v.extend(PSEUDO_ELEMENTS.iter().map(|p| p.0.to_string()));
        v.extend(STRUCTURAL.iter().map(|p| p.0.to_string()));
        v.extend(MEDIA.iter().map(|m| m.name.to_string()));
        v.extend(BREAKPOINTS.sizes.iter().map(|s| s.0.to_string()));
        v
    })
}

/// The text of a string literal's source (`raw` with its quotes) when it has no escapes, so byte
/// offsets in it are offsets in the source.
fn literal_text(raw: &str) -> Option<&str> {
    let inner = raw.get(1..raw.len().checked_sub(1)?)?;
    (!inner.contains('\\')).then_some(inner)
}

/// `utility_shadowed_class`: a class selector of the component's `<style>` that is also a utility
/// (the component's rule and the generated one both style elements with that class).
fn shadowed(root: &Root, src: &str, cx: &Cx, diags: &mut Diagnostics) {
    use crate::svelte::css::ast::{Node, SimpleKind};
    let Ok(Some(sheet)) = crate::svelte::css::parse_style(src, root) else { return };
    fn walk(nodes: &[Node], cx: &Cx, diags: &mut Diagnostics) {
        for n in nodes {
            match n {
                Node::Rule(r) => {
                    for c in &r.prelude.children {
                        for rel in &c.children {
                            for s in rel.selectors.iter().filter(|s| s.kind == SimpleKind::ClassSelector) {
                                let name = unescape(&s.name);
                                if let Resolved::Utility(_) = cx.resolver.resolve(&name) {
                                    diags.warning(SHADOWED_CLASS, (s.start as u32, s.end as u32), format!("`.{name}` is also a utility class: elements with class `{name}` get both this rule and the utility's"));
                                }
                            }
                        }
                    }
                    walk(&r.block.children, cx, diags);
                }
                Node::Atrule(a) => {
                    if let Some(b) = &a.block {
                        walk(&b.children, cx, diags);
                    }
                }
                Node::Declaration(_) => {}
            }
        }
    }
    walk(&sheet.children, cx, diags);
}

/// A CSS identifier's backslash escapes removed (`hover\:p-2` → `hover:p-2`).
fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(n) = chars.next() {
                out.push(n);
            }
        } else {
            out.push(c);
        }
    }
    out
}
