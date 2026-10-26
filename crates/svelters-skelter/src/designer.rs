//! DZ0/DZ1/DZ2b on skelter: what a visual designer reads from the running UI and forces on it, in
//! svelters-egui's shapes so cadatomic's designer (`cadatomic-ui/src/designer.rs`) moves over with
//! little change:
//!
//! ```ignore
//! backend.scene().read(|s| {
//!     let nodes: Vec<_> = s.by_source(&file, offset).map(|n| (n.id(), n.matched_rules())).collect();
//!     let under: Vec<_> = s.hit_stack(window, point).into_iter().filter_map(|t| s.get(t.node())).collect();
//! });
//! let bits = svelters_skelter::style::pseudo_state("hover").unwrap();
//! backend.force_state(handle, bits)?; // StateBits::NONE clears
//! backend.set_override(handle, "gap", &Value::F64(12.0))?; // a live preview; clear_override ends it
//! ```
//!
//! Nodes are named by their JS handle (`NodeRef::id`, what [`SkelterBackend::force_state`] takes);
//! nodes JS never saw (a kind's own parts, a Rust shell's nodes) are left out. Where a node was
//! declared comes from its `source-loc` prop, which the runtime sets in dev builds (`file#offset`,
//! then the component call sites, U+001F-separated). The rules come from skelter-core's cascade
//! ([`Document::matched_rules`]), with each sheet's name and, in dev builds, its `<style>` source.

use crate::backend::SkelterBackend;
use skelter_base::geom::{Point, Rect};
use skelter_base::ids::{NodeId, WindowId};
use skelter_core::Document;
use svelters_abi::{Backend, Value, svl_handle};
use svelters_style::MatchedRuleInfo;

pub use svelters_style::StateMask as StateBits;

/// One `file#offset` of a [`SourceLoc`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SourceSpot<'a> {
    /// The path the compiler was given for the `.svelte` file.
    pub file: &'a str,
    /// The byte offset of the template element (or component tag) in that file.
    pub offset: u32,
}

impl<'a> SourceSpot<'a> {
    /// Parses `file#offset` (the offset after the last `#`).
    pub fn parse(text: &'a str) -> Option<SourceSpot<'a>> {
        let (file, offset) = text.rsplit_once('#')?;
        Some(SourceSpot { file, offset: offset.parse().ok()? })
    }
}

/// A node's dev-build source location: its own template element, then the component call sites
/// around it (svelters-egui's `scene::SourceLoc`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceLoc<'a> {
    raw: &'a str,
}

impl<'a> SourceLoc<'a> {
    /// Wraps a `source-loc` text; `None` unless its first spot parses.
    pub fn parse(raw: &'a str) -> Option<SourceLoc<'a>> {
        SourceSpot::parse(raw.split('\u{1f}').next()?)?;
        Some(SourceLoc { raw })
    }

    /// The prop's text.
    pub fn raw(self) -> &'a str {
        self.raw
    }

    /// The node's own template element.
    pub fn element(self) -> SourceSpot<'a> {
        self.spots().next().expect("checked in parse")
    }

    /// The component call sites the node renders inside, innermost first.
    pub fn sites(self) -> impl Iterator<Item = SourceSpot<'a>> + 'a {
        self.spots().skip(1)
    }

    /// The element, then the call sites.
    pub fn spots(self) -> impl Iterator<Item = SourceSpot<'a>> + 'a {
        self.raw.split('\u{1f}').filter_map(SourceSpot::parse)
    }
}

/// A node's `source-loc`, owned ([`NodeRef::source_loc`]; the prop lives in the backend's tables, not
/// in the scene borrow). The same reads as [`SourceLoc`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceLocBuf {
    raw: String,
}

impl SourceLocBuf {
    pub fn parse(raw: String) -> Option<SourceLocBuf> {
        SourceLoc::parse(&raw)?;
        Some(SourceLocBuf { raw })
    }
    pub fn as_loc(&self) -> SourceLoc<'_> {
        SourceLoc { raw: &self.raw }
    }
    pub fn raw(&self) -> &str {
        &self.raw
    }
    pub fn element(&self) -> SourceSpot<'_> {
        self.as_loc().element()
    }
    pub fn sites(&self) -> impl Iterator<Item = SourceSpot<'_>> + '_ {
        self.as_loc().sites()
    }
    pub fn spots(&self) -> impl Iterator<Item = SourceSpot<'_>> + '_ {
        self.as_loc().spots()
    }
}

/// One entry of [`SceneRef::hit_stack`] (svelters-egui's `scene::Target`; skelter has no virtual
/// parts in it yet).
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Target {
    Node(svl_handle),
}

impl Target {
    /// The node's handle.
    pub fn node(self) -> svl_handle {
        match self {
            Target::Node(h) => h,
        }
    }
}

/// Where a node was drawn in a window's last frame ([`SceneRef::shown`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shown {
    /// Its border box in window points (the transformed bounding box).
    pub rect: Rect,
    /// The part of `rect` inside its clips.
    pub visible: Rect,
}

/// The designer's view of a [`SkelterBackend`]'s tree ([`SkelterBackend::scene`]).
#[derive(Clone)]
pub struct Scene {
    backend: SkelterBackend,
}

impl Scene {
    /// Runs `f` on the tree as it is now (the document borrowed for the call).
    pub fn read<R>(&self, f: impl FnOnce(SceneRef<'_>) -> R) -> R {
        let doc = self.backend.doc();
        f(SceneRef { backend: &self.backend, doc: &doc })
    }
}

/// The tree, borrowed ([`Scene::read`]).
#[derive(Clone, Copy)]
pub struct SceneRef<'a> {
    backend: &'a SkelterBackend,
    doc: &'a Document,
}

impl<'a> SceneRef<'a> {
    /// The document itself, for reads this view doesn't wrap.
    pub fn document(self) -> &'a Document {
        self.doc
    }

    /// The live node of handle `h` (`None`: stale, or not a node).
    pub fn get(self, h: svl_handle) -> Option<NodeRef<'a>> {
        let n = self.backend.node(h)?;
        self.doc.contains(n).then_some(NodeRef { s: self, h, n })
    }

    /// The node of skelter id `n`, when JS knows it.
    pub fn get_node(self, n: NodeId) -> Option<NodeRef<'a>> {
        self.get(self.backend.handle(n)?)
    }

    /// Every live node JS knows, in no particular order.
    pub fn nodes(self) -> impl Iterator<Item = NodeRef<'a>> + 'a {
        let mut hs: Vec<(NodeId, svl_handle)> = self.backend.0.handles.borrow().pairs().collect();
        hs.sort_by_key(|&(_, h)| h);
        hs.into_iter().filter(move |&(n, _)| self.doc.contains(n)).map(move |(n, h)| NodeRef { s: self, h, n })
    }

    /// DZ0: every live node declared at the template element `file#offset` (each row of an
    /// `{#each}`, each instance of a component), in handle (creation) order. A scan: for tools.
    pub fn by_source(self, file: &'a str, offset: u32) -> impl Iterator<Item = NodeRef<'a>> + 'a {
        let want = SourceSpot { file, offset };
        self.nodes().filter(move |n| n.source_loc().is_some_and(|l| l.element() == want))
    }

    /// DZ0: every node `window`'s last frame drew under `pos` (window points), topmost first and,
    /// within the paint order, a node before the boxes it is in: what a designer's pick cycles
    /// through ([`Document::hit_stack`]). Nodes JS doesn't know are left out.
    pub fn hit_stack(self, window: WindowId, pos: Point) -> Vec<Target> {
        let mut out = Vec::new();
        for n in self.doc.hit_stack(window, pos) {
            if let Some(h) = self.backend.handle(n)
                && !out.contains(&Target::Node(h))
            {
                out.push(Target::Node(h));
            }
        }
        out
    }

    /// Where node `h` was drawn in `window`'s last frame (`None`: not drawn there).
    pub fn shown(self, window: WindowId, h: svl_handle) -> Option<Shown> {
        let n = self.backend.node(h)?;
        let list = self.doc.hit_list(window)?;
        let e = list.entries.iter().find(|e| e.node == n)?;
        Some(Shown { rect: e.world, visible: e.clip.map_or(e.world, |c| e.world.intersect(&c)) })
    }
}

/// A live node JS knows ([`SceneRef::get`]).
#[derive(Clone, Copy)]
pub struct NodeRef<'a> {
    s: SceneRef<'a>,
    h: svl_handle,
    n: NodeId,
}

impl<'a> NodeRef<'a> {
    /// Its JS handle (what [`SkelterBackend::force_state`] takes).
    pub fn id(self) -> svl_handle {
        self.h
    }

    /// Its skelter-core id.
    pub fn node(self) -> NodeId {
        self.n
    }

    /// The element name JS created it with (`box`, `button`, an app kind's tag).
    pub fn tag(self) -> &'static str {
        self.s.backend.kind_map(self.h).map_or("", |m| m.tag())
    }

    pub fn parent(self) -> Option<NodeRef<'a>> {
        let mut at = self.s.doc.parent(self.n)?;
        loop {
            if let Some(r) = self.s.get_node(at) {
                return Some(r);
            }
            at = self.s.doc.parent(at)?;
        }
    }

    /// A string prop's value by name (`None`: the kind has no such prop, or it is empty).
    fn text_prop(self, name: &str) -> Option<String> {
        let km = self.s.backend.kind_map(self.h).ok()?;
        let wire = km.prop_named(name)?.wire;
        match Backend::get_prop(self.s.backend, self.h, wire).ok()? {
            Value::Str(s) if !s.is_empty() => Some(s),
            _ => None,
        }
    }

    /// Its `anchor` prop.
    pub fn anchor(self) -> Option<String> {
        self.text_prop("anchor")
    }

    /// DZ0: where a dev build declared this node, if the runtime set `source-loc`.
    pub fn source_loc(self) -> Option<SourceLocBuf> {
        SourceLocBuf::parse(self.text_prop("source-loc")?)
    }

    /// Its border box in window points from the last frame (`None`: never laid out in a window).
    pub fn drawn(self) -> Option<Rect> {
        self.s.doc.world_rect(self.n)
    }

    /// DZ0: the rules that match it now, in cascade order ([`Document::matched_rules`]).
    pub fn matched_rules(self) -> Vec<MatchedRuleInfo> {
        self.s.doc.matched_rules(self.n)
    }

    /// The state bits forced on it ([`SkelterBackend::force_state`]).
    pub fn forced_state(self) -> StateBits {
        self.s.doc.forced_state(self.n)
    }
}

impl SkelterBackend {
    /// DZ0 (SV30): overrides prop `prop` (by name) of node `h` with `v` until
    /// [`SkelterBackend::clear_override`], for a designer's live preview while a gesture lasts; the
    /// runtime's writes meanwhile are held back (the last one is kept and written on clear).
    /// Overriding again replaces the value. A freed node takes its overrides with it. The write is a
    /// host write: it announces nothing to `bind:`, so previewing a bound prop never changes app
    /// state (svelters-egui's `EguiBackend::set_override`).
    pub fn set_override(&self, h: svl_handle, prop: &str, v: &Value) -> Result<(), String> {
        let wire = self.prop_wire_named(h, prop)?;
        let under = match self.0.overrides.borrow_mut().remove(&(h, wire)) {
            Some(under) => under,
            None => Backend::get_prop(self, h, wire)?,
        };
        let r = self.write_prop_now(h, wire, v);
        self.0.overrides.borrow_mut().insert((h, wire), under);
        r?;
        Backend::request_frame(self);
        Ok(())
    }

    /// Ends the override of `prop` on `h`: the node takes the value the runtime last wrote (or had
    /// before the override). `false` if it was not overridden.
    pub fn clear_override(&self, h: svl_handle, prop: &str) -> Result<bool, String> {
        let wire = self.prop_wire_named(h, prop)?;
        let Some(under) = self.0.overrides.borrow_mut().remove(&(h, wire)) else { return Ok(false) };
        self.write_prop_now(h, wire, &under)?;
        Backend::request_frame(self);
        Ok(true)
    }

    /// Ends every override (each live node takes its runtime value again).
    pub fn clear_overrides(&self) {
        let all: Vec<_> = self.0.overrides.borrow_mut().drain().collect();
        for ((h, wire), under) in all {
            let _ = self.write_prop_now(h, wire, &under);
        }
        Backend::request_frame(self);
    }

    /// The live nodes' overridden props, as (node handle, prop name), sorted.
    pub fn overrides(&self) -> Vec<(svl_handle, &'static str)> {
        let mut out: Vec<_> = self
            .0
            .overrides
            .borrow()
            .keys()
            .filter_map(|&(h, wire)| Some((h, self.kind_map(h).ok()?.prop(wire)?.spec.name)))
            .collect();
        out.sort();
        out
    }

    fn prop_wire_named(&self, h: svl_handle, prop: &str) -> Result<svelters_abi::svl_id, String> {
        let (_, km) = self.live(h)?;
        km.prop_named(prop).map(|p| p.wire).ok_or_else(|| format!("<{}> has no prop {prop}", km.tag()))
    }

    /// The designer's view of the tree (see the module docs).
    pub fn scene(&self) -> Scene {
        Scene { backend: self.clone() }
    }

    /// DZ2b: forces state bits on node `h` (`StateBits::HOVERED`, a provider's `:state(…)` bit) for
    /// selector matching and painting, until replaced; `StateBits::NONE` clears. A designer forces a
    /// state to show and edit that state's rules. The bits are ORed over the node's own and never
    /// reach its events or accessibility; a freed node takes them with it.
    /// [`crate::style::pseudo_state`] maps a selector's pseudo-class to its bits.
    pub fn force_state(&self, h: svl_handle, bits: StateBits) -> Result<(), String> {
        let n = self.node(h).ok_or_else(|| format!("stale or foreign handle {h}"))?;
        if !self.doc_mut().force_state(n, bits) {
            return Err(format!("stale handle {h}"));
        }
        Backend::request_frame(self);
        Ok(())
    }

    /// The bits forced on node `h`, `NONE` when none.
    pub fn forced_state(&self, h: svl_handle) -> StateBits {
        self.node(h).map_or(StateBits::NONE, |n| self.doc().forced_state(n))
    }

    /// [`crate::style::pseudo_state`], with `:state(ns.name)` resolved through the host's provider
    /// states ([`SkelterBackend::set_state_names`]).
    pub fn pseudo_state(&self, name: &str) -> Option<StateBits> {
        let n = name.trim().trim_start_matches(':');
        if let Some(arg) = n.strip_prefix("state(").and_then(|r| r.strip_suffix(')')) {
            return self.doc().state_names()?.lookup(arg.trim());
        }
        crate::style::pseudo_state(n)
    }
}
