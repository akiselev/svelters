//! `extract_paths(param, initial)` (utils/ast.js) producing JS text: every leaf of a destructuring
//! pattern with the expression that reads it from `initial`, plus the intermediate `$$array`
//! declarations array patterns need (`$.to_array`, since the value may be any iterable).

use super::visit::{Rw, is_simple_expression, has_await};
use crate::codegen::component::js_str;
use crate::estree::*;
use crate::svelte::scope::ScopeId;

pub(crate) enum Target<'n> {
    Id(&'n Identifier),
    Member(&'n MemberExpression),
}

pub(crate) struct Path<'n> {
    pub node: Target<'n>,
    pub expression: String,
    #[allow(dead_code)]
    pub is_rest: bool,
    #[allow(dead_code)]
    pub has_default_value: bool,
}

pub(crate) struct Insert {
    pub id: String,
    pub value: String,
}

impl Rw<'_, '_, '_> {
    /// `derived_arrays`: the `$$array` inserts become deriveds (declarations), so reads of them
    /// are `$.get($$array)`; in assignments they are plain variables.
    pub(crate) fn extract_paths<'n>(&self, param: &'n Pattern, initial: String, scope: ScopeId, derived_arrays: bool) -> (Vec<Insert>, Vec<Path<'n>>) {
        let mut inserts = Vec::new();
        let mut paths = Vec::new();
        self.extract(&mut paths, &mut inserts, param, initial, false, scope, derived_arrays);
        (inserts, paths)
    }

    #[allow(clippy::too_many_arguments)]
    fn extract<'n>(&self, paths: &mut Vec<Path<'n>>, inserts: &mut Vec<Insert>, param: &'n Pattern, expression: String, has_default_value: bool, scope: ScopeId, derived: bool) {
        match param {
            Pattern::Identifier(id) => paths.push(Path { node: Target::Id(id), expression, is_rest: false, has_default_value }),
            Pattern::MemberExpression(m) => paths.push(Path { node: Target::Member(m), expression, is_rest: false, has_default_value }),
            Pattern::ObjectPattern(op) => {
                for prop in &op.properties {
                    match prop {
                        ObjectPatternMember::RestElement(r) => {
                            let mut props = Vec::new();
                            for p in &op.properties {
                                let ObjectPatternMember::Property(p) = p else { continue };
                                match &p.key {
                                    PropertyKey::Expression(k) => match k.as_ref() {
                                        Expression::Identifier(i) if !p.computed => props.push(js_str(&i.name)),
                                        Expression::Literal(l) => props.push(js_str(&literal_string(l))),
                                        other => props.push(format!("String({})", self.text(other, scope))),
                                    },
                                    PropertyKey::Private(_) => {}
                                }
                            }
                            let rest = format!("$.exclude_from_object({expression}, [{}])", props.join(", "));
                            match r.argument.as_ref() {
                                Pattern::Identifier(id) => paths.push(Path { node: Target::Id(id), expression: rest, is_rest: true, has_default_value }),
                                other => self.extract(paths, inserts, other, rest, has_default_value, scope, derived),
                            }
                        }
                        ObjectPatternMember::Property(p) => {
                            let member = match &p.key {
                                PropertyKey::Expression(k) => match k.as_ref() {
                                    Expression::Identifier(i) if !p.computed => format!("{expression}.{}", i.name),
                                    Expression::Literal(l) if !p.computed => format!("{expression}[{}]", l.raw),
                                    other => format!("{expression}[{}]", self.text(other, scope)),
                                },
                                PropertyKey::Private(_) => continue,
                            };
                            self.extract(paths, inserts, &p.value, member, has_default_value, scope, derived);
                        }
                    }
                }
            }
            Pattern::ArrayPattern(ap) => {
                let id = self.cx.generate("$$array");
                let len = if matches!(ap.elements.last(), Some(Some(Pattern::RestElement(_)))) { String::new() } else { format!(", {}", ap.elements.len()) };
                inserts.push(Insert { id: id.clone(), value: format!("$.to_array({expression}{len})") });
                let read = if derived { format!("$.get({id})") } else { id };
                for (i, el) in ap.elements.iter().enumerate() {
                    let Some(el) = el else { continue };
                    if let Pattern::RestElement(r) = el {
                        let rest = format!("{read}.slice({i})");
                        match r.argument.as_ref() {
                            Pattern::Identifier(id) => paths.push(Path { node: Target::Id(id), expression: rest, is_rest: true, has_default_value }),
                            other => self.extract(paths, inserts, other, rest, has_default_value, scope, derived),
                        }
                    } else {
                        self.extract(paths, inserts, el, format!("{read}[{i}]"), has_default_value, scope, derived);
                    }
                }
            }
            Pattern::AssignmentPattern(ap) => {
                let fallback = self.build_fallback(&expression, &ap.right, scope);
                match ap.left.as_ref() {
                    Pattern::Identifier(id) => paths.push(Path { node: Target::Id(id), expression: fallback, is_rest: false, has_default_value: true }),
                    other => self.extract(paths, inserts, other, fallback, true, scope, derived),
                }
            }
            Pattern::RestElement(r) => self.extract(paths, inserts, &r.argument, expression, has_default_value, scope, derived),
            Pattern::ParenthesizedExpression(_) => {}
            // TypeScript wrappers around a destructuring target (`(x as T) = ...`)
            other => self.cx.error("unsupported_feature", other.span(), "a TypeScript-wrapped destructuring target is not supported by the native compiler yet"),
        }
    }

    /// `build_fallback(expression, fallback)`.
    fn build_fallback(&self, expression: &str, fallback: &Expression, scope: ScopeId) -> String {
        if is_simple_expression(fallback) {
            return format!("$.fallback({expression}, {})", self.text(fallback, scope));
        }
        if let Expression::AwaitExpression(a) = fallback
            && is_simple_expression(&a.argument)
        {
            return format!("(await $.fallback({expression}, {}))", self.text(&a.argument, scope));
        }
        let text = self.text(fallback, scope);
        if has_await(fallback) {
            format!("(await $.fallback({expression}, async () => ({text}), true))")
        } else {
            format!("$.fallback({expression}, () => ({text}), true)")
        }
    }
}

fn literal_string(l: &Literal) -> String {
    match &l.value {
        LiteralValue::String(s) => s.clone(),
        LiteralValue::Number(n) => crate::svelte::scope::js::number_to_string(*n),
        _ => l.raw.clone(),
    }
}
