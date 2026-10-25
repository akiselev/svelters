//! W-C2: native templates, the lowering side (presto `compiler-and-js.md` §3.2). For a backend
//! with the `templates` capability, every element subtree whose structure and static writes are
//! known here becomes an [`Item::Template`]; codegen writes it as a module-scope table and one
//! `$n.instantiate` (`codegen::component::template`).
//!
//! A subtree is a template when each of its elements
//! - is placed in a slot (not a toplevel, not attached to its parent);
//! - has only ops the table holds or that act on the node after it is built: static and one-off
//!   writes, reactive writes, listeners, bindings, transitions; no spread, no actions, attachments
//!   or `bind:this` (they run between the children and the insertion), no style-rule ops (they
//!   capture the native value first);
//! - has children that are elements of a template, or holes: blocks, components, render tags and
//!   `<svelte:element>`s, which the instance builds in place through a function;
//! - declares no snippet, `{@const}` or `{@debug}` among its children.
//!
//! A single element without static writes stays as it is (the table would save nothing). Elements
//! that are not templates keep today's ops; their children are considered again.

use super::ir::*;

/// Wraps the template subtrees of `f` (and of every fragment inside it) into `Item::Template`s,
/// numbered from `next`.
pub fn wrap(f: &mut Fragment, next: &mut usize) {
    for s in &mut f.snippets {
        wrap(&mut s.body, next);
    }
    let items = std::mem::take(&mut f.items);
    for item in items {
        let item = match item {
            Item::Element(e) if templatable(&e) && worth(&e) => {
                let mut root = e;
                holes(&mut root, next);
                let index = *next;
                *next += 1;
                Item::Template(Template { index, root })
            }
            mut other => {
                inner(&mut other, next);
                other
            }
        };
        f.items.push(item);
    }
}

/// The fragments inside an item that is not a template.
fn inner(item: &mut Item, next: &mut usize) {
    match item {
        Item::Element(e) => wrap(&mut e.children, next),
        Item::If(b) => {
            wrap(&mut b.consequent, next);
            if let Some(a) = &mut b.alternate {
                wrap(a, next);
            }
        }
        Item::Each(b) => {
            wrap(&mut b.body, next);
            if let Some(f) = &mut b.fallback {
                wrap(f, next);
            }
        }
        Item::Key(b) => wrap(&mut b.body, next),
        Item::Await(b) => {
            for f in [&mut b.pending, &mut b.then, &mut b.catch].into_iter().flatten() {
                wrap(f, next);
            }
        }
        Item::Component(c) => {
            for s in &mut c.snippets {
                wrap(&mut s.body, next);
            }
            if let Some(f) = &mut c.children {
                wrap(f, next);
            }
        }
        Item::Dynamic(d) => wrap(&mut d.children, next),
        Item::Render(_) | Item::Template(_) => {}
    }
}

/// The holes of a template subtree hold fragments of their own, which may hold templates.
fn holes(e: &mut Element, next: &mut usize) {
    for item in &mut e.children.items {
        match item {
            Item::Element(c) => holes(c, next),
            hole => inner(hole, next),
        }
    }
}

fn templatable(e: &Element) -> bool {
    !e.toplevel
        && !e.attached
        && e.children.snippets.is_empty()
        && e.children.consts.is_empty()
        && e.children.debugs.is_empty()
        && e.ops.iter().all(allowed)
        && e.children.items.iter().all(|item| match item {
            Item::Element(c) => templatable(c),
            Item::Template(_) => false,
            _ => true,
        })
}

fn allowed(op: &Op) -> bool {
    match op {
        Op::Set { .. }
        | Op::SetDynamic { .. }
        | Op::Listen { .. }
        | Op::Bind { .. }
        | Op::BindProp { .. }
        | Op::BindReadonly { .. }
        | Op::BindOnce { .. }
        | Op::BindCommit { .. }
        | Op::BindLink { .. }
        | Op::Transition { .. }
        | Op::Animate { .. }
        | Op::Classes { .. }
        | Op::WidgetName { .. }
        | Op::InlineStyle { .. }
        | Op::ChildAttr { .. }
        | Op::Token { .. }
        | Op::Track { .. } => true,
        Op::ListenAttach { .. }
        | Op::Attach { .. }
        | Op::Action { .. }
        | Op::BindThis { .. }
        | Op::StyleProp { .. }
        | Op::StyleSetter { .. }
        | Op::Explicit { .. }
        | Op::Spread { .. }
        | Op::Text { .. } => false,
    }
}

/// More than one element, or one with a static write.
fn worth(e: &Element) -> bool {
    e.children.items.iter().any(|i| matches!(i, Item::Element(_)))
        || e.ops.iter().any(|op| match op {
            Op::Set { .. } | Op::ChildAttr { value: AttrValue::Static(_), .. } | Op::WidgetName { value: AttrValue::Static(_), .. } | Op::Track { .. } => true,
            Op::Classes { dynamic: None, toggles, .. } => toggles.is_empty(),
            Op::InlineStyle { dynamic: None, directives, .. } => directives.is_empty(),
            Op::Token { cond, .. } => cond.is_empty(),
            _ => false,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::estree::Span;
    use serde_json::json;

    fn el<'a>(id: usize, kind: &str, ops: Vec<Op<'a>>, items: Vec<Item<'a>>) -> Element<'a> {
        let span = Span::new(0, 0);
        Element {
            id,
            span,
            kind: kind.into(),
            present: None,
            toplevel: false,
            show: None,
            attached: false,
            slot: None,
            class_expr: None,
            id_expr: None,
            ops,
            children: Fragment { items, ..Fragment::default() },
        }
    }

    fn set<'a>(prop: &str) -> Op<'a> {
        Op::Set { prop: prop.into(), value: json!("x"), span: Span::new(0, 0) }
    }

    #[test]
    fn wraps_static_subtrees_and_numbers_them() {
        let mut f = Fragment::default();
        f.items.push(Item::Element(el(0, "box", vec![], vec![Item::Element(el(1, "label", vec![set("text")], vec![]))])));
        // a lone element without static writes stays
        f.items.push(Item::Element(el(2, "label", vec![], vec![])));
        let mut next = 0;
        wrap(&mut f, &mut next);
        assert!(matches!(&f.items[0], Item::Template(t) if t.index == 0 && t.root.kind == "box"));
        assert!(matches!(&f.items[1], Item::Element(_)));
        assert_eq!(next, 1);
    }

    #[test]
    fn an_action_keeps_the_element_and_its_children_are_considered() {
        let mut f = Fragment::default();
        let action = Op::Action { name: "a".into(), arg: None, span: Span::new(0, 0) };
        let child = el(1, "box", vec![], vec![Item::Element(el(2, "label", vec![set("text")], vec![]))]);
        f.items.push(Item::Element(el(0, "box", vec![action], vec![Item::Element(child)])));
        let mut next = 0;
        wrap(&mut f, &mut next);
        let Item::Element(outer) = &f.items[0] else { panic!("the element with an action stays") };
        assert!(matches!(&outer.children.items[0], Item::Template(t) if t.root.kind == "box"));
    }

    #[test]
    fn toplevels_are_not_templates() {
        let mut f = Fragment::default();
        let mut w = el(0, "window", vec![set("title")], vec![]);
        w.toplevel = true;
        f.items.push(Item::Element(w));
        let mut next = 0;
        wrap(&mut f, &mut next);
        assert!(matches!(&f.items[0], Item::Element(_)));
    }
}
