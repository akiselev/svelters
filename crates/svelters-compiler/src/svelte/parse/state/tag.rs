//! Port of `phases/1-parse/state/tag.js` (Svelte 5.57.2 @ f3de3f2).

use crate::estree::{
    ArrowFunctionExpression, Expression, Identifier, Pattern, Spanned, Statement, VariableDeclaration, VariableDeclarator,
    VariableKind,
};
use crate::svelte::ast::{ConstTag, DebugTag, DeclarationTag, ExpressionTag, FragmentNode, HtmlTag, RenderTag};
use crate::svelte::errors as e;
use crate::svelte::parse::acorn::{parse_expression_at, parse_statement_at};
use crate::svelte::parse::read::context::read_pattern;
use crate::svelte::parse::read::expression::{get_loose_identifier, read_expression};
use crate::svelte::parse::utils::bracket::{find_matching_bracket, match_bracket};
use crate::svelte::parse::utils::whitespace::is_js_regex_space;
use crate::svelte::parse::{
    AwaitData, EachData, IfData, Item, KeyData, OPEN_END, PResult, Parser, SnippetData, TemplateNode,
};

/// `/\s*}/y`
fn regex_whitespace_with_closing_curly_brace(s: &str, at: usize) -> Option<usize> {
    let rest = s.get(at..)?;
    let ws: usize = rest.chars().take_while(|c| is_js_regex_space(*c)).map(char::len_utf8).sum();
    rest[ws..].starts_with('}').then_some(ws + 1)
}

/// `\b` after an ASCII word: the next character is not `[A-Za-z0-9_]`.
fn word_at(s: &str, at: usize, words: &[&str]) -> Option<usize> {
    let rest = s.get(at..)?;
    for w in words {
        if rest.starts_with(w) && !rest.as_bytes().get(w.len()).is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_') {
            return Some(w.len());
        }
    }
    None
}

/// `/(?:let|const)\b/y`
fn regex_supported_declaration(s: &str, at: usize) -> Option<usize> {
    word_at(s, at, &["let", "const"])
}

/// `/(?:var|interface|enum)\b/y`
fn regex_unsupported_declaration(s: &str, at: usize) -> Option<usize> {
    word_at(s, at, &["var", "interface", "enum"])
}

// `type` is a contextual keyword; this is just a shape hint, confirmed by parsing.
/// `/type\b/y`
fn regex_maybe_type_declaration(s: &str, at: usize) -> Option<usize> {
    word_at(s, at, &["type"])
}

const POINTY_BOIS: &[(char, char)] = &[('<', '>')];

pub fn tag(parser: &mut Parser) -> PResult<()> {
    let start = parser.index;
    parser.index += 1;

    parser.allow_whitespace();

    if parser.eat_opt("#") {
        return open(parser);
    }
    if parser.eat_opt(":") {
        return next(parser);
    }
    if parser.eat_opt("@") {
        return special(parser);
    }
    if parser.match_str("/") && !parser.match_str("/*") && !parser.match_str("//") {
        parser.eat_opt("/");
        return close(parser);
    }

    if let Some(declaration) = read_declaration(parser)? {
        parser.append(Item::Leaf(FragmentNode::DeclarationTag(DeclarationTag {
            start: start as u32,
            end: parser.index as u32,
            declaration,
        })));
        return Ok(());
    }

    let expression = read_expression(parser, None, false)?;

    parser.allow_whitespace();
    parser.eat_required("}")?;

    parser.append(Item::Leaf(FragmentNode::ExpressionTag(ExpressionTag {
        start: start as u32,
        end: parser.index as u32,
        expression,
    })));
    Ok(())
}

fn read_declaration(parser: &mut Parser) -> PResult<Option<VariableDeclaration>> {
    let start = parser.index;

    if let Some(unsupported) = parser.match_regex(regex_unsupported_declaration) {
        return Err(e::declaration_tag_invalid_type((start, start + unsupported.len())));
    }

    if parser.match_regex(regex_supported_declaration).is_none() &&
        // `type` is special, since it is not a reserved keyword and can be used
        // as part of a valid expression. We gotta parse first and then see what it is.
        parser.match_regex(regex_maybe_type_declaration).is_none()
    {
        return Ok(None);
    }

    let initial_comment_count = parser.root.comments.len();

    let declaration = match parse_statement_at(&parser.template, &mut parser.root.comments, start) {
        Ok(d) => d,
        Err(error) => {
            if !parser.loose {
                return Err(error);
            }

            let Some(end) = find_matching_bracket(&parser.template, start, '{') else { return Err(error) };

            parser.index = end;
            let kind = if parser.template[start..].starts_with("const") { VariableKind::Const } else { VariableKind::Let };

            let i = parser.index as u32;
            Statement::VariableDeclaration(VariableDeclaration {
                start: start as u32,
                end: end as u32,
                kind,
                declarations: vec![VariableDeclarator {
                    start: i,
                    end: i,
                    id: Pattern::Identifier(Identifier { start: i, end: i, name: String::new() }),
                    init: None,
                }],
            })
        }
    };

    let declaration = match declaration {
        Statement::VariableDeclaration(d) => d,
        Statement::ExpressionStatement(_) => {
            parser.root.comments.truncate(initial_comment_count); // Else they show up duplicated
            return Ok(None);
        }
        other => {
            // This is a TSTypeAliasDeclaration
            return Err(e::declaration_tag_invalid_type(&other));
        }
    };

    // TODO support using
    if declaration.kind != VariableKind::Let && declaration.kind != VariableKind::Const {
        return Err(e::declaration_tag_invalid_type(&declaration));
    }

    parser.index = declaration.end as usize;
    parser.allow_whitespace();
    parser.eat_required("}")?;

    Ok(Some(declaration))
}

fn open(parser: &mut Parser) -> PResult<()> {
    let mut start = parser.index - 2;
    while parser.byte_at(start) != Some(b'{') {
        start -= 1;
    }

    if parser.eat_opt("if") {
        parser.require_whitespace()?;

        let test = read_expression(parser, None, false)?;
        let consequent = parser.create_fragment(false);
        let block = parser.append_node(TemplateNode::IfBlock(IfData {
            elseif: false,
            start: start as u32,
            end: OPEN_END,
            test,
            consequent,
            alternate: None,
        }));

        parser.allow_whitespace();
        parser.eat_required("}")?;

        parser.stack.push(block);
        parser.fragments.push(consequent);

        return Ok(());
    }

    if parser.eat_opt("each") {
        parser.require_whitespace()?;

        let template = parser.template.clone();

        let mut expression: Option<Expression> = None;

        // we have to do this loop because `{#each x as { y = z }}` fails to parse —
        // the `as { y = z }` is treated as an Expression but it's actually a Pattern.
        // the 'fix' is to backtrack and hide everything from the `as` onwards, until
        // we get a valid expression
        while expression.is_none() {
            match read_expression(parser, None, true) {
                Ok(expr) => expression = Some(expr),
                Err(err) => {
                    let mut end = err.start as isize - 2;

                    while end > start as isize && parser.template.as_bytes().get(end.max(0) as usize..end as usize + 2) != Some(b"as") {
                        end -= 1;
                    }

                    if end <= start as isize {
                        if parser.loose
                            && let Some(expr) = get_loose_identifier(parser, None)
                        {
                            expression = Some(expr);
                            break;
                        }
                        parser.template = template;
                        return Err(err);
                    }

                    // parser.template is meant to be readonly, this is a special case
                    parser.template = template[..end as usize].to_string();
                }
            }
        }

        parser.template = template;
        let mut expression = expression.unwrap();

        parser.allow_whitespace();

        // {#each} blocks must declare a context – {#each list as item}
        if !parser.match_str("as") {
            // this could be a TypeScript assertion that was erroneously eaten.

            if let Expression::SequenceExpression(mut seq) = expression {
                expression = seq.expressions.remove(0);
            }

            // SVELTERS: the `TSAsExpression` unwrapping that follows upstream cannot apply:
            // TypeScript is not supported, so the expression contains no TS nodes.
        }

        let mut context: Option<Pattern> = None;
        let mut index: Option<String> = None;
        let mut key: Option<Expression> = None;

        if parser.eat_opt("as") {
            parser.require_whitespace()?;

            context = Some(read_pattern(parser)?);
        } else {
            // {#each Array.from({ length: 10 }), i} is read as a sequence expression,
            // which is set back above - we now gotta reset the index as a consequence
            // to properly read the , i part
            parser.index = expression.span().end as usize;
        }

        parser.allow_whitespace();

        if parser.eat_opt(",") {
            parser.allow_whitespace();
            let name = parser.read_identifier()?.name;
            if name.is_empty() {
                return Err(e::expected_identifier(parser.index));
            }
            index = Some(name);

            parser.allow_whitespace();
        }

        if parser.eat_opt("(") {
            parser.allow_whitespace();

            key = Some(read_expression(parser, Some('('), false)?);
            parser.allow_whitespace();
            parser.eat_required(")")?;
            parser.allow_whitespace();
        }

        let matches = parser.eat("}", true, false)?;

        if !matches {
            // Parser may have read the `as` as part of the expression (e.g. in `{#each foo. as x}`)
            if parser.index >= 4 && parser.template.get(parser.index - 4..parser.index) == Some(" as ") {
                let prev_index = parser.index;
                context = Some(read_pattern(parser)?);
                parser.eat_required("}")?;
                expression = Expression::Identifier(Identifier {
                    start: expression.span().start,
                    end: (prev_index - 4) as u32,
                    name: String::new(),
                });
            } else {
                parser.eat_required("}")?; // rerun to produce the parser error
            }
        }

        let body = parser.create_fragment(false);
        let block = parser.append_node(TemplateNode::EachBlock(EachData {
            start: start as u32,
            end: OPEN_END,
            expression,
            body,
            context,
            index,
            key,
            fallback: None,
        }));

        parser.stack.push(block);
        parser.fragments.push(body);

        return Ok(());
    }

    if parser.eat_opt("await") {
        parser.require_whitespace()?;
        let expression = read_expression(parser, None, false)?;
        parser.allow_whitespace();

        let expression_start = expression.span().start;
        let id = parser.append_node(TemplateNode::AwaitBlock(AwaitData {
            start: start as u32,
            end: OPEN_END,
            expression,
            value: None,
            error: None,
            pending: None,
            then: None,
            catch: None,
        }));
        macro_rules! block {
            () => {
                match &mut parser.nodes[id] {
                    TemplateNode::AwaitBlock(b) => b,
                    _ => unreachable!(),
                }
            };
        }

        if parser.eat_opt("then") {
            if parser.match_regex(regex_whitespace_with_closing_curly_brace).is_some() {
                parser.allow_whitespace();
            } else {
                parser.require_whitespace()?;
                let value = read_pattern(parser)?;
                block!().value = Some(value);
                parser.allow_whitespace();
            }

            let f = parser.create_fragment(false);
            block!().then = Some(f);
            parser.fragments.push(f);
        } else if parser.eat_opt("catch") {
            if parser.match_regex(regex_whitespace_with_closing_curly_brace).is_some() {
                parser.allow_whitespace();
            } else {
                parser.require_whitespace()?;
                let error = read_pattern(parser)?;
                block!().error = Some(error);
                parser.allow_whitespace();
            }

            let f = parser.create_fragment(false);
            block!().catch = Some(f);
            parser.fragments.push(f);
        } else {
            let f = parser.create_fragment(false);
            block!().pending = Some(f);
            parser.fragments.push(f);
        }

        let matches = parser.eat("}", true, false)?;

        // Parser may have read the `then/catch` as part of the expression (e.g. in `{#await foo. then x}`)
        if !matches {
            let ends_with = |p: &Parser, s: &str| p.index >= s.len() && p.template.get(p.index - s.len()..p.index) == Some(s);
            if ends_with(parser, " then ") {
                let prev_index = parser.index;
                let value = read_pattern(parser)?;
                block!().value = Some(value);
                parser.eat_required("}")?;
                let b = block!();
                b.expression = Expression::Identifier(Identifier {
                    start: expression_start,
                    end: (prev_index - 6) as u32,
                    name: String::new(),
                });
                b.then = b.pending.take();
            } else if ends_with(parser, " catch ") {
                let prev_index = parser.index;
                let error = read_pattern(parser)?;
                block!().error = Some(error);
                parser.eat_required("}")?;
                let b = block!();
                b.expression = Expression::Identifier(Identifier {
                    start: expression_start,
                    end: (prev_index - 7) as u32,
                    name: String::new(),
                });
                b.catch = b.pending.take();
            } else {
                parser.eat_required("}")?; // rerun to produce the parser error
            }
        }

        parser.stack.push(id);

        return Ok(());
    }

    if parser.eat_opt("key") {
        parser.require_whitespace()?;

        let expression = read_expression(parser, None, false)?;
        parser.allow_whitespace();

        parser.eat_required("}")?;

        let fragment = parser.create_fragment(false);
        let block = parser.append_node(TemplateNode::KeyBlock(KeyData { start: start as u32, end: OPEN_END, expression, fragment }));

        parser.stack.push(block);
        parser.fragments.push(fragment);

        return Ok(());
    }

    if parser.eat_opt("snippet") {
        parser.require_whitespace()?;

        let id = parser.read_identifier()?;

        if id.name.is_empty() && !parser.loose {
            return Err(e::expected_identifier(parser.index));
        }

        parser.allow_whitespace();

        let params_start = parser.index;

        // snippets could have a generic signature, e.g. `#snippet foo<T>(...)`
        let mut type_params: Option<String> = None;

        // if we match a generic opening
        if parser.ts && parser.match_str("<") {
            let start = parser.index;
            let end = match_bracket(parser, start, POINTY_BOIS)?;

            type_params = Some(parser.template[start + 1..end - 1].to_string());

            parser.index = end;
        }

        parser.allow_whitespace();

        let matched = parser.eat("(", true, false)?;

        if matched {
            let mut parentheses = 1;

            while parser.index < parser.template.len() && (!parser.match_str(")") || parentheses != 1) {
                if parser.match_str("(") {
                    parentheses += 1;
                }
                if parser.match_str(")") {
                    parentheses -= 1;
                }
                parser.index += 1;
            }

            parser.eat_required(")")?;
        }

        let parameters = if matched {
            let source = format!("{} => {{}}", &parser.template[..parser.index]);
            match parse_expression_at(&source, &mut parser.root.comments, params_start)? {
                Expression::ArrowFunctionExpression(ArrowFunctionExpression { params, .. }) => params,
                // `.params` of anything else is undefined; unreachable for `(...) => {}`
                _ => Vec::new(),
            }
        } else {
            Vec::new()
        };

        parser.allow_whitespace();
        parser.eat_required("}")?;

        let body = parser.create_fragment(false);
        let block = parser.append_node(TemplateNode::SnippetBlock(SnippetData {
            start: start as u32,
            end: OPEN_END,
            expression: id,
            type_params,
            parameters,
            body,
        }));
        parser.stack.push(block);
        parser.fragments.push(body);

        return Ok(());
    }

    Err(e::expected_block_type(parser.index))
}

fn next(parser: &mut Parser) -> PResult<()> {
    let start = parser.index - 1;

    let block = parser.current(); // TODO type should not be TemplateNode, that's much too broad

    match &parser.nodes[block] {
        TemplateNode::IfBlock(b) => {
            let has_alternate = b.alternate.is_some();
            if !parser.eat_opt("else") {
                return Err(e::expected_token(start, "{:else} or {:else if}"));
            }
            if parser.eat_opt("if") {
                return Err(e::block_invalid_elseif(start));
            }
            if has_alternate {
                return Err(e::block_duplicate_clause(start, "{:else}"));
            }

            parser.allow_whitespace();

            parser.fragments.pop();

            let alternate = parser.create_fragment(false);
            if let TemplateNode::IfBlock(b) = &mut parser.nodes[block] {
                b.alternate = Some(alternate);
            }
            parser.fragments.push(alternate);

            // :else if
            if parser.eat_opt("if") {
                parser.require_whitespace()?;

                let expression = read_expression(parser, None, false)?;

                parser.allow_whitespace();
                parser.eat_required("}")?;

                let mut elseif_start = start - 1;
                while parser.byte_at(elseif_start) != Some(b'{') {
                    elseif_start -= 1;
                }

                let consequent = parser.create_fragment(false);
                let child = parser.append_node(TemplateNode::IfBlock(IfData {
                    start: elseif_start as u32,
                    end: OPEN_END,
                    elseif: true,
                    test: expression,
                    consequent,
                    alternate: None,
                }));

                parser.stack.push(child);
                parser.fragments.pop();
                parser.fragments.push(consequent);
            } else {
                // :else
                parser.allow_whitespace();
                parser.eat_required("}")?;
            }

            Ok(())
        }

        TemplateNode::EachBlock(b) => {
            let has_fallback = b.fallback.is_some();
            if !parser.eat_opt("else") {
                return Err(e::expected_token(start, "{:else}"));
            }
            if has_fallback {
                return Err(e::block_duplicate_clause(start, "{:else}"));
            }

            parser.allow_whitespace();
            parser.eat_required("}")?;

            let fallback = parser.create_fragment(false);
            if let TemplateNode::EachBlock(b) = &mut parser.nodes[block] {
                b.fallback = Some(fallback);
            }

            parser.fragments.pop();
            parser.fragments.push(fallback);

            Ok(())
        }

        TemplateNode::AwaitBlock(b) => {
            let (has_then, has_catch) = (b.then.is_some(), b.catch.is_some());
            macro_rules! block {
                () => {
                    match &mut parser.nodes[block] {
                        TemplateNode::AwaitBlock(b) => b,
                        _ => unreachable!(),
                    }
                };
            }

            if parser.eat_opt("then") {
                if has_then {
                    return Err(e::block_duplicate_clause(start, "{:then}"));
                }

                if !parser.eat_opt("}") {
                    parser.require_whitespace()?;
                    let value = read_pattern(parser)?;
                    block!().value = Some(value);
                    parser.allow_whitespace();
                    parser.eat_required("}")?;
                }

                let then = parser.create_fragment(false);
                block!().then = Some(then);
                parser.fragments.pop();
                parser.fragments.push(then);

                return Ok(());
            }

            if parser.eat_opt("catch") {
                if has_catch {
                    return Err(e::block_duplicate_clause(start, "{:catch}"));
                }

                if !parser.eat_opt("}") {
                    parser.require_whitespace()?;
                    let error = read_pattern(parser)?;
                    block!().error = Some(error);
                    parser.allow_whitespace();
                    parser.eat_required("}")?;
                }

                let catch = parser.create_fragment(false);
                block!().catch = Some(catch);
                parser.fragments.pop();
                parser.fragments.push(catch);

                return Ok(());
            }

            Err(e::expected_token(start, "{:then ...} or {:catch ...}"))
        }

        _ => Err(e::block_invalid_continuation_placement(start)),
    }
}

fn close(parser: &mut Parser) -> PResult<()> {
    let start = parser.index - 1;

    let mut block = parser.current();
    // Only relevant/reached for loose parsing mode
    let matched = match parser.nodes[block].type_name() {
        "IfBlock" => {
            let matched = parser.eat("if", true, false)?;

            if !matched {
                parser.nodes[block].set_end((start - 1) as u32);
                parser.pop();
                return close(parser);
            }

            parser.allow_whitespace();
            parser.eat_required("}")?;

            while matches!(&parser.nodes[block], TemplateNode::IfBlock(b) if b.elseif) {
                parser.nodes[block].set_end(parser.index as u32);
                parser.stack.pop();
                block = parser.current();
            }

            parser.nodes[block].set_end(parser.index as u32);
            parser.pop();
            return Ok(());
        }

        "EachBlock" => parser.eat("each", true, false)?,
        "KeyBlock" => parser.eat("key", true, false)?,
        "AwaitBlock" => parser.eat("await", true, false)?,
        "SnippetBlock" => parser.eat("snippet", true, false)?,

        "RegularElement" if parser.loose => false,
        // TODO handle implicitly closed elements
        "RegularElement" => return Err(e::block_unexpected_close(start)),

        _ => return Err(e::block_unexpected_close(start)),
    };

    if !matched {
        parser.nodes[block].set_end((start - 1) as u32);
        parser.pop();
        return close(parser);
    }

    parser.allow_whitespace();
    parser.eat_required("}")?;
    parser.nodes[block].set_end(parser.index as u32);
    parser.pop();
    Ok(())
}

fn special(parser: &mut Parser) -> PResult<()> {
    let mut start = parser.index;
    while parser.byte_at(start) != Some(b'{') {
        start -= 1;
    }

    if parser.eat_opt("html") {
        // {@html content} tag
        parser.require_whitespace()?;

        let expression = read_expression(parser, None, false)?;

        parser.allow_whitespace();
        parser.eat_required("}")?;

        parser.append(Item::Leaf(FragmentNode::HtmlTag(HtmlTag { start: start as u32, end: parser.index as u32, expression })));

        return Ok(());
    }

    if parser.eat_opt("debug") {
        let identifiers: Vec<Identifier>;

        // Implies {@debug} which indicates "debug all"
        if parser.read(regex_whitespace_with_closing_curly_brace).is_some() {
            identifiers = Vec::new();
        } else {
            let expression = read_expression(parser, None, false)?;

            let nodes = match expression {
                Expression::SequenceExpression(seq) => seq.expressions,
                other => vec![other],
            };

            let mut ids = Vec::with_capacity(nodes.len());
            for node in nodes {
                match node {
                    Expression::Identifier(id) => ids.push(id),
                    other => return Err(e::debug_tag_invalid_arguments(other.span().start)),
                }
            }
            identifiers = ids;

            parser.allow_whitespace();
            parser.eat_required("}")?;
        }

        parser.append(Item::Leaf(FragmentNode::DebugTag(DebugTag { start: start as u32, end: parser.index as u32, identifiers })));

        return Ok(());
    }

    if parser.eat_opt("const") {
        parser.require_whitespace()?;

        let id = read_pattern(parser)?;
        parser.allow_whitespace();

        parser.eat_required("=")?;
        parser.allow_whitespace();

        let expression_start = parser.index;
        let init = read_expression(parser, None, false)?;
        // parser is past wrapping parens, but `init.end` is not — use the parser position
        let declarator_end = parser.index;
        if matches!(init, Expression::SequenceExpression(_))
            && !parser.template[expression_start..(init.span().start as usize).max(expression_start)].contains('(')
        {
            // const a = (b, c) is allowed but a = b, c = d is not;
            return Err(e::const_tag_invalid_expression(&init));
        }
        parser.allow_whitespace();

        parser.eat_required("}")?;

        let id_start = id.span().start;
        parser.append(Item::Leaf(FragmentNode::ConstTag(ConstTag {
            start: start as u32,
            end: parser.index as u32,
            declaration: VariableDeclaration {
                kind: VariableKind::Const,
                declarations: vec![VariableDeclarator { id, init: Some(init), start: id_start, end: declarator_end as u32 }],
                start: start as u32 + 2, // start at const, not at @const
                end: parser.index as u32 - 1,
            },
        })));
        return Ok(());
    }

    if parser.eat_opt("render") {
        // {@render foo(...)}
        parser.require_whitespace()?;

        let expression = read_expression(parser, None, false)?;

        let ok = match &expression {
            Expression::CallExpression(_) => true,
            Expression::ChainExpression(c) => matches!(c.expression.as_ref(), Expression::CallExpression(_)),
            _ => false,
        };
        if !ok {
            return Err(e::render_tag_invalid_expression(&expression));
        }

        parser.allow_whitespace();
        parser.eat_required("}")?;

        parser.append(Item::Leaf(FragmentNode::RenderTag(RenderTag { start: start as u32, end: parser.index as u32, expression })));
        return Ok(());
    }
    Err(e::expected_tag(parser.index))
}
