//! Resolving a [`Selector`] over the scene (E2E-INSPECTOR §1.3): every engine runs on the scene,
//! with the same name function AccessKit gets (`svelters_egui::accessible_name`), so the same
//! selector gives the same `NodeId` to every client (Rust in process, JSON, later IPC).
//!
//! Each part searches below the previous part's matches (the whole scene for the first part), in
//! document order: roots in scene order, then pre-order. `root=` yields the root's top node, and
//! the next part searches the top and its subtree. Roots the session skips (tooling roots, §1.2)
//! are left out.
//!
//! The engine runs on any [`SceneSource`] ([`crate::source::resolve_source`]); [`resolve`] is it on
//! svelters-egui's scene.

use crate::Error;
use crate::selector::{Part, Selector};
use crate::source::SceneSource;
use svelters_egui::scene::{NodeId, SceneRef};

/// Which roots a resolution sees: `skip(key)` is `true` for a root key to leave out.
pub struct Roots<'f> {
    pub skip: &'f dyn Fn(&str) -> bool,
}

enum Scope<N> {
    All,
    /// These nodes and their subtrees (after `root=`).
    Inclusive(Vec<N>),
    /// The subtrees below these nodes.
    Below(Vec<N>),
    /// Exactly these nodes (after `nth=`; the next filtering part searches below them).
    Exact(Vec<N>),
}

/// The nodes `sel` matches, in document order, without duplicates.
pub fn resolve(s: SceneRef<'_>, sel: &Selector, roots: &Roots<'_>) -> Result<Vec<NodeId>, Error> {
    resolve_in(&s, sel, roots.skip)
}

/// [`resolve`] over any [`SceneSource`] (the egui scene is one).
pub(crate) fn resolve_in<S: SceneSource>(s: &S, sel: &Selector, skip: &dyn Fn(&str) -> bool) -> Result<Vec<S::Node>, Error> {
    let mut scope = Scope::All;
    for part in &sel.parts {
        scope = match part {
            Part::Root(key) => {
                if !matches!(scope, Scope::All) {
                    return Err(Error::new("selector.invalid", "root= must come first"));
                }
                let top = s.root_top(key).filter(|_| !skip(key));
                Scope::Inclusive(top.into_iter().collect())
            }
            Part::Nth(k) => {
                let all = matched(&scope);
                let i = if *k < 0 { all.len() as i64 + k } else { *k };
                Scope::Exact(
                    usize::try_from(i)
                        .ok()
                        .and_then(|i| all.get(i).copied())
                        .into_iter()
                        .collect(),
                )
            }
            Part::Deferred(engine) => {
                return Err(Error::new(
                    "capability.absent",
                    format!(
                        "the `{engine}=` engine lands with AU3 (compiler identity) or AU2 (the AccessKit tree)"
                    ),
                ));
            }
            Part::Pos(p) => {
                let hit = s.hit(*p);
                let inside = |n: S::Node| candidates(s, &scope, skip).contains(&n);
                Scope::Below(
                    hit.filter(|&n| matches!(scope, Scope::All) || inside(n))
                        .into_iter()
                        .collect(),
                )
            }
            Part::Css(css) => Scope::Below(s.select_css(css, &candidates(s, &scope, skip))?),
            Part::Key(k) => {
                // the item's top nodes (their own `anchor-scope` ends with the key): the next part
                // searches them and their subtrees
                let v = candidates(s, &scope, skip)
                    .into_iter()
                    .filter(|&n| s.item_key(n).as_deref() == Some(k.as_str()))
                    .collect();
                Scope::Inclusive(v)
            }
            Part::Source { file, offset } => {
                let v = candidates(s, &scope, skip)
                    .into_iter()
                    .filter(|&n| s.declared_in(n, file, *offset))
                    .collect();
                Scope::Below(v)
            }
            Part::Anchor(a) => {
                let v = candidates(s, &scope, skip)
                    .into_iter()
                    .filter(|&n| s.anchor(n).as_deref() == Some(a.as_str()))
                    .collect();
                Scope::Below(v)
            }
            Part::Text(t) => {
                let v = candidates(s, &scope, skip)
                    .into_iter()
                    .filter(|&n| {
                        let text = s.text(n);
                        !text.is_empty() && t.matches(&text)
                    })
                    .collect();
                Scope::Below(v)
            }
            Part::Label(t) => {
                let v = candidates(s, &scope, skip)
                    .into_iter()
                    .filter(|&n| !matches!(s.role(n), "label" | "generic" | "group") && t.matches(&s.name(n)))
                    .collect();
                Scope::Below(v)
            }
            Part::Role { role, name, states } => {
                let v = candidates(s, &scope, skip)
                    .into_iter()
                    .filter(|&n| {
                        s.role(n) == role
                            && name.as_ref().is_none_or(|t| t.matches(&s.name(n)))
                            && states.iter().all(|st| s.has_state(n, st))
                    })
                    .collect();
                Scope::Below(v)
            }
        };
    }
    Ok(matched(&scope))
}

/// The nodes the last part produced.
fn matched<N: Copy>(scope: &Scope<N>) -> Vec<N> {
    match scope {
        Scope::All => Vec::new(),
        Scope::Inclusive(v) | Scope::Below(v) | Scope::Exact(v) => v.clone(),
    }
}

/// The nodes the next part filters, in document order, without duplicates.
fn candidates<S: SceneSource>(s: &S, scope: &Scope<S::Node>, skip: &dyn Fn(&str) -> bool) -> Vec<S::Node> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut push = |n: S::Node, out: &mut Vec<S::Node>| {
        if seen.insert(n) {
            out.push(n);
        }
    };
    match scope {
        Scope::All => {
            for r in s.roots() {
                if r.key.as_deref().is_some_and(skip) {
                    continue;
                }
                push(r.top, &mut out);
                for d in s.descendants(r.top) {
                    push(d, &mut out);
                }
            }
        }
        Scope::Inclusive(v) => {
            for &t in v {
                push(t, &mut out);
                for d in s.descendants(t) {
                    push(d, &mut out);
                }
            }
        }
        Scope::Below(v) | Scope::Exact(v) => {
            for &t in v {
                for d in s.descendants(t) {
                    push(d, &mut out);
                }
            }
        }
    }
    out
}
