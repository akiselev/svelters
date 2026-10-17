//! Port of `phases/1-parse/remove_typescript_nodes.js` (Svelte 5.57.2 @ f3de3f2). Owner: agent TS.
//!
//! Upstream runs it in `compile()` (not `parse()`) on `fragment`, `instance` and `module` when
//! the component is `lang="ts"`: TypeScript-only nodes are replaced by their JavaScript operand or
//! by `b.empty`, type properties are deleted, and unsupported features (enums, decorators,
//! accessor fields, parameter properties of constructors, namespaces with values) are
//! `typescript_invalid_feature` errors.
//!
//! SVELTERS: upstream reprints the stripped AST (esrap); this compiler emits the user's text
//! through span edits, so the walk also records the source ranges that hold the removed
//! TypeScript ([`TsStrip`], applied by `crate::svelte::ts`). Deliberate differences
//! (docs/notes/native-compiler/deviations.md, "remove_typescript_nodes"):
//! - a removed statement becomes an `EmptyStatement` with the original span; a removed class
//!   member (abstract method, `declare` field) is dropped from the class body;
//! - nodes upstream leaves for esrap to print as TypeScript are removed as well: method overloads
//!   (`TSDeclareMethod` values), class index signatures, `TSTypeCastExpression` (`(x: T)` that is
//!   not an arrow parameter list);
//! - `import x = require()`, `export =`, `export as namespace` are `typescript_invalid_feature`
//!   errors (upstream would print them unchanged);
//! - a nested `namespace A.B {}` is checked like a flat one (upstream throws a TypeError).

use crate::estree::*;
use crate::svelte::ast::{self, AttributeLike, AttributeValue, ExpressionTagNode, Fragment, FragmentNode, Root, TextOrExpressionTag};
use crate::svelte::errors::{self as e, CompileError};

/// Source ranges (byte offsets, `[start, end)`) holding TypeScript removed from the AST.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TsStrip {
    pub ranges: Vec<(u32, u32)>,
}

type R<T> = Result<T, CompileError>;

/// `remove_typescript_nodes` on the component's `fragment`, `instance` and `module`.
pub fn remove_typescript_nodes(root: &mut Root, src: &str, strip: Option<&mut TsStrip>) -> R<()> {
    let mut rt = Rt { src, strip };
    rt.fragment(&mut root.fragment)?;
    if let Some(s) = &mut root.instance {
        rt.stmts(&mut s.content.body)?;
    }
    if let Some(s) = &mut root.module {
        rt.stmts(&mut s.content.body)?;
    }
    Ok(())
}

/// Removes TypeScript from one (unwrapped) expression, e.g. a template expression parsed alone.
pub fn remove_typescript_nodes_expression(expr: &mut Expression, src: &str, strip: Option<&mut TsStrip>) -> R<()> {
    Rt { src, strip }.expr(expr)
}

const MODIFIER_WORDS: [&str; 8] = ["public", "private", "protected", "readonly", "override", "abstract", "declare", "accessor"];

struct Rt<'s> {
    src: &'s str,
    strip: Option<&'s mut TsStrip>,
}

fn take_expr(e: &mut Expression) -> Expression {
    std::mem::replace(e, Expression::ThisExpression(ThisExpression { start: 0, end: 0 }))
}

fn take_pattern(p: &mut Pattern) -> Pattern {
    std::mem::replace(p, Pattern::Identifier(Identifier { start: 0, end: 0, name: String::new(), ts: None }))
}

fn empty(start: u32, end: u32) -> Statement {
    Statement::EmptyStatement(EmptyStatement { start, end })
}

impl<'s> Rt<'s> {
    // --- strip ranges

    fn cut(&mut self, start: u32, end: u32) {
        if let Some(s) = self.strip.as_deref_mut()
            && end > start
        {
            s.ranges.push((start, end));
        }
    }

    fn bytes(&self) -> &[u8] {
        self.src.as_bytes()
    }

    /// Skips whitespace forwards from `p`.
    fn ws_fwd(&self, mut p: u32) -> u32 {
        while (p as usize) < self.src.len() && self.bytes()[p as usize].is_ascii_whitespace() {
            p += 1;
        }
        p
    }

    /// Skips whitespace backwards from `p` (exclusive end).
    fn ws_back(&self, mut p: u32) -> u32 {
        while p > 0 && self.bytes()[p as usize - 1].is_ascii_whitespace() {
            p -= 1;
        }
        p
    }

    /// Start of `word` ending (modulo whitespace) at `before`.
    fn word_before(&self, before: u32, word: &str) -> Option<u32> {
        let p = self.ws_back(before) as usize;
        if self.src.get(..p)?.ends_with(word) { Some((p - word.len()) as u32) } else { None }
    }

    /// The end of an identifier's name (its `end` may include `?`, `!` and the annotation).
    fn name_end(&self, id: &Identifier) -> u32 {
        let s = id.start as usize;
        if self.src.get(s..s + id.name.len()) == Some(id.name.as_str()) {
            return (s + id.name.len()) as u32;
        }
        // escapes in the name: scan identifier characters
        let mut p = s;
        for c in self.src[s..].chars() {
            if c.is_alphanumeric() || c == '_' || c == '$' || c == '\\' {
                p += c.len_utf8();
            } else {
                break;
            }
        }
        p as u32
    }

    /// Strips the TypeScript words (`public`, `readonly`, ...) in `[start, end)`.
    fn cut_modifier_words(&mut self, start: u32, end: u32) {
        let text = &self.src[start as usize..end as usize];
        let mut words = Vec::new();
        let mut i = 0;
        let b = text.as_bytes();
        while i < b.len() {
            if b[i].is_ascii_alphabetic() {
                let w0 = i;
                while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'$') {
                    i += 1;
                }
                let word = &text[w0..i];
                if MODIFIER_WORDS.contains(&word) {
                    let mut w1 = i;
                    while w1 < b.len() && b[w1].is_ascii_whitespace() {
                        w1 += 1;
                    }
                    words.push((start + w0 as u32, start + w1 as u32));
                }
            } else {
                i += 1;
            }
        }
        for (a, z) in words {
            self.cut(a, z);
        }
    }

    /// `delete n.typeAnnotation; delete n.optional` (+ `decorators`: an error) on a pattern.
    fn pattern_extra(&mut self, p: &mut Pattern) -> R<()> {
        let (start, end, name_end, ts) = match p {
            Pattern::Identifier(n) => {
                let ne = self.name_end(n);
                (n.start, n.end, Some(ne), n.ts.take())
            }
            Pattern::ObjectPattern(n) => (n.start, n.end, None, n.ts.take()),
            Pattern::ArrayPattern(n) => (n.start, n.end, None, n.ts.take()),
            Pattern::RestElement(n) => (n.start, n.end, None, n.ts.take()),
            Pattern::AssignmentPattern(n) => (n.start, n.end, None, n.ts.take()),
            _ => return Ok(()),
        };
        let Some(ts) = ts else { return Ok(()) };
        if let Some(d) = ts.decorators.as_ref().and_then(|d| d.first()) {
            return Err(e::typescript_invalid_feature(d.span(), "decorators (related TSC proposal is not stage 4 yet)"));
        }
        let _ = start;
        match (name_end, &ts.type_annotation) {
            (Some(ne), Some(ta)) => self.cut(ne, ta.end),
            (Some(ne), None) if ts.optional => self.cut(ne, end),
            (None, Some(ta)) => {
                let mut a = ta.start;
                if ts.optional {
                    let q = self.ws_back(a);
                    if q > 0 && self.bytes()[q as usize - 1] == b'?' {
                        a = q - 1;
                    }
                }
                self.cut(a, ta.end)
            }
            _ => {}
        }
        Ok(())
    }

    /// The `?` / `!` after a class member key, if any (`start`: the key's end).
    fn cut_member_marker(&mut self, from: u32, computed: bool) {
        let mut p = self.ws_fwd(from);
        if computed && self.bytes().get(p as usize) == Some(&b']') {
            p = self.ws_fwd(p + 1);
        }
        if matches!(self.bytes().get(p as usize), Some(b'?') | Some(b'!')) {
            self.cut(p, p + 1);
        }
    }

    // --- template

    fn fragment(&mut self, f: &mut Fragment) -> R<()> {
        for n in &mut f.nodes {
            self.fragment_node(n)?;
        }
        Ok(())
    }

    fn opt_fragment(&mut self, f: &mut Option<Fragment>) -> R<()> {
        if let Some(f) = f {
            self.fragment(f)?;
        }
        Ok(())
    }

    fn attributes(&mut self, attrs: &mut [AttributeLike]) -> R<()> {
        for a in attrs {
            match a {
                AttributeLike::Attribute(a) => self.attribute_value(&mut a.value)?,
                AttributeLike::SpreadAttribute(s) => self.expr(&mut s.expression)?,
                AttributeLike::AttachTag(t) => self.expr(&mut t.expression)?,
                AttributeLike::AnimateDirective(d)
                | AttributeLike::BindDirective(d)
                | AttributeLike::ClassDirective(d)
                | AttributeLike::LetDirective(d)
                | AttributeLike::OnDirective(d)
                | AttributeLike::UseDirective(d) => {
                    if let Some(x) = &mut d.expression {
                        self.expr(x)?;
                    }
                }
                AttributeLike::StyleDirective(d) => self.attribute_value(&mut d.value)?,
                AttributeLike::TransitionDirective(d) => {
                    if let Some(x) = &mut d.expression {
                        self.expr(x)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn attribute_value(&mut self, v: &mut AttributeValue) -> R<()> {
        match v {
            AttributeValue::True(_) => {}
            AttributeValue::Expression(ExpressionTagNode::ExpressionTag(t)) => self.expr(&mut t.expression)?,
            AttributeValue::Sequence(parts) => {
                for p in parts {
                    if let TextOrExpressionTag::ExpressionTag(t) = p {
                        self.expr(&mut t.expression)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn element(&mut self, el: &mut ast::Element) -> R<()> {
        self.attributes(&mut el.attributes)?;
        self.fragment(&mut el.fragment)
    }

    fn fragment_node(&mut self, n: &mut FragmentNode) -> R<()> {
        match n {
            FragmentNode::Text(_) | FragmentNode::Comment(_) => {}
            FragmentNode::ExpressionTag(t) => self.expr(&mut t.expression)?,
            FragmentNode::HtmlTag(t) => self.expr(&mut t.expression)?,
            FragmentNode::ConstTag(t) => self.var_decl(&mut t.declaration)?,
            FragmentNode::DeclarationTag(t) => self.var_decl(&mut t.declaration)?,
            FragmentNode::DebugTag(_) => {}
            FragmentNode::RenderTag(t) => self.expr(&mut t.expression)?,
            FragmentNode::AttachTag(t) => self.expr(&mut t.expression)?,
            FragmentNode::Component(el)
            | FragmentNode::TitleElement(el)
            | FragmentNode::SlotElement(el)
            | FragmentNode::RegularElement(el)
            | FragmentNode::SvelteBody(el)
            | FragmentNode::SvelteDocument(el)
            | FragmentNode::SvelteFragment(el)
            | FragmentNode::SvelteBoundary(el)
            | FragmentNode::SvelteHead(el)
            | FragmentNode::SvelteOptionsRaw(el)
            | FragmentNode::SvelteSelf(el)
            | FragmentNode::SvelteWindow(el) => self.element(el)?,
            FragmentNode::SvelteComponent(c) => {
                self.attributes(&mut c.attributes)?;
                self.fragment(&mut c.fragment)?;
                self.expr(&mut c.expression)?;
            }
            FragmentNode::SvelteElement(c) => {
                self.attributes(&mut c.attributes)?;
                self.fragment(&mut c.fragment)?;
                self.expr(&mut c.tag)?;
            }
            FragmentNode::EachBlock(b) => {
                self.expr(&mut b.expression)?;
                if let Some(c) = &mut b.context {
                    self.pattern(c)?;
                }
                self.fragment(&mut b.body)?;
                self.opt_fragment(&mut b.fallback)?;
                if let Some(k) = &mut b.key {
                    self.expr(k)?;
                }
            }
            FragmentNode::IfBlock(b) => {
                self.expr(&mut b.test)?;
                self.fragment(&mut b.consequent)?;
                self.opt_fragment(&mut b.alternate)?;
            }
            FragmentNode::AwaitBlock(b) => {
                self.expr(&mut b.expression)?;
                if let Some(v) = &mut b.value {
                    self.pattern(v)?;
                }
                if let Some(v) = &mut b.error {
                    self.pattern(v)?;
                }
                self.opt_fragment(&mut b.pending)?;
                self.opt_fragment(&mut b.then)?;
                self.opt_fragment(&mut b.catch)?;
            }
            FragmentNode::KeyBlock(b) => {
                self.expr(&mut b.expression)?;
                self.fragment(&mut b.fragment)?;
            }
            FragmentNode::SnippetBlock(b) => {
                if let Some(tp) = b.type_params.take() {
                    // `#snippet name<T>(...)`: the `<T>` after the name
                    let lt = self.ws_fwd(b.expression.end);
                    if self.bytes().get(lt as usize) == Some(&b'<') {
                        self.cut(lt, lt + tp.len() as u32 + 2);
                    }
                }
                for p in &mut b.parameters {
                    self.pattern(p)?;
                }
                self.fragment(&mut b.body)?;
            }
        }
        Ok(())
    }

    // --- statements

    fn stmts(&mut self, body: &mut [Statement]) -> R<()> {
        for s in body.iter_mut() {
            self.stmt(s)?;
        }
        Ok(())
    }

    /// Visits a statement; replaces it by `b.empty` (an EmptyStatement) when upstream does.
    /// Returns whether it became empty.
    fn stmt(&mut self, s: &mut Statement) -> R<bool> {
        let span = s.span();
        let remove = match s {
            Statement::ImportDeclaration(n) => self.import_declaration(n),
            Statement::ExportNamedDeclaration(n) => self.export_named_declaration(n)?,
            Statement::ExportDefaultDeclaration(n) => {
                if n.ts.as_ref().and_then(|t| t.export_kind) == Some("type") {
                    true
                } else {
                    n.ts = None;
                    match &mut n.declaration {
                        ExportDefaultKind::Declaration(d) => self.stmt(d)?,
                        ExportDefaultKind::Expression(x) => {
                            self.expr(x)?;
                            false
                        }
                    }
                }
            }
            Statement::ExportAllDeclaration(n) => {
                let ty = n.ts.as_ref().and_then(|t| t.export_kind) == Some("type");
                n.ts = None;
                ty
            }
            Statement::TSInterfaceDeclaration(_) | Statement::TSTypeAliasDeclaration(_) | Statement::TSDeclareFunction(_) => true,
            Statement::TSEnumDeclaration(n) => return Err(e::typescript_invalid_feature(n.span(), "enums")),
            Statement::TSModuleDeclaration(n) => self.module_declaration(n)?,
            Statement::TSImportEqualsDeclaration(n) => {
                return Err(e::typescript_invalid_feature(n.span(), "`import =` declarations"));
            }
            Statement::TSExportAssignment(n) => return Err(e::typescript_invalid_feature(n.span(), "`export =` assignments")),
            Statement::TSNamespaceExportDeclaration(n) => {
                return Err(e::typescript_invalid_feature(n.span(), "`export as namespace` declarations"));
            }
            Statement::VariableDeclaration(d) => {
                if d.ts.as_ref().is_some_and(|t| t.declare) {
                    true
                } else {
                    self.var_decl(d)?;
                    false
                }
            }
            Statement::ClassDeclaration(c) => {
                if c.ts.as_ref().is_some_and(|t| t.declare) {
                    true
                } else {
                    self.class(c)?;
                    false
                }
            }
            Statement::FunctionDeclaration(f) => {
                self.function(f)?;
                false
            }
            Statement::ExpressionStatement(n) => {
                self.expr(&mut n.expression)?;
                false
            }
            Statement::BlockStatement(n) => {
                self.stmts(&mut n.body)?;
                false
            }
            Statement::StaticBlock(n) => {
                self.stmts(&mut n.body)?;
                false
            }
            Statement::EmptyStatement(_)
            | Statement::DebuggerStatement(_)
            | Statement::BreakStatement(_)
            | Statement::ContinueStatement(_) => false,
            Statement::WithStatement(n) => {
                self.expr(&mut n.object)?;
                self.stmt(&mut n.body)?;
                false
            }
            Statement::ReturnStatement(n) => {
                if let Some(a) = &mut n.argument {
                    self.expr(a)?;
                }
                false
            }
            Statement::LabeledStatement(n) => {
                self.stmt(&mut n.body)?;
                false
            }
            Statement::IfStatement(n) => {
                self.expr(&mut n.test)?;
                self.stmt(&mut n.consequent)?;
                if let Some(a) = &mut n.alternate {
                    self.stmt(a)?;
                }
                false
            }
            Statement::SwitchStatement(n) => {
                self.expr(&mut n.discriminant)?;
                for c in &mut n.cases {
                    if let Some(t) = &mut c.test {
                        self.expr(t)?;
                    }
                    self.stmts(&mut c.consequent)?;
                }
                false
            }
            Statement::ThrowStatement(n) => {
                self.expr(&mut n.argument)?;
                false
            }
            Statement::TryStatement(n) => {
                self.stmts(&mut n.block.body)?;
                if let Some(h) = &mut n.handler {
                    if let Some(p) = &mut h.param {
                        self.pattern(p)?;
                    }
                    self.stmts(&mut h.body.body)?;
                }
                if let Some(f) = &mut n.finalizer {
                    self.stmts(&mut f.body)?;
                }
                false
            }
            Statement::WhileStatement(n) => {
                self.expr(&mut n.test)?;
                self.stmt(&mut n.body)?;
                false
            }
            Statement::DoWhileStatement(n) => {
                self.stmt(&mut n.body)?;
                self.expr(&mut n.test)?;
                false
            }
            Statement::ForStatement(n) => {
                match &mut n.init {
                    Some(ForInit::VariableDeclaration(VariableDeclarationNode::VariableDeclaration(d))) => self.var_decl(d)?,
                    Some(ForInit::Expression(x)) => self.expr(x)?,
                    None => {}
                }
                if let Some(t) = &mut n.test {
                    self.expr(t)?;
                }
                if let Some(u) = &mut n.update {
                    self.expr(u)?;
                }
                self.stmt(&mut n.body)?;
                false
            }
            Statement::ForInStatement(n) => {
                self.for_left(&mut n.left)?;
                self.expr(&mut n.right)?;
                self.stmt(&mut n.body)?;
                false
            }
            Statement::ForOfStatement(n) => {
                self.for_left(&mut n.left)?;
                self.expr(&mut n.right)?;
                self.stmt(&mut n.body)?;
                false
            }
        };
        if remove {
            self.cut(span.start, span.end);
            *s = empty(span.start, span.end);
        }
        Ok(remove)
    }

    fn for_left(&mut self, l: &mut ForLeft) -> R<()> {
        match l {
            ForLeft::VariableDeclaration(VariableDeclarationNode::VariableDeclaration(d)) => self.var_decl(d),
            ForLeft::Pattern(p) => self.pattern(p),
        }
    }

    fn var_decl(&mut self, d: &mut VariableDeclaration) -> R<()> {
        d.ts = None;
        for decl in &mut d.declarations {
            // `let x!: T`: `pattern_extra` cuts from the name's end; `let x!;` cuts the `!`
            if decl.ts.take().is_some_and(|t| t.definite)
                && let Pattern::Identifier(id) = &decl.id
                && id.ts.as_ref().is_none_or(|t| t.type_annotation.is_none())
            {
                let ne = self.name_end(id);
                let p = self.ws_fwd(ne);
                if self.bytes().get(p as usize) == Some(&b'!') {
                    self.cut(p, p + 1);
                }
            }
            self.pattern(&mut decl.id)?;
            if let Some(init) = &mut decl.init {
                self.expr(init)?;
            }
        }
        Ok(())
    }

    /// `ImportDeclaration(node)`: true when it becomes `b.empty`.
    fn import_declaration(&mut self, n: &mut ImportDeclaration) -> bool {
        let ts = n.ts.take();
        if ts.as_ref().is_some_and(|t| t.import_kind == "type") {
            return true;
        }
        if !n.specifiers.is_empty() {
            let is_type = |s: &ImportSpecifierKind| match s {
                ImportSpecifierKind::ImportSpecifier(s) => s.ts.as_ref().is_some_and(|t| t.import_kind == "type"),
                _ => false,
            };
            if n.specifiers.iter().all(is_type) {
                return true;
            }
            let spans: Vec<(u32, u32, bool)> = n.specifiers.iter().map(|s| (s.span().start, s.span().end, is_type(s))).collect();
            self.cut_list_items(&spans);
            n.specifiers.retain(|s| !is_type(s));
            for s in &mut n.specifiers {
                if let ImportSpecifierKind::ImportSpecifier(s) = s {
                    s.ts = None;
                }
            }
        }
        false
    }

    /// Cuts the removed items of a comma-separated list (with their comma).
    fn cut_list_items(&mut self, items: &[(u32, u32, bool)]) {
        for (i, &(start, end, removed)) in items.iter().enumerate() {
            if !removed {
                continue;
            }
            if let Some(next) = items.get(i + 1) {
                self.cut(start, next.0);
            } else if i > 0 {
                // last item: from the end of the previous kept item
                let prev = items[..i].iter().rev().find(|x| !x.2);
                match prev {
                    Some(p) => self.cut(p.1, end),
                    None => self.cut(start, end),
                }
            } else {
                self.cut(start, end);
            }
        }
    }

    /// `ExportNamedDeclaration(node, context)`: true when it becomes `b.empty`.
    fn export_named_declaration(&mut self, n: &mut ExportNamedDeclaration) -> R<bool> {
        if n.ts.take().and_then(|t| t.export_kind) == Some("type") {
            return Ok(true);
        }
        if let Some(d) = &mut n.declaration {
            return self.stmt(d);
        }
        let is_type = |s: &ExportSpecifier| s.ts.as_ref().and_then(|t| t.export_kind) == Some("type");
        if n.specifiers.iter().all(is_type) {
            return Ok(true);
        }
        let spans: Vec<(u32, u32, bool)> = n.specifiers.iter().map(|s| (s.start, s.end, is_type(s))).collect();
        self.cut_list_items(&spans);
        n.specifiers.retain(|s| !is_type(s));
        for s in &mut n.specifiers {
            s.ts = None;
        }
        Ok(false)
    }

    /// `TSModuleDeclaration(node, context)`: always `b.empty` (or an error).
    fn module_declaration(&mut self, n: &mut TSModuleDeclaration) -> R<bool> {
        let span = n.span();
        match &mut n.body {
            None => Ok(true),
            Some(TSModuleBody::TSModuleBlock(b)) => {
                // namespaces can contain non-type nodes
                let mut all_empty = true;
                for entry in &mut b.body {
                    if !self.stmt(entry)? {
                        all_empty = false;
                    }
                }
                if !all_empty {
                    return Err(e::typescript_invalid_feature(span, "namespaces with non-type nodes"));
                }
                Ok(true)
            }
            Some(TSModuleBody::TSModuleDeclaration(inner)) => {
                self.module_declaration(inner)?;
                Ok(true)
            }
        }
    }

    // --- functions, classes

    /// `remove_this_param` + the function's own type properties.
    fn function(&mut self, f: &mut Function) -> R<()> {
        if matches!(f.params.first(), Some(Pattern::Identifier(i)) if i.name == "this") {
            let p0 = f.params[0].span();
            match f.params.get(1) {
                Some(p1) => self.cut(p0.start, p1.span().start),
                None => self.cut(p0.start, p0.end),
            }
            f.params.remove(0);
        }
        self.function_extra(f.ts.take());
        for p in &mut f.params {
            self.pattern(p)?;
        }
        let BlockStatementNode::BlockStatement(b) = &mut f.body;
        self.stmts(&mut b.body)
    }

    fn function_extra(&mut self, ts: Option<Box<TsFunctionExtra>>) {
        if let Some(ts) = ts {
            if let Some(tp) = &ts.type_parameters {
                self.cut(tp.start, tp.end);
            }
            if let Some(rt) = &ts.return_type {
                self.cut(rt.start, rt.end);
            }
        }
    }

    fn class(&mut self, c: &mut Class) -> R<()> {
        if let Some(ts) = c.ts.take() {
            if let Some(d) = ts.decorators.as_ref().and_then(|d| d.first()) {
                return Err(e::typescript_invalid_feature(d.span(), "decorators (related TSC proposal is not stage 4 yet)"));
            }
            if ts.is_abstract && self.src[c.start as usize..].starts_with("abstract") {
                let z = self.ws_fwd(c.start + 8);
                self.cut(c.start, z);
            }
            if let Some(tp) = &ts.type_parameters {
                self.cut(tp.start, tp.end);
            }
            if let Some(tp) = &ts.super_type_parameters {
                self.cut(tp.start, tp.end);
            }
            if let Some(list) = &ts.implements
                && let (Some(first), Some(last)) = (list.first(), list.last())
            {
                let a = self.word_before(first.start, "implements").unwrap_or(first.start);
                self.cut(self.ws_back(a), last.end);
            }
        }
        if let Some(s) = &mut c.super_class {
            self.expr(s)?;
        }
        // ClassBody(node, context)
        let mut body = Vec::with_capacity(c.body.body.len());
        for mut el in std::mem::take(&mut c.body.body) {
            if self.class_element(&mut el)? {
                body.push(el);
            } else {
                let s = el.span();
                self.cut(s.start, s.end);
            }
        }
        c.body.body = body;
        Ok(())
    }

    /// Visits a class member; false when it is removed from the body.
    fn class_element(&mut self, el: &mut ClassElement) -> R<bool> {
        match el {
            ClassElement::MethodDefinition(m) => {
                let ts = m.ts.take();
                if let Some(ts) = &ts {
                    if let Some(d) = ts.decorators.as_ref().and_then(|d| d.first()) {
                        return Err(e::typescript_invalid_feature(d.span(), "decorators (related TSC proposal is not stage 4 yet)"));
                    }
                    if ts.is_abstract {
                        return Ok(false);
                    }
                }
                let key_span = key_span(&m.key);
                if matches!(m.value, FunctionNode::TSDeclareMethod(_)) {
                    return Ok(false);
                }
                if let Some(ts) = &ts {
                    self.cut_modifier_words(m.start, key_span.start);
                    if ts.optional {
                        self.cut_member_marker(key_span.end, m.computed);
                    }
                    if let Some(tp) = &ts.type_parameters {
                        self.cut(tp.start, tp.end);
                    }
                }
                self.property_key(&mut m.key)?;
                let is_constructor = m.kind == MethodKind::Constructor;
                if let FunctionNode::FunctionExpression(f) = &mut m.value {
                    for p in &mut f.params {
                        if let Pattern::TSParameterProperty(pp) = p
                            && (pp.readonly || pp.accessibility.is_some())
                            && is_constructor
                        {
                            return Err(e::typescript_invalid_feature(
                                pp.span(),
                                "accessibility modifiers on constructor parameters",
                            ));
                        }
                    }
                    self.function(f)?;
                }
                Ok(true)
            }
            ClassElement::PropertyDefinition(p) => {
                let ts = p.ts.take();
                if let Some(ts) = &ts {
                    if let Some(d) = ts.decorators.as_ref().and_then(|d| d.first()) {
                        return Err(e::typescript_invalid_feature(d.span(), "decorators (related TSC proposal is not stage 4 yet)"));
                    }
                    if ts.accessor {
                        return Err(e::typescript_invalid_feature(p.span(), "accessor fields (related TSC proposal is not stage 4 yet)"));
                    }
                    if ts.declare {
                        return Ok(false);
                    }
                    let key_span = key_span(&p.key);
                    self.cut_modifier_words(p.start, key_span.start);
                    if ts.optional || ts.definite {
                        self.cut_member_marker(key_span.end, p.computed);
                    }
                    if let Some(ta) = &ts.type_annotation {
                        self.cut(ta.start, ta.end);
                    }
                }
                self.property_key(&mut p.key)?;
                if let Some(v) = &mut p.value {
                    self.expr(v)?;
                }
                Ok(true)
            }
            ClassElement::StaticBlock(s) => {
                self.stmts(&mut s.body)?;
                Ok(true)
            }
            ClassElement::TSIndexSignature(_) => Ok(false),
        }
    }

    fn property_key(&mut self, k: &mut PropertyKey) -> R<()> {
        if let PropertyKey::Expression(x) = k {
            self.expr(x)?;
        }
        Ok(())
    }

    // --- patterns

    fn pattern(&mut self, p: &mut Pattern) -> R<()> {
        loop {
            match p {
                Pattern::TSParameterProperty(pp) => {
                    // `TSParameterProperty(node, context)`: the constructor check is done by the
                    // enclosing method (`class_element`)
                    let inner_start = pp.parameter.span().start;
                    self.cut_modifier_words(pp.start, inner_start);
                    let inner = take_pattern(&mut pp.parameter);
                    *p = inner;
                }
                Pattern::TSAsExpression(w) | Pattern::TSSatisfiesExpression(w) | Pattern::TSTypeAssertion(w) => {
                    let w_span = (w.start, w.end);
                    let inner_span = w.expression.span();
                    let type_span = w.type_annotation.as_ref().map(|t| t.span());
                    let inner = take_pattern(&mut w.expression);
                    let is_assertion = matches!(p, Pattern::TSTypeAssertion(_));
                    self.cut_wrapper(w_span, inner_span, type_span, is_assertion, matches!(p, Pattern::TSSatisfiesExpression(_)));
                    *p = inner;
                }
                Pattern::TSNonNullExpression(w) => {
                    let end = w.end;
                    let inner = take_pattern(&mut w.expression);
                    self.cut(end - 1, end);
                    *p = inner;
                }
                _ => break,
            }
        }
        self.pattern_extra(p)?;
        match p {
            Pattern::Identifier(_) => {}
            Pattern::ObjectPattern(o) => {
                for m in &mut o.properties {
                    match m {
                        ObjectPatternMember::Property(prop) => {
                            self.property_key(&mut prop.key)?;
                            self.pattern(&mut prop.value)?;
                        }
                        ObjectPatternMember::RestElement(r) => {
                            self.pattern(&mut r.argument)?;
                        }
                    }
                }
            }
            Pattern::ArrayPattern(a) => {
                for el in a.elements.iter_mut().flatten() {
                    self.pattern(el)?;
                }
            }
            Pattern::RestElement(r) => self.pattern(&mut r.argument)?,
            Pattern::AssignmentPattern(a) => {
                self.pattern(&mut a.left)?;
                self.expr(&mut a.right)?;
            }
            Pattern::MemberExpression(m) => {
                self.expr(&mut m.object)?;
                self.property_key(&mut m.property)?;
            }
            Pattern::ParenthesizedExpression(x) => self.expr(&mut x.expression)?,
            _ => {}
        }
        Ok(())
    }

    /// Cuts the TypeScript of `x as T` / `x satisfies T` / `<T>x`.
    fn cut_wrapper(&mut self, node: (u32, u32), inner: Span, type_span: Option<Span>, is_assertion: bool, satisfies: bool) {
        let Some(ts) = type_span else { return };
        if is_assertion {
            // `<T>x`: from `<` to the `>` after the type
            let gt = self.ws_fwd(ts.end);
            let z = if self.bytes().get(gt as usize) == Some(&b'>') { gt + 1 } else { ts.end };
            self.cut(node.0, z);
            return;
        }
        let kw = if satisfies { "satisfies" } else { "as" };
        let a = self.word_before(ts.start, kw).unwrap_or(ts.start);
        // drop the whitespace before the keyword too when it directly follows the operand
        let a = if self.src[inner.end as usize..a as usize].trim().is_empty() { inner.end } else { a };
        self.cut(a, node.1);
    }

    // --- expressions

    fn expr(&mut self, x: &mut Expression) -> R<()> {
        // TSAsExpression / TSSatisfiesExpression / TSNonNullExpression / TSTypeAssertion /
        // TSInstantiationExpression: `context.visit(node.expression)`
        loop {
            match x {
                Expression::TSAsExpression(_) | Expression::TSSatisfiesExpression(_) => {
                    let satisfies = matches!(x, Expression::TSSatisfiesExpression(_));
                    let (Expression::TSAsExpression(n) | Expression::TSSatisfiesExpression(n)) = x else { unreachable!() };
                    let node = (n.start, n.end);
                    let inner_span = n.expression.span();
                    let ts = n.type_annotation.span();
                    let inner = take_expr(&mut n.expression);
                    self.cut_wrapper(node, inner_span, Some(ts), false, satisfies);
                    *x = inner;
                }
                Expression::TSTypeAssertion(n) => {
                    let node = (n.start, n.end);
                    let inner_span = n.expression.span();
                    let ts = n.type_annotation.span();
                    let inner = take_expr(&mut n.expression);
                    self.cut_wrapper(node, inner_span, Some(ts), true, false);
                    *x = inner;
                }
                Expression::TSNonNullExpression(n) => {
                    let end = n.end;
                    let inner = take_expr(&mut n.expression);
                    self.cut(end - 1, end);
                    *x = inner;
                }
                Expression::TSInstantiationExpression(n) => {
                    let (a, z) = (n.type_arguments.start, n.type_arguments.end);
                    let inner = take_expr(&mut n.expression);
                    self.cut(a, z);
                    *x = inner;
                }
                Expression::TSTypeCastExpression(n) => {
                    let (a, z) = (n.type_annotation.start, n.type_annotation.end);
                    let inner = take_expr(&mut n.expression);
                    self.cut(a, z);
                    *x = inner;
                }
                _ => break,
            }
        }
        match x {
            Expression::Identifier(n) => {
                n.ts = None;
            }
            Expression::Literal(_)
            | Expression::ThisExpression(_)
            | Expression::Super(_)
            | Expression::MetaProperty(_) => {}
            Expression::ArrayExpression(n) => {
                n.ts = None;
                for el in n.elements.iter_mut().flatten() {
                    self.array_element(el)?;
                }
            }
            Expression::ObjectExpression(n) => {
                n.ts = None;
                for p in &mut n.properties {
                    match p {
                        ObjectMember::Property(p) => {
                            self.property_key(&mut p.key)?;
                            self.expr(&mut p.value)?;
                        }
                        ObjectMember::SpreadElement(s) => {
                            s.ts = None;
                            self.expr(&mut s.argument)?;
                        }
                    }
                }
            }
            Expression::FunctionExpression(f) => self.function(f)?,
            Expression::ArrowFunctionExpression(f) => {
                self.function_extra(f.ts.take());
                for p in &mut f.params {
                    self.pattern(p)?;
                }
                match &mut f.body {
                    ArrowBody::Block(BlockStatementNode::BlockStatement(b)) => self.stmts(&mut b.body)?,
                    ArrowBody::Expression(b) => self.expr(b)?,
                }
            }
            Expression::ClassExpression(c) => self.class(c)?,
            Expression::TemplateLiteral(t) => {
                for e in &mut t.expressions {
                    self.expr(e)?;
                }
            }
            Expression::TaggedTemplateExpression(t) => {
                if let Some(ta) = t.ts.take().and_then(|t| t.type_arguments) {
                    self.cut(ta.start, ta.end);
                }
                self.expr(&mut t.tag)?;
                let TemplateLiteralNode::TemplateLiteral(q) = &mut t.quasi;
                for e in &mut q.expressions {
                    self.expr(e)?;
                }
            }
            Expression::UnaryExpression(n) => self.expr(&mut n.argument)?,
            Expression::UpdateExpression(n) => self.expr(&mut n.argument)?,
            Expression::BinaryExpression(n) => {
                if let BinaryLeft::Expression(l) = &mut n.left {
                    self.expr(l)?;
                }
                self.expr(&mut n.right)?;
            }
            Expression::LogicalExpression(n) => {
                self.expr(&mut n.left)?;
                self.expr(&mut n.right)?;
            }
            Expression::AssignmentExpression(n) => {
                self.pattern(&mut n.left)?;
                self.expr(&mut n.right)?;
            }
            Expression::ConditionalExpression(n) => {
                self.expr(&mut n.test)?;
                self.expr(&mut n.consequent)?;
                self.expr(&mut n.alternate)?;
            }
            Expression::CallExpression(n) => {
                if let Some(ta) = n.ts.take().and_then(|t| t.type_arguments) {
                    self.cut(ta.start, ta.end);
                }
                self.expr(&mut n.callee)?;
                for a in &mut n.arguments {
                    self.array_element(a)?;
                }
            }
            Expression::NewExpression(n) => {
                if let Some(ta) = n.ts.take().and_then(|t| t.type_arguments) {
                    self.cut(ta.start, ta.end);
                }
                self.expr(&mut n.callee)?;
                for a in &mut n.arguments {
                    self.array_element(a)?;
                }
            }
            Expression::MemberExpression(n) => {
                self.expr(&mut n.object)?;
                self.property_key(&mut n.property)?;
            }
            Expression::ChainExpression(n) => self.expr(&mut n.expression)?,
            Expression::SequenceExpression(n) => {
                for e in &mut n.expressions {
                    self.expr(e)?;
                }
            }
            Expression::YieldExpression(n) => {
                if let Some(a) = &mut n.argument {
                    self.expr(a)?;
                }
            }
            Expression::AwaitExpression(n) => self.expr(&mut n.argument)?,
            Expression::ImportExpression(n) => {
                self.expr(&mut n.source)?;
                if let Some(o) = &mut n.options {
                    self.expr(o)?;
                }
                if let Some(t) = n.ts.take() {
                    // `import(source, expr)` of the TypeScript parser: `arguments: [expr]`
                    n.options = t.arguments.and_then(|mut a| a.pop()).map(Box::new);
                    if let Some(o) = &mut n.options {
                        self.expr(o)?;
                    }
                }
            }
            Expression::ParenthesizedExpression(n) => self.expr(&mut n.expression)?,
            Expression::TSAsExpression(_)
            | Expression::TSSatisfiesExpression(_)
            | Expression::TSTypeAssertion(_)
            | Expression::TSNonNullExpression(_)
            | Expression::TSInstantiationExpression(_)
            | Expression::TSTypeCastExpression(_) => unreachable!("unwrapped above"),
        }
        Ok(())
    }

    fn array_element(&mut self, el: &mut ArrayElement) -> R<()> {
        match el {
            ArrayElement::Spread(SpreadElementNode::SpreadElement(s)) => {
                s.ts = None;
                self.expr(&mut s.argument)
            }
            ArrayElement::Expression(x) => self.expr(x),
        }
    }
}

fn key_span(k: &PropertyKey) -> Span {
    match k {
        PropertyKey::Expression(x) => x.span(),
        PropertyKey::Private(PrivateIdentifierNode::PrivateIdentifier(p)) => p.span(),
    }
}
