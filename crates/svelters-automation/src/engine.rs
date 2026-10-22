//! Resolving a [`Selector`] over the scene (E2E-INSPECTOR §1.3): every engine runs on the scene,
//! with the same name function AccessKit gets (`svelters_egui::accessible_name`), so the same
//! selector gives the same `NodeId` to every client (Rust in process, JSON, later IPC).
//!
//! Each part searches below the previous part's matches (the whole scene for the first part), in
//! document order: roots in scene order, then pre-order. `root=` yields the root's top node, and
//! the next part searches the top and its subtree. Roots the session skips (tooling roots, §1.2)
//! are left out.

use crate::Error;
use crate::role::{has_state, role_of};
use crate::selector::{Part, Selector};
use svelters_egui::accessible_name;
use svelters_egui::scene::{NodeId, NodeRef, SceneRef};
use svelters_style::{MatchCaches, parse_selector_list};

/// Which roots a resolution sees: `skip(key)` is `true` for a root key to leave out.
pub struct Roots<'f> {
    pub skip: &'f dyn Fn(&str) -> bool,
}

enum Scope<'a> {
    All,
    /// These nodes and their subtrees (after `root=`).
    Inclusive(Vec<NodeRef<'a>>),
    /// The subtrees below these nodes.
    Below(Vec<NodeRef<'a>>),
    /// Exactly these nodes (after `nth=`; the next filtering part searches below them).
    Exact(Vec<NodeRef<'a>>),
}

/// The nodes `sel` matches, in document order, without duplicates.
pub fn resolve(s: SceneRef<'_>, sel: &Selector, roots: &Roots<'_>) -> Result<Vec<NodeId>, Error> {
    let mut scope = Scope::All;
    let mut caches = MatchCaches::default();
    for part in &sel.parts {
        scope = match part {
            Part::Root(key) => {
                if !matches!(scope, Scope::All) {
                    return Err(Error::new("selector.invalid", "root= must come first"));
                }
                let top = s
                    .root_by_key(key)
                    .filter(|_| !(roots.skip)(key))
                    .and_then(|r| s.root_top(r));
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
                let hit = s.hit_node(egui::ViewportId::ROOT, *p);
                let inside = |n: NodeRef<'_>| {
                    candidates(s, &scope, roots)
                        .iter()
                        .any(|c| c.id() == n.id())
                };
                Scope::Below(
                    hit.filter(|&n| matches!(scope, Scope::All) || inside(n))
                        .into_iter()
                        .collect(),
                )
            }
            Part::Css(css) => {
                let list = parse_selector_list(css, Some(&svelters_egui::style::ProviderStates))
                    .map_err(|e| Error::new("selector.invalid", format!("css `{css}`: {e}")))?;
                let v = candidates(s, &scope, roots)
                    .into_iter()
                    .filter(|&n| svelters_style::matches(&list, n, &mut caches))
                    .collect();
                Scope::Below(v)
            }
            Part::Anchor(a) => {
                let v = candidates(s, &scope, roots)
                    .into_iter()
                    .filter(|n| n.anchor() == Some(a.as_str()))
                    .collect();
                Scope::Below(v)
            }
            Part::Text(t) => {
                let v = candidates(s, &scope, roots)
                    .into_iter()
                    .filter(|n| !n.text().is_empty() && t.matches(n.text()))
                    .collect();
                Scope::Below(v)
            }
            Part::Label(t) => {
                let v = candidates(s, &scope, roots)
                    .into_iter()
                    .filter(|&n| {
                        !matches!(role_of(n), "label" | "generic" | "group")
                            && t.matches(&accessible_name(n))
                    })
                    .collect();
                Scope::Below(v)
            }
            Part::Role { role, name, states } => {
                let v = candidates(s, &scope, roots)
                    .into_iter()
                    .filter(|&n| {
                        role_of(n) == role
                            && name.as_ref().is_none_or(|t| t.matches(&accessible_name(n)))
                            && states.iter().all(|st| has_state(n, st))
                    })
                    .collect();
                Scope::Below(v)
            }
        };
    }
    Ok(matched(&scope).into_iter().map(|n| n.id()).collect())
}

/// The nodes the last part produced.
fn matched<'a>(scope: &Scope<'a>) -> Vec<NodeRef<'a>> {
    match scope {
        Scope::All => Vec::new(),
        Scope::Inclusive(v) | Scope::Below(v) | Scope::Exact(v) => v.clone(),
    }
}

/// The nodes the next part filters, in document order, without duplicates.
fn candidates<'a>(s: SceneRef<'a>, scope: &Scope<'a>, roots: &Roots<'_>) -> Vec<NodeRef<'a>> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut push = |n: NodeRef<'a>, out: &mut Vec<NodeRef<'a>>| {
        if seen.insert(n.id()) {
            out.push(n);
        }
    };
    match scope {
        Scope::All => {
            for (r, _) in s.roots() {
                if r.is_detached() || s.root_key(r).is_some_and(|k| (roots.skip)(k)) {
                    continue;
                }
                if let Some(top) = s.root_top(r) {
                    push(top, &mut out);
                    for d in top.descendants() {
                        push(d, &mut out);
                    }
                }
            }
        }
        Scope::Inclusive(v) => {
            for &t in v {
                push(t, &mut out);
                for d in t.descendants() {
                    push(d, &mut out);
                }
            }
        }
        Scope::Below(v) | Scope::Exact(v) => {
            for &t in v {
                for d in t.descendants() {
                    push(d, &mut out);
                }
            }
        }
    }
    out
}
