//! Port of the helpers of `svelte/src/compiler/utils/ast.js` (5.57.2 @ f3de3f2) that scope
//! creation and the ported analysis slices use: `object`, `unwrap_pattern`, `extract_identifiers`,
//! `extract_identifiers_from_destructuring`, `extract_paths`.

use crate::estree::*;

/// A leaf of a pattern: what `unwrap_pattern` collects.
#[derive(Clone, Copy, Debug)]
pub enum PatternTarget<'a> {
    Identifier(&'a Identifier),
    MemberExpression(&'a MemberExpression),
}

/// Gets the left-most identifier of a member expression or identifier.
pub fn object(expression: PatternTarget<'_>) -> Option<&Identifier> {
    let mut expression: &Expression = match expression {
        PatternTarget::Identifier(i) => return Some(i),
        PatternTarget::MemberExpression(m) => &m.object,
    };

    while let Expression::MemberExpression(m) = expression {
        expression = &m.object;
    }

    match expression {
        Expression::Identifier(i) => Some(i),
        _ => None,
    }
}

/// `unwrap_pattern(pattern, nodes)`.
pub fn unwrap_pattern<'a>(pattern: &'a Pattern, nodes: &mut Vec<PatternTarget<'a>>) {
    match pattern {
        Pattern::Identifier(i) => nodes.push(PatternTarget::Identifier(i)),

        // member expressions can be part of an assignment pattern, but not a binding pattern
        // see https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Operators/Destructuring_assignment#binding_and_assignment
        Pattern::MemberExpression(m) => nodes.push(PatternTarget::MemberExpression(m)),

        Pattern::ObjectPattern(o) => {
            for prop in &o.properties {
                match prop {
                    ObjectPatternMember::RestElement(r) => unwrap_pattern(&r.argument, nodes),
                    ObjectPatternMember::Property(p) => unwrap_pattern(&p.value, nodes),
                }
            }
        }

        Pattern::ArrayPattern(a) => {
            for element in a.elements.iter().flatten() {
                unwrap_pattern(element, nodes);
            }
        }

        Pattern::RestElement(r) => unwrap_pattern(&r.argument, nodes),

        Pattern::AssignmentPattern(a) => unwrap_pattern(&a.left, nodes),

        Pattern::ParenthesizedExpression(_) => {}

        // SVELTERS: TypeScript nodes are removed (`remove_typescript_nodes`) before analysis.
        Pattern::TSParameterProperty(p) => unwrap_pattern(&p.parameter, nodes),
        Pattern::TSAsExpression(w)
        | Pattern::TSSatisfiesExpression(w)
        | Pattern::TSTypeAssertion(w)
        | Pattern::TSNonNullExpression(w) => unwrap_pattern(&w.expression, nodes),
    }
}

/// `unwrap_pattern` applied to an expression in pattern position (UpdateExpression argument,
/// `bind:` expression): only `Identifier` and `MemberExpression` match.
pub fn unwrap_expression_pattern(expression: &Expression) -> Vec<PatternTarget<'_>> {
    match expression {
        Expression::Identifier(i) => vec![PatternTarget::Identifier(i)],
        Expression::MemberExpression(m) => vec![PatternTarget::MemberExpression(m)],
        _ => Vec::new(),
    }
}

/// `extract_identifiers(pattern)`.
pub fn extract_identifiers(pattern: &Pattern) -> Vec<&Identifier> {
    let mut nodes = Vec::new();
    unwrap_pattern(pattern, &mut nodes);
    nodes
        .into_iter()
        .filter_map(|n| match n {
            PatternTarget::Identifier(i) => Some(i),
            PatternTarget::MemberExpression(_) => None,
        })
        .collect()
}

/// `extract_identifiers_from_destructuring(node, nodes)`.
pub fn extract_identifiers_from_destructuring<'a>(node: &'a Expression, nodes: &mut Vec<&'a Identifier>) {
    // TODO This isn't complete, but it should be enough for our purposes
    match node {
        Expression::Identifier(i) => nodes.push(i),

        Expression::ObjectExpression(o) => {
            for prop in &o.properties {
                match prop {
                    ObjectMember::Property(p) => extract_identifiers_from_destructuring(&p.value, nodes),
                    ObjectMember::SpreadElement(s) => extract_identifiers_from_destructuring(&s.argument, nodes),
                }
            }
        }

        Expression::ArrayExpression(a) => {
            for element in a.elements.iter().flatten() {
                // a SpreadElement element has no case upstream
                if let ArrayElement::Expression(e) = element {
                    extract_identifiers_from_destructuring(e, nodes);
                }
            }
        }

        _ => {}
    }
}

/// One entry of `extract_paths(...).paths`.
///
/// SVELTERS: only the fields scope/analysis need (`node`, `is_rest`, `has_default_value`); the
/// `expression` / `update_expression` builders and the `inserts` belong to the transform phase.
#[derive(Clone, Copy, Debug)]
pub struct DestructuredAssignment<'a> {
    /// The node the destructuring path end in. Can be a member expression only for assignment expressions
    pub node: PatternTarget<'a>,
    /// `true` if this is a `...rest` destructuring
    pub is_rest: bool,
    /// `true` if this has a fallback value like `const { foo = 'bar } = ..`
    pub has_default_value: bool,
}

/// `extract_paths(param, initial).paths` (see [`DestructuredAssignment`]).
pub fn extract_paths(param: &Pattern) -> Vec<DestructuredAssignment<'_>> {
    let mut paths = Vec::new();
    _extract_paths(&mut paths, param, false);
    paths
}

fn _extract_paths<'a>(paths: &mut Vec<DestructuredAssignment<'a>>, param: &'a Pattern, has_default_value: bool) {
    match param {
        Pattern::Identifier(i) => paths.push(DestructuredAssignment {
            node: PatternTarget::Identifier(i),
            is_rest: false,
            has_default_value,
        }),
        Pattern::MemberExpression(m) => paths.push(DestructuredAssignment {
            node: PatternTarget::MemberExpression(m),
            is_rest: false,
            has_default_value,
        }),

        Pattern::ObjectPattern(o) => {
            for prop in &o.properties {
                match prop {
                    ObjectPatternMember::RestElement(r) => {
                        if let Pattern::Identifier(i) = &*r.argument {
                            paths.push(DestructuredAssignment {
                                node: PatternTarget::Identifier(i),
                                is_rest: true,
                                has_default_value,
                            });
                        } else {
                            _extract_paths(paths, &r.argument, has_default_value);
                        }
                    }
                    ObjectPatternMember::Property(p) => _extract_paths(paths, &p.value, has_default_value),
                }
            }
        }

        Pattern::ArrayPattern(a) => {
            for element in a.elements.iter().flatten() {
                if let Pattern::RestElement(r) = element {
                    if let Pattern::Identifier(i) = &*r.argument {
                        paths.push(DestructuredAssignment {
                            node: PatternTarget::Identifier(i),
                            is_rest: true,
                            has_default_value,
                        });
                    } else {
                        _extract_paths(paths, &r.argument, has_default_value);
                    }
                } else {
                    _extract_paths(paths, element, has_default_value);
                }
            }
        }

        Pattern::AssignmentPattern(a) => {
            if let Pattern::Identifier(i) = &*a.left {
                paths.push(DestructuredAssignment {
                    node: PatternTarget::Identifier(i),
                    is_rest: false,
                    has_default_value: true,
                });
            } else {
                _extract_paths(paths, &a.left, true);
            }
        }

        // no case upstream (a top-level RestElement only occurs as a function param)
        Pattern::RestElement(_) | Pattern::ParenthesizedExpression(_) => {}

        // SVELTERS: TypeScript nodes are removed (`remove_typescript_nodes`) before analysis.
        Pattern::TSParameterProperty(p) => _extract_paths(paths, &p.parameter, has_default_value),
        Pattern::TSAsExpression(w)
        | Pattern::TSSatisfiesExpression(w)
        | Pattern::TSTypeAssertion(w)
        | Pattern::TSNonNullExpression(w) => _extract_paths(paths, &w.expression, has_default_value),
    }
}
