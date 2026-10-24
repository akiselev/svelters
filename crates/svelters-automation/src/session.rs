//! [`Session`]: one client's view of the app: grants, tooling roots, the host's idle predicate,
//! element refs with their epoch, and every method (Rust API and [`Session::call`]).

use crate::driver::{A11yNode, Driver};
use crate::engine::{Roots, resolve};
use crate::role::{has_state, role_of, states_of};
use crate::selector::{Selector, TextMatch, parse};
use crate::snapshot::{value_of, yaml};
use crate::{Error, PROTOCOL, is_enabled};
use egui::accesskit::{Action, ActionData, ActionRequest};
use serde_json::{Value, json};
use svelters_egui::accessible_name;
use svelters_egui::scene::{ChangeFilter, NodeId, NodeRef, Observer, SceneRef};

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
/// or to a freed node, fails with `node.stale`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ref {
    pub id: NodeId,
    pub epoch: u64,
}

/// One node as `scene.query` returns it.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeInfo {
    pub r#ref: Ref,
    pub tag: String,
    pub role: &'static str,
    pub name: String,
    pub text: String,
    pub value: Option<String>,
    pub anchor: Option<String>,
    pub root: Option<String>,
    /// Screen rect in egui points (`None`: not drawn).
    pub rect: Option<egui::Rect>,
    pub states: Vec<&'static str>,
}

impl NodeInfo {
    pub fn to_json(&self) -> Value {
        json!({
            "ref": ref_json(self.r#ref),
            "tag": self.tag, "role": self.role, "name": self.name, "text": self.text, "value": self.value,
            "anchor": self.anchor, "root": self.root,
            "rect": self.rect.map(|r| json!([r.min.x, r.min.y, r.width(), r.height()])),
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
    Local(egui::Pos2),
}

impl At {
    fn in_rect(self, rect: egui::Rect) -> egui::Pos2 {
        match self {
            At::Center => rect.center(),
            At::Local(p) => rect.min + p.to_vec2(),
        }
    }
}

/// SV51: where a drag ends.
#[derive(Clone, Debug)]
pub enum DragTo {
    /// A point in the target's local points.
    Local(egui::Pos2),
    /// The centre of another node (resolved when the drag starts).
    Node(Target),
}

/// SV51: `input.drag`: a press at `from`, `steps` moves (one frame each) to `to`, a release there.
#[non_exhaustive]
#[derive(Clone, Debug)]
pub struct Drag {
    pub from: At,
    pub to: DragTo,
    /// Moves between the press and the release (at least 1), each in its own frame.
    pub steps: u32,
    pub button: egui::PointerButton,
    /// Held for the whole drag (`ModifiersChanged` before the press, released after).
    pub modifiers: egui::Modifiers,
}

impl Drag {
    /// From the target's centre to `to` (its local points), 10 steps, primary button.
    pub fn to(to: egui::Pos2) -> Drag {
        Drag::new(DragTo::Local(to))
    }
    /// From the target's centre to `node`'s centre.
    pub fn to_node(node: impl Into<Target>) -> Drag {
        Drag::new(DragTo::Node(node.into()))
    }
    fn new(to: DragTo) -> Drag {
        Drag { from: At::Center, to, steps: 10, button: egui::PointerButton::Primary, modifiers: egui::Modifiers::NONE }
    }
    pub fn from(mut self, at: At) -> Drag {
        self.from = at;
        self
    }
    pub fn steps(mut self, n: u32) -> Drag {
        self.steps = n.max(1);
        self
    }
    pub fn button(mut self, b: egui::PointerButton) -> Drag {
        self.button = b;
        self
    }
    pub fn modifiers(mut self, m: egui::Modifiers) -> Drag {
        self.modifiers = m;
        self
    }
}

/// SV51: `input.wheel`: a scroll at a point of the target.
#[non_exhaustive]
#[derive(Clone, Copy, Debug)]
pub struct Wheel {
    pub at: At,
    /// egui's wheel delta (positive `y` scrolls up, as egui sends it), in `unit`s.
    pub delta: egui::Vec2,
    /// `Line` (a mouse wheel's notches), `Point` (a trackpad's pixels) or `Page`.
    pub unit: egui::MouseWheelUnit,
    pub modifiers: egui::Modifiers,
}

impl Wheel {
    /// `lines` notches (positive `y`: up) at the target's centre.
    pub fn lines(lines: egui::Vec2) -> Wheel {
        Wheel { at: At::Center, delta: lines, unit: egui::MouseWheelUnit::Line, modifiers: egui::Modifiers::NONE }
    }
    /// `points` of trackpad scroll (positive `y`: up) at the target's centre.
    pub fn points(points: egui::Vec2) -> Wheel {
        Wheel { at: At::Center, delta: points, unit: egui::MouseWheelUnit::Point, modifiers: egui::Modifiers::NONE }
    }
    pub fn at(mut self, at: At) -> Wheel {
        self.at = at;
        self
    }
    pub fn modifiers(mut self, m: egui::Modifiers) -> Wheel {
        self.modifiers = m;
        self
    }
}

/// What an act did, read after the app settled.
#[derive(Clone, Debug, PartialEq)]
pub struct ActResult {
    pub frame: u64,
    pub epoch: u64,
    /// The tier that ran (a semantic act without an AccessKit node runs synthetic).
    pub tier: Tier,
    /// Nodes that changed, with what changed (`ChangeMask` names).
    pub changed: Vec<(NodeId, String)>,
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

/// One client of the protocol, over a [`Driver`].
pub struct Session<D: Driver> {
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
    journal: Option<Observer>,
}

impl<D: Driver> Session<D> {
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
        })
    }

    /// Tests: turns automation on and opens a session with every grant.
    pub fn for_test(driver: D) -> Session<D> {
        crate::enable();
        Session::new(driver, Grants::ALL).expect("enabled")
    }

    /// The host's idle predicate (§6, agreed with cadatomic): waits never answer while it is
    /// `false` (cadatomic's IdleBarrier: jobs, sessions, `BusyProbe`).
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

    fn with_scene<R>(&self, f: impl FnOnce(SceneRef<'_>, &Roots<'_>) -> R) -> R {
        let skip_tooling = !self.include_tooling;
        let tooling = &self.tooling;
        let skip = move |k: &str| skip_tooling && tooling(k);
        let roots = Roots { skip: &skip };
        self.driver.scene().read(|s| f(s, &roots))
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
            "capabilities": ["scene.query", "scene.node", "scene.snapshot", "a11y.tree", "app.roots", "input.click", "input.focus",
                "input.fill", "input.set_value", "input.press", "input.type", "input.hover", "input.pointer_move", "input.wheel", "input.drag", "wait.settled", "wait.for",
                "journal.subscribe", "journal.take", "capture.screenshot", "clock.now", "clock.advance"],
            "clock": self.clock_json(),
        }))
    }

    /// `app.roots`: the mounted roots (tooling roots only with `include_tooling`).
    pub fn roots(&self) -> Result<Vec<RootInfo>, Error> {
        self.check(false)?;
        Ok(self.with_scene(|s, roots| {
            s.roots()
                .filter(|&(r, _)| !r.is_detached() && s.root_top(r).is_some())
                .filter_map(|(r, _)| {
                    let key = s.root_key(r);
                    (!key.is_some_and(roots.skip)).then(|| RootInfo {
                        key: key.map(str::to_string),
                        visible: s.root_visible(r),
                        tooling: key.is_some_and(|k| (self.tooling)(k)),
                    })
                })
                .collect()
        }))
    }

    // ---- query and read ----

    /// The nodes `selector` matches (§1.4 `scene.query`), at most `limit`.
    pub fn query(&self, selector: &str, limit: Option<usize>) -> Result<Vec<NodeInfo>, Error> {
        self.check(false)?;
        let sel = parse(selector)?;
        self.with_scene(|s, roots| {
            let ids = resolve(s, &sel, roots)?;
            Ok(ids
                .into_iter()
                .take(limit.unwrap_or(usize::MAX))
                .filter_map(|id| s.get(id).map(|n| self.info(s, n)))
                .collect())
        })
    }

    /// The single node `selector` matches (strict, §1.3): `node.not_found` for none,
    /// `node.ambiguous` for more than one.
    pub fn locate(&self, selector: &str) -> Result<Ref, Error> {
        self.check(false)?;
        let sel = parse(selector)?;
        let id = self.with_scene(|s, roots| strict(&sel, resolve(s, &sel, roots)?))?;
        Ok(Ref {
            id,
            epoch: self.epoch,
        })
    }

    /// The node behind `r` (§1.4 `scene.node`).
    pub fn node(&self, r: Ref) -> Result<NodeInfo, Error> {
        self.check(false)?;
        self.live(r)?;
        self.with_scene(|s, _| s.get(r.id).map(|n| self.info(s, n)))
            .ok_or_else(|| stale(r))
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
                        if let Some(n) = s.get(id) {
                            yaml(n, 0, &mut out);
                        }
                    }
                }
                None => {
                    for (r, _) in s.roots() {
                        if r.is_detached()
                            || s.root_key(r).is_some_and(roots.skip)
                            || !s.root_visible(r)
                        {
                            continue;
                        }
                        if let Some(top) = s.root_top(r) {
                            yaml(top, 0, &mut out);
                        }
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

    fn info(&self, s: SceneRef<'_>, n: NodeRef<'_>) -> NodeInfo {
        NodeInfo {
            r#ref: Ref {
                id: n.id(),
                epoch: self.epoch,
            },
            tag: n.tag().to_string(),
            role: role_of(n),
            name: accessible_name(n),
            text: n.text().to_string(),
            value: value_of(n),
            anchor: n.anchor().map(str::to_string),
            root: s.root_key(n.root()).map(str::to_string),
            rect: n.screen_rect().map(|(_, r)| r),
            states: states_of(n),
        }
    }

    fn live(&self, r: Ref) -> Result<(), Error> {
        let alive = self.with_scene(|s, _| s.get(r.id).is_some());
        if r.epoch != self.epoch || !alive {
            Err(stale(r))
        } else {
            Ok(())
        }
    }

    // ---- wait ----

    fn step(&mut self) {
        self.driver.step();
        self.frame += 1;
        self.now_ms += self.frame_ms();
        self.pending_input = false;
    }

    fn frame_ms(&self) -> f64 {
        let ms = self.driver.frame_ms();
        if ms > 0.0 { ms } else { 16.0 }
    }

    /// Why the app is not settled (`None`: settled): the embed, pending input, the host predicate.
    fn not_settled(&self) -> Option<String> {
        if self.pending_input {
            return Some("INPUT".into());
        }
        if let Some(b) = self.driver.busy() {
            return Some(b);
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
    /// Fails with `timeout` and the chain of busy reasons after `timeout_ms` virtual ms.
    pub fn wait_settled(&mut self, timeout_ms: f64) -> Result<u32, Error> {
        self.check(false)?;
        let (start, mut frames, mut chain) = (self.now_ms, 0u32, Vec::<String>::new());
        loop {
            match self.not_settled() {
                None => return Ok(frames),
                Some(why) => {
                    if chain.last() != Some(&why) {
                        chain.push(why);
                    }
                }
            }
            if self.now_ms - start >= timeout_ms {
                return Err(Error::new(
                    "timeout",
                    format!("not settled after {timeout_ms} ms: {}", chain.join(" → ")),
                ));
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
    ) -> Result<Vec<NodeInfo>, Error> {
        self.check(false)?;
        let sel = parse(selector)?;
        let start = self.now_ms;
        let mut last = String::new();
        loop {
            let left = (timeout_ms - (self.now_ms - start)).max(0.0);
            // a busy app gets the rest of the time to settle; the check runs on settled frames
            let settled = self.wait_settled(left).is_ok();
            if settled {
                let (ok, why, infos) = self.with_scene(|s, roots| -> Result<_, Error> {
                    let ids = resolve(s, &sel, roots)?;
                    let nodes: Vec<NodeRef<'_>> = ids.iter().filter_map(|&id| s.get(id)).collect();
                    let in_state = match state {
                        WaitState::Attached => !nodes.is_empty(),
                        WaitState::Detached => nodes.is_empty(),
                        WaitState::Visible => {
                            !nodes.is_empty() && nodes.iter().all(|n| n.is_visible())
                        }
                        WaitState::Hidden => nodes.iter().all(|n| !n.is_visible()),
                        WaitState::Enabled => {
                            !nodes.is_empty() && nodes.iter().all(|&n| has_state(n, "enabled"))
                        }
                    };
                    let (met, why) = match expect {
                        None => (true, String::new()),
                        Some(Expect::Count(c)) => (
                            nodes.len() == *c,
                            format!("count {} (want {c})", nodes.len()),
                        ),
                        Some(Expect::Text(t)) => {
                            let texts: Vec<String> = nodes.iter().map(|&n| text_of(n)).collect();
                            (
                                texts.iter().any(|x| t.matches(x)),
                                format!("texts {texts:?} (want {t:?})"),
                            )
                        }
                        Some(Expect::Value(t)) => {
                            let vals: Vec<Option<String>> =
                                nodes.iter().map(|&n| value_of(n)).collect();
                            (
                                vals.iter().flatten().any(|x| t.matches(x)),
                                format!("values {vals:?} (want {t:?})"),
                            )
                        }
                    };
                    let why = if in_state {
                        why
                    } else {
                        format!("{} matches, not {state:?}; {why}", nodes.len())
                    };
                    Ok((
                        in_state && met,
                        why,
                        nodes.iter().map(|&n| self.info(s, n)).collect::<Vec<_>>(),
                    ))
                })?;
                if ok {
                    return Ok(infos);
                }
                last = why;
            }
            if self.now_ms - start >= timeout_ms {
                return Err(Error::new(
                    "timeout",
                    format!("`{selector}` after {timeout_ms} ms: {last}"),
                ));
            }
            self.step();
        }
    }

    // ---- act ----

    /// Resolves `target` (a selector, strictly, or a ref) and waits until it is actionable (§1.5):
    /// attached, visible, stable over two frames, enabled, and it (or a descendant or ancestor)
    /// is what `hit_test` finds at its centre. Returns the node and its centre.
    fn actionable(
        &mut self,
        target: &Target,
        opts: &ActOptions,
    ) -> Result<(NodeId, egui::Rect), Error> {
        if let Target::Invalid(e) = target {
            return Err(e.clone());
        }
        let start = self.now_ms;
        let mut prev: Option<egui::Rect> = None;
        let mut why;
        loop {
            let found: Result<Probe, Error> = self.with_scene(|s, roots| {
                let id = match target {
                    Target::Selector(sel) => strict(sel, resolve(s, sel, roots)?)?,
                    Target::Invalid(e) => return Err(e.clone()),
                    Target::Ref(r) => {
                        if r.epoch != self.epoch || s.get(r.id).is_none() {
                            return Err(stale(*r));
                        }
                        r.id
                    }
                };
                let n = s.get(id).expect("resolved");
                let rect = node_rect(s, n);
                let check = if !n.is_visible() || rect.is_none_or(|r| r.area() <= 0.0) {
                    Err("not visible".to_string())
                } else if !has_state(n, "enabled") {
                    Err("disabled".to_string())
                } else {
                    let c = rect.expect("checked").center();
                    match s.hit_node(egui::ViewportId::ROOT, c) {
                        Some(h)
                            if h.id() != id
                                && !h.ancestors().any(|a| a.id() == id)
                                && !n.ancestors().any(|a| a.id() == h.id()) =>
                        {
                            Err(format!(
                                "{} {} receives the events at {c:?}",
                                h.tag(),
                                h.id()
                            ))
                        }
                        _ => Ok(()),
                    }
                };
                Ok((id, rect, check))
            });
            let (id, rect, check) = match found {
                Ok(f) => f,
                // a selector that does not match yet may match after the next settle
                Err(e) if e.code == "node.not_found" => {
                    why = e.message;
                    if self.now_ms - start >= opts.timeout_ms {
                        return Err(Error::new("timeout", why));
                    }
                    self.step();
                    continue;
                }
                Err(e) => return Err(e),
            };
            if opts.force {
                return Ok((id, rect.unwrap_or(egui::Rect::NOTHING)));
            }
            let stable = rect.is_some() && prev == rect;
            match check {
                Ok(()) if stable => return Ok((id, rect.expect("stable"))),
                Ok(()) => why = "not stable yet".to_string(),
                Err(w) => why = w,
            }
            prev = rect;
            if self.now_ms - start >= opts.timeout_ms {
                return Err(Error::new(
                    "node.not_actionable",
                    format!("{} after {} ms: {why}", target.describe(), opts.timeout_ms),
                ));
            }
            self.step();
        }
    }

    /// The AccessKit node drawn for `rect` (best overlap, at least half), for semantic acts.
    fn a11y_target(&self, rect: egui::Rect) -> Option<A11yNode> {
        let iou = |a: egui::Rect| {
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

    fn send(&mut self, e: egui::Event) {
        self.driver.send(e);
        self.pending_input = true;
    }

    fn accesskit(&mut self, n: &A11yNode, action: Action, data: Option<ActionData>) {
        self.send(egui::Event::AccessKitActionRequest(ActionRequest {
            action,
            target_tree: n.tree,
            target_node: n.node,
            data,
        }));
    }

    fn pointer_click(&mut self, at: egui::Pos2, button: egui::PointerButton) {
        let m = egui::Modifiers::NONE;
        self.send(egui::Event::PointerMoved(at));
        self.send(egui::Event::PointerButton {
            pos: at,
            button,
            pressed: true,
            modifiers: m,
        });
        self.send(egui::Event::PointerButton {
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
        target: Target,
        opts: ActOptions,
        inject: impl FnOnce(&mut Self, NodeId, egui::Rect) -> Tier,
    ) -> Result<ActResult, Error> {
        self.check(true)?;
        let (id, rect) = self.actionable(&target, &opts)?;
        let scene = self.driver.scene();
        let obs = scene.observe(ChangeFilter::all());
        let tier = inject(self, id, rect);
        let settled = self.wait_settled(opts.timeout_ms);
        let set = scene.take_changes(&obs);
        settled?;
        Ok(ActResult {
            frame: self.frame,
            epoch: self.epoch,
            tier,
            changed: set.by_node().map(|(n, m)| (n, format!("{m:?}"))).collect(),
        })
    }

    /// `input.click`: a click on the target (AccessKit `Click`, or a primary press and release at
    /// its centre in one frame).
    pub fn click(
        &mut self,
        target: impl Into<Target>,
        opts: ActOptions,
    ) -> Result<ActResult, Error> {
        self.act(target.into(), opts, |me, _, rect| {
            if opts.tier == Tier::Semantic
                && let Some(n) = me.a11y_target(rect)
            {
                me.accesskit(&n, Action::Click, None);
                return Tier::Semantic;
            }
            me.pointer_click(rect.center(), egui::PointerButton::Primary);
            Tier::Synthetic
        })
    }

    /// `input.hover`: the pointer moves to the target's centre.
    pub fn hover(
        &mut self,
        target: impl Into<Target>,
        opts: ActOptions,
    ) -> Result<ActResult, Error> {
        self.act(target.into(), opts, |me, _, rect| {
            me.send(egui::Event::PointerMoved(rect.center()));
            Tier::Synthetic
        })
    }

    /// SV51 `input.pointer_move`: the pointer moves to `at` in the target (its local points), with
    /// `modifiers` held during the move. Synthetic only.
    pub fn pointer_move(
        &mut self,
        target: impl Into<Target>,
        at: At,
        modifiers: egui::Modifiers,
        opts: ActOptions,
    ) -> Result<ActResult, Error> {
        self.act(target.into(), opts, |me, _, rect| {
            me.with_modifiers(modifiers, |me| me.send(egui::Event::PointerMoved(at.in_rect(rect))));
            Tier::Synthetic
        })
    }

    /// SV51 `input.wheel`: the pointer moves to the point, then one wheel event there (egui's
    /// `MouseWheel`: line, point or page deltas, with its modifiers). Synthetic only.
    pub fn wheel(
        &mut self,
        target: impl Into<Target>,
        wheel: Wheel,
        opts: ActOptions,
    ) -> Result<ActResult, Error> {
        self.act(target.into(), opts, |me, _, rect| {
            let p = wheel.at.in_rect(rect);
            me.send(egui::Event::PointerMoved(p));
            me.step(); // the wheel scrolls what is hovered
            me.with_modifiers(wheel.modifiers, |me| {
                me.send(egui::Event::MouseWheel {
                    unit: wheel.unit,
                    delta: wheel.delta,
                    phase: egui::TouchPhase::Move,
                    source: egui::MouseWheelSource::Unknown,
                    modifiers: wheel.modifiers,
                })
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
        target: impl Into<Target>,
        drag: Drag,
        opts: ActOptions,
    ) -> Result<ActResult, Error> {
        let end = match &drag.to {
            DragTo::Local(_) => None,
            DragTo::Node(t) => Some(self.resolve_rect(t)?),
        };
        self.act(target.into(), opts, |me, _, rect| {
            let from = drag.from.in_rect(rect);
            let to = match (&drag.to, end) {
                (DragTo::Local(p), _) => rect.min + p.to_vec2(),
                (_, Some(r)) => r.center(),
                (_, None) => from,
            };
            let (b, m) = (drag.button, drag.modifiers);
            if m != egui::Modifiers::NONE {
                me.send(egui::Event::ModifiersChanged(m));
            }
            me.send(egui::Event::PointerMoved(from));
            me.step();
            me.send(egui::Event::PointerButton { pos: from, button: b, pressed: true, modifiers: m });
            me.step();
            let n = drag.steps.max(1);
            for i in 1..=n {
                me.send(egui::Event::PointerMoved(from.lerp(to, i as f32 / n as f32)));
                me.step();
            }
            me.send(egui::Event::PointerButton { pos: to, button: b, pressed: false, modifiers: m });
            if m != egui::Modifiers::NONE {
                me.send(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
            }
            Tier::Synthetic
        })
    }

    /// `f`'s events with `m` held (a `ModifiersChanged` before and after, when not none).
    fn with_modifiers(&mut self, m: egui::Modifiers, f: impl FnOnce(&mut Self)) {
        if m == egui::Modifiers::NONE {
            return f(self);
        }
        self.send(egui::Event::ModifiersChanged(m));
        f(self);
        self.send(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
    }

    /// The screen rect of `target` now (one node), for a drag's end.
    fn resolve_rect(&self, target: &Target) -> Result<egui::Rect, Error> {
        self.with_scene(|s, roots| {
            let id = match target {
                Target::Selector(sel) => strict(sel, resolve(s, sel, roots)?)?,
                Target::Invalid(e) => return Err(e.clone()),
                Target::Ref(r) => {
                    if r.epoch != self.epoch || s.get(r.id).is_none() {
                        return Err(stale(*r));
                    }
                    r.id
                }
            };
            s.get(id)
                .and_then(|n| node_rect(s, n))
                .ok_or_else(|| Error::new("node.not_actionable", format!("{} is not drawn", target.describe())))
        })
    }

    /// `input.focus`: keyboard focus on the target (AccessKit `Focus`; synthetic: a click).
    pub fn focus(
        &mut self,
        target: impl Into<Target>,
        opts: ActOptions,
    ) -> Result<ActResult, Error> {
        self.act(target.into(), opts, |me, _, rect| {
            if let Some(n) = me.a11y_target(rect) {
                me.accesskit(&n, Action::Focus, None);
                return Tier::Semantic;
            }
            me.pointer_click(rect.center(), egui::PointerButton::Primary);
            Tier::Synthetic
        })
    }

    /// `input.fill`: replaces a text field's text. Semantic: AccessKit `SetValue`. Synthetic:
    /// focus by click, select all, then the text as one `Text` event (as typed).
    pub fn fill(
        &mut self,
        target: impl Into<Target>,
        text: &str,
        opts: ActOptions,
    ) -> Result<ActResult, Error> {
        let text = text.to_string();
        self.act(target.into(), opts, move |me, _, rect| {
            if opts.tier == Tier::Semantic
                && let Some(n) = me.a11y_target(rect)
            {
                me.accesskit(&n, Action::SetValue, Some(ActionData::Value(text.into())));
                return Tier::Semantic;
            }
            me.pointer_click(rect.center(), egui::PointerButton::Primary);
            me.step(); // focus lands before the keys
            me.key(egui::Key::A, egui::Modifiers::COMMAND);
            me.send(egui::Event::Text(text));
            Tier::Synthetic
        })
    }

    /// `input.set_value`: a numeric value (slider, drag value) through AccessKit `SetValue`.
    pub fn set_value(
        &mut self,
        target: impl Into<Target>,
        value: f64,
        opts: ActOptions,
    ) -> Result<ActResult, Error> {
        self.act(target.into(), opts, move |me, _, rect| {
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
        target: impl Into<Target>,
        handle: &str,
        value: f64,
        opts: ActOptions,
    ) -> Result<ActResult, Error> {
        let mut missing = None;
        let r = self.act(target.into(), opts, |me, _, rect| {
            let area = rect.expand(2.0);
            let mut handles: Vec<A11yNode> = me
                .driver
                .a11y_nodes()
                .into_iter()
                .filter(|n| n.role == egui::accesskit::Role::Slider && area.contains(n.rect.center()))
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

    fn key(&mut self, key: egui::Key, modifiers: egui::Modifiers) {
        for pressed in [true, false] {
            self.send(egui::Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers,
            });
        }
    }

    /// `input.press`: a key chord to the focused widget, e.g. `"Enter"`, `"Ctrl+A"`,
    /// `"Shift+Tab"` (egui key names; `Cmd`/`Command` is Ctrl on Linux and Windows).
    pub fn press(&mut self, chord: &str) -> Result<ActResult, Error> {
        self.check(true)?;
        let (key, m) = parse_chord(chord)?;
        self.untargeted(move |me| me.key(key, m))
    }

    /// `input.type`: text to the focused widget, as typed (one `Text` event).
    pub fn type_text(&mut self, text: &str) -> Result<ActResult, Error> {
        self.check(true)?;
        let text = text.to_string();
        self.untargeted(move |me| me.send(egui::Event::Text(text)))
    }

    fn untargeted(&mut self, inject: impl FnOnce(&mut Self)) -> Result<ActResult, Error> {
        let scene = self.driver.scene();
        let obs = scene.observe(ChangeFilter::all());
        inject(self);
        let settled = self.wait_settled(ActOptions::default().timeout_ms);
        let set = scene.take_changes(&obs);
        settled?;
        Ok(ActResult {
            frame: self.frame,
            epoch: self.epoch,
            tier: Tier::Synthetic,
            changed: set.by_node().map(|(n, m)| (n, format!("{m:?}"))).collect(),
        })
    }

    // ---- capture ----

    /// `capture.screenshot`: the last frame, cropped to `target`'s screen rect when given.
    /// `capability.absent` on drivers without a renderer (kittest without wgpu).
    pub fn screenshot(&mut self, target: Option<&str>) -> Result<egui::ColorImage, Error> {
        self.check(false)?;
        let crop = match target {
            Some(sel) => Some(self.with_scene(|s, roots| {
                let sel = parse(sel)?;
                let id = strict(&sel, resolve(s, &sel, roots)?)?;
                s.get(id)
                    .and_then(|n| n.screen_rect())
                    .map(|(_, r)| r)
                    .ok_or_else(|| Error::new("node.not_actionable", "not drawn"))
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
    pub fn journal_subscribe(&mut self, filter: ChangeFilter) -> Result<(), Error> {
        self.check(false)?;
        self.journal = Some(self.driver.scene().observe(filter));
        Ok(())
    }

    /// `journal.take`: the changes since the last take, coalesced per node (`overflowed`: records
    /// were lost, re-read).
    pub fn journal_take(&mut self) -> Result<(Vec<(NodeId, String)>, bool), Error> {
        self.check(false)?;
        let obs = self
            .journal
            .as_ref()
            .ok_or_else(|| Error::new("params.invalid", "journal.subscribe first"))?;
        let set = self.driver.scene().take_changes(obs);
        Ok((
            set.by_node().map(|(n, m)| (n, format!("{m:?}"))).collect(),
            set.overflowed,
        ))
    }

    // ---- JSON ----

    /// One method by name with JSON params (§1.1's schema; the in-process JSON path and, from AU2,
    /// the IPC transport). Targets are `{"selector": "…"}` or `{"ref": {"id": n, "epoch": e}}`.
    pub fn call(&mut self, method: &str, p: &Value) -> Result<Value, Error> {
        let opts = || act_options(p);
        Ok(match method {
            "session.hello" => self.hello()?,
            "app.roots" => json!(self.roots()?.iter().map(|r| json!({"key": r.key, "visible": r.visible, "tooling": r.tooling})).collect::<Vec<_>>()),
            "scene.query" => {
                let limit = p.get("limit").and_then(Value::as_u64).map(|l| l as usize);
                json!(self.query(str_param(p, "selector")?, limit)?.iter().map(NodeInfo::to_json).collect::<Vec<_>>())
            }
            "scene.locate" => ref_json(self.locate(str_param(p, "selector")?)?),
            "scene.node" => {
                let r = match target_param(p)? {
                    Target::Ref(r) => r,
                    Target::Selector(s) => self.locate(&s.source)?,
                    Target::Invalid(e) => return Err(e),
                };
                self.node(r)?.to_json()
            }
            "scene.snapshot" => json!({"format": "yaml", "text": self.snapshot(p.get("selector").and_then(Value::as_str))?}),
            "a11y.tree" => json!(
                self.a11y_tree()?
                    .iter()
                    .map(|n| json!({"role": format!("{:?}", n.role), "label": n.label, "author_id": n.author_id, "rect": [n.rect.min.x, n.rect.min.y, n.rect.width(), n.rect.height()]}))
                    .collect::<Vec<_>>()
            ),
            "input.click" => act_json(self.click(target_param(p)?, opts()?)?),
            "input.hover" => act_json(self.hover(target_param(p)?, opts()?)?),
            "input.focus" => act_json(self.focus(target_param(p)?, opts()?)?),
            "input.pointer_move" => act_json(self.pointer_move(target_param(p)?, at_param(p, "at")?, modifiers_param(p)?, opts()?)?),
            "input.wheel" => {
                let unit = match p.get("unit").and_then(Value::as_str).unwrap_or("line") {
                    "line" => egui::MouseWheelUnit::Line,
                    "pixel" | "point" => egui::MouseWheelUnit::Point,
                    "page" => egui::MouseWheelUnit::Page,
                    u => return Err(Error::new("params.invalid", format!("unknown wheel unit `{u}` (line, pixel, page)"))),
                };
                let num = |k: &str| p.get(k).map_or(Ok(0.0), |v| v.as_f64().ok_or_else(|| Error::new("params.invalid", format!("`{k}` wants a number"))));
                let wheel = Wheel { at: at_param(p, "at")?, delta: egui::vec2(num("dx")? as f32, num("dy")? as f32), unit, modifiers: modifiers_param(p)? };
                act_json(self.wheel(target_param(p)?, wheel, opts()?)?)
            }
            "input.drag" => {
                let to = match (p.get("to"), p.get("to_selector"), p.get("to_ref")) {
                    (Some(_), None, None) => match at_param(p, "to")? {
                        At::Local(l) => DragTo::Local(l),
                        At::Center => return Err(Error::new("params.invalid", "`to` wants [x, y]")),
                    },
                    (None, Some(_), None) => DragTo::Node(Target::Selector(parse(str_param(p, "to_selector")?)?)),
                    (None, None, Some(_)) => DragTo::Node(target_param(&json!({"ref": p["to_ref"]}))?),
                    _ => return Err(Error::new("params.invalid", "input.drag wants one of `to` ([x, y] in the target), `to_selector` or `to_ref`")),
                };
                let button = match p.get("button").and_then(Value::as_str).unwrap_or("primary") {
                    "primary" | "left" => egui::PointerButton::Primary,
                    "secondary" | "right" => egui::PointerButton::Secondary,
                    "middle" => egui::PointerButton::Middle,
                    b => return Err(Error::new("params.invalid", format!("unknown button `{b}`"))),
                };
                let steps = p.get("steps").and_then(Value::as_u64).unwrap_or(10) as u32;
                let drag = Drag { from: at_param(p, "from")?, to, steps: steps.max(1), button, modifiers: modifiers_param(p)? };
                act_json(self.drag(target_param(p)?, drag, opts()?)?)
            }
            "input.fill" => act_json(self.fill(target_param(p)?, str_param(p, "text")?, opts()?)?),
            "input.set_value" => {
                let v = p.get("value").and_then(Value::as_f64).ok_or_else(|| Error::new("params.invalid", "`value` wants a number"))?;
                match p.get("handle") {
                    Some(Value::String(h)) => act_json(self.set_value_handle(target_param(p)?, h, v, opts()?)?),
                    Some(Value::Number(i)) => act_json(self.set_value_handle(target_param(p)?, &i.to_string(), v, opts()?)?),
                    _ => act_json(self.set_value(target_param(p)?, v, opts()?)?),
                }
            }
            "input.press" => act_json(self.press(str_param(p, "key")?)?),
            "input.type" => act_json(self.type_text(str_param(p, "text")?)?),
            "wait.settled" => json!({"frames": self.wait_settled(timeout(p))?}),
            "wait.for" => {
                let state = match p.get("state").and_then(Value::as_str).unwrap_or("visible") {
                    "attached" => WaitState::Attached,
                    "detached" => WaitState::Detached,
                    "visible" => WaitState::Visible,
                    "hidden" => WaitState::Hidden,
                    "enabled" => WaitState::Enabled,
                    s => return Err(Error::new("params.invalid", format!("unknown state `{s}`"))),
                };
                let expect = match p.get("expect") {
                    None | Some(Value::Null) => None,
                    Some(e) => Some(expect_param(e)?),
                };
                let found = self.wait_for(str_param(p, "selector")?, state, expect.as_ref(), timeout(p))?;
                json!(found.iter().map(NodeInfo::to_json).collect::<Vec<_>>())
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
                self.journal_subscribe(ChangeFilter::all())?;
                json!({})
            }
            "journal.take" => {
                let (changes, overflowed) = self.journal_take()?;
                json!({"changes": changes.iter().map(|(n, m)| json!({"id": n.to_raw(), "what": m})).collect::<Vec<_>>(), "overflowed": overflowed})
            }
            m => return Err(Error::new("method.unknown", format!("`{m}`"))),
        })
    }
}

/// One actionability probe: the node, its screen rect, and why it is not actionable yet.
type Probe = (NodeId, Option<egui::Rect>, Result<(), String>);

/// What an act aims at: a selector (resolved strictly before each try) or an element ref.
#[derive(Clone, Debug)]
pub enum Target {
    Selector(Selector),
    Ref(Ref),
    /// A selector string that did not parse; the act fails with its error.
    Invalid(Error),
}

impl Target {
    fn describe(&self) -> String {
        match self {
            Target::Selector(s) => format!("`{}`", s.source),
            Target::Ref(r) => format!("{} (epoch {})", r.id, r.epoch),
            Target::Invalid(e) => e.message.clone(),
        }
    }
}

impl From<Ref> for Target {
    fn from(r: Ref) -> Target {
        Target::Ref(r)
    }
}

impl From<Selector> for Target {
    fn from(s: Selector) -> Target {
        Target::Selector(s)
    }
}

/// A selector string; an invalid one fails at the act with `selector.invalid`.
impl From<&str> for Target {
    fn from(s: &str) -> Target {
        parse(s).map_or_else(Target::Invalid, Target::Selector)
    }
}

fn text_of(n: NodeRef<'_>) -> String {
    let t = n.text();
    if !t.is_empty() {
        t.to_string()
    } else {
        accessible_name(n)
    }
}

fn strict(sel: &Selector, ids: Vec<NodeId>) -> Result<NodeId, Error> {
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

fn stale(r: Ref) -> Error {
    Error::new(
        "node.stale",
        format!(
            "{} from epoch {} is gone; re-resolve its locator",
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

fn ref_json(r: Ref) -> Value {
    json!({"id": r.id.to_raw(), "epoch": r.epoch})
}

fn act_json(r: ActResult) -> Value {
    json!({
        "frame": r.frame, "epoch": r.epoch,
        "tier": match r.tier { Tier::Semantic => "semantic", Tier::Synthetic => "synthetic" },
        "changed": r.changed.iter().map(|(n, m)| json!({"id": n.to_raw(), "what": m})).collect::<Vec<_>>(),
    })
}

fn str_param<'v>(p: &'v Value, k: &str) -> Result<&'v str, Error> {
    p.get(k)
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("params.invalid", format!("`{k}` wants a string")))
}

/// Where `n` was drawn last pass on screen: a widget's rect, else (CV1) a container's layout rect
/// from the hit list (a `box` canvas has no widget rect).
fn node_rect(s: SceneRef<'_>, n: NodeRef<'_>) -> Option<egui::Rect> {
    n.screen_rect().map(|(_, r)| r).or_else(|| s.box_rect(s.root_viewport(n.root())?, n.id()))
}

/// SV51: `[x, y]` in the target's local points under `k`; absent: its centre.
fn at_param(p: &Value, k: &str) -> Result<At, Error> {
    match p.get(k) {
        None | Some(Value::Null) => Ok(At::Center),
        Some(Value::Array(a)) if a.len() == 2 && a.iter().all(Value::is_number) => {
            Ok(At::Local(egui::pos2(a[0].as_f64().unwrap_or(0.0) as f32, a[1].as_f64().unwrap_or(0.0) as f32)))
        }
        _ => Err(Error::new("params.invalid", format!("`{k}` wants [x, y] (local points of the target)"))),
    }
}

/// SV51: `modifiers`: `"Shift+Ctrl"` (the names of `input.press`'s chords) or `["Shift", "Ctrl"]`.
fn modifiers_param(p: &Value) -> Result<egui::Modifiers, Error> {
    let names: Vec<String> = match p.get("modifiers") {
        None | Some(Value::Null) => return Ok(egui::Modifiers::NONE),
        Some(Value::String(s)) => s.split('+').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect(),
        Some(Value::Array(a)) => a.iter().map(|v| v.as_str().unwrap_or("?").to_string()).collect(),
        _ => return Err(Error::new("params.invalid", "`modifiers` wants \"Shift+Ctrl\" or [\"Shift\", \"Ctrl\"]")),
    };
    let mut m = egui::Modifiers::NONE;
    for n in names {
        match n.as_str() {
            "Ctrl" | "Control" => m.ctrl = true,
            "Cmd" | "Command" | "Mod" => m |= egui::Modifiers::COMMAND,
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

fn target_param(p: &Value) -> Result<Target, Error> {
    if let Some(r) = p.get("ref") {
        let id = r
            .get("id")
            .and_then(Value::as_u64)
            .and_then(NodeId::from_raw)
            .ok_or_else(|| Error::new("params.invalid", "`ref.id` wants a node id"))?;
        let epoch = r.get("epoch").and_then(Value::as_u64).unwrap_or(0);
        return Ok(Target::Ref(Ref { id, epoch }));
    }
    Ok(Target::Selector(parse(str_param(p, "selector")?)?))
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

fn parse_chord(chord: &str) -> Result<(egui::Key, egui::Modifiers), Error> {
    let mut m = egui::Modifiers::NONE;
    let mut key = None;
    for part in chord.split('+') {
        match part.trim() {
            "Ctrl" | "Control" => m.ctrl = true,
            "Cmd" | "Command" | "Mod" => m |= egui::Modifiers::COMMAND,
            "Shift" => m.shift = true,
            "Alt" | "Option" => m.alt = true,
            k => {
                key = Some(egui::Key::from_name(k).ok_or_else(|| {
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
