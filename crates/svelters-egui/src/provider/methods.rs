//! Kind m    // a `NativeError` (a `Result` method's `Err`, an argument that did not convert) reaches JS as a
    // native error with its domain and code (`SVL_ERROR`), not as a plain string
    Ok(r.unwrap_or_else(|e| Value::Error(Box::new(NativeError { message: format!("<{}>.{name}: {}", entry.tag, e.message), ..e }))))thods (RA3, ARCH-EMBED §8.2 "methods", ARCH-CRATES §4.2 "Methods"): Rust methods of a
//! provider kind that JS calls on an element, e.g. `<code-editor>`'s `focus()`, `revealLine(n)`,
//! `setSelection(sel)`, `executeCommand(cmd)` and `setDiagnostics(source, version, diags)`.
//!
//! ```ignore
//! #[svelters::widget(namespace = "cad", tag = "code-editor")]
//! pub struct CodeEditor { #[prop(bind)] selection: Sel, #[state] editor: EditorState, /* … */ }
//!
//! #[svelters::widget]
//! impl CodeEditor {
//!     /// Focuses the editor.
//!     pub fn focus(&mut self, cx: &mut MethodCx<'_>) { cx.request_focus(); }
//!     /// Scrolls line `n` (1-based) into view.
//!     pub fn reveal_line(&mut self, n: u32) { self.editor.reveal_line(n); }
//!     pub fn set_selection(&mut self, cx: &mut MethodCx<'_>, sel: Sel) { cx.set(&mut self.selection, sel); }
//! }
//! ```
//!
//! ```svelte
//! <script>let ed; $effect(() => ed?.revealLine(40));</script>
//! <code-editor bind:this={ed} … />
//! ```
//!
//! - Every fn of the `impl` with a `&self`/`&mut self` receiver (pub or not) is a method; its JS name
//!   is the Rust name in lowerCamelCase (`#[method(rename = "x")]` or `#[js(rename = "x")]` renames,
//!   `#[method(skip)]`/`#[js(skip)]` skips; fns without a receiver stay Rust-only). Parameters
//!   and results are any `FromJs`/`IntoJs` type (records, enums, `Option`, `Vec`, `Json<T>`), described
//!   in the export schema like props (`svelters:app/<namespace>` declares their named types); a
//!   `&mut MethodCx` parameter, anywhere, is the call context, not a JS argument.
//! - JS calls them on the element (`bind:this`): `ed.revealLine(40)` (the runtime resolves members a
//!   provider kind's node does not have as its methods) or `ed.call('revealLine', 40)`. The host
//!   checks the arguments against the method's descriptor (count, types) before Rust runs.
//! - A method runs between passes, on the UI thread, with the widget taken out of the scene (as for
//!   `ui`, SG4): `&mut self` is the node's widget. It never runs JS. Writes through the
//!   [`MethodCx`] (an exact prop write with [`MethodCx::set`], events with [`MethodCx::emit`]) are
//!   delivered to JS after the next pass, like the widget's own; state bits, ext data, focus and
//!   remeasure requests apply at once. The node's root is repainted.
//!
//! Every call goes through [`call`]: the place a capability policy (ARCH-SECURITY, deny by default)
//! would check which principal may call which kind's methods. No policy is built yet.
//!
//! Writer lane: RUSTAPI (RA3).

use super::{IntoStateBits, KindDecl, KindEntry, StateBits, flush_writes, queue_emit, with_taken};
use super::{Event, ExtEntry, ExtKey};
use crate::scene::{NodeFlags, NodeId, Origin, Scene};
use crate::walk::Out;
use std::any::Any;
use svelters_host::exports::{CallCx, IntoJs, NativeError, Value};

/// One method of a provider kind, as `#[svelters::widget] impl` registers it.
pub struct MethodEntry {
    /// The JS name (`revealLine`).
    pub name: &'static str,
    /// The Rust name (`reveal_line`).
    pub rust: &'static str,
    pub docs: &'static str,
    /// The JS parameter names, in order (the `MethodCx` parameter is not one).
    pub params: &'static [&'static str],
}

/// The methods of one provider kind (what `#[svelters::widget] impl T` submits to `inventory`).
pub struct KindMethods {
    /// The kind (`<T as KindDecl>::KIND`).
    pub kind: fn() -> &'static KindEntry,
    pub methods: &'static [MethodEntry],
    /// Calls method `m` on the widget (`&mut T` behind `Any`) with the converted arguments.
    pub call: fn(&mut dyn Any, usize, Vec<Value>, &mut MethodCx<'_>, &CallCx<'_>) -> Result<Value, NativeError>,
}

inventory::collect!(KindMethods);

/// The spec id the types of a kind's methods are registered under in the export schema
/// (`KindTypesDef`): `<spec id>#methods`, each method's parameters then its result.
pub fn types_key(id: &str) -> String {
    format!("{id}#methods")
}

/// What a kind method gets besides its arguments: the node, and the writes back to the scene and JS.
pub struct MethodCx<'a> {
    s: &'a mut Scene,
    i: usize,
    node: NodeId,
    entry: &'static KindEntry,
    base: usize,
    state: StateBits,
    focus: bool,
    remeasure: bool,
    changed: Vec<usize>,
    emits: Vec<(usize, Value)>,
    errors: Vec<String>,
}

impl MethodCx<'_> {
    /// The node.
    pub fn node_id(&self) -> NodeId {
        self.node
    }
    /// The node's egui id (the widget's own id: focus and memory follow the node).
    pub fn id(&self) -> egui::Id {
        self.s.core[self.i].egui_id
    }
    /// The whole scene, read-only. This node's own payload is out while the method runs (as in
    /// [`super::WidgetCx::scene`]).
    pub fn scene(&self) -> crate::scene::SceneRef<'_> {
        crate::scene::SceneRef::new(self.s)
    }
    /// This node (see [`MethodCx::scene`]).
    pub fn node(&self) -> crate::scene::NodeRef<'_> {
        crate::scene::NodeRef::at(self.s, self.node)
    }
    /// The rect the node was drawn at last (screen), if it was.
    pub fn rect(&self) -> egui::Rect {
        self.s.core[self.i].drawn
    }

    /// Whether every bit of `bits` is set on the node.
    pub fn state(&self, bits: impl IntoStateBits) -> bool {
        self.state.contains(bits.state_bits())
    }
    pub fn set_state(&mut self, bits: impl IntoStateBits, on: bool) {
        self.state.set(bits.state_bits(), on);
    }

    /// The exact write of a prop field, as [`super::WidgetCx::set`]: stores `v` and, if it differs,
    /// announces `prop_changed` to an observer (`bind:`) after the next pass.
    pub fn set<T: PartialEq>(&mut self, field: &mut T, v: T) {
        if *field == v {
            return;
        }
        *field = v;
        if let Some(i) = super::written_prop(self.entry, self.base, field, &mut self.changed, &mut self.errors)
            && self.entry.props[i].measure
        {
            self.remeasure = true;
        }
    }

    /// Queues `payload` for the event's JS listeners (delivered after the next pass, in order).
    pub fn emit<T: IntoJs>(&mut self, event: &Event<T>, payload: T) {
        queue_emit(self.entry, event, payload, &mut self.emits, &mut self.errors);
    }

    /// The node's provider data for `key`.
    pub fn ext<T: 'static>(&self, key: &ExtKey<T>) -> Option<&T> {
        self.s.ext.get(key, self.node.index())
    }
    /// The node's entry for `key`; writes are journaled with `Origin::Widget`.
    pub fn ext_mut<'b, T: 'static>(&'b mut self, key: &'b ExtKey<T>) -> ExtEntry<'b, T> {
        ExtEntry::new(self.s.ext_cx(), key, self.node, Origin::Widget)
    }

    /// Gives the node keyboard focus at the next pass (as the `request-focus` method does).
    pub fn request_focus(&mut self) {
        self.focus = true;
    }
    /// Measures the node again before the next layout (its content size changed).
    pub fn request_measure(&mut self) {
        self.remeasure = true;
    }
    /// Repaints the node's root (every method call does; this says so explicitly).
    pub fn request_repaint(&mut self) {}
}

/// `<T as KindDecl>::KIND` as a function (what the generated `KindMethods` stores).
#[doc(hidden)]
pub fn __kind_of<T: KindDecl>() -> &'static KindEntry {
    T::KIND
}

/// The widget of a method call, typed (an internal error if the node holds another kind).
#[doc(hidden)]
pub fn __this<'w, T: 'static>(w: &'w mut dyn Any, entry: &KindEntry) -> Result<&'w mut T, NativeError> {
    w.downcast_mut::<T>()
        .ok_or_else(|| svelters_host::exports::__private::host_error("internal", format!("<{}>: the node does not hold this kind's widget", entry.tag)))
}

/// Calls method `m` (an index into the kind's [`KindMethods::methods`]) on provider node `i` with
/// `args` (already checked against the method's descriptor by the host). The writes the method made
/// through its [`MethodCx`] that JS hears (`prop_changed`, events) are appended to `out`.
pub(crate) fn call(s: &mut Scene, i: usize, km: &'static KindMethods, m: usize, args: &[Value], out: &mut Vec<Out>) -> Result<Value, String> {
    let node = s.slots.id_at(i).ok_or("stale node")?;
    let state0 = s.core[i].state;
    let ran = with_taken(s, i, |w, s| {
        let entry = w.entry();
        let mut cx = MethodCx {
            s,
            i,
            node,
            entry,
            base: w.base(),
            state: state0,
            focus: false,
            remeasure: false,
            changed: Vec::new(),
            emits: Vec::new(),
            errors: Vec::new(),
        };
        let r = svelters_host::exports::__private::with_cx(entry.namespace, |ccx| (km.call)(w.as_any_mut(), m, args.to_vec(), &mut cx, ccx));
        let MethodCx { state, focus, remeasure, changed, emits, errors, .. } = cx;
        (r, entry, state, focus, remeasure, changed, emits, errors)
    });
    let Some((r, entry, state, focus, remeasure, changed, emits, errors)) = ran else {
        return Err("the node holds no provider widget (or it is being drawn)".into());
    };
    if state != s.core[i].state {
        let before = s.core[i].state;
        s.core[i].state = state;
        let t = s.stamp();
        s.core[i].changed = t;
        s.record_state(i, before, Origin::Widget);
    }
    if focus {
        s.core[i].flags.insert(NodeFlags::WANT_FOCUS);
    }
    flush_writes(s, i, changed, emits, errors, out);
    if remeasure {
        s.layout[i].intrinsic = None;
        s.invalidate_content(node);
    }
    let name = km.methods.get(m).map_or("?", |d| d.name);
    // a `NativeError` (a `Result` method's `Err`, an argument that did not convert) reaches JS as a
    // native error with its domain and code (`SVL_ERROR`), not as a plain string
    Ok(r.unwrap_or_else(|e| Value::Error(Box::new(NativeError { message: format!("<{}>.{name}: {}", entry.tag, e.message), ..e }))))
}
