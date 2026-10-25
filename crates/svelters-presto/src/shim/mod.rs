//! Stand-ins for presto-core's and presto-widgets' public API, which their wave-1 and wave-2 lanes are
//! still writing (`plans/cloud/presto-b2/BRIEF.md`: "the smallest thing you need in your own crate
//! behind a module named `shim`"). The consolidation replaces each item with the real one; every item
//! here names the API it stands in for.
//!
//! - [`KindSpec`] and its member specs, [`KindRegistry`], [`Value`], [`ValueType`], the id newtypes,
//!   [`ListenFlags`], [`StyleInput`], [`TreeError`], [`PropError`]: presto-core's kind and value
//!   vocabulary (b1 brief, "presto-core"; architecture §4.2–§4.3).
//! - [`Document`]: the tree half of `presto_core::Document` (create, insert, remove, destroy, set_prop,
//!   get_prop, listen, set_style, transaction). It has no style, layout, recording, hit testing or
//!   `dispatch`/`frame`: those are presto-core's, and nothing here pretends to do them. Native-side
//!   output (a kind's signal, a native-owned prop change) enters through [`Document::emit`] and
//!   [`Document::native_set`], the two calls presto-core's `EventCx` makes in the real crate; the
//!   result comes out as [`Dispatched`], as `Document::dispatch` returns it.
//! - [`tier0`]: the Tier 0 kinds' specs (presto-widgets `controls`), transcribed from the egui spec so
//!   the JS-facing surface is the same.

pub mod tier0;

use presto_base::change::ChangeClass;
use presto_base::color::Color;
use presto_base::ids::{NodeId, Revision};
use std::collections::HashMap;
use std::sync::Arc;

macro_rules! id {
    ($(#[$m:meta])* $name:ident) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
        pub struct $name(pub u32);
    };
}

id!(
    /// A kind in a [`KindRegistry`] (shim: `presto_core::KindId`).
    KindId
);
id!(
    /// A prop of a kind, unique within its registry (shim: `presto_core::PropId`).
    PropId
);
id!(
    /// A signal of a kind (shim: `presto_core::SignalId`).
    SignalId
);
id!(
    /// A method of a kind (shim: `presto_core::MethodId`).
    MethodId
);
id!(
    /// A named child slot of a kind; 0 is the default slot (shim: `presto_core::SlotId`).
    SlotId
);

/// The type of a prop, signal argument or method parameter (shim: `presto_core::ValueType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ValueType {
    Bool,
    I64,
    F64,
    Str,
    /// One of the listed names.
    Enum(&'static [&'static str]),
    Color,
    StrList,
    NumList,
    IdList,
    Record,
}

impl ValueType {
    /// The spec's spelling (`presto.json` `type`).
    pub fn name(self) -> &'static str {
        match self {
            ValueType::Bool => "bool",
            ValueType::I64 => "i64",
            ValueType::F64 => "f64",
            ValueType::Str => "string",
            ValueType::Enum(_) => "enum",
            ValueType::Color => "color",
            ValueType::StrList | ValueType::NumList | ValueType::IdList => "list",
            ValueType::Record => "record",
        }
    }

    /// The element type of a list (`presto.json` `of`).
    pub fn of(self) -> Option<&'static str> {
        match self {
            ValueType::StrList => Some("string"),
            ValueType::NumList => Some("f64"),
            ValueType::IdList => Some("i64"),
            _ => None,
        }
    }
}

/// A prop value (shim: `presto_core::Value`).
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    I64(i64),
    F64(f64),
    Str(String),
    /// Index into the prop type's names.
    Enum(u16),
    Color(Color),
    List(Vec<Value>),
    Record(Vec<(String, Value)>),
}

impl Value {
    /// Whether `self` is a value of `ty` (`Null` only where `nullable`).
    pub fn fits(&self, ty: ValueType, nullable: bool) -> bool {
        match (self, ty) {
            (Value::Null, _) => nullable,
            (Value::Bool(_), ValueType::Bool) | (Value::I64(_), ValueType::I64) | (Value::F64(_), ValueType::F64) => true,
            (Value::Str(_), ValueType::Str) | (Value::Color(_), ValueType::Color) | (Value::Record(_), ValueType::Record) => true,
            (Value::Enum(i), ValueType::Enum(names)) => (*i as usize) < names.len(),
            (Value::List(v), ValueType::StrList) => v.iter().all(|x| matches!(x, Value::Str(_))),
            (Value::List(v), ValueType::NumList) => v.iter().all(|x| matches!(x, Value::F64(_) | Value::I64(_))),
            (Value::List(v), ValueType::IdList) => v.iter().all(|x| matches!(x, Value::I64(_))),
            _ => false,
        }
    }
}

/// A prop's default (`const` so specs can be `static`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DefaultValue {
    Null,
    Bool(bool),
    I64(i64),
    F64(f64),
    Str(&'static str),
    /// Index into the enum's names.
    Enum(u16),
}

impl DefaultValue {
    pub fn value(self) -> Value {
        match self {
            DefaultValue::Null => Value::Null,
            DefaultValue::Bool(b) => Value::Bool(b),
            DefaultValue::I64(i) => Value::I64(i),
            DefaultValue::F64(f) => Value::F64(f),
            DefaultValue::Str(s) => Value::Str(s.to_string()),
            DefaultValue::Enum(i) => Value::Enum(i),
        }
    }
}

#[derive(Debug)]
pub struct PropSpec {
    pub name: &'static str,
    pub ty: ValueType,
    pub default: DefaultValue,
    pub nullable: bool,
    /// The work a write causes (architecture §4.3).
    pub change: ChangeClass,
    /// Native-owned (architecture §4.5): the kind changes it itself and announces each change, so it
    /// is observable exactly and bindable.
    pub native: bool,
    pub doc: &'static str,
}

#[derive(Debug)]
pub struct SignalSpec {
    pub name: &'static str,
    pub args: &'static [(&'static str, ValueType)],
    /// Listeners are passive unless they say otherwise (scroll and drag paths, architecture §11.5).
    pub passive: bool,
    /// A listener can cancel the native default (`preventDefault`).
    pub cancelable: bool,
    /// Delivered synchronously, its handler's result read back (the DOM-style events).
    pub sync: bool,
    /// Propagates along the tree with capture and bubble phases (EV1).
    pub bubbles: bool,
    pub doc: &'static str,
}

#[derive(Debug)]
pub struct MethodSpec {
    pub name: &'static str,
    pub params: &'static [(&'static str, ValueType)],
    pub ret: Option<ValueType>,
    pub doc: &'static str,
}

#[derive(Debug)]
pub struct SlotSpec {
    pub name: &'static str,
    /// Kind names (or base kind names) a child in this slot may have.
    pub accepts: &'static [&'static str],
    pub many: bool,
}

#[derive(Debug)]
pub struct A11ySpec {
    /// AccessKit role name (`Button`); empty for no node of its own.
    pub role: &'static str,
    /// Where the accessible name comes from, in order.
    pub name_from: &'static [&'static str],
    /// A missing name is a diagnostic (compiler-and-js §3.9).
    pub name_required: bool,
}

/// What a kind is, statically (shim: `presto_core::KindSpec`, architecture §4.2).
#[derive(Debug)]
pub struct KindSpec {
    /// The kind's name (`Button`).
    pub name: &'static str,
    /// The element tag (`button`); `None` for an abstract base.
    pub tag: Option<&'static str>,
    /// The base kind whose members this one has too.
    pub extends: Option<&'static KindSpec>,
    pub doc: &'static str,
    pub props: &'static [PropSpec],
    pub signals: &'static [SignalSpec],
    pub methods: &'static [MethodSpec],
    /// Child slots; the first is the default.
    pub slots: &'static [SlotSpec],
    pub a11y: A11ySpec,
    /// The prop a text child sets (`text`).
    pub text_content: Option<&'static str>,
    /// A window: a root of its own, presented with `present`.
    pub toplevel: bool,
    /// Native-owned state this kind exposes (scroll offset, selection, drag value).
    pub native_state: &'static [&'static str],
}

impl KindSpec {
    /// This kind and its bases, nearest first.
    pub fn chain(&'static self) -> impl Iterator<Item = &'static KindSpec> {
        std::iter::successors(Some(self), |k| k.extends)
    }
}

/// The kinds of one app (shim: `presto_core::KindRegistry`). Ids are positions; the wire ids the
/// svelters spec publishes are stable and separate (`crate::ids`).
#[derive(Debug, Default)]
pub struct KindRegistry {
    kinds: Vec<&'static KindSpec>,
    by_tag: HashMap<&'static str, KindId>,
    by_name: HashMap<&'static str, KindId>,
    /// (kind, prop name) → spec, through the chain.
    props: HashMap<(KindId, &'static str), (PropId, &'static PropSpec)>,
    prop_specs: Vec<&'static PropSpec>,
    signals: HashMap<(KindId, &'static str), (SignalId, &'static SignalSpec)>,
    signal_specs: Vec<&'static SignalSpec>,
}

impl KindRegistry {
    pub fn new() -> KindRegistry {
        KindRegistry::default()
    }

    /// Adds `spec` (and its bases' members); a kind already registered keeps its id.
    pub fn register(&mut self, spec: &'static KindSpec) -> KindId {
        if let Some(&id) = self.by_name.get(spec.name) {
            return id;
        }
        let id = KindId(self.kinds.len() as u32 + 1);
        self.kinds.push(spec);
        self.by_name.insert(spec.name, id);
        if let Some(t) = spec.tag {
            self.by_tag.insert(t, id);
        }
        for k in spec.chain() {
            for p in k.props {
                self.props.entry((id, p.name)).or_insert_with(|| {
                    self.prop_specs.push(p);
                    (PropId(self.prop_specs.len() as u32), p)
                });
            }
            for s in k.signals {
                self.signals.entry((id, s.name)).or_insert_with(|| {
                    self.signal_specs.push(s);
                    (SignalId(self.signal_specs.len() as u32), s)
                });
            }
        }
        id
    }

    pub fn kinds(&self) -> impl Iterator<Item = (KindId, &'static KindSpec)> + '_ {
        self.kinds.iter().enumerate().map(|(i, k)| (KindId(i as u32 + 1), *k))
    }

    pub fn spec(&self, k: KindId) -> Option<&'static KindSpec> {
        self.kinds.get((k.0 as usize).checked_sub(1)?).copied()
    }

    pub fn by_tag(&self, tag: &str) -> Option<KindId> {
        self.by_tag.get(tag).copied()
    }

    pub fn by_name(&self, name: &str) -> Option<KindId> {
        self.by_name.get(name).copied()
    }

    pub fn prop(&self, k: KindId, name: &str) -> Option<(PropId, &'static PropSpec)> {
        self.props.get(&(k, name)).copied()
    }

    pub fn prop_spec(&self, p: PropId) -> Option<&'static PropSpec> {
        self.prop_specs.get((p.0 as usize).checked_sub(1)?).copied()
    }

    pub fn signal(&self, k: KindId, name: &str) -> Option<(SignalId, &'static SignalSpec)> {
        self.signals.get(&(k, name)).copied()
    }

    pub fn signal_spec(&self, s: SignalId) -> Option<&'static SignalSpec> {
        self.signal_specs.get((s.0 as usize).checked_sub(1)?).copied()
    }
}

/// How a listener listens (shim: `presto_core::ListenFlags`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ListenFlags {
    pub capture: bool,
    /// Never cancels: a scroll or drag through this node need not wait for JS.
    pub passive: bool,
}

/// A node's style inputs (shim: `presto_core::StyleInput`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StyleInput {
    pub classes: Vec<String>,
    pub id: Option<String>,
    pub inline: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TreeError {
    StaleNode(NodeId),
    /// The child would contain its own parent.
    Cycle,
    /// `before` is not a child of the parent.
    NotAChild(NodeId),
    /// The parent has no such slot, or the slot does not accept the child's kind.
    Slot(String),
}

impl std::fmt::Display for TreeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TreeError::StaleNode(n) => write!(f, "stale node {:#x}", n.0),
            TreeError::Cycle => write!(f, "the child contains the parent"),
            TreeError::NotAChild(n) => write!(f, "node {:#x} is not a child of the parent", n.0),
            TreeError::Slot(m) => f.write_str(m),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum PropError {
    StaleNode(NodeId),
    UnknownProp(PropId),
    WrongType { prop: &'static str, expected: ValueType },
}

impl std::fmt::Display for PropError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PropError::StaleNode(n) => write!(f, "stale node {:#x}", n.0),
            PropError::UnknownProp(p) => write!(f, "unknown prop {}", p.0),
            PropError::WrongType { prop, expected } => write!(f, "{prop}: expected {}", expected.name()),
        }
    }
}

/// A JS-facing event a native default produced (shim: an entry of `presto_core::Dispatched`).
#[derive(Clone, Debug, PartialEq)]
pub struct JsEvent {
    pub node: NodeId,
    pub signal: SignalId,
    pub args: Vec<Value>,
}

/// What one input (or one native step) produced for JS (shim: `presto_core::Dispatched`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Dispatched {
    /// Events to deliver, in order.
    pub events: Vec<JsEvent>,
    /// Native-owned props that changed, in order (observed or not).
    pub changed: Vec<(NodeId, PropId)>,
}

impl Dispatched {
    pub fn is_empty(&self) -> bool {
        self.events.is_empty() && self.changed.is_empty()
    }
}

#[derive(Debug)]
struct Node {
    kind: KindId,
    generation: u32,
    parent: Option<NodeId>,
    slot: SlotId,
    children: Vec<NodeId>,
    props: HashMap<PropId, Value>,
    listeners: HashMap<SignalId, ListenFlags>,
    style: StyleInput,
    visual: Visual,
}

/// `prefers-color-scheme` (shim: presto-core's media state).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorScheme {
    Light,
    Dark,
}

/// A node's property-class channels set from JS (TR1: `style:` opacity, transform, clip, inert); shim
/// for presto-core's effect and transform tree inputs.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Visual {
    /// `None`: opaque.
    pub opacity: Option<f32>,
    /// A 2D affine `[a, b, c, d, e, f]` (CSS `matrix()` order).
    pub transform: Option<[f32; 6]>,
    /// A clip to the leading fraction of the box, `[x, y]` in 0..=1 (`slide`).
    pub clip: Option<[f32; 2]>,
    /// Takes no input (an element playing its out transition).
    pub inert: bool,
}

/// The tree half of `presto_core::Document` (see the module docs for what it leaves out).
pub struct Document {
    registry: Arc<KindRegistry>,
    nodes: Vec<Option<Node>>,
    generations: Vec<u32>,
    free: Vec<u32>,
    revision: Revision,
    depth: u32,
    out: Dispatched,
    sheets: std::collections::BTreeMap<String, String>,
    color_scheme: Option<ColorScheme>,
}

impl Document {
    pub fn new(registry: Arc<KindRegistry>) -> Document {
        // index 0 is never a node, so `NodeId(0)` is never live
        Document { registry, nodes: vec![None], generations: vec![0], free: Vec::new(), revision: Revision(0), depth: 0, out: Dispatched::default(), sheets: Default::default(), color_scheme: None }
    }

    pub fn registry(&self) -> &Arc<KindRegistry> {
        &self.registry
    }

    pub fn revision(&self) -> Revision {
        self.revision
    }

    fn node(&self, n: NodeId) -> Option<&Node> {
        self.nodes.get(n.index() as usize)?.as_ref().filter(|x| x.generation == n.generation())
    }

    fn node_mut(&mut self, n: NodeId) -> Option<&mut Node> {
        self.nodes.get_mut(n.index() as usize)?.as_mut().filter(|x| x.generation == n.generation())
    }

    pub fn is_live(&self, n: NodeId) -> bool {
        self.node(n).is_some()
    }

    pub fn kind(&self, n: NodeId) -> Option<KindId> {
        self.node(n).map(|x| x.kind)
    }

    pub fn parent(&self, n: NodeId) -> Option<NodeId> {
        self.node(n)?.parent
    }

    pub fn children(&self, n: NodeId) -> &[NodeId] {
        self.node(n).map_or(&[], |x| &x.children)
    }

    pub fn slot(&self, n: NodeId) -> Option<SlotId> {
        self.node(n).map(|x| x.slot)
    }

    fn bump(&mut self) {
        if self.depth == 0 {
            self.revision.0 += 1;
        }
    }

    /// Runs `f` as one transaction: one revision however many edits it makes (architecture §4.4).
    pub fn transaction<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        self.depth += 1;
        let r = f(self);
        self.depth -= 1;
        if self.depth == 0 {
            self.revision.0 += 1;
        }
        r
    }

    pub fn create(&mut self, kind: KindId) -> NodeId {
        let index = self.free.pop().unwrap_or_else(|| {
            self.nodes.push(None);
            self.generations.push(0);
            self.nodes.len() as u32 - 1
        });
        let generation = self.generations[index as usize] + 1;
        self.generations[index as usize] = generation;
        self.nodes[index as usize] = Some(Node {
            kind,
            generation,
            parent: None,
            slot: SlotId(0),
            children: Vec::new(),
            props: HashMap::new(),
            listeners: HashMap::new(),
            style: StyleInput::default(),
            visual: Visual::default(),
        });
        self.bump();
        NodeId::new(index, generation)
    }

    /// Inserts (or moves) `child` under `parent` before `before` (the end when `None`), in `slot`
    /// (the default when `None`).
    pub fn insert(&mut self, parent: NodeId, child: NodeId, before: Option<NodeId>, slot: Option<SlotId>) -> Result<(), TreeError> {
        let pk = self.node(parent).ok_or(TreeError::StaleNode(parent))?.kind;
        self.node(child).ok_or(TreeError::StaleNode(child))?;
        let spec = self.registry.spec(pk).ok_or(TreeError::StaleNode(parent))?;
        let slot = slot.unwrap_or(SlotId(0));
        if spec.chain().all(|k| k.slots.is_empty()) {
            return Err(TreeError::Slot(format!("<{}> takes no children", spec.tag.unwrap_or(spec.name))));
        }
        let nslots = spec.chain().find(|k| !k.slots.is_empty()).map_or(0, |k| k.slots.len());
        if slot.0 as usize >= nslots {
            return Err(TreeError::Slot(format!("<{}> has no slot {}", spec.tag.unwrap_or(spec.name), slot.0)));
        }
        let mut up = Some(parent);
        while let Some(p) = up {
            if p == child {
                return Err(TreeError::Cycle);
            }
            up = self.parent(p);
        }
        if let Some(b) = before
            && (b == child || self.parent(b) != Some(parent))
        {
            if b == child {
                return Ok(()); // already in place
            }
            return Err(TreeError::NotAChild(b));
        }
        if let Some(old) = self.parent(child)
            && let Some(o) = self.node_mut(old)
        {
            o.children.retain(|&c| c != child);
        }
        let p = self.node_mut(parent).expect("checked live");
        let at = before.and_then(|b| p.children.iter().position(|&c| c == b)).unwrap_or(p.children.len());
        p.children.insert(at, child);
        let c = self.node_mut(child).expect("checked live");
        c.parent = Some(parent);
        c.slot = slot;
        self.bump();
        Ok(())
    }

    pub fn remove(&mut self, parent: NodeId, child: NodeId) -> Result<(), TreeError> {
        self.node(parent).ok_or(TreeError::StaleNode(parent))?;
        if self.parent(child) != Some(parent) {
            return Err(TreeError::NotAChild(child));
        }
        self.node_mut(parent).expect("checked live").children.retain(|&c| c != child);
        self.node_mut(child).expect("checked live").parent = None;
        self.bump();
        Ok(())
    }

    /// Frees `node`. Children still attached are detached (orphans), not freed: their owner frees them.
    pub fn destroy(&mut self, node: NodeId) {
        let Some(n) = self.node(node) else { return };
        let (parent, children) = (n.parent, n.children.clone());
        if let Some(p) = parent
            && let Some(pn) = self.node_mut(p)
        {
            pn.children.retain(|&c| c != node);
        }
        for c in children {
            if let Some(cn) = self.node_mut(c) {
                cn.parent = None;
            }
        }
        self.nodes[node.index() as usize] = None;
        self.free.push(node.index());
        self.bump();
    }

    pub fn set_prop(&mut self, node: NodeId, prop: PropId, v: Value) -> Result<ChangeClass, PropError> {
        let spec = self.registry.prop_spec(prop).ok_or(PropError::UnknownProp(prop))?;
        if !v.fits(spec.ty, spec.nullable) {
            return Err(PropError::WrongType { prop: spec.name, expected: spec.ty });
        }
        let n = self.node_mut(node).ok_or(PropError::StaleNode(node))?;
        n.props.insert(prop, v);
        self.bump();
        Ok(spec.change)
    }

    /// The prop's value, or its default when never set; `None` for a stale node or unknown prop.
    pub fn get_prop(&self, node: NodeId, prop: PropId) -> Option<Value> {
        let spec = self.registry.prop_spec(prop)?;
        let n = self.node(node)?;
        Some(n.props.get(&prop).cloned().unwrap_or_else(|| spec.default.value()))
    }

    /// Starts listening (`Some`) or stops (`None`).
    pub fn listen(&mut self, node: NodeId, signal: SignalId, flags: Option<ListenFlags>) {
        if let Some(n) = self.node_mut(node) {
            match flags {
                Some(f) => {
                    n.listeners.insert(signal, f);
                }
                None => {
                    n.listeners.remove(&signal);
                }
            }
        }
    }

    pub fn listening(&self, node: NodeId, signal: SignalId) -> Option<ListenFlags> {
        self.node(node)?.listeners.get(&signal).copied()
    }

    pub fn set_style(&mut self, node: NodeId, style: StyleInput) {
        if let Some(n) = self.node_mut(node) {
            n.style = style;
            self.bump();
        }
    }

    pub fn style(&self, node: NodeId) -> Option<&StyleInput> {
        self.node(node).map(|n| &n.style)
    }

    /// Installs (`Some`) or removes a style sheet by name; `None` name clears every sheet. Stored for
    /// presto-core's cascade (CS5e on the tree), which this shim does not run.
    pub fn set_sheet(&mut self, name: Option<&str>, css: Option<String>) {
        match (name, css) {
            (None, _) => self.sheets.clear(),
            (Some(n), Some(css)) => {
                self.sheets.insert(n.to_string(), css);
            }
            (Some(n), None) => {
                self.sheets.remove(n);
            }
        }
        self.bump();
    }

    pub fn sheets(&self) -> &std::collections::BTreeMap<String, String> {
        &self.sheets
    }

    /// The `prefers-color-scheme` the cascade evaluates; `None` follows the platform.
    pub fn set_color_scheme(&mut self, scheme: Option<ColorScheme>) {
        self.color_scheme = scheme;
        self.bump();
    }

    pub fn color_scheme(&self) -> Option<ColorScheme> {
        self.color_scheme
    }

    /// Sets a node's property-class visual channels (architecture §4.3: compositor only).
    pub fn set_visual(&mut self, node: NodeId, v: Visual) -> bool {
        let Some(n) = self.node_mut(node) else { return false };
        if n.visual == v {
            return false;
        }
        n.visual = v;
        self.bump();
        true
    }

    pub fn visual(&self, node: NodeId) -> Option<Visual> {
        self.node(node).map(|n| n.visual)
    }

    /// A kind's native default produced `signal` (presto-core's `EventCx::emit`).
    pub fn emit(&mut self, node: NodeId, signal: SignalId, args: Vec<Value>) {
        self.out.events.push(JsEvent { node, signal, args });
    }

    /// A kind changed native-owned `prop` itself (presto-core's `EventCx::set`): stored and announced.
    pub fn native_set(&mut self, node: NodeId, prop: PropId, v: Value) -> Result<(), PropError> {
        self.set_prop(node, prop, v)?;
        self.out.changed.push((node, prop));
        Ok(())
    }

    /// What native code produced since the last call (`Document::dispatch`'s result in presto-core).
    pub fn take_dispatched(&mut self) -> Dispatched {
        std::mem::take(&mut self.out)
    }
}
