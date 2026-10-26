//! `use:` behaviours (A2): Rust code an app attaches to an element from Svelte, `use:pick={params}`
//! with `import { pick } from 'svelters:behaviors/cad'`, on the same JS path as egui's behaviours
//! (EV2b, `nativeBehavior` in `@svelters/native-runtime`): the action creates a node of kind
//! `<ns>.<name>`, sets each param as a prop, inserts it into the element, sets the params that change
//! on update and destroys it on destroy.
//!
//! On skelter the behaviour is no tree node: the backend gives its handle to an instance of the
//! app's [`Behavior`] (made by the factory [`SkelterBackend::register_behavior`] took) and routes
//! the ops to it: the insert is [`Behavior::attach`] with the element's node and the props record so
//! far, a prop write after it is [`Behavior::update`] with the whole record, and `remove`, `destroy`
//! or the element's node being freed is [`Behavior::detach`]. Params are untyped (any ABI value);
//! a `null` write removes the param.
//!
//! [`SkelterBackend::add_event_tap`] is the other half of a behaviour that works without JS: it sees
//! every input the backend dispatches with the JS-facing events it produced (a behaviour listens
//! natively on its node with `Document::listen`, so core lists the event).

use crate::backend::SkelterBackend;
use skelter_base::event::InputEnvelope;
use skelter_base::ids::NodeId;
use skelter_core::{Document, JsEvent};
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;
use svelters_abi::{StrResult, Value, svl_handle, svl_id};

/// A behaviour's params, by name.
pub type Props = BTreeMap<String, Value>;

/// What a behaviour hook gets: the element's node and the tree.
pub struct BehaviorCx<'a> {
    /// The behaviour's own handle (one per `use:`).
    pub handle: svl_handle,
    /// The element it is attached to.
    pub node: NodeId,
    pub doc: &'a mut Document,
}

/// An app's `use:` behaviour, one instance per `use:` directive.
pub trait Behavior: 'static {
    /// The element's node got the behaviour, with the params set so far.
    fn attach(&mut self, cx: &mut BehaviorCx<'_>, props: &Props);
    /// A param changed (or went away); `props` is the whole record.
    fn update(&mut self, _cx: &mut BehaviorCx<'_>, _props: &Props) {}
    /// The behaviour left the element (the action was destroyed, or the element's node freed).
    fn detach(&mut self, _cx: &mut BehaviorCx<'_>) {}
}

/// Makes a behaviour's instance for each `use:`.
pub type BehaviorFactory = Rc<dyn Fn() -> Box<dyn Behavior>>;

/// Sees each dispatched input and the JS-facing events it produced, before JS does (as skelter's
/// `CoreDocument::add_event_tap`).
pub type EventTap = Box<dyn FnMut(&mut Document, &InputEnvelope, &[JsEvent])>;

struct Live {
    kind: svl_id,
    behavior: Box<dyn Behavior>,
    props: Props,
    /// The element's node, while attached.
    host: Option<NodeId>,
}

/// The registered behaviours and the live instances.
#[derive(Default)]
pub(crate) struct Behaviors {
    /// Wire kind → (tag `<ns>.<name>`, factory).
    kinds: HashMap<svl_id, (String, BehaviorFactory)>,
    /// Param and event ids resolved so far → their names.
    names: HashMap<svl_id, String>,
    live: HashMap<svl_handle, Live>,
}

impl Behaviors {
    pub(crate) fn kind_named(&self, tag: &str) -> Option<svl_id> {
        self.kinds.iter().find(|(_, (t, _))| t == tag).map(|(k, _)| *k)
    }

    /// The id of param or event `name` of behaviour kind `kind`: skelter-core's hash, untyped.
    pub(crate) fn member(&mut self, kind: svl_id, name: &str) -> svl_id {
        let Some((tag, _)) = self.kinds.get(&kind) else { return 0 };
        let id = skelter_core::PropId::of(tag, name, "any").0 & crate::kinds::WIRE_MASK;
        self.names.insert(id, name.to_string());
        id
    }

    pub(crate) fn is_kind(&self, kind: svl_id) -> bool {
        self.kinds.contains_key(&kind)
    }

    pub(crate) fn is_live(&self, h: svl_handle) -> bool {
        self.live.contains_key(&h)
    }

    pub(crate) fn kind_of(&self, h: svl_handle) -> Option<svl_id> {
        self.live.get(&h).map(|l| l.kind)
    }

    pub(crate) fn get(&self, h: svl_handle, prop: svl_id) -> Option<Value> {
        let l = self.live.get(&h)?;
        let name = self.names.get(&prop)?;
        Some(l.props.get(name).cloned().unwrap_or(Value::Null))
    }

    /// The live behaviours attached to `n`.
    pub(crate) fn attached_to(&self, n: NodeId) -> Vec<svl_handle> {
        let mut v: Vec<svl_handle> = self.live.iter().filter(|(_, l)| l.host == Some(n)).map(|(h, _)| *h).collect();
        v.sort_unstable();
        v
    }
}

impl SkelterBackend {
    /// Registers behaviour `<ns>.<name>`: each `use:name` of `svelters:behaviors/<ns>` makes one
    /// instance with `factory` and gets its attach, update and detach ([`crate::behavior`]).
    pub fn register_behavior(&self, ns: &str, name: &str, factory: impl Fn() -> Box<dyn Behavior> + 'static) {
        let tag = format!("{ns}.{name}");
        let kind = skelter_core::KindId::of(&tag).0 & crate::kinds::WIRE_MASK;
        self.0.behaviors.borrow_mut().kinds.insert(kind, (tag, Rc::new(factory)));
    }

    /// The `svelters:behaviors/<ns>` modules of the registered behaviours, `(ns, source)` sorted by
    /// namespace: one `nativeBehavior` action per behaviour, as egui's `behavior_modules` writes them
    /// (`<root>/.svelters/app/behaviors/<ns>.js`, where live compiles and vite resolve the import).
    pub fn behavior_modules(&self) -> Vec<(String, String)> {
        let bs = self.0.behaviors.borrow();
        let mut tags: Vec<&str> = bs.kinds.values().map(|(t, _)| t.as_str()).collect();
        tags.sort_unstable();
        let mut out: Vec<(String, String)> = Vec::new();
        for tag in tags {
            let (ns, name) = tag.split_once('.').expect("<ns>.<name>");
            if out.last().is_none_or(|(n, _)| n != ns) {
                let head = format!("// svelters:behaviors/{ns} - @generated by svelters-skelter (SkelterBackend::behavior_modules). Do not edit.\nimport {{ nativeBehavior }} from '@svelters/native-runtime';\n");
                out.push((ns.to_string(), head));
            }
            let src = &mut out.last_mut().expect("pushed").1;
            src.push_str(&format!("\nexport const {name} = nativeBehavior('{tag}', []);\n"));
        }
        out
    }

    /// Calls `tap` after every input the backend dispatches, with the input and the JS-facing events
    /// skelter-core produced for it (the cancelable ones it delivered at event time first, then the
    /// rest, before they reach JS), in the order added: what a behaviour or
    /// another host service reacts to with no JS (a command button starting its command on click).
    pub fn add_event_tap(&self, tap: impl FnMut(&mut Document, &InputEnvelope, &[JsEvent]) + 'static) {
        self.0.taps.borrow_mut().push(Box::new(tap));
    }

    /// Runs the taps on one dispatched input: the events delivered at event time (cancelable ones,
    /// `sync.rs`), then `events`.
    pub(crate) fn run_taps(&self, ev: &InputEnvelope, events: &[JsEvent]) {
        let mut all = std::mem::take(&mut *self.0.tapped.borrow_mut());
        let mut taps = std::mem::take(&mut *self.0.taps.borrow_mut());
        if taps.is_empty() {
            return;
        }
        all.extend_from_slice(events);
        for tap in &mut taps {
            tap(&mut self.doc_mut(), ev, &all);
        }
        // taps added by a tap run from the next input on
        let mut slot = self.0.taps.borrow_mut();
        taps.append(&mut slot);
        *slot = taps;
    }

    /// A new instance of behaviour kind `kind` at handle `h`.
    pub(crate) fn behavior_create(&self, h: svl_handle, kind: svl_id) -> StrResult<()> {
        let mut bs = self.0.behaviors.borrow_mut();
        let (_, f) = bs.kinds.get(&kind).ok_or_else(|| format!("unknown kind id {kind}"))?;
        let behavior = f();
        bs.live.insert(h, Live { kind, behavior, props: Props::new(), host: None });
        Ok(())
    }

    /// Runs `f` on live behaviour `h` with its element's node and the tree (none when detached).
    fn with_behavior(&self, h: svl_handle, f: impl FnOnce(&mut dyn Behavior, &mut BehaviorCx<'_>, &Props)) {
        let Some(mut l) = self.0.behaviors.borrow_mut().live.remove(&h) else { return };
        if let Some(node) = l.host {
            let mut doc = self.doc_mut();
            let mut cx = BehaviorCx { handle: h, node, doc: &mut doc };
            f(&mut *l.behavior, &mut cx, &l.props);
        }
        self.0.behaviors.borrow_mut().live.insert(h, l);
    }

    pub(crate) fn behavior_set(&self, h: svl_handle, prop: svl_id, v: &Value) -> StrResult<()> {
        {
            let mut bs = self.0.behaviors.borrow_mut();
            let name = bs.names.get(&prop).cloned().ok_or_else(|| format!("unknown param id {prop}"))?;
            let l = bs.live.get_mut(&h).ok_or_else(|| format!("stale behaviour handle {h}"))?;
            match v {
                Value::Null => l.props.remove(&name),
                v => l.props.insert(name, v.clone()),
            };
        }
        self.with_behavior(h, |b, cx, props| b.update(cx, props));
        Ok(())
    }

    /// `insert(element, behaviour)`: attaches it to the element's node (moving it off another).
    pub(crate) fn behavior_attach(&self, h: svl_handle, node: NodeId) {
        if self.0.behaviors.borrow().live.get(&h).and_then(|l| l.host) == Some(node) {
            return;
        }
        self.behavior_detach(h);
        if let Some(l) = self.0.behaviors.borrow_mut().live.get_mut(&h) {
            l.host = Some(node);
        }
        self.with_behavior(h, |b, cx, props| b.attach(cx, props));
    }

    pub(crate) fn behavior_detach(&self, h: svl_handle) {
        self.with_behavior(h, |b, cx, _| b.detach(cx));
        if let Some(l) = self.0.behaviors.borrow_mut().live.get_mut(&h) {
            l.host = None;
        }
    }

    pub(crate) fn behavior_destroy(&self, h: svl_handle) {
        self.behavior_detach(h);
        self.0.behaviors.borrow_mut().live.remove(&h);
    }
}
