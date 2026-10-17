//! Port of the `is-reference` package 3.0.3 (`src/index.js`), which `phases/scope.js` uses to
//! decide whether an Identifier is a reference. `node === parent.x` identity checks compare the
//! node's address with the parent's field (see `walk::NodeKey`).

use super::walk::{NodeRef, expr_ref, module_export_name_ref, pattern_ref};

fn same(a: NodeRef<'_>, b: NodeRef<'_>) -> bool {
    a.key() == b.key()
}

pub fn is_reference(node: NodeRef<'_>, parent: Option<NodeRef<'_>>) -> bool {
    if let NodeRef::MemberExpression(m) = node {
        return !m.computed && is_reference(expr_ref(&m.object), Some(node));
    }

    if !matches!(node, NodeRef::Identifier(_)) {
        return false;
    }

    match parent {
        // disregard `bar` in `foo.bar`
        Some(NodeRef::MemberExpression(p)) => p.computed || same(node, expr_ref(&p.object)),

        // disregard the `foo` in `class {foo(){}}` but keep it in `class {[foo](){}}`
        Some(NodeRef::MethodDefinition(p)) => p.computed,

        // disregard the `meta` in `import.meta`
        Some(NodeRef::MetaProperty(p)) => same(node, NodeRef::Identifier(&p.meta)),

        // disregard the `foo` in `class {foo=bar}` but keep it in `class {[foo]=bar}` and `class {bar=foo}`
        Some(NodeRef::PropertyDefinition(p)) => p.computed || p.value.as_ref().is_some_and(|v| same(node, expr_ref(v))),

        // disregard the `bar` in `{ bar: foo }`, but keep it in `{ [bar]: foo }`
        Some(NodeRef::Property(p)) => p.computed || same(node, expr_ref(&p.value)),
        Some(NodeRef::PatternProperty(p)) => p.computed || same(node, pattern_ref(&p.value)),

        // disregard the `bar` in `export { foo as bar }` or
        // the foo in `import { foo as bar }`
        Some(NodeRef::ExportSpecifier(p)) => same(node, module_export_name_ref(&p.local)),
        Some(NodeRef::ImportSpecifier(p)) => same(node, NodeRef::Identifier(&p.local)),

        // disregard the `foo` in `foo: while (...) { ... break foo; ... continue foo;}`
        Some(NodeRef::LabeledStatement(_) | NodeRef::BreakStatement(_) | NodeRef::ContinueStatement(_)) => false,

        _ => true,
    }
}
