//! Port of `phases/1-parse/read/options.js` (Svelte 5.57.2 @ f3de3f2).
//!
//! `customElement` is fully validated as upstream, but only its span is stored
//! (`SvelteOptions.custom_element`; this compiler does not build custom elements).

use crate::estree::{Expression, LiteralValue, ObjectExpression, ObjectMember, PropertyKey, Span};
use crate::svelte::ast::{Attribute, AttributeLike, AttributeValue, ExpressionTagNode, SvelteOptions, TextOrExpressionTag};
use crate::svelte::errors as e;
use crate::svelte::parse::{ElementData, PResult};

const NAMESPACE_SVG: &str = "http://www.w3.org/2000/svg";
const NAMESPACE_MATHML: &str = "http://www.w3.org/1998/Math/MathML";

/// The JS value `get_static_value` returns.
#[derive(Clone, Debug, PartialEq)]
enum StaticValue {
    Bool(bool),
    Null,
    Str(String),
    /// numbers, regexps, bigints: never a boolean or a string
    Other,
}

fn literal_value(v: &LiteralValue) -> StaticValue {
    match v {
        LiteralValue::String(s) => StaticValue::Str(s.clone()),
        LiteralValue::Boolean(b) => StaticValue::Bool(*b),
        LiteralValue::Null => StaticValue::Null,
        _ => StaticValue::Other,
    }
}

/// `value[0]` (or the value itself) of an attribute: `Text` or `ExpressionTag`.
enum Chunk<'a> {
    Text(&'a str),
    Expression(&'a Expression),
}

/// `Array.isArray(value) ? value[0] : value` for non-boolean values.
fn first_chunk(value: &AttributeValue) -> Option<Chunk<'_>> {
    match value {
        AttributeValue::True(_) => None,
        AttributeValue::Expression(ExpressionTagNode::ExpressionTag(t)) => Some(Chunk::Expression(&t.expression)),
        AttributeValue::Sequence(v) => v.first().map(|c| match c {
            TextOrExpressionTag::Text(t) => Chunk::Text(&t.data),
            TextOrExpressionTag::ExpressionTag(t) => Chunk::Expression(&t.expression),
        }),
    }
}

pub fn read_options(node: ElementData) -> PResult<SvelteOptions> {
    let span = Span { start: node.start, end: node.end };

    let mut attributes: Vec<Attribute> = Vec::with_capacity(node.attributes.len());
    for attribute in node.attributes {
        match attribute {
            AttributeLike::Attribute(a) => attributes.push(a),
            other => return Err(e::svelte_options_invalid_attribute(&other)),
        }
    }

    let mut component_options = SvelteOptions {
        start: node.start,
        end: node.end,
        runes: None,
        immutable: None,
        accessors: None,
        preserve_whitespace: None,
        namespace: None,
        css: None,
        custom_renderer: None,
        custom_element: None,
        attributes: Vec::new(),
    };

    for attribute in &attributes {
        let name = attribute.name.as_str();

        match name {
            "runes" => {
                component_options.runes = Some(get_boolean_value(attribute)?);
            }
            "tag" => {
                return Err(e::svelte_options_deprecated_tag(attribute));
            }
            "customRenderer" => {
                let chunk = first_chunk(&attribute.value);
                let multi = matches!(&attribute.value, AttributeValue::Sequence(v) if v.len() != 1);

                let Some(chunk) = chunk.filter(|_| !multi) else {
                    return Err(e::svelte_options_invalid_attribute_value(attribute, "a string or null"));
                };

                match chunk {
                    Chunk::Text(data) => component_options.custom_renderer = Some(Some(data.to_string())),
                    Chunk::Expression(Expression::Literal(l)) if matches!(l.value, LiteralValue::String(_) | LiteralValue::Null) => {
                        component_options.custom_renderer = Some(match &l.value {
                            LiteralValue::String(s) => Some(s.clone()),
                            _ => None,
                        });
                    }
                    _ => return Err(e::svelte_options_invalid_attribute_value(attribute, "a string or null")),
                }
            }
            "customElement" => {
                if read_custom_element(attribute)? {
                    component_options.custom_element = Some(Span { start: attribute.start, end: attribute.end });
                }
            }
            "namespace" => {
                let value = get_static_value(attribute);

                match &value {
                    StaticValue::Str(s) if s == NAMESPACE_SVG => component_options.namespace = Some("svg".into()),
                    StaticValue::Str(s) if s == NAMESPACE_MATHML => component_options.namespace = Some("mathml".into()),
                    StaticValue::Str(s) if s == "html" || s == "mathml" || s == "svg" => {
                        component_options.namespace = Some(s.clone())
                    }
                    _ => return Err(e::svelte_options_invalid_attribute_value(attribute, "\"html\", \"mathml\" or \"svg\"")),
                }
            }
            "css" => {
                let value = get_static_value(attribute);

                if value == StaticValue::Str("injected".into()) {
                    component_options.css = Some("injected".into());
                } else {
                    return Err(e::svelte_options_invalid_attribute_value(attribute, "\"injected\""));
                }
            }
            "immutable" => {
                component_options.immutable = Some(get_boolean_value(attribute)?);
            }
            "preserveWhitespace" => {
                component_options.preserve_whitespace = Some(get_boolean_value(attribute)?);
            }
            "accessors" => {
                component_options.accessors = Some(get_boolean_value(attribute)?);
            }
            _ => return Err(e::svelte_options_unknown_attribute(attribute, name)),
        }
    }

    if component_options.css.as_deref() == Some("injected") && matches!(component_options.custom_renderer, Some(Some(_))) {
        // Find the css attribute node for the error position
        let css_attribute = attributes.iter().find(|a| a.name == "css");
        let pos = css_attribute.map_or(span, |a| Span { start: a.start, end: a.end });
        return Err(e::incompatible_with_custom_renderer(pos, "`css: 'injected'`"));
    }

    component_options.attributes = attributes;
    Ok(component_options)
}

/// The `customElement` case. Returns whether `component_options.customElement` is set.
fn read_custom_element(attribute: &Attribute) -> PResult<bool> {
    let first = match &attribute.value {
        AttributeValue::True(_) => return Err(e::svelte_options_invalid_customelement(attribute)),
        _ => first_chunk(&attribute.value),
    };

    let object: &ObjectExpression = match first {
        Some(Chunk::Text(_)) => {
            let tag = get_static_value(attribute);
            validate_tag(attribute.into(), &tag)?;
            return Ok(true);
        }
        Some(Chunk::Expression(Expression::ObjectExpression(o))) => o,
        Some(Chunk::Expression(expr)) => {
            // Before Svelte 4 it was necessary to explicitly set customElement to null or else you'd get a warning.
            // This is no longer necessary, but for backwards compat just skip in this case now.
            if let Expression::Literal(l) = expr
                && l.value == LiteralValue::Null
            {
                return Ok(false);
            }
            return Err(e::svelte_options_invalid_customelement(attribute));
        }
        // `value[0]` of an empty array: never produced by the parser
        None => return Err(e::svelte_options_invalid_customelement(attribute)),
    };

    let mut properties: Vec<(&str, &Expression)> = Vec::new();
    for property in &object.properties {
        match property {
            ObjectMember::Property(p) if !p.computed => match &p.key {
                PropertyKey::Expression(k) => match k.as_ref() {
                    Expression::Identifier(id) => properties.push((&id.name, &p.value)),
                    _ => return Err(e::svelte_options_invalid_customelement(attribute)),
                },
                _ => return Err(e::svelte_options_invalid_customelement(attribute)),
            },
            _ => return Err(e::svelte_options_invalid_customelement(attribute)),
        }
    }

    if let Some((_, value)) = properties.iter().find(|(name, _)| *name == "tag") {
        // `tag[1]?.value`: only a Literal has a `value`
        let tag_value = match value {
            Expression::Literal(l) => literal_value(&l.value),
            _ => StaticValue::Other,
        };
        // SVELTERS: upstream passes the `[name, value]` tuple as the error node, which has no
        // position; we report the attribute's span.
        validate_tag(attribute.into(), &tag_value)?;
    }

    if let Some((_, props)) = properties.iter().find(|(name, _)| *name == "props") {
        let Expression::ObjectExpression(props) = props else {
            return Err(e::svelte_options_invalid_customelement_props(attribute));
        };
        for property in &props.properties {
            let ObjectMember::Property(property) = property else {
                return Err(e::svelte_options_invalid_customelement_props(attribute));
            };
            let key_is_identifier = matches!(&property.key, PropertyKey::Expression(k) if matches!(k.as_ref(), Expression::Identifier(_)));
            let Expression::ObjectExpression(value) = &property.value else {
                return Err(e::svelte_options_invalid_customelement_props(attribute));
            };
            if property.computed || !key_is_identifier {
                return Err(e::svelte_options_invalid_customelement_props(attribute));
            }
            for prop in &value.properties {
                let ObjectMember::Property(prop) = prop else {
                    return Err(e::svelte_options_invalid_customelement_props(attribute));
                };
                let key = match &prop.key {
                    PropertyKey::Expression(k) => match k.as_ref() {
                        Expression::Identifier(id) => Some(id.name.as_str()),
                        _ => None,
                    },
                    _ => None,
                };
                let (Some(key), Expression::Literal(lit), false) = (key, &prop.value, prop.computed) else {
                    return Err(e::svelte_options_invalid_customelement_props(attribute));
                };

                match key {
                    "type" => {
                        let ok = matches!(&lit.value, LiteralValue::String(s) if ["String", "Number", "Boolean", "Array", "Object"].contains(&s.as_str()));
                        if !ok {
                            return Err(e::svelte_options_invalid_customelement_props(attribute));
                        }
                    }
                    "reflect" => {
                        if !matches!(lit.value, LiteralValue::Boolean(_)) {
                            return Err(e::svelte_options_invalid_customelement_props(attribute));
                        }
                    }
                    "attribute" => {
                        if !matches!(lit.value, LiteralValue::String(_)) {
                            return Err(e::svelte_options_invalid_customelement_props(attribute));
                        }
                    }
                    _ => return Err(e::svelte_options_invalid_customelement_props(attribute)),
                }
            }
        }
    }

    if let Some((_, shadow)) = properties.iter().find(|(name, _)| *name == "shadow") {
        let ok = match shadow {
            Expression::Literal(l) => matches!(&l.value, LiteralValue::String(s) if s == "open" || s == "none"),
            Expression::ObjectExpression(_) => true,
            _ => false,
        };
        if !ok {
            return Err(e::svelte_options_invalid_customelement_shadow(attribute));
        }
    }

    Ok(true)
}

fn get_static_value(attribute: &Attribute) -> StaticValue {
    let value = &attribute.value;

    if let AttributeValue::True(_) = value {
        return StaticValue::Bool(true);
    }

    let Some(chunk) = first_chunk(value) else { return StaticValue::Bool(true) };
    if let AttributeValue::Sequence(v) = value
        && v.len() > 1
    {
        return StaticValue::Null;
    }
    match chunk {
        Chunk::Text(data) => StaticValue::Str(data.to_string()),
        Chunk::Expression(Expression::Literal(l)) => literal_value(&l.value),
        Chunk::Expression(_) => StaticValue::Null,
    }
}

fn get_boolean_value(attribute: &Attribute) -> PResult<bool> {
    match get_static_value(attribute) {
        StaticValue::Bool(b) => Ok(b),
        _ => Err(e::svelte_options_invalid_attribute_value(attribute, "true or false")),
    }
}

// https://html.spec.whatwg.org/multipage/custom-elements.html#valid-custom-element-name
fn is_tag_name_char(c: char) -> bool {
    c.is_ascii_lowercase()
        || c.is_ascii_digit()
        || matches!(c, '_' | '.' | '\u{B7}' | '-')
        || ('\u{C0}'..='\u{D6}').contains(&c)
        || ('\u{D8}'..='\u{F6}').contains(&c)
        || ('\u{F8}'..='\u{37D}').contains(&c)
        || ('\u{37F}'..='\u{1FFF}').contains(&c)
        || ('\u{200C}'..='\u{200D}').contains(&c)
        || ('\u{203F}'..='\u{2040}').contains(&c)
        || ('\u{2070}'..='\u{218F}').contains(&c)
        || ('\u{2C00}'..='\u{2FEF}').contains(&c)
        || ('\u{3001}'..='\u{D7FF}').contains(&c)
        || ('\u{F900}'..='\u{FDCF}').contains(&c)
        || ('\u{FDF0}'..='\u{FFFD}').contains(&c)
        || ('\u{10000}'..='\u{EFFFF}').contains(&c)
}

/// `^[a-z]${tag_name_char}*-${tag_name_char}*$` (u): since `-` is itself a tag name char, this
/// is "starts with a-z, contains a `-`, all characters are tag name chars".
fn regex_valid_tag_name(tag: &str) -> bool {
    tag.starts_with(|c: char| c.is_ascii_lowercase()) && tag.contains('-') && tag.chars().all(is_tag_name_char)
}

const RESERVED_TAG_NAMES: [&str; 8] = [
    "annotation-xml",
    "color-profile",
    "font-face",
    "font-face-src",
    "font-face-uri",
    "font-face-format",
    "font-face-name",
    "missing-glyph",
];

fn validate_tag(node: e::NodeLike, tag: &StaticValue) -> PResult<()> {
    let StaticValue::Str(tag) = tag else {
        return Err(e::svelte_options_invalid_tagname(node));
    };
    if !tag.is_empty() {
        if !regex_valid_tag_name(tag) {
            return Err(e::svelte_options_invalid_tagname(node));
        } else if RESERVED_TAG_NAMES.contains(&tag.as_str()) {
            return Err(e::svelte_options_reserved_tagname(node));
        }
    }
    Ok(())
}
