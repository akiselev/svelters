//! [`Session`]: one client's view of the app: grants, tooling roots, the host's idle predicate,
//! element refs with their epoch, and every method (Rust API and [`Session::call`]). AU7's waits
//! (event journal, busy sources, clocks, reports) are in [`waits`].

mod waits;
pub use waits::{Actionability, Budget, Condition, NeverId, SeenId, Unmet, WaitReport};
use waits::{Deadline, SeenLatch};

use crate::busy::BusyRegistry;
use crate::driver::{A11yNode, Changes, SourceDriver};
use crate::journal::{Cursor, Journal, Matcher};
use crate::engine::Roots;
use crate::selector::{Selector, TextMatch, parse};
use crate::snapshot::yaml_in;
use crate::source::SceneSource;
use crate::{Error, PROTOCOL, is_enabled};
use crate::geom::{Pos2, Rect, Vec2, pos2, vec2};
use crate::image::Image;
use crate::input::{Event, Key, Modifiers, MouseWheelUnit, PointerButton};
use accesskit::{Action, ActionData, ActionRequest};
use serde_json::{Value, json};
use std::borrow::Cow;
use std::collections::BTreeSet;
use std::time::{Duration, Instant};
#[cfg(feature = "egui")]
use svelters_egui::scene::NodeId;
use svelters_style::{MatchedRuleInfo, StyleSource};

/// The node type the generic types default to: svelters-egui's `NodeId` with feature `egui`
/// (`Ref`, `NodeInfo`, … as before the split), else `u64` (name the scene's node, e.g.
/// `Ref<NodeKey>`, without it).
#[cfg(feature = "egui")]
pub type DefaultNode = NodeId;
/// The node type the generic types default to: svelters-egui's `NodeId` with feature `egui`, else
/// `u64` (name the scene's node, e.g. `Ref<NodeKey>`, without it).
#[cfg(not(feature = "egui"))]
pub type DefaultNode = u64;

/// A JSON id of a node ([`SourceDriver::node_raw`]).
type RawFn<'a, N> = &'a dyn Fn(N) -> u64;

/// What a session may do (§1.2). `os_input`, `clock` and `eval` are test-build grants (§6 Q1);
/// AU1b adds `clock` (`clock.advance`); `os_input` and `eval` have no method yet.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Grants {
    pub read: bool,
    pub act: bool,
    /// Reads and acts may reach tooling roots (the inspector, agent panels) when asked.
    pub tooling: bool,
    /// AU1b: `clock.advance` may move the virtual clock (a test-build grant).
    pub clock: bool,
}

impl Grants {
    pub const READ: Grants = Grants {
        read: true,
        act: false,
        tooling: false,
        clock: false,
    };
    pub const ACT: Grants = Grants {
        read: true,
        act: true,
        tooling: false,
        clock: false,
    };
    pub const ALL: Grants = Grants {
        read: true,
        act: true,
        tooling: true,
        clock: true,
    };
}

/// An element ref (§1.3): a node and the reload epoch it was read in. A ref from an older epoch,
/// or to a freed node, fails with `node.stale`. `N` is the scene's node ([`SourceDriver::Node`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ref<N = DefaultNode> {
    pub id: N,
    pub epoch: u64,
}

/// One node as `scene.query` returns it.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeInfo<N = DefaultNode> {
    pub r#ref: Ref<N>,
    pub tag: String,
    pub role: &'static str,
    pub name: String,
    pub text: String,
    pub value: Option<String>,
    pub anchor: Option<String>,
    pub root: Option<String>,
    /// Screen rect in egui points, as drawn last pass (`None`: not drawn in that pass: scrolled
    /// out of view, an unrealised row, a closed menu).
    pub rect: Option<Rect>,
    /// AX1: the part of `rect` in view (inside its clip): empty when scrolled out of a
    /// `scroll-area`, smaller than `rect` when partly.
    pub visible_rect: Option<Rect>,
    pub states: Vec<&'static str>,
    /// DZ0: where a dev build declared the node (`source-loc`): `None` in release builds.
    pub source: Option<SourceInfo>,
}

/// DZ0: a node's template source location, as `scene.query` returns it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceInfo {
    /// The `.svelte` path the compiler was given, and the element's byte offset in it.
    pub file: String,
    pub offset: u32,
    /// The component call sites the node renders inside, innermost first.
    pub sites: Vec<(String, u32)>,
}

impl SourceInfo {
    pub fn to_json(&self) -> Value {
        json!({
            "file": self.file, "offset": self.offset,
            "sites": self.sites.iter().map(|(f, o)| json!({"file": f, "offset": o})).collect::<Vec<_>>(),
        })
    }
}

/// DZ0b2: a node's computed style, as `style.computed` returns it.
#[derive(Clone, Debug, PartialEq)]
pub struct ComputedInfo<N = DefaultNode> {
    /// Every set property with its computed value as CSS text, in the backend table's order.
    pub values: Vec<(String, String)>,
    /// Custom properties (`--name`) with their computed text, by name.
    pub custom: Vec<(String, String)>,
    /// The inherited properties (custom ones included) the node doesn't declare, by name, each
    /// with the nearest ancestor that declares it (`None`: none does).
    pub inherited: Vec<(String, Option<Ref<N>>)>,
    /// The matched rules, as `style.matched` returns them.
    pub rules: Vec<MatchedRuleInfo>,
}

#[cfg(feature = "egui")]
impl ComputedInfo {
    pub fn to_json(&self) -> Value {
        self.to_json_with(&|n: NodeId| n.to_raw())
    }
}

impl<N: Copy> ComputedInfo<N> {
    /// [`ComputedInfo::to_json`] with `raw` giving a node's JSON id.
    pub fn to_json_with(&self, raw: RawFn<'_, N>) -> Value {
        let map = |v: &[(String, String)]| Value::Object(v.iter().map(|(k, v)| (k.clone(), json!(v))).collect());
        json!({
            "values": map(&self.values),
            "custom": map(&self.custom),
            "inherited": Value::Object(self.inherited.iter().map(|(k, r)| (k.clone(), r.map_or(Value::Null, |r| ref_json(r, raw)))).collect()),
            "rules": self.rules.iter().map(rule_json).collect::<Vec<_>>(),
        })
    }
}

/// DZ0b2: one matched rule as JSON (`style.matched`, `style.computed`'s `rules`).
pub fn rule_json(r: &MatchedRuleInfo) -> Value {
    let src = |s: &Option<StyleSource>| s.as_ref().map_or(Value::Null, |s| json!({"file": s.file, "style_offset": s.style_offset}));
    json!({
        "sheet": r.sheet, "selectors": r.selectors, "specificity": r.specificity,
        "line": r.line, "column": r.column, "source": src(&r.source),
        "decls": r.decls.iter().map(|d| json!({
            "name": d.name, "value": d.value, "important": d.important, "winner": d.winner, "source": src(&d.source),
        })).collect::<Vec<_>>(),
    })
}

#[cfg(feature = "egui")]
impl NodeInfo {
    pub fn to_json(&self) -> Value {
        self.to_json_with(&|n: NodeId| n.to_raw())
    }
}

impl<N: Copy> NodeInfo<N> {
    /// [`NodeInfo::to_json`] with `raw` giving a node's JSON id.
    pub fn to_json_with(&self, raw: RawFn<'_, N>) -> Value {
        let mut v = self.json(raw);
        if let Some(src) = &self.source {
            v["source"] = src.to_json();
        }
        v
    }

    fn json(&self, raw: RawFn<'_, N>) -> Value {
        json!({
            "ref": ref_json(self.r#ref, raw),
            "tag": self.tag, "role": self.role, "name": self.name, "text": self.text, "value": self.value,
            "anchor": self.anchor, "root": self.root,
            "rect": self.rect.map(|r| json!([r.min.x, r.min.y, r.width(), r.height()])),
            "visible_rect": self.visible_rect.map(|r| json!([r.min.x, r.min.y, r.width(), r.height()])),
            "states": self.states,
        })
    }
}

/// A mounted root as `app.roots` returns it.
#[derive(Clone, Debug, PartialEq)]
pub struct RootInfo {
    /// The embed's `RootKey` (`None`: an app window root, mounted without one).
    pub key: Option<String>,
    pub visible: bool,
    pub tooling: bool,
}

/// How an action reaches the node (§1.5). OS input is AU6.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tier {
    /// AccessKit action requests (`Click`, `Focus`, `SetValue`): no geometry, the default for
    /// agents. Falls back to synthetic input when the node has no AccessKit node.
    Semantic,
    /// egui pointer and key events at the node's actionable point: egui's real hit path.
    #[default]
    Synthetic,
}

/// Options of an act.
#[derive(Clone, Copy, Debug)]
pub struct ActOptions {
    pub tier: Tier,
    /// Skip the actionability checks.
    pub force: bool,
    /// Virtual ms to wait for actionability and for the settle after the act.
    pub timeout_ms: f64,
}

impl Default for ActOptions {
    fn default() -> Self {
        ActOptions {
            tier: Tier::Synthetic,
            force: false,
            timeout_ms: 5_000.0,
        }
    }
}

/// SV51: a point in an act's target, in its local points (from the top left of its rect drawn last
/// pass; a point outside the rect is allowed: a canvas drag that leaves the node), or its centre.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum At {
    #[default]
    Center,
    Local(Pos2),
}

impl At {
    /// The point in `rect`; `At::Center` is `center`, the actionable point (AX1: the centre of the
    /// part in view, for a node partly clipped).
    fn in_rect(self, rect: Rect, center: Pos2) -> Pos2 {
        match self {
            At::Center => center,
            At::Local(p) => rect.min + p.to_vec2(),
        }
    }
}

/// Where an act aims (AX1): the target's rect as drawn and its actionable point (the centre of
/// the part in view).
#[derive(Clone, Copy, Debug)]
struct Aim {
    rect: Rect,
    at: Pos2,
}

/// SV51: where a drag ends.
#[derive(Clone, Debug)]
pub enum DragTo<N = DefaultNode> {
    /// A point in the target's local points.
    Local(Pos2),
    /// The centre of another node (resolved when the drag starts).
    Node(Target<N>),
}

/// SV51: `input.drag`: a press at `from`, `steps` moves (one frame each) to `to`, a release there.
#[non_exhaustive]
#[derive(Clone, Debug)]
pub struct Drag<N = DefaultNode> {
    pub from: At,
    pub to: DragTo<N>,
    /// Moves between the press and the release (at least 1), each in its own frame.
    pub steps: u32,
    pub button: PointerButton,
    /// Held for the whole drag (`ModifiersChanged` before the press, released after).
    pub modifiers: Modifiers,
}

impl<N> Drag<N> {
    /// From the target's centre to `to` (its local points), 10 steps, primary button.
    pub fn to(to: impl Into<Pos2>) -> Drag<N> {
        let to = to.into();
        Drag::new(DragTo::Local(to))
    }
    /// From the target's centre to `node`'s centre.
    pub fn to_node(node: impl Into<Target<N>>) -> Drag<N> {
        Drag::new(DragTo::Node(node.into()))
    }
    fn new(to: DragTo<N>) -> Drag<N> {
        Drag { from: At::Center, to, steps: 10, button: PointerButton::Primary, modifiers: Modifiers::NONE }
    }
    pub fn from(mut self, at: At) -> Drag<N> {
        self.from = at;
        self
    }
    pub fn steps(mut self, n: u32) -> Drag<N> {
        self.steps = n.max(1);
        self
    }
    pub fn button(mut self, b: impl Into<PointerButton>) -> Drag<N> {
        self.button = b.into();
        self
    }
    pub fn modifiers(mut self, m: impl Into<Modifiers>) -> Drag<N> {
        self.modifiers = m.into();
        self
    }
}

/// SV51: `input.wheel`: a scroll at a point of the target.
#[non_exhaustive]
#[derive(Clone, Copy, Debug)]
pub struct Wheel {
    pub at: At,
    /// egui's wheel delta (positive `y` scrolls up, as egui sends it), in `unit`s.
    pub delta: Vec2,
    /// `Line` (a mouse wheel's notches), `Point` (a trackpad's pixels) or `Page`.
    pub unit: MouseWheelUnit,
    pub modifiers: Modifiers,
}

impl Wheel {
    /// `lines` notches (positive `y`: up) at the target's centre.
    pub fn lines(lines: impl Into<Vec2>) -> Wheel {
        Wheel { at: At::Center, delta: lines.into(), unit: MouseWheelUnit::Line, modifiers: Modifiers::NONE }
    }
    /// `points` of trackpad scroll (positive `y`: up) at the target's centre.
    pub fn points(points: impl Into<Vec2>) -> Wheel {
        Wheel { at: At::Center, delta: points.into(), unit: MouseWheelUnit::Point, modifiers: Modifiers::NONE }
    }
    pub fn at(mut self, at: At) -> Wheel {
        self.at = at;
        self
    }
    pub fn modifiers(mut self, m: impl Into<Modifiers>) -> Wheel {
        self.modifiers = m.into();
        self
    }
}

/// What an act did, read after the app settled.
#[derive(Clone, Debug, PartialEq)]
pub struct ActResult<N = DefaultNode> {
    pub frame: u64,
    pub epoch: u64,
    /// The tier that ran (a semantic act without an AccessKit node runs synthetic).
    pub tier: Tier,
    /// Nodes that changed, with what changed (`ChangeMask` names).
    pub changed: Vec<(N, String)>,
}

/// What `wait.for` waits for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaitState {
    Attached,
    Detached,
    Visible,
    Hidden,
    Enabled,
}

/// An expectation `wait.for` checks on the matches after each settle.
#[derive(Clone, Debug, PartialEq)]
pub enum Expect {
    Count(usize),
    Text(TextMatch),
    Value(TextMatch),
}

/// One client of the protocol, over a [`SourceDriver`] (every [`Driver`](crate::Driver) is one).
pub struct Session<D: SourceDriver> {
    driver: D,
    grants: Grants,
    include_tooling: bool,
    tooling: Box<dyn Fn(&str) -> bool>,
    idle: Option<Box<dyn Fn() -> bool>>,
    epoch: u64,
    frame: u64,
    /// Virtual ms run by this session's frames.
    now_ms: f64,
    pending_input: bool,
    journal: Option<D::Observer>,
    // ---- AU7 ----
    /// The app's event journal (shared: [`Session::set_journal`]).
    events: Journal,
    busy: BusyRegistry,
    /// Events consumed by [`Session::wait_event`]: two identical waits match two events.
    consumed: BTreeSet<u64>,
    nevers: Vec<(NeverId, Matcher, Cursor)>,
    seen: Vec<SeenLatch>,
    next_id: u64,
    /// Wall time of the frames run while a foreground wall-clock source held the virtual clock.
    held_wall: Duration,
    clock_held: bool,
    /// [`Session::bridge_scene_changes`]: SG2's journal as `scene.changed` events.
    scene_bridge: Option<D::Observer>,
}

/// The node of a session's scene.
type N<D> = <D as SourceDriver>::Node;

impl<D: SourceDriver> Session<D> {
    /// A session with `grants`. Fails with `automation.disabled` unless [`crate::enable`] ran.
    pub fn new(driver: D, grants: Grants) -> Result<Session<D>, Error> {
        if !is_enabled() {
            return Err(disabled());
        }
        Ok(Session {
            driver,
            grants,
            include_tooling: false,
            tooling: Box::new(|_| false),
            idle: None,
            epoch: 0,
            frame: 0,
            now_ms: 0.0,
            pending_input: false,
            journal: None,
            events: Journal::default(),
            busy: BusyRegistry::new(),
            consumed: BTreeSet::new(),
            nevers: Vec::new(),
            seen: Vec::new(),
            next_id: 1,
            held_wall: Duration::ZERO,
            clock_held: false,
            scene_bridge: None,
        })
    }

    /// Tests: turns automation on and opens a session with every grant.
    pub fn for_test(driver: D) -> Session<D> {
        crate::enable();
        Session::new(driver, Grants::ALL).expect("enabled")
    }

    /// The host's idle predicate (§6, agreed with cadatomic): waits never answer while it is
    /// `false` (cadatomic's IdleBarrier: jobs, sessions, `BusyProbe`).
    ///
    /// **Deprecated (AU7):** register named sources with [`Session::register_busy_source`] instead;
    /// a timeout can then say which source was busy. Kept until cadatomic moves.
    pub fn set_idle(&mut self, idle: impl Fn() -> bool + 'static) {
        self.idle = Some(Box::new(idle));
    }

    /// Which roots are tooling roots, by key (§1.2): queries, snapshots and the journal skip them
    /// unless [`Session::include_tooling`] is on (it needs the `tooling` grant).
    pub fn set_tooling_roots(&mut self, is_tooling: impl Fn(&str) -> bool + 'static) {
        self.tooling = Box::new(is_tooling);
    }

    pub fn include_tooling(&mut self, on: bool) -> Result<(), Error> {
        if on && !self.grants.tooling {
            return Err(Error::new(
                "grant.denied",
                "including tooling roots needs the `tooling` grant",
            ));
        }
        self.include_tooling = on;
        Ok(())
    }

    /// Starts a new reload epoch: refs read before fail with `node.stale` (AU3 calls this on every
    /// swap).
    pub fn bump_epoch(&mut self) {
        self.epoch += 1;
    }

    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Virtual ms run by this session's frames.
    pub fn now_ms(&self) -> f64 {
        self.now_ms
    }

    pub fn driver(&self) -> &D {
        &self.driver
    }

    pub fn driver_mut(&mut self) -> &mut D {
        &mut self.driver
    }

    pub fn into_driver(self) -> D {
        self.driver
    }

    fn check(&self, act: bool) -> Result<(), Error> {
        if !is_enabled() {
            return Err(disabled());
        }
        if !self.grants.read || act && !self.grants.act {
            return Err(Error::new(
                "grant.denied",
                if act {
                    "acting needs the `act` grant"
                } else {
                    "reading needs the `read` grant"
                },
            ));
        }
        Ok(())
    }

    fn with_scene<R>(&self, f: impl for<'s> FnOnce(&D::Scene<'s>, &Roots<'_>) -> R) -> R {
        let skip_tooling = !self.include_tooling;
        let tooling = &self.tooling;
        let skip = move |k: &str| skip_tooling && tooling(k);
        let roots = Roots { skip: &skip };
        self.driver.read(|s| f(s, &roots))
    }

    /// A node's JSON id.
    fn raw(&self) -> impl Fn(N<D>) -> u64 + '_ {
        |n| self.driver.node_raw(n)
    }

    /// Resolves `target` to one live node.
    fn target_node<S: SceneSource<Node = N<D>>>(&self, s: &S, target: &Target<N<D>>, roots: &Roots<'_>) -> Result<N<D>, Error> {
        match target {
            Target::Selector(sel) => strict(sel, resolve(s, sel, roots)?),
            Target::Invalid(e) => Err(e.clone()),
            Target::Ref(r) => {
                if r.epoch != self.epoch || !s.contains(r.id) {
                    return Err(stale(*r));
                }
                Ok(r.id)
            }
        }
    }

    // ---- session ----

    /// `session.hello`: the protocol version, roots and capabilities.
    pub fn hello(&self) -> Result<Value, Error> {
        self.check(false)?;
        let roots: Vec<Value> = self
            .roots()?
            .iter()
            .map(|r| json!({"key": r.key, "visible": r.visible, "tooling": r.tooling}))
            .collect();
        Ok(json!({
            "protocol": PROTOCOL,
            "roots": roots,
            "grants": {"read": self.grants.read, "act": self.grants.act, "tooling": self.grants.tooling, "clock": self.grants.clock},
            "capabilities": ["scene.query", "scene.node", "scene.snapshot", "style.matched", "style.computed", "a11y.tree", "app.roots", "input.click", "input.focus",
                "input.fill", "input.set_value", "input.press", "input.type", "input.hover", "input.pointer_move", "input.wheel", "input.drag", "input.scroll_into_view", "wait.settled", "wait.for",
                "journal.subscribe", "journal.take", "capture.screenshot", "clock.now", "clock.advance",
                "journal.cursor", "journal.events", "journal.kinds", "wait.event", "wait.never", "wait.seen", "wait.step_end", "busy.sources"],
            "clock": self.clock_json(),
        }))
    }

    /// `app.roots`: the mounted roots (tooling roots only with `include_tooling`).
    pub fn roots(&self) -> Result<Vec<RootInfo>, Error> {
        self.check(false)?;
        Ok(self.with_scene(|s, roots| {
            s.roots()
                .into_iter()
                .filter(|r| !r.key.as_deref().is_some_and(roots.skip))
                .map(|r| RootInfo {
                    tooling: r.key.as_deref().is_some_and(|k| (self.tooling)(k)),
                    key: r.key,
                    visible: r.visible,
                })
                .collect()
        }))
    }

    // ---- query and read ----

    /// The nodes `selector` matches (§1.4 `scene.query`), at most `limit`.
    pub fn query(&self, selector: &str, limit: Option<usize>) -> Result<Vec<NodeInfo<N<D>>>, Error> {
        self.check(false)?;
        let sel = parse(selector)?;
        self.with_scene(|s, roots| {
            let ids = resolve(s, &sel, roots)?;
            Ok(ids
                .into_iter()
                .take(limit.unwrap_or(usize::MAX))
                .filter(|&id| s.contains(id))
                .map(|id| self.info(s, id))
                .collect())
        })
    }

    /// The single node `selector` matches (strict, §1.3): `node.not_found` for none,
    /// `node.ambiguous` for more than one.
    pub fn locate(&self, selector: &str) -> Result<Ref<N<D>>, Error> {
        self.check(false)?;
        let sel = parse(selector)?;
        let id = self.with_scene(|s, roots| strict(&sel, resolve(s, &sel, roots)?))?;
        Ok(Ref {
            id,
            epoch: self.epoch,
        })
    }

    /// The node behind `r` (§1.4 `scene.node`).
    pub fn node(&self, r: Ref<N<D>>) -> Result<NodeInfo<N<D>>, Error> {
        self.check(false)?;
        self.live(r)?;
        self.with_scene(|s, _| s.contains(r.id).then(|| self.info(s, r.id)))
            .ok_or_else(|| stale(r))
    }

    /// DZ0b2 `style.matched`: the rules that match the node, in cascade order (lowest priority
    /// first), each declaration marked when it wins its property (egui's
    /// `NodeRef::matched_rules`).
    pub fn style_matched(&self, r: Ref<N<D>>) -> Result<Vec<MatchedRuleInfo>, Error> {
        self.check(false)?;
        self.live(r)?;
        self.with_scene(|s, _| s.matched_rules(r.id)).ok_or_else(no_style)
    }

    /// DZ0b2 `style.computed` (E2E-INSPECTOR §1.4): the computed values, the matched rules and,
    /// for each inherited property the node doesn't declare, the ancestor it comes from.
    pub fn style_computed(&self, r: Ref<N<D>>) -> Result<ComputedInfo<N<D>>, Error> {
        self.check(false)?;
        self.live(r)?;
        let epoch = self.epoch;
        self.with_scene(|s, _| {
            let c = s.computed_style(r.id)?;
            Some(ComputedInfo {
                values: c.values,
                custom: c.custom,
                inherited: c.inherited.into_iter().map(|(k, at)| (k, at.map(|id| Ref { id, epoch }))).collect(),
                rules: c.rules,
            })
        })
        .ok_or_else(no_style)
    }

    /// A text snapshot of `scope` (every visible non-tooling root when `None`), §1.8.
    pub fn snapshot(&self, scope: Option<&str>) -> Result<String, Error> {
        self.check(false)?;
        let sel = scope.map(parse).transpose()?;
        self.with_scene(|s, roots| {
            let mut out = String::new();
            match &sel {
                Some(sel) => {
                    for id in resolve(s, sel, roots)? {
                        yaml_in(s, id, 0, &mut out);
                    }
                }
                None => {
                    for r in s.roots() {
                        if r.key.as_deref().is_some_and(roots.skip) || !r.visible {
                            continue;
                        }
                        yaml_in(s, r.top, 0, &mut out);
                    }
                }
            }
            Ok(out)
        })
    }

    /// `a11y.tree`: the AccessKit nodes of the last frame (Rust-drawn egui included).
    pub fn a11y_tree(&self) -> Result<Vec<A11yNode>, Error> {
        self.check(false)?;
        Ok(self.driver.a11y_nodes())
    }

    fn info<S: SceneSource<Node = N<D>>>(&self, s: &S, n: N<D>) -> NodeInfo<N<D>> {
        let drawn = s.drawn(n);
        NodeInfo {
            r#ref: Ref {
                id: n,
                epoch: self.epoch,
            },
            tag: s.tag(n).into_owned(),
            role: s.role(n),
            name: s.name(n).into_owned(),
            text: s.text(n).into_owned(),
            value: s.value(n),
            anchor: s.anchor(n).map(Cow::into_owned),
            root: s.root_key_of(n).map(Cow::into_owned),
            rect: drawn.map(|sh| sh.rect),
            visible_rect: drawn.map(|sh| sh.visible),
            states: s.states(n),
            source: s.source_info(n),
        }
    }

    fn live(&self, r: Ref<N<D>>) -> Result<(), Error> {
        let alive = self.with_scene(|s, _| s.contains(r.id));
        if r.epoch != self.epoch || !alive {
            Err(stale(r))
        } else {
            Ok(())
        }
    }

    // ---- wait ----

    /// Runs one frame. While a foreground wall-clock source is busy the virtual clock is held
    /// (AU7, Compose's `autoAdvance`): the frame runs, its time counts as wall time, not virtual.
    fn step(&mut self) {
        let hold = self.wall_work_busy();
        if hold != self.clock_held {
            self.driver.hold_clock(hold);
            self.clock_held = hold;
        }
        let t0 = Instant::now();
        self.driver.step();
        self.frame += 1;
        if hold {
            // the work runs on other threads: give it time rather than spinning frames
            std::thread::sleep(Duration::from_millis(1));
            self.held_wall += t0.elapsed();
        } else {
            self.now_ms += self.frame_ms();
        }
        self.pending_input = false;
        self.after_frame();
    }

    fn frame_ms(&self) -> f64 {
        let ms = self.driver.frame_ms();
        if ms > 0.0 { ms } else { 16.0 }
    }

    /// Why the app is not settled (`None`: settled): pending input, the embed, the foreground
    /// busy sources (AU7), the host predicate.
    fn not_settled(&self) -> Option<String> {
        if self.pending_input {
            return Some("INPUT".into());
        }
        if let Some(b) = self.driver.busy() {
            return Some(b);
        }
        let fg = self.foreground_busy();
        if !fg.is_empty() {
            return Some(fg.iter().map(|b| format!("{}×{}", b.id, b.count)).collect::<Vec<_>>().join(", "));
        }
        match &self.idle {
            Some(idle) if !idle() => Some("HOST".into()),
            _ => None,
        }
    }

    // ---- clock (AU1b) ----

    fn clock_json(&self) -> Value {
        let ms = self.driver.clock_ms();
        json!({"mode": if ms.is_some() { "virtual" } else { "real" }, "ms": ms, "seed": self.driver.seed(), "session_ms": self.now_ms})
    }

    /// `clock.now`: `{mode: "virtual" | "real", ms, seed, session_ms}`. `ms` is the virtual clock's
    /// time (`null` on the wall clock); `session_ms` the frame time this session ran.
    pub fn clock_now(&self) -> Result<Value, Error> {
        self.check(false)?;
        Ok(self.clock_json())
    }

    /// `clock.advance {ms}`: lets `ms` of virtual time pass, frame by frame (timers fire, jobs
    /// complete, animations run). Needs the `clock` grant and a virtual clock. Returns the frames run.
    pub fn advance_clock(&mut self, ms: f64) -> Result<u32, Error> {
        self.check(false)?;
        if !self.grants.clock {
            return Err(Error::new("grant.denied", "moving the clock needs the `clock` grant"));
        }
        if self.driver.clock_ms().is_none() {
            return Err(Error::new("capability.absent", "this app runs on the wall clock (start it with a virtual clock)"));
        }
        if !(ms.is_finite() && ms >= 0.0) {
            return Err(Error::new("params.invalid", "clock.advance wants ms >= 0"));
        }
        let (start, mut frames) = (self.now_ms, 0u32);
        while self.now_ms - start < ms {
            self.step();
            frames += 1;
        }
        Ok(frames)
    }

    /// `wait.settled` (§1.6): steps frames until the app is settled; returns the frames it ran.
    /// Fails with `timeout` and the chain of busy reasons after `timeout_ms` virtual ms (AU7: and
    /// a [`WaitReport`] naming every busy foreground source, longest first).
    pub fn wait_settled(&mut self, timeout_ms: f64) -> Result<u32, Error> {
        self.wait_settled_within(Budget::from(timeout_ms))
    }

    /// [`Session::wait_settled`] with a budget per clock domain (AU7).
    pub fn wait_settled_within(&mut self, budget: Budget) -> Result<u32, Error> {
        self.check(false)?;
        self.check_nevers()?;
        let d = self.deadline(budget, false);
        match self.settle(&d) {
            Ok(frames) => Ok(frames),
            Err(chain) => {
                let head = format!("not settled after {} ms: {}", budget.virtual_ms, chain.join(" → "));
                Err(self.timeout("timeout", head, "wait.settled".into(), &d, Vec::new(), None))
            }
        }
    }

    /// Steps until settled or `d` runs out; `Err`: the chain of busy reasons.
    fn settle(&mut self, d: &Deadline) -> Result<u32, Vec<String>> {
        let (mut frames, mut chain) = (0u32, Vec::<String>::new());
        loop {
            match self.not_settled() {
                None => return Ok(frames),
                Some(why) => {
                    if chain.last() != Some(&why) {
                        chain.push(why);
                    }
                }
            }
            if self.expired(d).is_some() {
                return Err(chain);
            }
            self.step();
            frames += 1;
        }
    }

    /// `wait.for` (§1.6): waits, settle by settle, until `selector`'s matches are in `state` and
    /// meet `expect`. Returns the matches.
    pub fn wait_for(
        &mut self,
        selector: &str,
        state: WaitState,
        expect: Option<&Expect>,
        timeout_ms: f64,
    ) -> Result<Vec<NodeInfo<N<D>>>, Error> {
        self.wait_for_within(&Condition::new(selector, state, expect.cloned()), Budget::from(timeout_ms))
    }

    /// [`Session::wait_for`] on a [`Condition`], with a budget per clock domain (AU7).
    pub fn wait_for_within(&mut self, cond: &Condition, budget: Budget) -> Result<Vec<NodeInfo<N<D>>>, Error> {
        self.check(false)?;
        let sel = parse(&cond.selector)?;
        let d = self.deadline(budget, false);
        let mut last = String::new();
        loop {
            self.check_nevers()?;
            // a busy app gets the rest of the time to settle; the check runs on settled frames
            if self.settle(&d).is_ok() {
                let (ok, why, infos) = self.evaluate(&sel, cond)?;
                if ok {
                    return Ok(infos);
                }
                last = why;
            }
            if self.expired(&d).is_some() {
                let unmet = vec![Unmet { condition: cond.describe(), last: last.clone() }];
                let head = format!("`{}` after {} ms: {last}", cond.selector, budget.virtual_ms);
                return Err(self.timeout("timeout", head, format!("wait.for {}", cond.describe()), &d, unmet, None));
            }
            self.step();
        }
    }

    /// Whether `cond` holds now: `(met, last value for reports, the matches)`.
    fn evaluate(&self, sel: &Selector, cond: &Condition) -> Result<(bool, String, Vec<NodeInfo<N<D>>>), Error> {
        let (state, expect) = (cond.state, cond.expect.as_ref());
        self.with_scene(|s, roots| -> Result<_, Error> {
            let ids = resolve(s, sel, roots)?;
            let nodes: Vec<N<D>> = ids.into_iter().filter(|&id| s.contains(id)).collect();
            let in_state = match state {
                WaitState::Attached => !nodes.is_empty(),
                WaitState::Detached => nodes.is_empty(),
                WaitState::Visible => !nodes.is_empty() && nodes.iter().all(|&n| s.is_shown(n)),
                WaitState::Hidden => nodes.iter().all(|&n| !s.is_shown(n)),
                WaitState::Enabled => !nodes.is_empty() && nodes.iter().all(|&n| s.has_state(n, "enabled")),
            };
            let (met, why) = match expect {
                None => (true, String::new()),
                Some(Expect::Count(c)) => (nodes.len() == *c, format!("count {} (want {c})", nodes.len())),
                Some(Expect::Text(t)) => {
                    let texts: Vec<String> = nodes.iter().map(|&n| text_of(s, n)).collect();
                    (texts.iter().any(|x| t.matches(x)), format!("texts {texts:?} (want {t:?})"))
                }
                Some(Expect::Value(t)) => {
                    let vals: Vec<Option<String>> = nodes.iter().map(|&n| s.value(n)).collect();
                    (vals.iter().flatten().any(|x| t.matches(x)), format!("values {vals:?} (want {t:?})"))
                }
            };
            let why = if in_state { why } else { format!("{} matches, not {state:?}; {why}", nodes.len()) };
            Ok((in_state && met, why, nodes.iter().map(|&n| self.info(s, n)).collect::<Vec<_>>()))
        })
    }

    // ---- act ----

    /// Resolves `target` (a selector, strictly, or a ref) and waits until it is actionable (§1.5,
    /// AU7 WAITS §3.4): attached, visible, stable (the same rect over two consecutive frames),
    /// enabled, editable (`fill`), and it (or a descendant or ancestor) is what `hit_test` finds at
    /// its actionable point. AX1: a node out of view (scrolled out of a `scroll-area`, a realised
    /// list row out of view, or partly clipped) is scrolled into view first, as a user would; the
    /// point is the centre of its part in view. Returns the node, its rect and that point. At the
    /// timeout the failing check is named ("still moving", "covered by …") in the error and its
    /// [`WaitReport`].
    fn actionable(
        &mut self,
        target: &Target<N<D>>,
        opts: &ActOptions,
        editable: bool,
    ) -> Result<(N<D>, Aim), Error> {
        if let Target::Invalid(e) = target {
            return Err(e.clone());
        }
        let d = self.deadline(Budget::from(opts.timeout_ms), false);
        let mut prev: Option<Rect> = None;
        let mut scrolled = false;
        let mut failed: Actionability<N<D>>;
        loop {
            self.check_nevers()?;
            let found: Result<Probe<N<D>>, Error> = self.with_scene(|s, roots| {
                let id = self.target_node(s, target, roots)?;
                let shown = s.drawn(id).filter(|sh| sh.rect.area() > 0.0);
                // out of view: wholly (not drawn this pass, below a culled container or row) or
                // partly (clipped); only once for a node larger than its view
                let out = s.is_visible(id)
                    && match shown {
                        None => s.is_shown(id),
                        Some(sh) => !scrolled && sh.visible.area() + 0.5 < sh.rect.area(),
                    };
                let check = if out {
                    Err(Actionability::new("visible", OUT_OF_VIEW))
                } else if !s.is_shown(id) || shown.is_none_or(|sh| sh.visible.area() <= 0.0) {
                    Err(Actionability::new("visible", "not visible"))
                } else if !s.has_state(id, "enabled") {
                    Err(Actionability::new("enabled", "disabled"))
                } else if editable && !is_editable(s, id) {
                    Err(Actionability::new("editable", format!("not editable (a {}{})", s.role(id), if s.has_state(id, "read-only") { ", read-only" } else { "" })))
                } else {
                    let c = shown.expect("checked").visible.center();
                    match s.hit(c) {
                        Some(h) if h != id && !is_ancestor(s, id, h) && !is_ancestor(s, h, id) => {
                            let mut a = Actionability::new(
                                "hit",
                                format!("covered by {} at ({:.0}, {:.0})", describe_node(s, h), c.x, c.y),
                            );
                            a.cover = Some(h);
                            Err(a)
                        }
                        _ => Ok(()),
                    }
                };
                Ok((id, shown.map(|sh| Aim { rect: sh.rect, at: sh.visible.center() }), check))
            });
            let (id, aim, check) = match found {
                Ok(f) => f,
                // a selector that does not match yet may match after the next settle
                Err(e) if e.code == "node.not_found" => {
                    failed = Actionability::new("attached", format!("not attached: {}", e.message));
                    if self.expired(&d).is_some() {
                        let head = e.message.clone();
                        return Err(self.timeout("timeout", head, format!("actionable {}", target.describe()), &d, Vec::new(), Some(failed)));
                    }
                    self.step();
                    continue;
                }
                Err(e) => return Err(e),
            };
            if opts.force {
                let rect = aim.map_or(Rect::NOTHING, |a| a.rect);
                return Ok((id, aim.unwrap_or(Aim { rect, at: rect.center() })));
            }
            let rect = aim.map(|a| a.rect);
            let stable = rect.is_some() && prev == rect;
            match check {
                Ok(()) if stable => return Ok((id, aim.expect("stable"))),
                Ok(()) => {
                    failed = match (prev, rect) {
                        (Some(p), Some(r)) => Actionability::new(
                            "stable",
                            format!("still moving: from {:?} to {:?} in the last frame", [p.min.x, p.min.y, p.width(), p.height()], [r.min.x, r.min.y, r.width(), r.height()]),
                        ),
                        _ => Actionability::new("stable", "still moving: first frame drawn"),
                    }
                }
                Err(a) => {
                    if a.detail == OUT_OF_VIEW {
                        self.request_scroll(id, false);
                        scrolled = true;
                    }
                    failed = a;
                }
            }
            prev = rect;
            if self.expired(&d).is_some() {
                let head = format!("{} after {} ms: {}", target.describe(), opts.timeout_ms, failed.detail);
                return Err(self.timeout("node.not_actionable", head, format!("actionable {}", target.describe()), &d, Vec::new(), Some(failed)));
            }
            self.step();
        }
    }

    /// AX1: asks the scrollables around `id` (every `scroll-area`, a `virtual-list` whose realised
    /// row it is in) to bring it into view at the next pass.
    fn request_scroll(&mut self, id: N<D>, center: bool) {
        self.driver.scroll_into_view(id, center);
        self.pending_input = true;
    }

    /// The AccessKit node drawn for `rect` (best overlap, at least half), for semantic acts.
    fn a11y_target(&self, rect: Rect) -> Option<A11yNode> {
        let iou = |a: Rect| {
            let i = a.intersect(rect);
            if i.is_negative() {
                return 0.0;
            }
            let inter = i.area();
            inter / (a.area() + rect.area() - inter).max(f32::EPSILON)
        };
        let mut best: Option<(f32, A11yNode)> = None;
        for n in self.driver.a11y_nodes() {
            let v = iou(n.rect);
            // later nodes are deeper: they win ties (the widget over its container)
            if v >= 0.5 && best.as_ref().is_none_or(|(b, _)| v >= *b) {
                best = Some((v, n));
            }
        }
        best.map(|(_, n)| n)
    }

    fn send(&mut self, e: Event) {
        self.driver.send(e);
        self.pending_input = true;
    }

    fn accesskit(&mut self, n: &A11yNode, action: Action, data: Option<ActionData>) {
        self.send(Event::AccessKit(ActionRequest {
            action,
            target_tree: n.tree,
            target_node: n.node,
            data,
        }));
    }

    fn pointer_click(&mut self, at: Pos2, button: PointerButton) {
        let m = Modifiers::NONE;
        self.send(Event::PointerMoved(at));
        self.send(Event::PointerButton {
            pos: at,
            button,
            pressed: true,
            modifiers: m,
        });
        self.send(Event::PointerButton {
            pos: at,
            button,
            pressed: false,
            modifiers: m,
        });
    }

    /// Runs `inject` after the target is actionable, then waits for the settle and reports what
    /// changed.
    fn act(
        &mut self,
        target: Target<N<D>>,
        opts: ActOptions,
        inject: impl FnOnce(&mut Self, N<D>, Aim) -> Tier,
    ) -> Result<ActResult<N<D>>, Error> {
        self.act_on(target, opts, false, inject)
    }

    /// [`Session::act`]; `editable`: the target must also take text (`fill`).
    fn act_on(
        &mut self,
        target: Target<N<D>>,
        opts: ActOptions,
        editable: bool,
        inject: impl FnOnce(&mut Self, N<D>, Aim) -> Tier,
    ) -> Result<ActResult<N<D>>, Error> {
        self.check(true)?;
        let (id, aim) = self.actionable(&target, &opts, editable)?;
        let obs = self.driver.observe(D::Filter::default());
        let tier = inject(self, id, aim);
        let settled = self.wait_settled(opts.timeout_ms);
        let set = self.driver.take_changes(&obs);
        settled?;
        Ok(self.act_result(tier, set))
    }

    fn act_result(&self, tier: Tier, set: Changes<N<D>>) -> ActResult<N<D>> {
        ActResult { frame: self.frame, epoch: self.epoch, tier, changed: set.by_node }
    }

    /// `input.click`: a click on the target (AccessKit `Click`, or a primary press and release at
    /// its centre in one frame).
    pub fn click(
        &mut self,
        target: impl Into<Target<N<D>>>,
        opts: ActOptions,
    ) -> Result<ActResult<N<D>>, Error> {
        self.act(target.into(), opts, |me, _, Aim { rect, at: aim }| {
            if opts.tier == Tier::Semantic
                && let Some(n) = me.a11y_target(rect)
            {
                me.accesskit(&n, Action::Click, None);
                return Tier::Semantic;
            }
            me.pointer_click(aim, PointerButton::Primary);
            Tier::Synthetic
        })
    }

    /// AX1 `input.click {button: "secondary"}`: a secondary press and release at the target's
    /// point (a right click: opens its `context-menu`, raises a list's `row-context`). Synthetic
    /// only.
    pub fn context_click(
        &mut self,
        target: impl Into<Target<N<D>>>,
        opts: ActOptions,
    ) -> Result<ActResult<N<D>>, Error> {
        self.act(target.into(), opts, |me, _, Aim { at, .. }| {
            me.pointer_click(at, PointerButton::Secondary);
            Tier::Synthetic
        })
    }

    /// AX1 `input.scroll_into_view`: scrolls every `scroll-area` around the target (and a
    /// `virtual-list` whose realised row it is in) until it is wholly in view, or as much of it
    /// as fits (`center`: centred). The target must be attached and visible (scrolled-out
    /// counts); it need not be enabled. Replies after the settle, with what changed. Acts do this
    /// themselves; call it to look at (snapshot, screenshot) a node out of view.
    pub fn scroll_into_view(
        &mut self,
        target: impl Into<Target<N<D>>>,
        center: bool,
        opts: ActOptions,
    ) -> Result<ActResult<N<D>>, Error> {
        self.check(true)?;
        let target = target.into();
        let obs = self.driver.observe(D::Filter::default());
        let start = self.now_ms;
        let mut asked = 0u32;
        let mut prev = None;
        loop {
            let probe = self.with_scene(|s, roots| -> Result<_, Error> {
                let id = self.target_node(s, &target, roots)?;
                Ok((id, s.is_shown(id), s.drawn(id).filter(|sh| sh.rect.area() > 0.0)))
            });
            let (id, visible, shown) = match probe {
                Err(e) if e.code == "node.not_found" && self.now_ms - start < opts.timeout_ms => {
                    self.step();
                    continue;
                }
                p => p?,
            };
            let whole = shown.is_some_and(|sh| sh.visible.area() + 0.5 >= sh.rect.area());
            // in view (or as much as fits, after a request), and still there a frame later
            if visible && shown.is_some() && (whole || asked > 0) && prev == shown.map(|sh| sh.rect) {
                break;
            }
            if visible && !whole && (asked == 0 || shown.is_none()) {
                self.request_scroll(id, center);
                asked += 1;
            }
            prev = shown.map(|sh| sh.rect);
            if self.now_ms - start >= opts.timeout_ms {
                return Err(Error::new(
                    "node.not_actionable",
                    format!("{} after {} ms: {}", target.describe(), opts.timeout_ms, if visible { "not in view" } else { "not visible" }),
                ));
            }
            self.step();
        }
        let settled = self.wait_settled(opts.timeout_ms);
        let set = self.driver.take_changes(&obs);
        settled?;
        Ok(self.act_result(Tier::Synthetic, set))
    }

    /// AX1 `input.scroll_into_view {row}`: scrolls `list` (a `virtual-list`, so a `ListView` or
    /// a `TreeView`) until position `pos` is in view and its row is realised; then its content
    /// can be found (`key=<item key> >> anchor=…`). Replies after the settle.
    pub fn scroll_to_row(
        &mut self,
        list: impl Into<Target<N<D>>>,
        pos: usize,
        opts: ActOptions,
    ) -> Result<ActResult<N<D>>, Error> {
        self.check(true)?;
        let target = list.into();
        let id = self.with_scene(|s, roots| self.target_node(s, &target, roots))?;
        let obs = self.driver.observe(D::Filter::default());
        self.driver.scroll_list_to(id, pos).map_err(|e| match e.code {
            "params.invalid" => Error::new(e.code, format!("{}: {}", target.describe(), e.message)),
            _ => e,
        })?;
        self.pending_input = true;
        // the offset is decided next pass, the rows bound in it and drawn in the one after
        self.step();
        let settled = self.wait_settled(opts.timeout_ms);
        let set = self.driver.take_changes(&obs);
        settled?;
        Ok(self.act_result(Tier::Synthetic, set))
    }

    /// `input.hover`: the pointer moves to the target's centre.
    pub fn hover(
        &mut self,
        target: impl Into<Target<N<D>>>,
        opts: ActOptions,
    ) -> Result<ActResult<N<D>>, Error> {
        self.act(target.into(), opts, |me, _, Aim { at: aim, .. }| {
            me.send(Event::PointerMoved(aim));
            Tier::Synthetic
        })
    }

    /// SV51 `input.pointer_move`: the pointer moves to `at` in the target (its local points), with
    /// `modifiers` held during the move. Synthetic only.
    pub fn pointer_move(
        &mut self,
        target: impl Into<Target<N<D>>>,
        at: At,
        modifiers: Modifiers,
        opts: ActOptions,
    ) -> Result<ActResult<N<D>>, Error> {
        self.act(target.into(), opts, |me, _, Aim { rect, at: aim }| {
            me.with_modifiers(modifiers, |me| me.send(Event::PointerMoved(at.in_rect(rect, aim))));
            Tier::Synthetic
        })
    }

    /// SV51 `input.wheel`: the pointer moves to the point, then one wheel event there (egui's
    /// `MouseWheel`: line, point or page deltas, with its modifiers). Synthetic only.
    pub fn wheel(
        &mut self,
        target: impl Into<Target<N<D>>>,
        wheel: Wheel,
        opts: ActOptions,
    ) -> Result<ActResult<N<D>>, Error> {
        self.act(target.into(), opts, |me, _, Aim { rect, at: aim }| {
            let p = wheel.at.in_rect(rect, aim);
            me.send(Event::PointerMoved(p));
            me.step(); // the wheel scrolls what is hovered
            me.with_modifiers(wheel.modifiers, |me| {
                me.send(Event::MouseWheel { unit: wheel.unit, delta: wheel.delta, modifiers: wheel.modifiers })
            });
            Tier::Synthetic
        })
    }

    /// SV51 `input.drag`: a press at `drag.from` in the target, `drag.steps` pointer moves on a
    /// straight line to `drag.to`, one frame each, and the release there, with `drag.modifiers`
    /// held throughout. Deterministic: frames run at the driver's step (the virtual clock).
    /// Synthetic only. A `DragTo::Node` end must resolve (to one node) when the drag starts.
    pub fn drag(
        &mut self,
        target: impl Into<Target<N<D>>>,
        drag: Drag<N<D>>,
        opts: ActOptions,
    ) -> Result<ActResult<N<D>>, Error> {
        let end = match &drag.to {
            DragTo::Local(_) => None,
            DragTo::Node(t) => Some(self.resolve_rect(t)?),
        };
        self.act(target.into(), opts, |me, _, Aim { rect, at: aim }| {
            let from = drag.from.in_rect(rect, aim);
            let to = match (&drag.to, end) {
                (DragTo::Local(p), _) => rect.min + p.to_vec2(),
                (_, Some(r)) => r.center(),
                (_, None) => from,
            };
            let (b, m) = (drag.button, drag.modifiers);
            if m != Modifiers::NONE {
                me.send(Event::ModifiersChanged(m));
            }
            me.send(Event::PointerMoved(from));
            me.step();
            me.send(Event::PointerButton { pos: from, button: b, pressed: true, modifiers: m });
            me.step();
            let n = drag.steps.max(1);
            for i in 1..=n {
                me.send(Event::PointerMoved(from.lerp(to, i as f32 / n as f32)));
                me.step();
            }
            me.send(Event::PointerButton { pos: to, button: b, pressed: false, modifiers: m });
            if m != Modifiers::NONE {
                me.send(Event::ModifiersChanged(Modifiers::NONE));
            }
            Tier::Synthetic
        })
    }

    /// `f`'s events with `m` held (a `ModifiersChanged` before and after, when not none).
    fn with_modifiers(&mut self, m: Modifiers, f: impl FnOnce(&mut Self)) {
        if m == Modifiers::NONE {
            return f(self);
        }
        self.send(Event::ModifiersChanged(m));
        f(self);
        self.send(Event::ModifiersChanged(Modifiers::NONE));
    }

    /// The screen rect of `target` now (one node), for a drag's end.
    fn resolve_rect(&self, target: &Target<N<D>>) -> Result<Rect, Error> {
        self.with_scene(|s, roots| {
            let id = self.target_node(s, target, roots)?;
            s.drawn(id)
                .map(|sh| sh.rect)
                .ok_or_else(|| Error::new("node.not_actionable", format!("{} is not drawn", target.describe())))
        })
    }

    /// `input.focus`: keyboard focus on the target (AccessKit `Focus`; synthetic: a click).
    pub fn focus(
        &mut self,
        target: impl Into<Target<N<D>>>,
        opts: ActOptions,
    ) -> Result<ActResult<N<D>>, Error> {
        self.act(target.into(), opts, |me, _, Aim { rect, at: aim }| {
            if let Some(n) = me.a11y_target(rect) {
                me.accesskit(&n, Action::Focus, None);
                return Tier::Semantic;
            }
            me.pointer_click(aim, PointerButton::Primary);
            Tier::Synthetic
        })
    }

    /// `input.fill`: replaces a text field's text. Semantic: AccessKit `SetValue`. Synthetic:
    /// focus by click, select all, then the text as one `Text` event (as typed).
    pub fn fill(
        &mut self,
        target: impl Into<Target<N<D>>>,
        text: &str,
        opts: ActOptions,
    ) -> Result<ActResult<N<D>>, Error> {
        let text = text.to_string();
        self.act_on(target.into(), opts, true, move |me, _, Aim { rect, at: aim }| {
            if opts.tier == Tier::Semantic
                && let Some(n) = me.a11y_target(rect)
            {
                me.accesskit(&n, Action::SetValue, Some(ActionData::Value(text.into())));
                return Tier::Semantic;
            }
            me.pointer_click(aim, PointerButton::Primary);
            me.step(); // focus lands before the keys
            me.key(Key::A, Modifiers::COMMAND);
            me.send(Event::Text(text));
            Tier::Synthetic
        })
    }

    /// `input.set_value`: a numeric value (slider, drag value) through AccessKit `SetValue`.
    pub fn set_value(
        &mut self,
        target: impl Into<Target<N<D>>>,
        value: f64,
        opts: ActOptions,
    ) -> Result<ActResult<N<D>>, Error> {
        self.act(target.into(), opts, move |me, _, Aim { rect, .. }| {
            // no AccessKit node: nothing to set, and the reply shows no change
            if let Some(n) = me.a11y_target(rect) {
                me.accesskit(&n, Action::SetValue, Some(ActionData::NumericValue(value)));
            }
            Tier::Semantic
        })
    }

    /// `input.set_value {handle}` (AU2b): one handle of a multi-handle widget (a `range-slider`
    /// is one scene node with two AccessKit sliders, "<name> low" and "<name> high"). `handle` is
    /// matched against the label's end (`"high"`), or is an index into the widget's slider handles
    /// in left-to-right order (`"1"`). Fails with `node.not_found` when no handle matches.
    pub fn set_value_handle(
        &mut self,
        target: impl Into<Target<N<D>>>,
        handle: &str,
        value: f64,
        opts: ActOptions,
    ) -> Result<ActResult<N<D>>, Error> {
        let mut missing = None;
        let r = self.act(target.into(), opts, |me, _, Aim { rect, .. }| {
            let area = rect.expand(2.0);
            let mut handles: Vec<A11yNode> = me
                .driver
                .a11y_nodes()
                .into_iter()
                .filter(|n| n.role == accesskit::Role::Slider && area.contains(n.rect.center()))
                .collect();
            handles.sort_by(|a, b| a.rect.left().total_cmp(&b.rect.left()).then(a.rect.top().total_cmp(&b.rect.top())));
            let want = handle.to_lowercase();
            let pick = handles
                .iter()
                .find(|n| n.label.as_deref().is_some_and(|l| l.to_lowercase().ends_with(&want)))
                .or_else(|| handle.parse::<usize>().ok().and_then(|i| handles.get(i)))
                .cloned();
            match pick {
                Some(n) => me.accesskit(&n, Action::SetValue, Some(ActionData::NumericValue(value))),
                None => {
                    let labels: Vec<_> = handles.iter().map(|n| n.label.clone().unwrap_or_default()).collect();
                    missing = Some(format!("no handle {handle:?} (handles: {labels:?})"));
                }
            }
            Tier::Semantic
        });
        match missing {
            Some(why) => Err(Error::new("node.not_found", why)),
            None => r,
        }
    }

    fn key(&mut self, key: Key, modifiers: Modifiers) {
        for pressed in [true, false] {
            self.send(Event::Key {
                key,
                pressed,
                repeat: false,
                modifiers,
            });
        }
    }

    /// `input.press`: a key chord to the focused widget, e.g. `"Enter"`, `"Ctrl+A"`,
    /// `"Shift+Tab"` (egui key names; `Cmd`/`Command` is Ctrl on Linux and Windows).
    pub fn press(&mut self, chord: &str) -> Result<ActResult<N<D>>, Error> {
        self.check(true)?;
        let (key, m) = parse_chord(chord)?;
        self.untargeted(move |me| me.key(key, m))
    }

    /// `input.type`: text to the focused widget, as typed (one `Text` event).
    pub fn type_text(&mut self, text: &str) -> Result<ActResult<N<D>>, Error> {
        self.check(true)?;
        let text = text.to_string();
        self.untargeted(move |me| me.send(Event::Text(text)))
    }

    fn untargeted(&mut self, inject: impl FnOnce(&mut Self)) -> Result<ActResult<N<D>>, Error> {
        let obs = self.driver.observe(D::Filter::default());
        inject(self);
        let settled = self.wait_settled(ActOptions::default().timeout_ms);
        let set = self.driver.take_changes(&obs);
        settled?;
        Ok(self.act_result(Tier::Synthetic, set))
    }

    // ---- capture ----

    /// `capture.screenshot`: the last frame, cropped to `target`'s screen rect when given.
    /// `capability.absent` on drivers without a renderer (kittest without wgpu).
    pub fn screenshot(&mut self, target: Option<&str>) -> Result<Image, Error> {
        self.check(false)?;
        let crop = match target {
            Some(sel) => Some(self.with_scene(|s, roots| {
                let sel = parse(sel)?;
                let id = strict(&sel, resolve(s, &sel, roots)?)?;
                s.screen_rect(id).ok_or_else(|| Error::new("node.not_actionable", "not drawn"))
            })?),
            None => None,
        };
        let img = self
            .driver
            .screenshot()
            .ok_or_else(|| Error::new("capability.absent", "this driver has no renderer"))?;
        Ok(match crop {
            Some(r) => img.region(&r, None),
            None => img,
        })
    }

    // ---- journal ----

    /// `journal.subscribe`: from now on, scene changes are collected for [`Session::journal_take`].
    pub fn journal_subscribe(&mut self, filter: D::Filter) -> Result<(), Error> {
        self.check(false)?;
        self.journal = Some(self.driver.observe(filter));
        Ok(())
    }

    /// `journal.take`: the changes since the last take, coalesced per node (`overflowed`: records
    /// were lost, re-read).
    pub fn journal_take(&mut self) -> Result<(Vec<(N<D>, String)>, bool), Error> {
        self.check(false)?;
        let obs = self
            .journal
            .as_ref()
            .ok_or_else(|| Error::new("params.invalid", "journal.subscribe first"))?;
        let set = self.driver.take_changes(obs);
        Ok((set.by_node, set.overflowed))
    }

    // ---- JSON ----

    /// A JSON target (`{"selector"}` or `{"ref"}`) as a ref: a selector must match one node.
    fn target_ref(&self, p: &Value) -> Result<Ref<N<D>>, Error> {
        match self.target_param(p)? {
            Target::Ref(r) => Ok(r),
            Target::Selector(s) => self.locate(&s.source),
            Target::Invalid(e) => Err(e),
        }
    }

    /// One method by name with JSON params (§1.1's schema; the in-process JSON path and, from AU2,
    /// the IPC transport). Targets are `{"selector": "…"}` or `{"ref": {"id": n, "epoch": e}}`.
    /// A JSON target: `{"ref": {"id", "epoch"}}` or `{"selector"}`.
    fn target_param(&self, p: &Value) -> Result<Target<N<D>>, Error> {
        if let Some(r) = p.get("ref") {
            let id = r
                .get("id")
                .and_then(Value::as_u64)
                .and_then(|raw| self.driver.node_from_raw(raw))
                .ok_or_else(|| Error::new("params.invalid", "`ref.id` wants a node id"))?;
            let epoch = r.get("epoch").and_then(Value::as_u64).unwrap_or(0);
            return Ok(Target::Ref(Ref { id, epoch }));
        }
        Ok(Target::Selector(parse(str_param(p, "selector")?)?))
    }

    fn act_json(&self, r: ActResult<N<D>>) -> Value {
        json!({
            "frame": r.frame, "epoch": r.epoch,
            "tier": match r.tier { Tier::Semantic => "semantic", Tier::Synthetic => "synthetic" },
            "changed": self.changes_json(&r.changed),
        })
    }

    fn changes_json(&self, changed: &[(N<D>, String)]) -> Value {
        json!(changed.iter().map(|&(n, ref m)| json!({"id": self.driver.node_raw(n), "what": m})).collect::<Vec<_>>())
    }

    pub fn call(&mut self, method: &str, p: &Value) -> Result<Value, Error> {
        let opts = || act_options(p);
        Ok(match method {
            "session.hello" => self.hello()?,
            "app.roots" => json!(self.roots()?.iter().map(|r| json!({"key": r.key, "visible": r.visible, "tooling": r.tooling})).collect::<Vec<_>>()),
            "scene.query" => {
                let limit = p.get("limit").and_then(Value::as_u64).map(|l| l as usize);
                let found = self.query(str_param(p, "selector")?, limit)?;
                json!(found.iter().map(|n| n.to_json_with(&self.raw())).collect::<Vec<_>>())
            }
            "scene.locate" => ref_json(self.locate(str_param(p, "selector")?)?, &self.raw()),
            "scene.node" => {
                let r = self.target_ref(p)?;
                self.node(r)?.to_json_with(&self.raw())
            }
            "style.matched" => {
                let r = self.target_ref(p)?;
                json!(self.style_matched(r)?.iter().map(rule_json).collect::<Vec<_>>())
            }
            "style.computed" => {
                let r = self.target_ref(p)?;
                self.style_computed(r)?.to_json_with(&self.raw())
            }
            "scene.snapshot" => json!({"format": "yaml", "text": self.snapshot(p.get("selector").and_then(Value::as_str))?}),
            "a11y.tree" => json!(
                self.a11y_tree()?
                    .iter()
                    .map(|n| json!({"role": format!("{:?}", n.role), "label": n.label, "author_id": n.author_id, "rect": [n.rect.min.x, n.rect.min.y, n.rect.width(), n.rect.height()]}))
                    .collect::<Vec<_>>()
            ),
            "input.click" => match p.get("button").and_then(Value::as_str).unwrap_or("primary") {
                "primary" | "left" => { let r = self.click(self.target_param(p)?, opts()?)?; self.act_json(r) },
                "secondary" | "right" => { let r = self.context_click(self.target_param(p)?, opts()?)?; self.act_json(r) },
                b => return Err(Error::new("params.invalid", format!("unknown button `{b}` (primary, secondary)"))),
            },
            "input.scroll_into_view" => match p.get("row") {
                None | Some(Value::Null) => {
                    let center = p.get("align").and_then(Value::as_str) == Some("center");
                    { let r = self.scroll_into_view(self.target_param(p)?, center, opts()?)?; self.act_json(r) }
                }
                Some(r) => {
                    let pos = r.as_u64().ok_or_else(|| Error::new("params.invalid", "`row` wants a position (0-based)"))?;
                    { let r = self.scroll_to_row(self.target_param(p)?, pos as usize, opts()?)?; self.act_json(r) }
                }
            },
            "input.hover" => { let r = self.hover(self.target_param(p)?, opts()?)?; self.act_json(r) },
            "input.focus" => { let r = self.focus(self.target_param(p)?, opts()?)?; self.act_json(r) },
            "input.pointer_move" => { let r = self.pointer_move(self.target_param(p)?, at_param(p, "at")?, modifiers_param(p)?, opts()?)?; self.act_json(r) },
            "input.wheel" => {
                let unit = match p.get("unit").and_then(Value::as_str).unwrap_or("line") {
                    "line" => MouseWheelUnit::Line,
                    "pixel" | "point" => MouseWheelUnit::Point,
                    "page" => MouseWheelUnit::Page,
                    u => return Err(Error::new("params.invalid", format!("unknown wheel unit `{u}` (line, pixel, page)"))),
                };
                let num = |k: &str| p.get(k).map_or(Ok(0.0), |v| v.as_f64().ok_or_else(|| Error::new("params.invalid", format!("`{k}` wants a number"))));
                let wheel = Wheel { at: at_param(p, "at")?, delta: vec2(num("dx")? as f32, num("dy")? as f32), unit, modifiers: modifiers_param(p)? };
                { let r = self.wheel(self.target_param(p)?, wheel, opts()?)?; self.act_json(r) }
            }
            "input.drag" => {
                let to = match (p.get("to"), p.get("to_selector"), p.get("to_ref")) {
                    (Some(_), None, None) => match at_param(p, "to")? {
                        At::Local(l) => DragTo::Local(l),
                        At::Center => return Err(Error::new("params.invalid", "`to` wants [x, y]")),
                    },
                    (None, Some(_), None) => DragTo::Node(Target::Selector(parse(str_param(p, "to_selector")?)?)),
                    (None, None, Some(_)) => DragTo::Node(self.target_param(&json!({"ref": p["to_ref"]}))?),
                    _ => return Err(Error::new("params.invalid", "input.drag wants one of `to` ([x, y] in the target), `to_selector` or `to_ref`")),
                };
                let button = match p.get("button").and_then(Value::as_str).unwrap_or("primary") {
                    "primary" | "left" => PointerButton::Primary,
                    "secondary" | "right" => PointerButton::Secondary,
                    "middle" => PointerButton::Middle,
                    b => return Err(Error::new("params.invalid", format!("unknown button `{b}`"))),
                };
                let steps = p.get("steps").and_then(Value::as_u64).unwrap_or(10) as u32;
                let drag = Drag { from: at_param(p, "from")?, to, steps: steps.max(1), button, modifiers: modifiers_param(p)? };
                { let r = self.drag(self.target_param(p)?, drag, opts()?)?; self.act_json(r) }
            }
            "input.fill" => { let r = self.fill(self.target_param(p)?, str_param(p, "text")?, opts()?)?; self.act_json(r) },
            "input.set_value" => {
                let v = p.get("value").and_then(Value::as_f64).ok_or_else(|| Error::new("params.invalid", "`value` wants a number"))?;
                match p.get("handle") {
                    Some(Value::String(h)) => { let r = self.set_value_handle(self.target_param(p)?, h, v, opts()?)?; self.act_json(r) },
                    Some(Value::Number(i)) => { let r = self.set_value_handle(self.target_param(p)?, &i.to_string(), v, opts()?)?; self.act_json(r) },
                    _ => { let r = self.set_value(self.target_param(p)?, v, opts()?)?; self.act_json(r) },
                }
            }
            "input.press" => { let r = self.press(str_param(p, "key")?)?; self.act_json(r) },
            "input.type" => { let r = self.type_text(str_param(p, "text")?)?; self.act_json(r) },
            "wait.settled" => json!({"frames": self.wait_settled_within(waits::budget(p))?}),
            "wait.for" => {
                let found = self.wait_for_within(&Condition::from_json(p)?, waits::budget(p))?;
                json!(found.iter().map(|n| n.to_json_with(&self.raw())).collect::<Vec<_>>())
            }
            "capture.screenshot" => {
                let img = self.screenshot(p.get("selector").and_then(Value::as_str))?;
                json!({"width": img.width(), "height": img.height()})
            }
            "clock.now" => self.clock_now()?,
            "clock.advance" => {
                let ms = p
                    .get("ms")
                    .and_then(Value::as_f64)
                    .ok_or_else(|| Error::new("params.invalid", "clock.advance wants `ms`"))?;
                let frames = self.advance_clock(ms)?;
                json!({"frames": frames, "ms": self.driver.clock_ms()})
            }
            "journal.subscribe" => {
                self.journal_subscribe(D::Filter::default())?;
                json!({})
            }
            "journal.take" => {
                let (changes, overflowed) = self.journal_take()?;
                json!({"changes": self.changes_json(&changes), "overflowed": overflowed})
            }
            m if waits::METHODS.contains(&m) => self.call_waits(m, p)?,
            m => return Err(Error::new("method.unknown", format!("`{m}`"))),
        })
    }
}

/// One actionability probe: the node, where to aim, and the check it fails.
type Probe<N> = (N, Option<Aim>, Result<(), Actionability<N>>);

/// [`crate::engine::resolve`] on a session's scene.
fn resolve<S: SceneSource>(s: &S, sel: &Selector, roots: &Roots<'_>) -> Result<Vec<S::Node>, Error> {
    crate::engine::resolve_in(s, sel, roots.skip)
}

/// Whether `a` is an ancestor of `n`.
fn is_ancestor<S: SceneSource>(s: &S, a: S::Node, n: S::Node) -> bool {
    let mut at = s.parent(n);
    while let Some(p) = at {
        if p == a {
            return true;
        }
        at = s.parent(p);
    }
    false
}

/// A scene without a style engine (`style.*`).
fn no_style() -> Error {
    Error::new("capability.absent", "this scene has no style engine")
}

/// AX1: the probe's detail for a node out of view (it is scrolled in, then probed again).
const OUT_OF_VIEW: &str = "out of view";

/// `fill`'s editable check: not read-only, and not a role that never takes text.
fn is_editable<S: SceneSource>(s: &S, n: S::Node) -> bool {
    !s.has_state(n, "read-only")
        && !matches!(
            s.role(n),
            "button" | "checkbox" | "radio" | "switch" | "label" | "link" | "img" | "separator" | "slider" | "progressbar" | "menu" | "tab"
        )
}

/// `button #12 "Overlay" (anchor=cover)`, for reports.
fn describe_node<S: SceneSource>(s: &S, n: S::Node) -> String
where
    S::Node: std::fmt::Debug,
{
    let mut out = format!("{} {n:?}", s.tag(n));
    let name = s.name(n);
    if !name.is_empty() {
        out.push_str(&format!(" {name:?}"));
    }
    if let Some(a) = s.anchor(n) {
        out.push_str(&format!(" (anchor={a})"));
    }
    out
}

/// What an act aims at: a selector (resolved strictly before each try) or an element ref.
#[derive(Clone, Debug)]
pub enum Target<N = DefaultNode> {
    Selector(Selector),
    Ref(Ref<N>),
    /// A selector string that did not parse; the act fails with its error.
    Invalid(Error),
}

impl<N: std::fmt::Debug> Target<N> {
    fn describe(&self) -> String {
        match self {
            Target::Selector(s) => format!("`{}`", s.source),
            Target::Ref(r) => format!("{:?} (epoch {})", r.id, r.epoch),
            Target::Invalid(e) => e.message.clone(),
        }
    }
}

impl<N> From<Ref<N>> for Target<N> {
    fn from(r: Ref<N>) -> Target<N> {
        Target::Ref(r)
    }
}

impl<N> From<Selector> for Target<N> {
    fn from(s: Selector) -> Target<N> {
        Target::Selector(s)
    }
}

/// A selector string; an invalid one fails at the act with `selector.invalid`.
impl<N> From<&str> for Target<N> {
    fn from(s: &str) -> Target<N> {
        parse(s).map_or_else(Target::Invalid, Target::Selector)
    }
}

fn text_of<S: SceneSource>(s: &S, n: S::Node) -> String {
    let t = s.text(n);
    if !t.is_empty() {
        t.into_owned()
    } else {
        s.name(n).into_owned()
    }
}

fn strict<N: Copy>(sel: &Selector, ids: Vec<N>) -> Result<N, Error> {
    match ids.as_slice() {
        [] => Err(Error::new(
            "node.not_found",
            format!("no node matches `{}`", sel.source),
        )),
        [id] => Ok(*id),
        many => Err(Error::new(
            "node.ambiguous",
            format!(
                "{} nodes match `{}`; add `>> nth=` or narrow the scope",
                many.len(),
                sel.source
            ),
        )),
    }
}

fn stale<N: std::fmt::Debug>(r: Ref<N>) -> Error {
    Error::new(
        "node.stale",
        format!(
            "{:?} from epoch {} is gone; re-resolve its locator",
            r.id, r.epoch
        ),
    )
}

fn disabled() -> Error {
    Error::new(
        "automation.disabled",
        "automation is off (svelters_automation::enable)",
    )
}

fn ref_json<N: Copy>(r: Ref<N>, raw: RawFn<'_, N>) -> Value {
    json!({"id": raw(r.id), "epoch": r.epoch})
}

fn str_param<'v>(p: &'v Value, k: &str) -> Result<&'v str, Error> {
    p.get(k)
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("params.invalid", format!("`{k}` wants a string")))
}

/// SV51: `[x, y]` in the target's local points under `k`; absent: its centre.
fn at_param(p: &Value, k: &str) -> Result<At, Error> {
    match p.get(k) {
        None | Some(Value::Null) => Ok(At::Center),
        Some(Value::Array(a)) if a.len() == 2 && a.iter().all(Value::is_number) => {
            Ok(At::Local(pos2(a[0].as_f64().unwrap_or(0.0) as f32, a[1].as_f64().unwrap_or(0.0) as f32)))
        }
        _ => Err(Error::new("params.invalid", format!("`{k}` wants [x, y] (local points of the target)"))),
    }
}

/// SV51: `modifiers`: `"Shift+Ctrl"` (the names of `input.press`'s chords) or `["Shift", "Ctrl"]`.
fn modifiers_param(p: &Value) -> Result<Modifiers, Error> {
    let names: Vec<String> = match p.get("modifiers") {
        None | Some(Value::Null) => return Ok(Modifiers::NONE),
        Some(Value::String(s)) => s.split('+').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect(),
        Some(Value::Array(a)) => a.iter().map(|v| v.as_str().unwrap_or("?").to_string()).collect(),
        _ => return Err(Error::new("params.invalid", "`modifiers` wants \"Shift+Ctrl\" or [\"Shift\", \"Ctrl\"]")),
    };
    let mut m = Modifiers::NONE;
    for n in names {
        match n.as_str() {
            "Ctrl" | "Control" => m.ctrl = true,
            "Cmd" | "Command" | "Mod" => m |= Modifiers::COMMAND,
            "Shift" => m.shift = true,
            "Alt" | "Option" => m.alt = true,
            x => return Err(Error::new("params.invalid", format!("unknown modifier `{x}`"))),
        }
    }
    Ok(m)
}

fn timeout(p: &Value) -> f64 {
    p.get("timeout").and_then(Value::as_f64).unwrap_or(5_000.0)
}

fn act_options(p: &Value) -> Result<ActOptions, Error> {
    let tier = match p.get("tier").and_then(Value::as_str) {
        None | Some("synthetic") => Tier::Synthetic,
        Some("semantic") => Tier::Semantic,
        Some(t) => {
            return Err(Error::new(
                "params.invalid",
                format!("unknown tier `{t}` (os input is AU6)"),
            ));
        }
    };
    Ok(ActOptions {
        tier,
        force: p.get("force").and_then(Value::as_bool).unwrap_or(false),
        timeout_ms: timeout(p),
    })
}

fn expect_param(e: &Value) -> Result<Expect, Error> {
    let tm = |v: &Value| -> Result<TextMatch, Error> {
        let s = v
            .as_str()
            .ok_or_else(|| Error::new("params.invalid", "expect text wants a string"))?;
        Ok(if e.get("exact").and_then(Value::as_bool).unwrap_or(true) {
            TextMatch::Exact(s.to_string())
        } else {
            TextMatch::Contains(s.to_string())
        })
    };
    if let Some(c) = e.get("count").and_then(Value::as_u64) {
        return Ok(Expect::Count(c as usize));
    }
    if let Some(t) = e.get("text") {
        return Ok(Expect::Text(tm(t)?));
    }
    if let Some(v) = e.get("value") {
        return Ok(Expect::Value(tm(v)?));
    }
    Err(Error::new(
        "params.invalid",
        "expect wants count, text or value",
    ))
}

fn parse_chord(chord: &str) -> Result<(Key, Modifiers), Error> {
    let mut m = Modifiers::NONE;
    let mut key = None;
    for part in chord.split('+') {
        match part.trim() {
            "Ctrl" | "Control" => m.ctrl = true,
            "Cmd" | "Command" | "Mod" => m |= Modifiers::COMMAND,
            "Shift" => m.shift = true,
            "Alt" | "Option" => m.alt = true,
            k => {
                key = Some(Key::from_name(k).ok_or_else(|| {
                    Error::new("params.invalid", format!("unknown key `{k}` in `{chord}`"))
                })?)
            }
        }
    }
    if m.ctrl && !m.command && cfg!(not(target_os = "macos")) {
        m.command = true;
    }
    Ok((
        key.ok_or_else(|| Error::new("params.invalid", format!("no key in `{chord}`")))?,
        m,
    ))
}
