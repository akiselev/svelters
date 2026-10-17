//! Class state fields: `phases/2-analyze/visitors/ClassBody.js` (which fields are state, their
//! backing private names, `state_field_duplicate`) — the transform itself is in `visit.rs`
//! (`class_body`, private member reads/writes, constructor assignments).

use super::Ctx;
use crate::estree::*;
use crate::svelte::scope::{ScopeId, get_rune};

pub(crate) const STATE_CREATION_RUNES: &[&str] = &["$state", "$state.raw", "$derived", "$derived.by"];

#[derive(Clone, Debug)]
pub(crate) struct Field {
    /// `get_name(key)`: `#x` for private fields, `x` for public ones.
    pub name: String,
    pub rune: &'static str,
    /// The backing private field name (without `#`): the field itself when private, else the
    /// deconflicted name of the generated backing field.
    pub private: String,
    pub public: bool,
    /// Address of the declaring `PropertyDefinition` or constructor `AssignmentExpression`.
    pub decl: usize,
    pub ctor: bool,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ClassFields {
    pub fields: Vec<Field>,
}

impl ClassFields {
    pub fn get(&self, name: &str) -> Option<&Field> {
        self.fields.iter().find(|f| f.name == name)
    }

    pub fn by_decl(&self, addr: usize) -> Option<&Field> {
        self.fields.iter().find(|f| f.decl == addr)
    }
}

/// `get_name(node)` (nodes.js).
pub(crate) fn get_name(key: &PropertyKey) -> Option<String> {
    match key {
        PropertyKey::Private(PrivateIdentifierNode::PrivateIdentifier(p)) => Some(format!("#{}", p.name)),
        PropertyKey::Expression(e) => match e.as_ref() {
            Expression::Identifier(i) => Some(i.name.clone()),
            Expression::Literal(l) => Some(match &l.value {
                LiteralValue::String(s) => s.clone(),
                LiteralValue::Number(n) => crate::svelte::scope::js::number_to_string(*n),
                _ => l.raw.clone(),
            }),
            _ => None,
        },
    }
}

pub(crate) fn addr<T>(x: &T) -> usize {
    x as *const T as usize
}

/// `ClassBody(node, context)` of the analysis (runes mode).
pub(crate) fn analyze(body: &ClassBody, cx: &Ctx<'_, '_>, scope: ScopeId) -> ClassFields {
    let mut private_ids: Vec<String> = Vec::new();
    for el in &body.body {
        let key = match el {
            ClassElement::MethodDefinition(m) => &m.key,
            ClassElement::PropertyDefinition(p) => &p.key,
            // type-only members (stripped from the output)
            ClassElement::StaticBlock(_) | ClassElement::TSIndexSignature(_) => continue,
        };
        if let PropertyKey::Private(PrivateIdentifierNode::PrivateIdentifier(p)) = key {
            private_ids.push(p.name.clone());
        }
    }
    let mut out = ClassFields::default();
    let handle = |out: &mut ClassFields, span: Span, decl: usize, ctor: bool, key: &PropertyKey, value: Option<&Expression>| {
        let Some(name) = get_name(key) else { return };
        let Some(rune) = get_rune(cx.arena, value, scope) else { return };
        if !STATE_CREATION_RUNES.contains(&rune) {
            return;
        }
        if out.get(&name).is_some() {
            cx.error("state_field_duplicate", span, format!("`{name}` has already been declared on this class"));
            return;
        }
        let public = !name.starts_with('#');
        out.fields.push(Field { private: if public { String::new() } else { name[1..].to_string() }, name, rune, public, decl, ctor });
    };
    let mut constructor = None;
    for el in &body.body {
        match el {
            ClassElement::PropertyDefinition(p) if !p.computed && !p.is_static => handle(&mut out, p.span(), addr(p), false, &p.key, p.value.as_ref()),
            ClassElement::MethodDefinition(m) if m.kind == MethodKind::Constructor => constructor = Some(m),
            _ => {}
        }
    }
    // an overload signature (`TSDeclareMethod`) has no body
    if let Some(c) = constructor
        && let FunctionNode::FunctionExpression(f) = &c.value
    {
        let BlockStatementNode::BlockStatement(fb) = &f.body;
        for stmt in &fb.body {
            let Statement::ExpressionStatement(s) = stmt else { continue };
            let Expression::AssignmentExpression(a) = &s.expression else { continue };
            let a: &AssignmentExpression = a;
            let Pattern::MemberExpression(left) = &a.left else { continue };
            if !matches!(left.object.as_ref(), Expression::ThisExpression(_)) {
                continue;
            }
            if left.computed && !matches!(&left.property, PropertyKey::Expression(e) if matches!(e.as_ref(), Expression::Literal(_))) {
                continue;
            }
            handle(&mut out, a.span(), addr(a), true, &left.property, Some(&a.right));
        }
    }
    for f in out.fields.iter_mut().filter(|f| f.public) {
        let mut deconflicted: String = f.name.chars().enumerate().map(|(i, c)| if c.is_ascii_alphabetic() || c == '_' || c == '$' || (i > 0 && c.is_ascii_digit()) { c } else { '_' }).collect();
        while private_ids.contains(&deconflicted) {
            deconflicted.insert(0, '_');
        }
        private_ids.push(deconflicted.clone());
        f.private = deconflicted;
    }
    out
}
