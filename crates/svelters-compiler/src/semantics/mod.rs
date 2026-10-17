//! Backend semantic specifications (`metadata/semantics/*.json`) and the `BackendSemantics`
//! trait. Owner: agent G.
//!
//! The compiler asks a [`BackendSemantics`] what an element, attribute, event, binding, child
//! placement or style declaration means on one backend, and gets typed answers or an
//! [`Unresolved`] with a stable diagnostic code. [`SpecBackend`] answers from a generated spec
//! (format: docs/notes/native-compiler/semantic-spec.md).

mod backend;
pub mod spec;

pub use backend::SpecBackend;
pub use spec::{
    AttrSpec, AttrType, MethodSpec, ParamSpec, TypeSpec, BindingDirection, BindingValue, Echo, HandlerResult, Lifetime, PropType, PropertySpec, SignalSpec, SlotOrder, Spec,
    StylePropSpec, ValueKind, Via,
};

use crate::diag::Diagnostics;
use crate::style::ValueToken;

/// Stable diagnostic codes produced by the semantics layer.
pub mod codes {
    pub const UNKNOWN_ELEMENT: &str = "unknown_element";
    pub const UNKNOWN_PROPERTY: &str = "unknown_property";
    pub const READ_ONLY_PROPERTY: &str = "read_only_property";
    pub const UNKNOWN_EVENT: &str = "unknown_event";
    pub const BIND_NO_PLAN: &str = "bind_no_plan";
    pub const CHILDREN_NOT_ALLOWED: &str = "children_not_allowed";
    pub const UNKNOWN_SLOT: &str = "unknown_slot";
    pub const SLOT_REJECTS_CHILD: &str = "slot_rejects_child";
    pub const NO_DEFAULT_SLOT: &str = "no_default_slot";
    pub const STYLE_UNKNOWN_DECLARATION: &str = "style_unknown_declaration";
    pub const STYLE_NOT_APPLICABLE: &str = "style_not_applicable";
    pub const STYLE_BAD_VALUE: &str = "style_bad_value";
    pub const UNKNOWN_CHILD_ATTR: &str = "unknown_child_attr";
    pub const CHILD_ATTR_BAD_VALUE: &str = "child_attr_bad_value";
    pub const LISTEN_PHASE_UNSUPPORTED: &str = "listen_phase_unsupported";
    pub const UNKNOWN_METHOD: &str = "unknown_method";
}

/// Why a question has no answer on this backend.
#[derive(Debug, Clone, PartialEq)]
pub struct Unresolved {
    pub code: &'static str,
    pub message: String,
    /// Closest valid names (edit distance), available slots, intents, ...
    pub suggestions: Vec<String>,
    /// Extra advice (e.g. a backend hint for a missing element).
    pub help: Option<String>,
}

impl Unresolved {
    pub fn new(code: &'static str, message: impl Into<String>) -> Unresolved {
        Unresolved { code, message: message.into(), suggestions: Vec::new(), help: None }
    }
    pub fn suggest(mut self, s: Vec<String>) -> Unresolved {
        self.suggestions = s;
        self
    }
    pub fn help(mut self, h: impl Into<String>) -> Unresolved {
        self.help = Some(h.into());
        self
    }

    /// `help` plus a "did you mean" line built from the suggestions.
    pub fn help_text(&self) -> Option<String> {
        let mean = match self.suggestions.as_slice() {
            [] => None,
            [one] => Some(format!("did you mean `{one}`?")),
            many => Some(format!("did you mean one of {}?", many.iter().map(|s| format!("`{s}`")).collect::<Vec<_>>().join(", "))),
        };
        match (&self.help, mean) {
            (Some(h), Some(m)) => Some(format!("{h}; {m}")),
            (Some(h), None) => Some(h.clone()),
            (None, m) => m,
        }
    }

    /// Record as a backend error at `span`.
    pub fn report(&self, diags: &mut Diagnostics, span: (u32, u32)) {
        let d = diags.native_error(self.code, span, self.message.clone());
        d.help = self.help_text();
    }
}

impl std::fmt::Display for Unresolved {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)?;
        if let Some(h) = self.help_text() {
            write!(f, " ({h})")?;
        }
        Ok(())
    }
}

impl std::error::Error for Unresolved {}

/// A resolved element: the source tag and the backend kind id it creates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementRef {
    /// What generated code passes to `host.create`.
    pub tag: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifetimePlan {
    pub lifetime: Lifetime,
    /// Zero-argument method called once a toplevel's subtree is complete.
    pub present: Option<String>,
}

/// A resolved event attribute.
#[derive(Debug, Clone, PartialEq)]
pub struct EventSpec {
    /// The signal name generated code passes to `host.listen` / `$n.on`.
    pub signal: String,
    /// Kind id declaring the signal.
    pub owner: String,
    pub args: Vec<(String, String)>,
    pub handler_result: HandlerResult,
    pub sync_required: bool,
    pub disconnect_on_drop: bool,
    /// Delivered through `call_sync`: listeners get `(event, ...args)` (a NativeEvent) and its
    /// result goes back to the toolkit.
    pub sync: bool,
    /// Listen phases accepted besides the plain one (`after`, `capture`).
    pub phases: Vec<String>,
    /// The DOM-style alias that was applied (`click` -> `clicked`).
    pub via_alias: Option<String>,
}

/// Listener phase flags (ABI v2.2 `listen_ex`, the renderer's LISTEN_AFTER / LISTEN_CAPTURE).
pub const LISTEN_AFTER: u32 = 1;
pub const LISTEN_CAPTURE: u32 = 2;

/// A resolved `parent:<name>` child attribute.
#[derive(Debug, Clone, PartialEq)]
pub struct ChildAttrPlan {
    /// What generated code passes to `host.setChildAttr` (the host resolves it by name).
    pub name: String,
    /// The parent's slot the attribute belongs to.
    pub slot: String,
    pub spec: AttrSpec,
}

/// Two-way binding plan (`$n.bind(node, prop, signal, arg, get, set)`).
#[derive(Debug, Clone, PartialEq)]
pub struct BindingPlan {
    /// JS name of the bound property (what `$n.bind` writes).
    pub prop: String,
    pub prop_type: PropType,
    pub signal: String,
    pub value: BindingValue,
    pub echo: Echo,
    pub direction: BindingDirection,
    pub compare_before_set: bool,
    pub read_back: bool,
    pub natively_normalizes: bool,
    pub sync_signals_on_set: Vec<String>,
}

impl BindingPlan {
    /// The runtime's `arg` operand: the argument index, or -1 for a read-back.
    pub fn arg_operand(&self) -> i64 {
        match self.value {
            BindingValue::Arg(n) => n as i64,
            BindingValue::Readback => -1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Routing {
    /// `slot="..."` (or a proxy) named it.
    Explicit,
    /// The child's kind is accepted by exactly one `route_by_kind` slot.
    ByKind,
    Default,
}

/// Where a child goes in its parent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotPlan {
    pub slot: String,
    /// True for the parent's default slot: generated code passes `null` (host `slot` undefined).
    pub is_default: bool,
    pub routing: Routing,
    /// Kind id declaring the slot.
    pub owner: String,
    pub many: bool,
    pub order: SlotOrder,
}

/// A typed constant for a static attribute or a style value.
#[derive(Debug, Clone, PartialEq)]
pub enum StaticValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    /// Enum nick (both backends take enums as names).
    Enum(String),
    /// Flags names, plus their combined bit value when the spec knows it (GTK). The host takes the
    /// name array (ABI v3), so `bits` is informational.
    Flags { nicks: Vec<String>, bits: Option<u64> },
    /// A list of strings (GTK `css-classes`; ABI v3 passes it as an array).
    StrList(Vec<String>),
    /// A record value (`{ top, right, bottom, left }` insets).
    Record(serde_json::Value),
}

impl StaticValue {
    /// The value as the JSON the JS emitter writes literally.
    pub fn to_json(&self) -> serde_json::Value {
        use serde_json::Value as J;
        match self {
            StaticValue::Null => J::Null,
            StaticValue::Bool(b) => J::Bool(*b),
            StaticValue::Int(i) => J::from(*i),
            StaticValue::Float(f) => serde_json::Number::from_f64(*f).map_or(J::Null, J::Number),
            StaticValue::Str(s) | StaticValue::Enum(s) => J::String(s.clone()),
            StaticValue::Flags { nicks, .. } => J::Array(nicks.iter().cloned().map(J::String).collect()),
            StaticValue::StrList(l) => J::Array(l.iter().cloned().map(J::String).collect()),
            StaticValue::Record(v) => v.clone(),
        }
    }
}

/// What a setter or child-attr style declaration lowers to.
#[derive(Debug, Clone, PartialEq)]
pub enum StyleLowering {
    /// `$n.set(node, prop, value)`.
    SetProp { prop: String, value: StaticValue },
    /// `$n.child_attr(node, attr, value)`: a child attribute owned by the native parent.
    ChildAttr { attr: String, value: StaticValue },
}

/// Native semantics of one backend, as the compiler needs them.
pub trait BackendSemantics {
    /// `gtk`, `qt`, ...
    fn backend(&self) -> &str;
    fn framework(&self) -> &str;
    /// Tag of the per-row outlet of virtualized lists (the spec's `row_outlet`), if the backend has one.
    fn row_outlet(&self) -> Option<&str> {
        None
    }

    fn resolve_element(&self, tag: &str) -> Result<ElementRef, Unresolved>;
    fn lifetime(&self, el: &ElementRef) -> LifetimePlan;
    /// Kind id or (Qt) class name in the element's chain; also accepts native type names.
    fn is_a(&self, el: &ElementRef, kind_or_class: &str) -> bool;

    /// An attribute naming a property, by JS or native name. Use `PropertySpec::js` in output.
    fn property(&self, el: &ElementRef, attr: &str) -> Result<&PropertySpec, Unresolved>;
    /// Like [`property`](Self::property) but also requires the property to be writable.
    fn writable_property(&self, el: &ElementRef, attr: &str) -> Result<&PropertySpec, Unresolved> {
        let p = self.property(el, attr)?;
        if !p.writable {
            return Err(Unresolved::new(
                codes::READ_ONLY_PROPERTY,
                format!("property `{}` of <{}> is read-only on {}", p.js, el.tag, self.backend()),
            ));
        }
        Ok(p)
    }
    /// An event attribute: `on<event>` (`onclicked`, `onclose-request`), a signal or `js_event` name.
    fn event(&self, el: &ElementRef, attr: &str) -> Result<EventSpec, Unresolved>;
    /// `bind:<name>` — only where the spec declares a binding (by binding name or property).
    fn plan_binding(&self, el: &ElementRef, prop: &str) -> Result<BindingPlan, Unresolved>;
    /// Routing order: explicit slot -> kind routing -> default slot.
    fn place_child(&self, parent: &ElementRef, child: &ElementRef, requested_slot: Option<&str>) -> Result<SlotPlan, Unresolved>;
    /// The property that text children fold into (`<label>Count: {n}</label>`).
    fn text_content(&self, el: &ElementRef) -> Option<&PropertySpec>;
    /// The style table entry of `name` (None: not a style property of this backend).
    fn style_property(&self, name: &str) -> Option<&StylePropSpec>;
    /// Whether `name` is a setter or child-attr property here (the compiler's part of a sheet;
    /// `css` declarations go to the X6 translator).
    fn is_setter_property(&self, name: &str) -> bool {
        self.style_property(name).is_some_and(|p| p.via != Via::Css)
    }
    /// A setter or child-attr declaration on `el` (whose native parent is `parent`, for a child
    /// attribute). `Ok(None)`: not the compiler's (a `css` property, or unknown here).
    fn style_declaration(&self, el: &ElementRef, parent: Option<&ElementRef>, name: &str, value: &[ValueToken]) -> Result<Option<StyleLowering>, Unresolved>;
    /// A static attribute value (`None` = bare attribute) as a typed constant.
    fn coerce_static(&self, prop: &PropertySpec, value: Option<&str>) -> Result<StaticValue, String>;
    /// Text nodes, when the backend has them (spec `text_node`): the text kind and its text
    /// property. None (GTK, Qt): text outside text-content elements is an error.
    fn text_node(&self) -> Option<(ElementRef, String)> {
        None
    }
    /// A `parent:<name>` attribute of a child placed in `parent`'s slot `slot`.
    fn child_attr(&self, _parent: &ElementRef, slot: &str, name: &str) -> Result<ChildAttrPlan, Unresolved> {
        Err(Unresolved::new(codes::UNKNOWN_CHILD_ATTR, format!("slot `{slot}` has no child attribute `{name}`")))
    }
    /// A static `parent:<name>="..."` value as a typed constant (`None` = bare attribute).
    fn coerce_child_attr(&self, plan: &ChildAttrPlan, value: Option<&str>) -> Result<StaticValue, String> {
        let _ = (plan, value);
        Err("child attributes are not supported".into())
    }
    /// An annotated method of the element's chain (for `bind:this` calls).
    fn method(&self, _el: &ElementRef, name: &str) -> Result<&MethodSpec, Unresolved> {
        Err(Unresolved::new(codes::UNKNOWN_METHOD, format!("no method `{name}`")))
    }
    /// Parse with HTML rules (spec `html_syntax`; the DOM conformance backend only).
    fn html_syntax(&self) -> bool {
        false
    }
}

/// Closest names to `name` by edit distance (at most 3).
pub fn closest<'a>(name: &str, candidates: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let limit = name.chars().count().div_ceil(2).max(2);
    let mut scored: Vec<(usize, &str)> = candidates
        .into_iter()
        .filter(|c| *c != name)
        .map(|c| (levenshtein(&name.to_lowercase(), &c.to_lowercase()), c))
        .filter(|(d, c)| *d <= limit || (name.len() >= 3 && (c.contains(name) || name.contains(*c))))
        .collect();
    scored.sort();
    scored.dedup_by(|a, b| a.1 == b.1);
    scored.into_iter().take(3).map(|(_, c)| c.to_string()).collect()
}

fn levenshtein(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for j in 0..b.len() {
            let cur = row[j + 1];
            row[j + 1] = (prev + usize::from(ca != b[j])).min(row[j] + 1).min(cur + 1);
            prev = cur;
        }
    }
    row[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_distance() {
        assert_eq!(levenshtein("kitten", "sitting"), 3);
        assert_eq!(levenshtein("", "abc"), 3);
        assert_eq!(closest("lable", ["label", "button", "box"]), vec!["label"]);
        assert!(closest("zzzzzz", ["label", "button"]).is_empty());
    }
}
