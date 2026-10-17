//! Port of `phases/1-parse/read/script.js` (Svelte 5.57.2 @ f3de3f2).

use crate::svelte::ast::{Attribute, AttributeValue, Script, ScriptContext, TextOrExpressionTag};
use crate::svelte::errors as e;
use crate::svelte::parse::shared::ast_utils::is_text_attribute;
use crate::svelte::parse::utils::whitespace::is_js_regex_space;
use crate::svelte::parse::{PResult, Parser, acorn};

/// `/<\/script\s*>/` (search): offset of the first match.
fn regex_closing_script_tag(s: &str) -> Option<usize> {
    let mut from = 0;
    while let Some(i) = s[from..].find("</script") {
        let at = from + i;
        if regex_starts_with_closing_script_tag(s, at).is_some() {
            return Some(at);
        }
        from = at + 1;
    }
    None
}

/// `/<\/script\s*>/y`
fn regex_starts_with_closing_script_tag(s: &str, at: usize) -> Option<usize> {
    let rest = s.get(at..)?.strip_prefix("</script")?;
    let ws: usize = rest.chars().take_while(|c| is_js_regex_space(*c)).map(char::len_utf8).sum();
    rest[ws..].starts_with('>').then_some(8 + ws + 1)
}

const RESERVED_ATTRIBUTES: [&str; 5] = ["server", "client", "worker", "test", "default"];
const ALLOWED_ATTRIBUTES: [&str; 4] = ["context", "generics", "lang", "module"];

pub fn read_script(parser: &mut Parser, start: usize, attributes: Vec<Attribute>) -> PResult<Script> {
    let script_start = parser.index;
    let data = parser.read_until_regex(regex_closing_script_tag)?;
    if parser.index >= parser.template.len() {
        return Err(e::element_unclosed(parser.template.len(), "script"));
    }

    // `template.slice(0, script_start).replace(/[^\n]/g, ' ') + data`.
    // SVELTERS: positions are byte offsets, so each non-newline character becomes as many spaces
    // as it has UTF-8 bytes (upstream: one space per UTF-16 unit).
    let mut source = String::with_capacity(script_start + data.len());
    for b in parser.template[..script_start].bytes() {
        // continuation bytes of a multi-byte character are never `\n`
        source.push(if b == b'\n' { '\n' } else { ' ' });
    }
    source.push_str(&data);
    parser.read(regex_starts_with_closing_script_tag);

    let mut ast = acorn::parse(&source, &mut parser.root.comments, parser.ts, true)?;

    ast.start = script_start as u32;

    let mut context = ScriptContext::Default;

    for attribute in &attributes {
        if RESERVED_ATTRIBUTES.contains(&attribute.name.as_str()) {
            return Err(e::script_reserved_attribute(attribute, &attribute.name));
        }

        if !ALLOWED_ATTRIBUTES.contains(&attribute.name.as_str()) {
            parser.warnings.push(e::script_unknown_attribute(attribute));
        }

        if attribute.name == "module" {
            if attribute.value != AttributeValue::True(true) {
                // Deliberately a generic code to future-proof for potential other attributes
                return Err(e::script_invalid_attribute_value(attribute, &attribute.name));
            }

            context = ScriptContext::Module;
        }

        if attribute.name == "context" {
            if matches!(attribute.value, AttributeValue::True(_)) || !is_text_attribute(attribute) {
                return Err(e::script_invalid_context(attribute));
            }

            let AttributeValue::Sequence(v) = &attribute.value else { unreachable!() };
            let TextOrExpressionTag::Text(t) = &v[0] else { unreachable!() };

            if t.data != "module" {
                return Err(e::script_invalid_context(attribute));
            }

            context = ScriptContext::Module;
        }
    }

    Ok(Script { start: start as u32, end: parser.index as u32, context, content: ast, attributes })
}
