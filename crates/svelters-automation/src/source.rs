//! [`SceneSource`]: the scene the selector engine ([`crate::engine`]) and the text snapshot read,
//! behind a trait (TEST-V2 Q2), so the same selectors and snapshots work on UI that is not
//! svelters-egui's retained scene: skelter-native UI (`skelter_test::SkelterScene` reads a skelter
//! document through its AccessKit projection), or any tree with roles, names and rects.
//!
//! svelters-egui's [`SceneRef`] implements it (node ids are the scene's [`NodeId`]s); the engine's
//! [`resolve`](crate::engine::resolve) on the egui scene is [`resolve_source`] on that impl, so both
//! give the same matches by construction.
//!
//! [`query`], [`snapshot`] and [`actionable_point`] are the source-generic reads and the synthetic
//! act's target point. The [`Session`](crate::Session) (acts, waits, journal, JSON methods) runs on
//! any source through a [`SourceDriver`](crate::SourceDriver) (every svelters-egui
//! [`Driver`](crate::Driver) is one); the methods below `select_css` are what it reads besides the
//! selectors' (ancestry for the hit check, the drawn rects, and the egui scene's tag, root, source
//! location and style).

use crate::Error;
use crate::selector::{Selector, parse};
use crate::session::SourceInfo;
use std::borrow::Cow;
use std::hash::Hash;
use svelters_egui::accessible_name;
use svelters_egui::scene::{NodeId, SceneRef};
use svelters_egui::style::MatchedRuleInfo;

/// Where a node was drawn ([`SceneSource::drawn`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drawn {
    /// Its rect on screen, as drawn.
    pub rect: egui::Rect,
    /// The part of `rect` in view (inside its clips).
    pub visible: egui::Rect,
}

/// A node's computed style ([`SceneSource::computed_style`]; `style.computed`).
#[derive(Clone, Debug, PartialEq)]
pub struct ComputedStyle<N> {
    /// Every set property with its computed value as CSS text.
    pub values: Vec<(String, String)>,
    /// Custom properties (`--name`) with their computed text.
    pub custom: Vec<(String, String)>,
    /// The inherited properties the node doesn't declare, each with the nearest ancestor that
    /// declares it (`None`: none does).
    pub inherited: Vec<(String, Option<N>)>,
    pub rules: Vec<MatchedRuleInfo>,
}

/// A root of a [`SceneSource`], in scene order.
#[derive(Clone, Debug, PartialEq)]
pub struct SourceRoot<N> {
    /// The key `root=KEY` names (`None`: the root has none).
    pub key: Option<String>,
    pub top: N,
    /// Shown; hidden roots are left out of a whole-scene snapshot.
    pub visible: bool,
}

/// A tree the selector engine and the snapshot can read. Roles are lower-case WAI-ARIA names
/// (`button`, `textbox`, `generic`), states the scene's state names (`checked`, `disabled`,
/// `focused`, …), rects window points (egui points for svelters-egui).
pub trait SceneSource {
    /// A node's identity, stable while the scene doesn't change.
    type Node: Copy + Eq + Hash;

    /// The roots in scene order, detached ones left out.
    fn roots(&self) -> Vec<SourceRoot<Self::Node>>;
    /// The top node of the root keyed `key` (`root=KEY`).
    fn root_top(&self, key: &str) -> Option<Self::Node> {
        self.roots().into_iter().find(|r| r.key.as_deref() == Some(key)).map(|r| r.top)
    }
    fn children(&self, n: Self::Node) -> Vec<Self::Node>;
    /// `n`'s subtree below it, in pre-order.
    fn descendants(&self, n: Self::Node) -> Vec<Self::Node> {
        let mut out = Vec::new();
        let mut stack: Vec<Self::Node> = self.children(n).into_iter().rev().collect();
        while let Some(c) = stack.pop() {
            out.push(c);
            stack.extend(self.children(c).into_iter().rev());
        }
        out
    }
    /// The lower-case ARIA role (`generic` when it has none).
    fn role(&self, n: Self::Node) -> &'static str;
    /// The accessible name (what AccessKit gets as the label).
    fn name(&self, n: Self::Node) -> Cow<'_, str>;
    /// The node's own text (`text=`); empty when it has none.
    fn text(&self, n: Self::Node) -> Cow<'_, str>;
    /// The value a snapshot shows after the name (`: value`), for value-bearing roles.
    fn value(&self, n: Self::Node) -> Option<String> {
        let _ = n;
        None
    }
    /// The state names `n` holds.
    fn states(&self, n: Self::Node) -> Vec<&'static str>;
    /// Whether `n` has state `name` (`focus` = `focused`, `enabled` = not `disabled`, `visible` =
    /// shown).
    fn has_state(&self, n: Self::Node, name: &str) -> bool {
        let states = self.states(n);
        match name {
            "focus" => states.contains(&"focused"),
            "enabled" => !states.contains(&"disabled"),
            "visible" => self.is_shown(n),
            _ => states.contains(&name),
        }
    }
    /// Laid out and not hidden by the node itself (a snapshot leaves out invisible subtrees).
    fn is_visible(&self, n: Self::Node) -> bool;
    /// On screen: visible, inside its clips and ancestors (`[visible]`, actionability).
    fn is_shown(&self, n: Self::Node) -> bool {
        self.is_visible(n)
    }
    fn rect(&self, n: Self::Node) -> Option<egui::Rect>;
    /// The node's anchor (`anchor=`).
    fn anchor(&self, n: Self::Node) -> Option<Cow<'_, str>> {
        let _ = n;
        None
    }
    /// The key of the keyed `{#each}` item whose top node `n` is (`key=`); `None` for other nodes.
    fn item_key(&self, n: Self::Node) -> Option<Cow<'_, str>> {
        let _ = n;
        None
    }
    /// `source=FILE#OFFSET`: `n` was declared in `file` (a path suffix at a `/`), at `offset`.
    fn declared_in(&self, n: Self::Node, file: &str, offset: Option<u32>) -> bool {
        let _ = (n, file, offset);
        false
    }
    /// The topmost node at `pos` (`pos=`).
    fn hit(&self, pos: egui::Pos2) -> Option<Self::Node> {
        let _ = pos;
        None
    }
    /// The nodes of `among` that match the CSS selector list `css`, in their order.
    fn select_css(&self, css: &str, among: &[Self::Node]) -> Result<Vec<Self::Node>, Error> {
        let _ = among;
        Err(Error::new("capability.absent", format!("this scene source has no CSS matcher (css `{css}`)")))
    }

    // ---- what the session reads besides the selectors ----

    /// `n`'s parent (`None`: a root's top, or gone). The default searches the roots.
    fn parent(&self, n: Self::Node) -> Option<Self::Node> {
        let mut stack: Vec<Self::Node> = self.roots().into_iter().map(|r| r.top).collect();
        while let Some(p) = stack.pop() {
            let kids = self.children(p);
            if kids.contains(&n) {
                return Some(p);
            }
            stack.extend(kids);
        }
        None
    }
    /// Whether `n` is in the scene (a ref to a node that is gone is stale). The default searches
    /// the roots.
    fn contains(&self, n: Self::Node) -> bool {
        self.roots().into_iter().any(|r| r.top == n || self.descendants(r.top).contains(&n))
    }
    /// The element name `scene.query` reports as `tag` (the role when the scene has none).
    fn tag(&self, n: Self::Node) -> Cow<'_, str> {
        Cow::Borrowed(self.role(n))
    }
    /// The key of the root `n` is in (`None`: unkeyed).
    fn root_key_of(&self, n: Self::Node) -> Option<Cow<'_, str>> {
        let _ = n;
        None
    }
    /// Where `n` was drawn in the last frame: its rect and the part of it in view (`None`: not
    /// drawn). The default is [`SceneSource::rect`], wholly in view, for a visible node.
    fn drawn(&self, n: Self::Node) -> Option<Drawn> {
        self.rect(n).filter(|_| self.is_visible(n)).map(|rect| Drawn { rect, visible: rect })
    }
    /// The rect a screenshot of `n` crops to.
    fn screen_rect(&self, n: Self::Node) -> Option<egui::Rect> {
        self.drawn(n).map(|d| d.rect)
    }
    /// DZ0: where a dev build declared `n`.
    fn source_info(&self, n: Self::Node) -> Option<SourceInfo> {
        let _ = n;
        None
    }
    /// DZ0b2: the rules that match `n`, in cascade order (`None`: this scene has no style engine).
    fn matched_rules(&self, n: Self::Node) -> Option<Vec<MatchedRuleInfo>> {
        let _ = n;
        None
    }
    /// DZ0b2: `n`'s computed style (`None`: this scene has no style engine).
    fn computed_style(&self, n: Self::Node) -> Option<ComputedStyle<Self::Node>> {
        let _ = n;
        None
    }
}

/// [`crate::engine::resolve`] over any [`SceneSource`]: the nodes `sel` matches, in document order,
/// without duplicates; roots whose key `skip` names are left out.
pub fn resolve_source<S: SceneSource>(s: &S, sel: &Selector, skip: &dyn Fn(&str) -> bool) -> Result<Vec<S::Node>, Error> {
    crate::engine::resolve_in(s, sel, skip)
}

/// The nodes the selector string `selector` matches (every root).
pub fn query<S: SceneSource>(s: &S, selector: &str) -> Result<Vec<S::Node>, Error> {
    resolve_source(s, &parse(selector)?, &|_| false)
}

/// The text snapshot (`scene.snapshot`'s YAML) of the nodes `scope` matches, or of every visible
/// root.
pub fn snapshot<S: SceneSource>(s: &S, scope: Option<&str>) -> Result<String, Error> {
    let mut out = String::new();
    match scope {
        Some(sel) => {
            for n in query(s, sel)? {
                crate::snapshot::yaml_in(s, n, 0, &mut out);
            }
        }
        None => {
            for r in s.roots().into_iter().filter(|r| r.visible) {
                crate::snapshot::yaml_in(s, r.top, 0, &mut out);
            }
        }
    }
    Ok(out)
}

/// Where a synthetic act on `n` points (its rect's centre), after the actionability checks that need
/// no frame: shown, enabled, and with a non-empty rect. Errors are `node.not_actionable`.
pub fn actionable_point<S: SceneSource>(s: &S, n: S::Node) -> Result<egui::Pos2, Error> {
    if !s.is_shown(n) {
        return Err(Error::new("node.not_actionable", format!("a {} that is not visible", s.role(n))));
    }
    if !s.has_state(n, "enabled") {
        return Err(Error::new("node.not_actionable", format!("a {} that is disabled", s.role(n))));
    }
    match s.rect(n) {
        Some(r) if r.is_positive() => Ok(r.center()),
        _ => Err(Error::new("node.not_actionable", format!("a {} with no area", s.role(n)))),
    }
}

/// svelters-egui's retained scene.
impl<'a> SceneSource for SceneRef<'a> {
    type Node = NodeId;

    fn roots(&self) -> Vec<SourceRoot<NodeId>> {
        let s = *self;
        s.roots()
            .filter(|(r, _)| !r.is_detached())
            .filter_map(|(r, _)| {
                Some(SourceRoot { key: s.root_key(r).map(str::to_string), top: s.root_top(r)?.id(), visible: s.root_visible(r) })
            })
            .collect()
    }
    fn root_top(&self, key: &str) -> Option<NodeId> {
        let s = *self;
        s.root_by_key(key).and_then(|r| s.root_top(r)).map(|n| n.id())
    }
    fn children(&self, n: NodeId) -> Vec<NodeId> {
        self.get(n).map_or_else(Vec::new, |n| n.children().map(|c| c.id()).collect())
    }
    fn descendants(&self, n: NodeId) -> Vec<NodeId> {
        self.get(n).map_or_else(Vec::new, |n| n.descendants().map(|c| c.id()).collect())
    }
    fn role(&self, n: NodeId) -> &'static str {
        self.get(n).map_or("generic", crate::role::role_of)
    }
    fn name(&self, n: NodeId) -> Cow<'_, str> {
        self.get(n).map_or(Cow::Borrowed(""), |n| Cow::Owned(accessible_name(n)))
    }
    fn text(&self, n: NodeId) -> Cow<'_, str> {
        Cow::Borrowed(self.get(n).map_or("", |n| n.text()))
    }
    fn value(&self, n: NodeId) -> Option<String> {
        self.get(n).and_then(crate::snapshot::value_of)
    }
    fn states(&self, n: NodeId) -> Vec<&'static str> {
        self.get(n).map_or_else(Vec::new, crate::role::states_of)
    }
    fn has_state(&self, n: NodeId, name: &str) -> bool {
        self.get(n).is_some_and(|n| crate::role::has_state(n, name))
    }
    fn is_visible(&self, n: NodeId) -> bool {
        self.get(n).is_some_and(|n| n.is_visible())
    }
    fn is_shown(&self, n: NodeId) -> bool {
        self.get(n).is_some_and(|n| n.is_shown())
    }
    fn rect(&self, n: NodeId) -> Option<egui::Rect> {
        self.get(n).map(|n| n.rect())
    }
    fn anchor(&self, n: NodeId) -> Option<Cow<'_, str>> {
        self.get(n)?.anchor().map(Cow::Borrowed)
    }
    fn item_key(&self, n: NodeId) -> Option<Cow<'_, str>> {
        let n = self.get(n)?;
        n.anchor_scope()?;
        n.item_key().map(Cow::Borrowed)
    }
    fn declared_in(&self, n: NodeId, file: &str, offset: Option<u32>) -> bool {
        self.get(n).and_then(|n| n.source_loc()).is_some_and(|l| {
            let e = l.element();
            crate::selector::path_matches(e.file, file) && offset.is_none_or(|o| o == e.offset)
        })
    }
    fn hit(&self, pos: egui::Pos2) -> Option<NodeId> {
        self.hit_node(egui::ViewportId::ROOT, pos).map(|n| n.id())
    }
    fn select_css(&self, css: &str, among: &[NodeId]) -> Result<Vec<NodeId>, Error> {
        let list = svelters_style::parse_selector_list(css, Some(&svelters_egui::style::ProviderStates))
            .map_err(|e| Error::new("selector.invalid", format!("css `{css}`: {e}")))?;
        let mut caches = svelters_style::MatchCaches::default();
        Ok(among
            .iter()
            .copied()
            .filter(|&id| self.get(id).is_some_and(|n| svelters_style::matches(&list, n, &mut caches)))
            .collect())
    }
    fn parent(&self, n: NodeId) -> Option<NodeId> {
        self.get(n)?.parent().map(|p| p.id())
    }
    fn contains(&self, n: NodeId) -> bool {
        self.get(n).is_some()
    }
    fn tag(&self, n: NodeId) -> Cow<'_, str> {
        Cow::Borrowed(self.get(n).map_or("", |n| n.tag()))
    }
    fn root_key_of(&self, n: NodeId) -> Option<Cow<'_, str>> {
        let s = *self;
        s.root_key(s.get(n)?.root()).map(Cow::Borrowed)
    }
    fn drawn(&self, n: NodeId) -> Option<Drawn> {
        self.get(n)?.shown().map(|sh| Drawn { rect: sh.rect, visible: sh.visible })
    }
    fn screen_rect(&self, n: NodeId) -> Option<egui::Rect> {
        self.get(n)?.screen_rect().map(|(_, r)| r)
    }
    fn source_info(&self, n: NodeId) -> Option<SourceInfo> {
        self.get(n)?.source_loc().map(|l| SourceInfo {
            file: l.element().file.to_string(),
            offset: l.element().offset,
            sites: l.sites().map(|x| (x.file.to_string(), x.offset)).collect(),
        })
    }
    fn matched_rules(&self, n: NodeId) -> Option<Vec<MatchedRuleInfo>> {
        self.get(n).map(|n| n.matched_rules())
    }
    fn computed_style(&self, n: NodeId) -> Option<ComputedStyle<NodeId>> {
        let n = self.get(n)?;
        let c = n.computed_style();
        Some(ComputedStyle {
            values: c.as_deref().map(|c| c.iter().map(|(p, v)| (p.name.to_string(), v.to_string())).collect()).unwrap_or_default(),
            custom: c.as_deref().map(|c| c.customs().map(|(k, v)| (k.to_string(), v.to_string())).collect()).unwrap_or_default(),
            inherited: n.inherited_from(),
            rules: n.matched_rules(),
        })
    }
}
