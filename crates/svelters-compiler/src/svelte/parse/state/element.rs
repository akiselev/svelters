//! Port of `phases/1-parse/state/element.js` (Svelte 5.57.2 @ f3de3f2).
//!
//! SVELTERS: with `ParseOptions.native`, the HTML-only rules are off (each marked below):
//! void elements, implicit closing (`closing_tag_omitted` and the implicit close of a mismatched
//! end tag), the `<textarea>` / nested `<script>`/`<style>` raw-text content models and the
//! declarative-shadow-root `<slot>` exception.

use crate::estree::{Expression, Literal, LiteralValue, Span, Spanned};
use crate::svelte::ast::{
    AttachTag, Attribute, AttributeLike, AttributeValue, Directive, ExpressionTag, ExpressionTagNode, FragmentNode,
    SpreadAttribute, StyleDirective, Text, TextOrExpressionTag, TransitionDirective,
};
use crate::svelte::errors as e;
use crate::svelte::parse::read::expression::read_expression;
use crate::svelte::parse::read::script::read_script;
use crate::svelte::parse::read::style::read_style;
use crate::svelte::parse::shared::ast_utils::{get_attribute_expression, is_expression_attribute};
use crate::svelte::parse::shared::html_tree_validation::closing_tag_omitted;
use crate::svelte::parse::shared::nodes::{create_attribute, is_element_node};
use crate::svelte::parse::shared::string::list;
use crate::svelte::parse::shared::utils::{is_id_continue_or_dollar, is_id_start, is_lu, is_void, regex_valid_tag_name};
use crate::svelte::parse::utils::html::decode_character_references;
use crate::svelte::parse::utils::whitespace::{is_js_regex_space, is_whitespace};
use crate::svelte::parse::{ElementData, ElementType, Item, LastAutoClosedTag, OPEN_END, PResult, Parser, TemplateNode};

/// `/(\/>|[\s"'=<>`])/y`
fn regex_invalid_unquoted_attribute_value(s: &str, at: usize) -> Option<usize> {
    let rest = s.get(at..)?;
    if rest.starts_with("/>") {
        return Some(2);
    }
    let c = rest.chars().next()?;
    (is_js_regex_space(c) || matches!(c, '"' | '\'' | '=' | '<' | '>' | '`')).then_some(c.len_utf8())
}

/// `/<\/textarea(\s[^>]*)?>/iy`
fn regex_closing_textarea_tag(s: &str, at: usize) -> Option<usize> {
    let rest = s.get(at..)?;
    if rest.len() < 10 || !rest.as_bytes()[..10].eq_ignore_ascii_case(b"</textarea") {
        return None;
    }
    let after = &rest[10..];
    if after.starts_with('>') {
        return Some(11);
    }
    let c = after.chars().next()?;
    if is_js_regex_space(c) {
        // `[^>]*>`
        return after.find('>').map(|i| 10 + i + 1);
    }
    None
}

/// `/["']/y`
fn regex_starts_with_quote_characters(s: &str, at: usize) -> Option<usize> {
    matches!(s.as_bytes().get(at), Some(b'"' | b'\'')).then_some(1)
}

/// `/(?:"([^"]*)"|'([^'])*'|([^>\s]+))/y`
fn regex_attribute_value(s: &str, at: usize) -> Option<usize> {
    let rest = s.get(at..)?;
    for q in ['"', '\''] {
        if let Some(r) = rest.strip_prefix(q)
            && let Some(end) = r.find(q)
        {
            return Some(end + 2);
        }
    }
    let n: usize = rest.chars().take_while(|c| *c != '>' && !is_js_regex_space(*c)).map(char::len_utf8).sum();
    (n > 0).then_some(n)
}

/// `/^![a-zA-Z]+$/`
fn regex_doctype_name(name: &str) -> bool {
    name.strip_prefix('!').is_some_and(|r| !r.is_empty() && r.bytes().all(|b| b.is_ascii_alphabetic()))
}

/// `/^[a-zA-Z][a-zA-Z0-9]*:[a-zA-Z][a-zA-Z0-9-]*[a-zA-Z0-9]$/`
fn regex_namespaced_name(name: &str) -> bool {
    let Some((ns, local)) = name.split_once(':') else { return false };
    let ns_ok = ns.bytes().next().is_some_and(|b| b.is_ascii_alphabetic()) && ns.bytes().all(|b| b.is_ascii_alphanumeric());
    let l = local.as_bytes();
    let local_ok = l.len() >= 2
        && l[0].is_ascii_alphabetic()
        && l[l.len() - 1].is_ascii_alphanumeric()
        && l[1..l.len() - 1].iter().all(|b| b.is_ascii_alphanumeric() || *b == b'-');
    ns_ok && local_ok
}

fn is_valid_element_name(name: &str) -> bool {
    // DOCTYPE (e.g. !DOCTYPE)
    if regex_doctype_name(name) {
        return true;
    }
    // svelte:* meta tags (e.g. svelte:element, svelte:head)
    if regex_namespaced_name(name) {
        return true;
    }
    // standard HTML/SVG/MathML elements and custom elements
    regex_valid_tag_name(name)
}

/// https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Lexical_grammar#identifiers adjusted for our needs
/// (must start with uppercase letter if no dots, can contain dots)
/// `/^(?:\p{Lu}[$‌‍\p{ID_Continue}.]*|\p{ID_Start}[$‌‍\p{ID_Continue}]*(?:\.[$‌‍\p{ID_Continue}]+)+)$/u`
pub fn regex_valid_component_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else { return false };
    let rest = chars.as_str();

    if is_lu(first) && rest.chars().all(|c| c == '.' || is_id_continue_or_dollar(c)) {
        return true;
    }

    if !is_id_start(first) {
        return false;
    }
    let mut segments = rest.split('.');
    let head = segments.next().unwrap_or("");
    if !head.chars().all(is_id_continue_or_dollar) {
        return false;
    }
    let mut count = 0;
    for seg in segments {
        if seg.is_empty() || !seg.chars().all(is_id_continue_or_dollar) {
            return false;
        }
        count += 1;
    }
    count >= 1
}

const ROOT_ONLY_META_TAGS: [(&str, ElementType); 5] = [
    ("svelte:head", ElementType::SvelteHead),
    ("svelte:options", ElementType::SvelteOptions),
    ("svelte:window", ElementType::SvelteWindow),
    ("svelte:document", ElementType::SvelteDocument),
    ("svelte:body", ElementType::SvelteBody),
];

const META_TAGS: [(&str, ElementType); 10] = [
    ("svelte:head", ElementType::SvelteHead),
    ("svelte:options", ElementType::SvelteOptions),
    ("svelte:window", ElementType::SvelteWindow),
    ("svelte:document", ElementType::SvelteDocument),
    ("svelte:body", ElementType::SvelteBody),
    ("svelte:element", ElementType::SvelteElement),
    ("svelte:component", ElementType::SvelteComponent),
    ("svelte:self", ElementType::SvelteSelf),
    ("svelte:fragment", ElementType::SvelteFragment),
    ("svelte:boundary", ElementType::SvelteBoundary),
];

fn meta_tag(name: &str) -> Option<ElementType> {
    META_TAGS.iter().find(|(n, _)| *n == name).map(|(_, t)| *t)
}

fn attribute_name(a: &AttributeLike) -> Option<&str> {
    match a {
        AttributeLike::Attribute(a) => Some(&a.name),
        _ => None,
    }
}

/// `{ type, start, end, raw, data }` of a Text node
fn text(start: usize, end: usize, raw: String, data: String) -> Text {
    Text { start: start as u32, end: end as u32, raw, data }
}

pub fn element(parser: &mut Parser) -> PResult<()> {
    let start = parser.index;
    parser.index += 1;

    let mut parent = parser.current();

    if parser.eat_opt("!--") {
        let data = parser.read_until("-->")?;
        parser.eat_required("-->")?;

        parser.append(Item::Leaf(FragmentNode::Comment(crate::svelte::ast::Comment {
            start: start as u32,
            end: parser.index as u32,
            data,
        })));

        return Ok(());
    }

    if parser.eat_opt("/") {
        let name = read_tag_name(parser, false)?;

        parser.allow_whitespace();
        parser.eat_required(">")?;

        // SVELTERS: native mode has no void elements
        if !parser.native && is_void(&name) {
            return Err(e::void_element_invalid_content(start));
        }

        // close any elements that don't have their own closing tags, e.g. <div><p></div>
        while parser.nodes[parent].name() != Some(name.as_str()) {
            if parser.loose {
                // If the previous element did interpret the next opening tag as an attribute, backtrack
                if let TemplateNode::Element(el) = &parser.nodes[parent]
                    && is_element_node(el.type_)
                    && let Some(AttributeLike::Attribute(last)) = el.attributes.last()
                    && last.name == format!("<{name}")
                {
                    parser.index = last.start as usize;
                    parser.element_mut(parent).attributes.pop();
                    break;
                }
            }

            let parent_type = parser.nodes[parent].type_name();
            // SVELTERS: native mode never closes elements implicitly; a mismatched end tag is
            // an error, as it is for components and blocks upstream.
            if parent_type == "RegularElement" && !parser.native {
                if parser.last_auto_closed_tag.as_ref().is_none_or(|t| t.tag != name) {
                    let el = parser.element(parent);
                    let end = parser.frags[el.fragment].nodes.first().map_or(start as u32, |n| parser.item_span(n).start);
                    let warning = e::element_implicitly_closed((el.start, end), &format!("</{name}>"), &format!("</{}>", el.name));
                    parser.warnings.push(warning);
                }
            } else if !parser.loose {
                if let Some(t) = &parser.last_auto_closed_tag
                    && t.tag == name
                {
                    return Err(e::element_invalid_closing_tag_autoclosed(start, &name, &t.reason));
                } else {
                    return Err(e::element_invalid_closing_tag(start, &name));
                }
            }

            parser.nodes[parent].set_end(start as u32);
            parser.pop();

            // SVELTERS: upstream crashes with a TypeError when loose mode pops the root
            if parser.stack.is_empty() {
                return Err(e::element_invalid_closing_tag(start, &name));
            }
            parent = parser.current();
        }

        parser.nodes[parent].set_end(parser.index as u32);
        parser.pop();

        if let Some(t) = &parser.last_auto_closed_tag
            && parser.stack.len() < t.depth
        {
            parser.last_auto_closed_tag = None;
        }

        return Ok(());
    }

    let tag = read_tag(parser, false)?;
    let tag_name = tag.0.clone();
    let tag_loc = tag.1;

    if tag_name.starts_with("svelte:") && meta_tag(&tag_name).is_none() {
        let bounds = (start + 1, start + 1 + tag_name.len());
        let names: Vec<&str> = META_TAGS.iter().map(|(n, _)| *n).collect();
        return Err(e::svelte_meta_invalid_tag(bounds, &list(&names, "or")));
    }

    if !is_valid_element_name(&tag_name) && !regex_valid_component_name(&tag_name) {
        // <div. -> in the middle of typing -> allow in loose mode
        if !parser.loose || !tag_name.ends_with('.') {
            let bounds = (start + 1, start + 1 + tag_name.len());
            return Err(e::tag_invalid_name(bounds));
        }
    }

    if ROOT_ONLY_META_TAGS.iter().any(|(n, _)| *n == tag_name) {
        if parser.meta_tags.contains(&tag_name) {
            return Err(e::svelte_meta_duplicate(start, &tag_name));
        }

        if parser.nodes[parent].type_name() != "Root" {
            return Err(e::svelte_meta_invalid_placement(start, &tag_name));
        }

        parser.meta_tags.insert(tag_name.clone());
    }

    let type_ = if let Some(t) = meta_tag(&tag_name) {
        t
    } else if regex_valid_component_name(&tag_name) || (parser.loose && tag_name.ends_with('.')) {
        ElementType::Component
    } else if tag_name == "title" && parent_is_head(parser) {
        ElementType::TitleElement
    } else if
    // TODO Svelte 6/7: once slots are removed in favor of snippets, always keep slot as a regular element
    tag_name == "slot" && !parent_is_shadowroot_template(parser) {
        ElementType::SlotElement
    } else {
        ElementType::RegularElement
    };

    let fragment = parser.create_fragment(true);
    let mut element = ElementData {
        type_,
        start: start as u32,
        end: OPEN_END,
        name: tag_name.clone(),
        name_span: tag_loc,
        attributes: Vec::new(),
        fragment,
        expression: None,
        tag: None,
    };

    parser.allow_whitespace();

    // SVELTERS: native mode has no implicit closing
    if !parser.native
        && let TemplateNode::Element(p) = &parser.nodes[parent]
        && p.type_ == ElementType::RegularElement
        && closing_tag_omitted(&p.name, Some(&tag_name))
    {
        let end = parser.frags[p.fragment].nodes.first().map_or(start as u32, |n| parser.item_span(n).start);
        let warning = e::element_implicitly_closed((p.start, end), &format!("<{tag_name}>"), &format!("</{}>", p.name));
        let parent_name = p.name.clone();
        parser.warnings.push(warning);
        parser.nodes[parent].set_end(start as u32);
        parser.pop();
        parser.last_auto_closed_tag = Some(LastAutoClosedTag { tag: parent_name, reason: tag_name.clone(), depth: parser.stack.len() });
    }

    let mut unique_names: Vec<String> = Vec::new();

    let current = parser.current();
    let is_top_level_script_or_style = (tag_name == "script" || tag_name == "style") && matches!(parser.nodes[current], TemplateNode::Root);

    loop {
        let attribute = if is_top_level_script_or_style {
            read_static_attribute(parser)?.map(AttributeLike::Attribute)
        } else {
            read_attribute(parser)?
        };
        let Some(attribute) = attribute else { break };

        // animate and transition can only be specified once per element so no need
        // to check here, use can be used multiple times, same for the on directive
        // finally let already has error handling in case of duplicate variable names
        let keyed = match &attribute {
            // `bind:attribute` and `attribute` are just the same but `class:attribute`,
            // `style:attribute` and `attribute` are different and should be allowed together
            // so we concatenate the type while normalizing the type for BindDirective
            AttributeLike::Attribute(a) => Some(("Attribute", &a.name)),
            AttributeLike::BindDirective(d) => Some(("Attribute", &d.name)),
            AttributeLike::StyleDirective(d) => Some(("StyleDirective", &d.name)),
            AttributeLike::ClassDirective(d) => Some(("ClassDirective", &d.name)),
            _ => None,
        };
        if let Some((t, name)) = keyed {
            let key = format!("{t}{name}");
            if unique_names.contains(&key) {
                return Err(e::attribute_duplicate(&attribute));
                // <svelte:element bind:this this=..> is allowed
            } else if name != "this" {
                unique_names.push(key);
            }
        }

        element.attributes.push(attribute);
        parser.allow_whitespace();
    }

    if element.type_ == ElementType::SvelteComponent {
        let index = element.attributes.iter().position(|attr| attribute_name(attr) == Some("this"));
        let Some(index) = index else {
            return Err(e::svelte_component_missing_this(start));
        };

        let AttributeLike::Attribute(definition) = element.attributes.remove(index) else { unreachable!() };
        if !is_expression_attribute(&definition) {
            return Err(e::svelte_component_invalid_this(definition.start));
        }

        element.expression = Some(get_attribute_expression(definition));
    }

    if element.type_ == ElementType::SvelteElement {
        let index = element.attributes.iter().position(|attr| attribute_name(attr) == Some("this"));
        let Some(index) = index else {
            return Err(e::svelte_element_missing_this(start));
        };

        let AttributeLike::Attribute(definition) = element.attributes.remove(index) else { unreachable!() };

        if definition.value == AttributeValue::True(true) {
            return Err(e::svelte_element_missing_this(&definition));
        }

        if !is_expression_attribute(&definition) {
            parser.warnings.push(e::svelte_element_invalid_this(&definition));

            // note that this is wrong, in the case of e.g. `this="h{n}"` — it will result in `<h>`.
            // it would be much better to just error here, but we are preserving the existing buggy
            // Svelte 4 behaviour out of an overabundance of caution regarding breaking changes.
            // TODO in 6.0, error
            let AttributeValue::Sequence(mut chunks) = definition.value else { unreachable!() };
            element.tag = Some(match chunks.remove(0) {
                TextOrExpressionTag::Text(chunk) => Expression::Literal(Literal {
                    start: chunk.start,
                    end: chunk.end,
                    value: LiteralValue::String(chunk.data),
                    raw: format!("'{}'", chunk.raw),
                    regex: None,
                    bigint: None,
                }),
                TextOrExpressionTag::ExpressionTag(chunk) => chunk.expression,
            });
        } else {
            element.tag = Some(get_attribute_expression(definition));
        }
    }

    if is_top_level_script_or_style {
        parser.eat_required(">")?;

        // SVELTERS: the HTML comment before the block (`leadingComments` of the script's
        // Program, `content.comment` of the style) is found as upstream but not stored: our
        // ESTree / StyleBlock have no field for it.
        let nodes = &parser.frags[0].nodes; // `current.fragment` (current is the root)
        let mut _prev_comment: Option<&crate::svelte::ast::Comment> = None;
        for i in (0..nodes.len()).rev() {
            let node = &nodes[i];

            if i == nodes.len() - 1 && parser.item_span(node).end != start as u32 {
                break;
            }

            match node {
                Item::Leaf(FragmentNode::Comment(c)) => {
                    _prev_comment = Some(c);
                    break;
                }
                Item::Leaf(FragmentNode::Text(t)) if js_trim(&t.data).is_empty() => {}
                _ => break,
            }
        }

        let attributes: Vec<Attribute> = element
            .attributes
            .into_iter()
            .map(|a| match a {
                AttributeLike::Attribute(a) => a,
                _ => unreachable!("read_static_attribute only returns attributes"),
            })
            .collect();

        if tag_name == "script" {
            let content = read_script(parser, start, attributes)?;

            if content.context == crate::svelte::ast::ScriptContext::Module {
                if parser.root.module.is_some() {
                    return Err(e::script_duplicate(start));
                }
                parser.root.module = Some(content);
            } else {
                if parser.root.instance.is_some() {
                    return Err(e::script_duplicate(start));
                }
                parser.root.instance = Some(content);
            }
        } else {
            let content = read_style(parser, start, attributes)?;

            if parser.root.css.is_some() {
                return Err(e::style_duplicate(start));
            }
            parser.root.css = Some(content);
        }
        return Ok(());
    }

    let id = parser.append_node(TemplateNode::Element(element));

    // SVELTERS: native mode has no void elements
    let self_closing = parser.eat_opt("/") || (!parser.native && is_void(&tag_name));
    let closed = parser.eat(">", true, false)?;

    // Loose parsing mode
    if !closed {
        // We may have eaten an opening `<` of the next element and treated it as an attribute...
        let el = parser.element(id);
        if let Some(AttributeLike::Attribute(last)) = el.attributes.last()
            && last.name == "<"
        {
            parser.index = last.start as usize;
            parser.element_mut(id).attributes.pop();
        } else {
            // ... or we may have eaten part of a following block ...
            let prev_1 = parser.index.checked_sub(1).and_then(|i| parser.byte_at(i));
            let prev_2 = parser.index.checked_sub(2).and_then(|i| parser.byte_at(i));
            let current = parser.byte_at(parser.index);
            if prev_2 == Some(b'{') && prev_1 == Some(b'/') {
                parser.index -= 2;
            } else if prev_1 == Some(b'{') && matches!(current, Some(b'#' | b'@' | b':')) {
                parser.index -= 1;
            } else {
                // ... or we're followed by whitespace, for example near the end of the template,
                // which we want to take in so that language tools has more room to work with
                parser.allow_whitespace();
            }
        }
    }

    if self_closing || !closed {
        // don't push self-closing elements onto the stack
        parser.element_mut(id).end = parser.index as u32;
    } else if tag_name == "textarea" && !parser.native {
        // special case (SVELTERS: HTML-only content model, off in native mode)
        let nodes = read_sequence(parser, |p| regex_closing_textarea_tag(&p.template, p.index).is_some(), "inside <textarea>")?;
        let frag = parser.element(id).fragment;
        parser.frags[frag].nodes = nodes
            .into_iter()
            .map(|n| {
                Item::Leaf(match n {
                    TextOrExpressionTag::Text(t) => FragmentNode::Text(t),
                    TextOrExpressionTag::ExpressionTag(t) => FragmentNode::ExpressionTag(t),
                })
            })
            .collect();
        parser.read(regex_closing_textarea_tag);
        parser.element_mut(id).end = parser.index as u32;
    } else if (tag_name == "script" || tag_name == "style") && !parser.native {
        // special case (SVELTERS: HTML-only content model, off in native mode)
        let start = parser.index;
        let close_tag = format!("</{tag_name}>");
        let close_index = parser.template[parser.index..].find(&close_tag).map(|i| i + parser.index);
        let end = close_index.unwrap_or(parser.template.len());
        let data = parser.template[parser.index..end].to_string();
        parser.index = end;

        let node = text(start, end, data.clone(), data);

        let frag = parser.element(id).fragment;
        parser.frags[frag].nodes.push(Item::Leaf(FragmentNode::Text(node)));
        parser.eat_required(&close_tag)?;
        parser.element_mut(id).end = parser.index as u32;
    } else {
        let frag = parser.element(id).fragment;
        parser.stack.push(id);
        parser.fragments.push(frag);
    }

    Ok(())
}

/// `String.prototype.trim` (JS whitespace).
fn js_trim(s: &str) -> &str {
    s.trim_matches(|c: char| is_whitespace(c as u32))
}

fn parent_is_head(parser: &Parser) -> bool {
    for &id in parser.stack.iter().rev() {
        let t = parser.nodes[id].type_name();
        if t == "SvelteHead" {
            return true;
        }
        if t == "RegularElement" || t == "Component" {
            return false;
        }
    }
    false
}

fn parent_is_shadowroot_template(parser: &Parser) -> bool {
    // SVELTERS: declarative shadow DOM is HTML-only
    if parser.native {
        return false;
    }
    // https://developer.chrome.com/docs/css-ui/declarative-shadow-dom#building_a_declarative_shadow_root
    parser.stack.iter().rev().any(|&id| match &parser.nodes[id] {
        TemplateNode::Element(el) if el.type_ == ElementType::RegularElement => {
            el.attributes.iter().any(|a| attribute_name(a) == Some("shadowrootmode"))
        }
        _ => false,
    })
}

fn read_static_attribute(parser: &mut Parser) -> PResult<Option<Attribute>> {
    let start = parser.index;

    let (name, loc) = read_tag(parser, true)?;
    if name.is_empty() {
        return Ok(None);
    }

    let mut value = AttributeValue::True(true);

    if parser.eat_opt("=") {
        parser.allow_whitespace();
        let Some(raw) = parser.match_regex(regex_attribute_value).map(str::to_string) else {
            return Err(e::expected_attribute_value(parser.index));
        };

        parser.index += raw.len();

        let quoted = raw.starts_with('"') || raw.starts_with('\'');
        // `raw.slice(1, -1)`
        let raw = if quoted { raw.get(1..raw.len().saturating_sub(1)).unwrap_or("").to_string() } else { raw };

        let t_start = parser.index - raw.len() - if quoted { 1 } else { 0 };
        let t_end = if quoted { parser.index - 1 } else { parser.index };
        let data = decode_character_references(&raw, true);
        value = AttributeValue::Sequence(vec![TextOrExpressionTag::Text(text(t_start, t_end, raw, data))]);
    }

    if parser.match_regex(regex_starts_with_quote_characters).is_some() {
        return Err(e::expected_token(parser.index, "="));
    }

    Ok(Some(create_attribute(name, Some(loc), start, parser.index, value)))
}

fn read_attribute(parser: &mut Parser) -> PResult<Option<AttributeLike>> {
    while let Some(comment) = read_comment(parser)? {
        parser.root.comments.push(comment);
        parser.allow_whitespace();
    }

    let start = parser.index;

    if parser.eat_opt("{") {
        parser.allow_whitespace();

        if parser.eat_opt("@attach") {
            parser.require_whitespace()?;

            let expression = read_expression(parser, None, false)?;
            parser.allow_whitespace();
            parser.eat_required("}")?;

            return Ok(Some(AttributeLike::AttachTag(AttachTag {
                start: start as u32,
                end: parser.index as u32,
                expression,
            })));
        }

        if parser.eat_opt("...") {
            let expression = read_expression(parser, None, false)?;

            parser.allow_whitespace();
            parser.eat_required("}")?;

            return Ok(Some(AttributeLike::SpreadAttribute(SpreadAttribute {
                start: start as u32,
                end: parser.index as u32,
                expression,
            })));
        } else {
            let id = parser.read_identifier()?;

            if id.name.is_empty() {
                if parser.loose && (parser.match_str("#") || parser.match_str("/") || parser.match_str("@") || parser.match_str(":")) {
                    // We're likely in an unclosed opening tag and did read part of a block.
                    // Return null to not crash the parser so it can continue with closing the tag.
                    return Ok(None);
                } else if parser.loose && parser.match_str("}") {
                    // Likely in the middle of typing, just created the shorthand
                } else {
                    return Err(e::attribute_empty_shorthand(start));
                }
            }

            parser.allow_whitespace();
            parser.eat_required("}")?;

            let name = id.name.clone();
            let loc = id.span();
            let expression = ExpressionTag { start: id.start, end: id.end, expression: Expression::Identifier(id) };

            return Ok(Some(AttributeLike::Attribute(create_attribute(
                name,
                Some(loc),
                start,
                parser.index,
                AttributeValue::Expression(ExpressionTagNode::ExpressionTag(expression)),
            ))));
        }
    }

    let (tag_name, tag_loc) = read_tag(parser, true)?;

    if tag_name.is_empty() {
        return Ok(None);
    }

    let mut end = parser.index;

    parser.allow_whitespace();

    let colon_index = tag_name.find(':');
    let type_ = colon_index.and_then(|i| get_directive_type(&tag_name[..i]));

    let mut value = AttributeValue::True(true);
    if parser.eat_opt("=") {
        parser.allow_whitespace();

        if parser.byte_at(parser.index) == Some(b'/') && parser.byte_at(parser.index + 1) == Some(b'>') {
            let char_start = parser.index;
            parser.index += 1; // consume '/'
            value = AttributeValue::Sequence(vec![TextOrExpressionTag::Text(text(char_start, char_start + 1, "/".into(), "/".into()))]);
            end = parser.index;
        } else {
            value = read_attribute_value(parser)?;
            end = parser.index;
        }
    } else if parser.match_regex(regex_starts_with_quote_characters).is_some() {
        return Err(e::expected_token(parser.index, "="));
    }

    if let (Some(type_), Some(colon_index)) = (type_, colon_index) {
        let mut parts = tag_name[colon_index + 1..].split('|');
        let directive_name = parts.next().unwrap_or("").to_string();
        let modifiers: Vec<String> = parts.map(str::to_string).collect();

        if directive_name.is_empty() {
            return Err(e::directive_missing_name((start, start + colon_index + 1), &tag_name));
        }

        if type_ == "StyleDirective" {
            return Ok(Some(AttributeLike::StyleDirective(StyleDirective {
                start: start as u32,
                end: end as u32,
                name: directive_name,
                name_span: Some(tag_loc),
                value,
                modifiers,
            })));
        }

        let mut expression: Option<Expression> = None;

        match value {
            AttributeValue::True(_) => {}
            AttributeValue::Expression(ExpressionTagNode::ExpressionTag(t)) => {
                // TODO throw a parser error in a future version here if this `[ExpressionTag]` instead of `ExpressionTag`,
                // which means stringified value, which isn't allowed for some directives?
                expression = Some(t.expression);
            }
            // `value[0]` of an empty sequence (loose mode only) is undefined
            AttributeValue::Sequence(v) if v.is_empty() => {}
            AttributeValue::Sequence(mut v) => {
                let attribute_contains_text = v.len() > 1 || matches!(v[0], TextOrExpressionTag::Text(_));
                if attribute_contains_text {
                    let first_start = match &v[0] {
                        TextOrExpressionTag::Text(t) => t.start,
                        TextOrExpressionTag::ExpressionTag(t) => t.start,
                    };
                    return Err(e::directive_invalid_value(first_start));
                }
                let TextOrExpressionTag::ExpressionTag(t) = v.remove(0) else { unreachable!() };
                expression = Some(t.expression);
            }
        }

        // Directive name is expression, e.g. <p class:isRed />
        if (type_ == "BindDirective" || type_ == "ClassDirective") && expression.is_none() {
            expression = Some(Expression::Identifier(crate::estree::Identifier {
                start: (start + colon_index + 1) as u32,
                end: end as u32,
                name: directive_name.clone(),
            }));
        }

        if type_ == "TransitionDirective" {
            let direction = &tag_name[..colon_index];
            return Ok(Some(AttributeLike::TransitionDirective(TransitionDirective {
                start: start as u32,
                end: end as u32,
                name: directive_name,
                name_span: Some(tag_loc),
                expression,
                modifiers,
                intro: direction == "in" || direction == "transition",
                outro: direction == "out" || direction == "transition",
            })));
        }

        let directive = Directive {
            start: start as u32,
            end: end as u32,
            name: directive_name,
            name_span: Some(tag_loc),
            expression,
            modifiers,
        };

        return Ok(Some(match type_ {
            "UseDirective" => AttributeLike::UseDirective(directive),
            "AnimateDirective" => AttributeLike::AnimateDirective(directive),
            "BindDirective" => AttributeLike::BindDirective(directive),
            "ClassDirective" => AttributeLike::ClassDirective(directive),
            "OnDirective" => AttributeLike::OnDirective(directive),
            "LetDirective" => AttributeLike::LetDirective(directive),
            _ => unreachable!(),
        }));
    }

    Ok(Some(AttributeLike::Attribute(create_attribute(tag_name, Some(tag_loc), start, end, value))))
}

fn read_comment(parser: &mut Parser) -> PResult<Option<crate::estree::Comment>> {
    use crate::estree::{Comment, CommentKind};
    let start = parser.index;

    if parser.eat_opt("//") {
        let value = parser.read_until("\n")?;
        let end = parser.index;

        return Ok(Some(Comment { kind: CommentKind::Line, start: start as u32, end: end as u32, value }));
    }

    if parser.eat_opt("/*") {
        let value = parser.read_until("*/")?;

        parser.eat_opt("*/");
        let end = parser.index;

        return Ok(Some(Comment { kind: CommentKind::Block, start: start as u32, end: end as u32, value }));
    }

    Ok(None)
}

fn get_directive_type(name: &str) -> Option<&'static str> {
    Some(match name {
        "use" => "UseDirective",
        "animate" => "AnimateDirective",
        "bind" => "BindDirective",
        "class" => "ClassDirective",
        "style" => "StyleDirective",
        "on" => "OnDirective",
        "let" => "LetDirective",
        "in" | "out" | "transition" => "TransitionDirective",
        _ => return None,
    })
}

fn read_attribute_value(parser: &mut Parser) -> PResult<AttributeValue> {
    let quote_mark = if parser.eat_opt("'") {
        Some('\'')
    } else if parser.eat_opt("\"") {
        Some('"')
    } else {
        None
    };
    if let Some(q) = quote_mark
        && parser.eat_opt(&q.to_string())
    {
        return Ok(AttributeValue::Sequence(vec![TextOrExpressionTag::Text(text(
            parser.index - 1,
            parser.index - 1,
            String::new(),
            String::new(),
        ))]));
    }

    let result = read_sequence(
        parser,
        |p| {
            // handle common case of quote marks existing outside of regex for performance reasons
            if let Some(q) = quote_mark {
                return p.byte_at(p.index) == Some(q as u8);
            }
            p.match_regex(regex_invalid_unquoted_attribute_value).is_some()
        },
        "in attribute value",
    );
    let value = match result {
        Ok(v) => v,
        Err(error) => {
            if error.code == "js_parse_error" {
                // if the attribute value didn't close + self-closing tag
                // eg: `<Component test={{a:1} />`
                // acorn may throw a `Unterminated regular expression` because of `/>`
                let pos = error.start as usize;
                if pos >= 1 && parser.template.get(pos - 1..pos + 1) == Some("/>") {
                    parser.index = pos;
                    return Err(e::expected_token(pos, &quote_mark.map_or("}".to_string(), |q| q.to_string())));
                }
            }
            return Err(error);
        }
    };

    if value.is_empty() && quote_mark.is_none() {
        return Err(e::expected_attribute_value(parser.index));
    }

    if quote_mark.is_some() {
        parser.index += 1;
    }

    if quote_mark.is_some() || value.len() > 1 || matches!(value[0], TextOrExpressionTag::Text(_)) {
        Ok(AttributeValue::Sequence(value))
    } else {
        let Some(TextOrExpressionTag::ExpressionTag(t)) = value.into_iter().next() else { unreachable!() };
        Ok(AttributeValue::Expression(ExpressionTagNode::ExpressionTag(t)))
    }
}

fn read_sequence(parser: &mut Parser, done: impl Fn(&Parser) -> bool, location: &str) -> PResult<Vec<TextOrExpressionTag>> {
    let mut chunks: Vec<TextOrExpressionTag> = Vec::new();
    let mut chunk_start = parser.index;

    fn flush(parser: &Parser, chunks: &mut Vec<TextOrExpressionTag>, chunk_start: usize, end: usize) {
        if end > chunk_start {
            let raw = parser.template[chunk_start..end].to_string();
            let data = decode_character_references(&raw, true);
            chunks.push(TextOrExpressionTag::Text(text(chunk_start, end, raw, data)));
        }
    }

    while parser.index < parser.template.len() {
        let index = parser.index;

        if done(parser) {
            flush(parser, &mut chunks, chunk_start, parser.index);
            return Ok(chunks);
        } else if parser.eat_opt("{") {
            if parser.match_str("#") {
                let index = parser.index - 1;
                parser.eat_opt("#");
                let name = read_lowercase_name(parser);
                return Err(e::block_invalid_placement(index, &name, location));
            } else if parser.match_str("@") {
                let index = parser.index - 1;
                parser.eat_opt("@");
                let name = read_lowercase_name(parser);
                return Err(e::tag_invalid_placement(index, &name, location));
            }

            flush(parser, &mut chunks, chunk_start, parser.index - 1);

            parser.allow_whitespace();
            let expression = read_expression(parser, None, false)?;
            parser.allow_whitespace();
            parser.eat_required("}")?;

            chunks.push(TextOrExpressionTag::ExpressionTag(ExpressionTag {
                start: index as u32,
                end: parser.index as u32,
                expression,
            }));
            chunk_start = parser.index;
        } else {
            // step one character (upstream: one UTF-16 unit; `done` only matches at ASCII
            // characters, so landing inside a surrogate pair there is unobservable)
            parser.index += parser.char_at(parser.index).map_or(1, char::len_utf8);
        }
    }

    if parser.loose { Ok(chunks) } else { Err(e::unexpected_eof(parser.template.len())) }
}

fn read_tag_name(parser: &mut Parser, attribute: bool) -> PResult<String> {
    let start = parser.index;
    if start >= parser.template.len() && !parser.loose {
        return Err(e::unexpected_eof(parser.template.len()));
    }

    while parser.index < parser.template.len() {
        let cc = parser.char_at(parser.index).unwrap();
        if is_whitespace(cc as u32)
            || cc == '/'
            || cc == '>'
            || (attribute && (cc == '"' || cc == '\'' || cc == '='))
        {
            break;
        }
        parser.index += cc.len_utf8();
    }

    Ok(parser.template[start..parser.index].to_string())
}

/// `read_tag`: the name and its span (upstream returns an Identifier with `loc`).
fn read_tag(parser: &mut Parser, attribute: bool) -> PResult<(String, Span)> {
    let start = parser.index;
    let name = read_tag_name(parser, attribute)?;
    let end = parser.index;

    Ok((name, Span { start: start as u32, end: end as u32 }))
}

fn read_lowercase_name(parser: &mut Parser) -> String {
    let start = parser.index;
    while parser.index < parser.template.len() {
        let cc = parser.template.as_bytes()[parser.index];
        // a-z
        if !cc.is_ascii_lowercase() {
            break;
        }
        parser.index += 1;
    }
    parser.template[start..parser.index].to_string()
}
