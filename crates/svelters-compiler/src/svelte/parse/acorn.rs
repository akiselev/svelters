//! Port of `phases/1-parse/acorn.js` (Svelte 5.57.2 @ f3de3f2): the wrapper around acorn.
//!
//! `TSParser` (`acorn.Parser.extend(tsPlugin())`) is `crate::acorn` with `typescript` set.
//!
//! SVELTERS deviations (docs/notes/native-compiler/deviations.md):
//! - `add_comments` (attaching `leadingComments`/`trailingComments` to ESTree nodes) is not
//!   ported: `crate::estree` has no such fields. `onComment` (the `root.comments` list) is.
//! - `startLocation`/`locations` are not needed (no `loc` in our ESTree).

use crate::acorn::{self as acorn_impl, Options, ParseError};
use crate::estree::*;
use crate::svelte::errors as e;
use crate::svelte::parse::PResult;

fn options(preserve_parens: bool, allow_undefined_exports: bool, typescript: bool) -> Options {
    Options { ecma_version: 16, source_type: SourceType::Module, preserve_parens, allow_undefined_exports, typescript }
}

/// `parse(source, comments, typescript, is_script)`
pub fn parse(source: &str, comments: &mut Vec<Comment>, typescript: bool, is_script: bool) -> PResult<Program> {
    let mut raw = Vec::new();
    // If we're dealing with a <script> then it might contain an export
    // for something that doesn't exist directly inside but is inside the
    // component instead, so we need to ensure that Acorn doesn't throw
    // an error in these cases (`allow_undefined_exports`)
    let result = acorn_impl::parse(source, &options(false, is_script, typescript), &mut raw);
    on_comments(source, comments, raw);
    result.map_err(|err| handle_parse_error(&err))
}

/// `parse_expression_at(parser, source, index)`; `ts`: `parser.ts`.
pub fn parse_expression_at(source: &str, comments: &mut Vec<Comment>, index: usize, ts: bool) -> PResult<Expression> {
    let mut raw = Vec::new();
    let result = acorn_impl::parse_expression_at(source, index as u32, &options(true, false, ts), &mut raw);
    on_comments(source, comments, raw);
    result.map_err(|err| handle_parse_error(&err))
}

/// `parse_statement_at(parser, source, index)`; `ts`: `parser.ts`.
pub fn parse_statement_at(source: &str, comments: &mut Vec<Comment>, index: usize, ts: bool) -> PResult<Statement> {
    let mut raw = Vec::new();
    let result = acorn_impl::parse_statement_at(source, index as u32, &options(false, false, ts), &mut raw);
    on_comments(source, comments, raw);
    result.map_err(|err| {
        // A statement that runs to the end of the source (e.g. an unterminated declaration tag)
        // is an EOF, not a stray token; preserve the friendlier `unexpected_eof` diagnostic.
        if err.pos as usize == source.len() {
            return e::unexpected_eof(source.len());
        }
        handle_parse_error(&err)
    })
}

/// acorn breaks lines on bare `\r`, ` ` and ` `, which the locator doesn't
pub fn has_lf_line_breaks_only(template: &str) -> bool {
    // regex_non_lf_line_break = /\r(?!\n)|[  ]/
    let b = template.as_bytes();
    for (i, c) in template.char_indices() {
        match c {
            '\r' if b.get(i + 1) != Some(&b'\n') => return false,
            '\u{2028}' | '\u{2029}' => return false,
            _ => {}
        }
    }
    true
}

/// `handle_parse_error(err)`: strips acorn's ` (line:col)` suffix.
fn handle_parse_error(err: &ParseError) -> e::CompileError {
    e::js_parse_error(err.pos, strip_position_indicator(&err.message))
}

/// `message.replace(/ \(\d+:\d+\)$/, '')`
fn strip_position_indicator(message: &str) -> &str {
    let Some(body) = message.strip_suffix(')') else { return message };
    let Some(open) = body.rfind(" (") else { return message };
    let inner = &body[open + 2..];
    let mut parts = inner.splitn(2, ':');
    let (Some(a), Some(b)) = (parts.next(), parts.next()) else { return message };
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit());
    if digits(a) && digits(b) { &message[..open] } else { message }
}

/// The `onComment` handler of `get_comment_handlers`, applied to each comment acorn reported.
fn on_comments(source: &str, comments: &mut Vec<Comment>, raw: Vec<Comment>) {
    for mut comment in raw {
        if comment.kind == CommentKind::Block && comment.value.contains('\n') {
            let start = comment.start as usize;
            let b = source.as_bytes();
            let mut a = start;
            while a > 0 && b[a - 1] != b'\n' {
                a -= 1;
            }

            let mut bb = a;
            while bb < b.len() && (b[bb] == b' ' || b[bb] == b'\t') {
                bb += 1;
            }

            let indentation = &source[a..bb];
            comment.value = remove_indentation(&comment.value, indentation);
        }

        comments.push(comment);
    }
}

/// `value.replace(new RegExp(`^${indentation}`, 'gm'), '')`: `^` in multiline mode matches at
/// the start and after `\n`, `\r`, ` `, ` `.
fn remove_indentation(value: &str, indentation: &str) -> String {
    if indentation.is_empty() {
        return value.to_string();
    }
    let mut out = String::with_capacity(value.len());
    let mut at_line_start = true;
    let mut rest = value;
    while !rest.is_empty() {
        if at_line_start && let Some(r) = rest.strip_prefix(indentation) {
            rest = r;
            at_line_start = false;
            continue;
        }
        let c = rest.chars().next().unwrap();
        out.push(c);
        rest = &rest[c.len_utf8()..];
        at_line_start = matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}');
    }
    out
}

// ---------------------------------------------------------------------------------------------
// remove_parens: `walk(node, null, { ParenthesizedExpression(node, ctx) { return ctx.visit(node.expression) } })`
// zimmerframe visits every child object with a `type`, so every ParenthesizedExpression in the
// tree (including inside functions, classes and patterns) is replaced by its expression.

pub fn remove_parens(mut node: Expression) -> Expression {
    rp_expr(&mut node);
    node
}

fn take_expr(e: &mut Expression) -> Expression {
    std::mem::replace(e, Expression::ThisExpression(ThisExpression { start: 0, end: 0 }))
}

fn rp_expr(node: &mut Expression) {
    while let Expression::ParenthesizedExpression(p) = node {
        let inner = take_expr(&mut p.expression);
        *node = inner;
    }
    match node {
        Expression::Identifier(_)
        | Expression::Literal(_)
        | Expression::ThisExpression(_)
        | Expression::Super(_)
        | Expression::MetaProperty(_) => {}
        Expression::ArrayExpression(n) => {
            for el in n.elements.iter_mut().flatten() {
                rp_array_element(el);
            }
        }
        Expression::ObjectExpression(n) => {
            for p in &mut n.properties {
                match p {
                    ObjectMember::Property(p) => {
                        rp_key(&mut p.key);
                        rp_expr(&mut p.value);
                    }
                    ObjectMember::SpreadElement(s) => rp_expr(&mut s.argument),
                }
            }
        }
        Expression::FunctionExpression(f) => rp_function(f),
        Expression::ArrowFunctionExpression(f) => {
            for p in &mut f.params {
                rp_pattern(p);
            }
            match &mut f.body {
                ArrowBody::Block(BlockStatementNode::BlockStatement(b)) => rp_stmts(&mut b.body),
                ArrowBody::Expression(e) => rp_expr(e),
            }
        }
        Expression::ClassExpression(c) => rp_class(c),
        Expression::TemplateLiteral(t) => rp_template(t),
        Expression::TaggedTemplateExpression(t) => {
            rp_expr(&mut t.tag);
            let TemplateLiteralNode::TemplateLiteral(q) = &mut t.quasi;
            rp_template(q);
        }
        Expression::UnaryExpression(n) => rp_expr(&mut n.argument),
        Expression::UpdateExpression(n) => rp_expr(&mut n.argument),
        Expression::BinaryExpression(n) => {
            if let BinaryLeft::Expression(l) = &mut n.left {
                rp_expr(l);
            }
            rp_expr(&mut n.right);
        }
        Expression::LogicalExpression(n) => {
            rp_expr(&mut n.left);
            rp_expr(&mut n.right);
        }
        Expression::AssignmentExpression(n) => {
            rp_pattern(&mut n.left);
            rp_expr(&mut n.right);
        }
        Expression::ConditionalExpression(n) => {
            rp_expr(&mut n.test);
            rp_expr(&mut n.consequent);
            rp_expr(&mut n.alternate);
        }
        Expression::CallExpression(n) => {
            rp_expr(&mut n.callee);
            for a in &mut n.arguments {
                rp_array_element(a);
            }
        }
        Expression::NewExpression(n) => {
            rp_expr(&mut n.callee);
            for a in &mut n.arguments {
                rp_array_element(a);
            }
        }
        Expression::MemberExpression(n) => rp_member(n),
        Expression::ChainExpression(n) => rp_expr(&mut n.expression),
        Expression::SequenceExpression(n) => {
            for e in &mut n.expressions {
                rp_expr(e);
            }
        }
        Expression::YieldExpression(n) => {
            if let Some(a) = &mut n.argument {
                rp_expr(a);
            }
        }
        Expression::AwaitExpression(n) => rp_expr(&mut n.argument),
        Expression::ImportExpression(n) => {
            rp_expr(&mut n.source);
            if let Some(o) = &mut n.options {
                rp_expr(o);
            }
            if let Some(args) = n.ts.as_mut().and_then(|t| t.arguments.as_mut()) {
                for a in args {
                    rp_expr(a);
                }
            }
        }
        Expression::ParenthesizedExpression(_) => unreachable!(),
        // TypeScript wrappers (type nodes hold no expressions that can be parenthesized)
        Expression::TSAsExpression(n) | Expression::TSSatisfiesExpression(n) => rp_expr(&mut n.expression),
        Expression::TSTypeAssertion(n) => rp_expr(&mut n.expression),
        Expression::TSNonNullExpression(n) => rp_expr(&mut n.expression),
        Expression::TSInstantiationExpression(n) => rp_expr(&mut n.expression),
        Expression::TSTypeCastExpression(n) => rp_expr(&mut n.expression),
    }
}

fn rp_member(n: &mut MemberExpression) {
    rp_expr(&mut n.object);
    rp_key(&mut n.property);
}

fn rp_template(t: &mut TemplateLiteral) {
    for e in &mut t.expressions {
        rp_expr(e);
    }
}

fn rp_array_element(el: &mut ArrayElement) {
    match el {
        ArrayElement::Spread(SpreadElementNode::SpreadElement(s)) => rp_expr(&mut s.argument),
        ArrayElement::Expression(e) => rp_expr(e),
    }
}

fn rp_key(k: &mut PropertyKey) {
    if let PropertyKey::Expression(e) = k {
        rp_expr(e);
    }
}

/// Converts the expression of a parenthesized pattern back into a pattern.
#[allow(clippy::result_large_err)]
fn expression_to_pattern(e: Expression) -> Result<Pattern, Expression> {
    match e {
        Expression::Identifier(i) => Ok(Pattern::Identifier(i)),
        Expression::MemberExpression(m) => Ok(Pattern::MemberExpression(Box::new(m))),
        other => Err(other),
    }
}

fn rp_pattern(p: &mut Pattern) {
    while let Pattern::ParenthesizedExpression(paren) = p {
        let mut inner = take_expr(&mut paren.expression);
        rp_expr(&mut inner);
        match expression_to_pattern(inner) {
            Ok(pat) => *p = pat,
            Err(inner) => {
                // not representable as a pattern: keep the (cleaned) inner expression wrapped
                *paren.expression = inner;
                return;
            }
        }
    }
    match p {
        Pattern::Identifier(_) => {}
        Pattern::ObjectPattern(o) => {
            for m in &mut o.properties {
                match m {
                    ObjectPatternMember::Property(prop) => {
                        rp_key(&mut prop.key);
                        rp_pattern(&mut prop.value);
                    }
                    ObjectPatternMember::RestElement(r) => rp_pattern(&mut r.argument),
                }
            }
        }
        Pattern::ArrayPattern(a) => {
            for el in a.elements.iter_mut().flatten() {
                rp_pattern(el);
            }
        }
        Pattern::RestElement(r) => rp_pattern(&mut r.argument),
        Pattern::AssignmentPattern(a) => {
            rp_pattern(&mut a.left);
            rp_expr(&mut a.right);
        }
        Pattern::MemberExpression(m) => rp_member(m),
        Pattern::ParenthesizedExpression(_) => {}
        Pattern::TSParameterProperty(n) => rp_pattern(&mut n.parameter),
        Pattern::TSAsExpression(n)
        | Pattern::TSSatisfiesExpression(n)
        | Pattern::TSTypeAssertion(n)
        | Pattern::TSNonNullExpression(n) => rp_pattern(&mut n.expression),
    }
}

fn rp_function(f: &mut Function) {
    for p in &mut f.params {
        rp_pattern(p);
    }
    let BlockStatementNode::BlockStatement(b) = &mut f.body;
    rp_stmts(&mut b.body);
}

fn rp_class(c: &mut Class) {
    if let Some(s) = &mut c.super_class {
        rp_expr(s);
    }
    for el in &mut c.body.body {
        match el {
            ClassElement::MethodDefinition(m) => {
                rp_key(&mut m.key);
                match &mut m.value {
                    FunctionNode::FunctionExpression(f) => rp_function(f),
                    FunctionNode::TSDeclareMethod(f) => {
                        for p in &mut f.params {
                            rp_pattern(p);
                        }
                    }
                }
            }
            ClassElement::PropertyDefinition(p) => {
                rp_key(&mut p.key);
                if let Some(v) = &mut p.value {
                    rp_expr(v);
                }
            }
            ClassElement::StaticBlock(s) => rp_stmts(&mut s.body),
            ClassElement::TSIndexSignature(_) => {}
        }
    }
}

fn rp_stmts(body: &mut [Statement]) {
    for s in body {
        rp_stmt(s);
    }
}

fn rp_var_decl(d: &mut VariableDeclaration) {
    for decl in &mut d.declarations {
        rp_pattern(&mut decl.id);
        if let Some(init) = &mut decl.init {
            rp_expr(init);
        }
    }
}

fn rp_stmt(s: &mut Statement) {
    match s {
        Statement::ExpressionStatement(n) => rp_expr(&mut n.expression),
        Statement::BlockStatement(n) => rp_stmts(&mut n.body),
        Statement::StaticBlock(n) => rp_stmts(&mut n.body),
        Statement::EmptyStatement(_)
        | Statement::DebuggerStatement(_)
        | Statement::BreakStatement(_)
        | Statement::ContinueStatement(_) => {}
        Statement::WithStatement(n) => {
            rp_expr(&mut n.object);
            rp_stmt(&mut n.body);
        }
        Statement::ReturnStatement(n) => {
            if let Some(a) = &mut n.argument {
                rp_expr(a);
            }
        }
        Statement::LabeledStatement(n) => rp_stmt(&mut n.body),
        Statement::IfStatement(n) => {
            rp_expr(&mut n.test);
            rp_stmt(&mut n.consequent);
            if let Some(a) = &mut n.alternate {
                rp_stmt(a);
            }
        }
        Statement::SwitchStatement(n) => {
            rp_expr(&mut n.discriminant);
            for c in &mut n.cases {
                if let Some(t) = &mut c.test {
                    rp_expr(t);
                }
                rp_stmts(&mut c.consequent);
            }
        }
        Statement::ThrowStatement(n) => rp_expr(&mut n.argument),
        Statement::TryStatement(n) => {
            rp_stmts(&mut n.block.body);
            if let Some(h) = &mut n.handler {
                if let Some(p) = &mut h.param {
                    rp_pattern(p);
                }
                rp_stmts(&mut h.body.body);
            }
            if let Some(f) = &mut n.finalizer {
                rp_stmts(&mut f.body);
            }
        }
        Statement::WhileStatement(n) => {
            rp_expr(&mut n.test);
            rp_stmt(&mut n.body);
        }
        Statement::DoWhileStatement(n) => {
            rp_stmt(&mut n.body);
            rp_expr(&mut n.test);
        }
        Statement::ForStatement(n) => {
            match &mut n.init {
                Some(ForInit::VariableDeclaration(VariableDeclarationNode::VariableDeclaration(d))) => rp_var_decl(d),
                Some(ForInit::Expression(e)) => rp_expr(e),
                None => {}
            }
            if let Some(t) = &mut n.test {
                rp_expr(t);
            }
            if let Some(u) = &mut n.update {
                rp_expr(u);
            }
            rp_stmt(&mut n.body);
        }
        Statement::ForInStatement(n) => {
            rp_for_left(&mut n.left);
            rp_expr(&mut n.right);
            rp_stmt(&mut n.body);
        }
        Statement::ForOfStatement(n) => {
            rp_for_left(&mut n.left);
            rp_expr(&mut n.right);
            rp_stmt(&mut n.body);
        }
        Statement::FunctionDeclaration(f) => rp_function(f),
        Statement::VariableDeclaration(d) => rp_var_decl(d),
        Statement::ClassDeclaration(c) => rp_class(c),
        Statement::ImportDeclaration(_) | Statement::ExportAllDeclaration(_) => {}
        Statement::ExportNamedDeclaration(n) => {
            if let Some(d) = &mut n.declaration {
                rp_stmt(d);
            }
        }
        Statement::ExportDefaultDeclaration(n) => match &mut n.declaration {
            ExportDefaultKind::Declaration(d) => rp_stmt(d),
            ExportDefaultKind::Expression(e) => rp_expr(e),
        },
        Statement::TSEnumDeclaration(n) => {
            for m in &mut n.members {
                if let Some(i) = &mut m.initializer {
                    rp_expr(i);
                }
            }
        }
        Statement::TSModuleDeclaration(n) => rp_module(n),
        Statement::TSDeclareFunction(n) => {
            for p in &mut n.params {
                rp_pattern(p);
            }
        }
        Statement::TSExportAssignment(n) => rp_expr(&mut n.expression),
        Statement::TSInterfaceDeclaration(_)
        | Statement::TSTypeAliasDeclaration(_)
        | Statement::TSImportEqualsDeclaration(_)
        | Statement::TSNamespaceExportDeclaration(_) => {}
    }
}

fn rp_module(n: &mut TSModuleDeclaration) {
    match &mut n.body {
        Some(TSModuleBody::TSModuleBlock(b)) => rp_stmts(&mut b.body),
        Some(TSModuleBody::TSModuleDeclaration(d)) => rp_module(d),
        None => {}
    }
}

fn rp_for_left(l: &mut ForLeft) {
    match l {
        ForLeft::VariableDeclaration(VariableDeclarationNode::VariableDeclaration(d)) => rp_var_decl(d),
        ForLeft::Pattern(p) => rp_pattern(p),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_indicator() {
        assert_eq!(strip_position_indicator("Unexpected token (1:5)"), "Unexpected token");
        assert_eq!(strip_position_indicator("Unexpected token (a:5)"), "Unexpected token (a:5)");
        assert_eq!(strip_position_indicator("x"), "x");
    }

    #[test]
    fn indentation() {
        assert_eq!(remove_indentation("*\n\t * a\n\t */", "\t"), "*\n * a\n */");
        assert_eq!(remove_indentation("a\n  b", ""), "a\n  b");
    }

    #[test]
    fn lf_only() {
        assert!(has_lf_line_breaks_only("a\r\nb\n"));
        assert!(!has_lf_line_breaks_only("a\rb"));
        assert!(!has_lf_line_breaks_only("a\u{2028}"));
    }
}
