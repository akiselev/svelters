//! Serde model of `svelters-semantics/v0` (`metadata/semantics/{gtk,qt}.json`), written by
//! `tools/metadata/semantics/gen_semantics.py`. Format: docs/notes/native-compiler/semantic-spec.md.

use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

pub const SCHEMA: &str = "svelters-semantics/v0";

#[derive(Debug, Clone, Deserialize)]
pub struct Spec {
    pub schema: String,
    pub backend: String,
    pub framework: String,
    pub event_aliases: BTreeMap<String, String>,
    /// Tag of the container a virtualized list renders each row into (`box` on GTK, `box-layout` on
    /// Qt); absent when the backend has none.
    #[serde(default)]
    pub row_outlet: Option<String>,
    /// Element tag -> kind id.
    pub tags: BTreeMap<String, String>,
    pub kinds: BTreeMap<String, KindSpec>,
    pub style: StyleSpec,
    /// Tags this backend lacks, with advice (diagnostics only).
    #[serde(default)]
    pub element_hints: BTreeMap<String, String>,
    #[serde(default)]
    pub source: Value,
    /// `typelib` (GTK: methods outside `methods` are still callable through GIRepository) or
    /// `annotated` (Qt: only listed methods exist); empty in older specs.
    #[serde(default)]
    pub method_lookup: String,
    /// Optional: the backend has text nodes (the DOM conformance backend, `metadata/semantics/dom.json`).
    /// Text and `{expression}` runs outside text-content elements then lower to elements of
    /// `kind` (a tag) whose `prop` holds the text, with Svelte's whitespace rules. Absent on GTK/Qt:
    /// text there is `text_not_allowed`.
    #[serde(default)]
    pub text_node: Option<TextNodeSpec>,
    /// Optional: parse templates with HTML rules (void elements, implicit closing), i.e.
    /// `ParseOptions.native == false`. Only the DOM conformance backend sets it.
    #[serde(default)]
    pub html_syntax: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TextNodeSpec {
    /// Tag of the text kind (must be in `tags`).
    pub kind: String,
    /// The property holding the text.
    pub prop: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lifetime {
    Owned,
    Toplevel,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KindSpec {
    /// GType (GTK) or C++ class (Qt).
    pub native: String,
    #[serde(rename = "abstract")]
    pub is_abstract: bool,
    pub interface: bool,
    /// Kind ids (and on Qt, class names) nearest first, starting with the kind itself. Lookups
    /// of props, signals, bindings and slots walk it.
    pub chain: Vec<String>,
    pub lifetime: Lifetime,
    pub present: Option<String>,
    pub text_content: Option<String>,
    /// Own properties, keyed by JS name.
    pub props: BTreeMap<String, PropertySpec>,
    /// Own signals, keyed by the name the host resolves.
    pub signals: BTreeMap<String, SignalSpec>,
    pub bindings: BTreeMap<String, BindingSpec>,
    pub children: Option<ChildrenSpec>,
    /// Annotated methods callable through `host.call` / `node.call`, by JS name. Older specs
    /// (and the DOM conformance spec) list names only; they load with untyped records.
    #[serde(default, deserialize_with = "methods_map")]
    pub methods: BTreeMap<String, MethodSpec>,
    #[serde(default)]
    pub provenance: BTreeMap<String, String>,
}

/// A parameter or return type of a method: a spec type name (`bool|i32|...|other`, or `void` for
/// returns), `nullable`, and the neutral `ref` for non-scalars.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct TypeSpec {
    #[serde(rename = "type")]
    pub ty: String,
    #[serde(default)]
    pub nullable: bool,
    #[serde(default, rename = "ref")]
    pub type_ref: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct ParamSpec {
    pub name: String,
    #[serde(flatten)]
    pub ty: TypeSpec,
    /// Qt: the C++ parameter has a default, so the argument may be left out.
    #[serde(default)]
    pub optional: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct MethodSpec {
    /// GTK `zero_arg` (generated call) or `dynamic` (GIRepository); Qt `generated`; empty when
    /// the spec gave only the name.
    #[serde(default)]
    pub dispatch: String,
    #[serde(default)]
    pub native: String,
    /// None when the spec gave only the name (arity unknown).
    #[serde(default)]
    pub params: Option<Vec<ParamSpec>>,
    #[serde(default)]
    pub returns: Option<TypeSpec>,
}

fn methods_map<'de, D: serde::Deserializer<'de>>(d: D) -> Result<BTreeMap<String, MethodSpec>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Raw {
        Names(Vec<String>),
        Map(BTreeMap<String, MethodSpec>),
    }
    Ok(match Raw::deserialize(d)? {
        Raw::Names(v) => v.into_iter().map(|n| (n, MethodSpec::default())).collect(),
        Raw::Map(m) => m,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PropType {
    Bool,
    I32,
    U32,
    I64,
    F64,
    String,
    Enum,
    Flags,
    Object,
    List,
    /// ABI v3 record (`Rect`, `Size`, `Color`, ...); `of` names it.
    Record,
    /// ABI v3 boxed value (`QIcon`, `QKeySequence`, a GTK boxed type); `of` names it.
    Boxed,
    /// No static form (date, time, variant, pointer, ...): only dynamic values.
    Other,
}

impl PropType {
    pub fn is_integer(self) -> bool {
        matches!(self, PropType::I32 | PropType::U32 | PropType::I64)
    }
    pub fn is_numeric(self) -> bool {
        self.is_integer() || self == PropType::F64
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Range {
    pub min: f64,
    pub max: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PropertySpec {
    /// Canonical name: what generated code passes to `host.setProp`.
    pub js: String,
    pub native: String,
    #[serde(rename = "type")]
    pub ty: PropType,
    pub nullable: bool,
    pub readable: bool,
    pub writable: bool,
    pub construct_only: bool,
    #[serde(default)]
    pub default: Option<Value>,
    /// Enum/flags nicks (absent for enum types outside the ingested namespace).
    #[serde(default)]
    pub values: Option<Vec<String>>,
    /// Flags: nick -> bit value.
    #[serde(default)]
    pub bits: Option<BTreeMap<String, u64>>,
    /// Enum/flags type id.
    #[serde(default, rename = "enum")]
    pub enum_type: Option<String>,
    /// Object: kind id; list: item type; record/boxed: type name.
    #[serde(default)]
    pub of: Option<String>,
    /// The neutral type ref (`record<Rect>`, `list<string>`, `date`, ...) for non-scalar types.
    #[serde(default, rename = "ref")]
    pub type_ref: Option<String>,
    #[serde(default)]
    pub range: Option<Range>,
    #[serde(default, rename = "virtual")]
    pub is_virtual: bool,
    /// The native change notification (GTK `notify::x`, Qt NOTIFY signature), if any. Its
    /// presence does not mean the backend exposes it as a signal.
    #[serde(default)]
    pub notify: Option<String>,
    /// A record/boxed/object value the backend also takes as a string (`"Ctrl+K"` for a
    /// QKeySequence, an icon name, `"#ff0000"`): static attributes pass the text through.
    #[serde(default)]
    pub string_form: bool,
    /// Kind id that declares the property (filled in at load).
    #[serde(skip)]
    pub owner: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ArgSpec {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HandlerResult {
    None,
    BoolStop,
    Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SignalSpec {
    /// The name the host resolves (`host.listen`).
    pub name: String,
    pub js_event: String,
    pub args: Vec<ArgSpec>,
    pub handler_result: HandlerResult,
    pub sync_required: bool,
    pub disconnect_on_drop: bool,
    #[serde(default)]
    pub synthetic: Option<String>,
    /// The backend delivers it through `call_sync` (`__svelters_dispatch_sync`) and uses the
    /// listeners' result: GTK `handler_result != none || sync_required`, Qt synthetic signals.
    #[serde(default)]
    pub sync: bool,
    /// Listen phases the backend accepts besides the plain one (`after`, `capture`).
    #[serde(default)]
    pub phases: Vec<String>,
    #[serde(skip)]
    pub owner: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Echo {
    /// The backend blocks its own handler while JS writes the property.
    BackendBlocks,
    /// Only the runtime's last-value comparison suppresses the echo.
    RuntimeCompare,
    None,
}

/// Where a binding's new value comes from when its event fires.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingValue {
    /// `args[n]` of the event.
    Arg(usize),
    /// `host.getProp` after the event.
    Readback,
}

impl<'de> Deserialize<'de> for BindingValue {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Arg { arg: usize },
            Word(String),
        }
        match Raw::deserialize(d)? {
            Raw::Arg { arg } => Ok(BindingValue::Arg(arg)),
            Raw::Word(w) if w == "readback" => Ok(BindingValue::Readback),
            Raw::Word(w) => Err(serde::de::Error::custom(format!("bad binding value {w:?}"))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BindingDirection {
    TwoWay,
    ReadOnly,
}

/// A `bind:<name>` binding, from the `bindings` annotations (docs/notes/fork-bindings.md section 6).
#[derive(Debug, Clone, Deserialize)]
pub struct BindingSpec {
    /// JS name of the bound property.
    pub property: String,
    pub direction: BindingDirection,
    /// The change signal the binding listens to (host name).
    pub event: String,
    pub value: BindingValue,
    pub echo: Echo,
    /// Compare with the last value before writing (GtkEntry re-notifies identical text).
    pub compare_before_set: bool,
    /// Read the property back after a write and converge state to it.
    pub read_back: bool,
    /// The native side clamps/validates writes.
    pub natively_normalizes: bool,
    /// Signals a programmatic set emits synchronously (the echo guard covers them).
    pub sync_signals_on_set: Vec<String>,
    /// `on_native_death`, `on_stale_update`, `on_destroy`.
    pub lifetime: BTreeMap<String, String>,
    #[serde(default)]
    pub provenance: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlotOrder {
    AppendOnly,
    BeforeSibling,
    NativeIndex,
    /// Position is not the order (a grid): `insert` ignores `before` (X3).
    Unordered,
    /// QWidget action lists (menus, toolbars: X4): ordered by `before` among the parent's actions.
    Actions,
    None,
    /// An order a newer generator emits that this compiler does not know yet: treated as
    /// ordered, so a new metadata value never stops a whole spec from loading.
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SlotSpec {
    pub many: bool,
    pub accepts: Vec<String>,
    pub route_by_kind: bool,
    pub order: SlotOrder,
    /// Per-child attributes of this slot (ABI v2.2 `set_child_attr`, `parent:<name>` in
    /// templates), by JS name.
    #[serde(default)]
    pub attrs: BTreeMap<String, AttrSpec>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AttrType {
    Bool,
    I32,
    U32,
    I64,
    F64,
    String,
    Enum,
}

/// A child attribute: its type, the value `null` (and remove + insert) resets it to, enum names.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct AttrSpec {
    #[serde(rename = "type")]
    pub ty: AttrType,
    #[serde(default)]
    pub default: Option<Value>,
    #[serde(default)]
    pub values: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChildrenSpec {
    pub policy: String,
    pub default_slot: Option<String>,
    pub slots: BTreeMap<String, SlotSpec>,
}

/// The style property table (metadata/style/<backend>.yaml through gen_semantics.py): how each
/// style property is applied. `css` ones are not the compiler's (they go to the X6 translator).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct StyleSpec {
    /// `css`, `qss` or `none` (a setter-only toolkit).
    #[serde(default)]
    pub dialect: String,
    /// What a setter rule's compound selector may hold: `type`, `class`, `id`.
    #[serde(default)]
    pub setter_selectors: Vec<String>,
    #[serde(default)]
    pub units: BTreeMap<String, UnitSpec>,
    #[serde(default)]
    pub properties: BTreeMap<String, StylePropSpec>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UnitSpec {
    pub scale: f64,
    #[serde(default)]
    pub provenance: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Via {
    /// The toolkit's own styling dialect (GTK CSS, QSS).
    Css,
    /// A widget property (`setProp`).
    Setter,
    /// A child attribute on the parent (`setChildAttr`).
    ChildAttr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueKind {
    /// Pixels: unitless or in one of `units`.
    Length,
    Int,
    Number,
    Bool,
    Enum,
    /// 1 to 4 lengths in CSS order, as `{ top, right, bottom, left }`.
    Insets,
    String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StylePropSpec {
    pub via: Via,
    /// Setter: the target property (JS name).
    #[serde(default)]
    pub prop: Option<String>,
    /// Child attribute: its name.
    #[serde(default)]
    pub attr: Option<String>,
    #[serde(default, rename = "type")]
    pub value: Option<ValueKind>,
    /// Setter: the tags it applies to; child attribute: the parents' tags.
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub values: Option<Vec<String>>,
    #[serde(default)]
    pub inline_only: bool,
    #[serde(default)]
    pub note: Option<String>,
}

impl Spec {
    pub fn parse(json: &str) -> Result<Spec, String> {
        let mut spec: Spec = serde_json::from_str(json).map_err(|e| format!("semantic spec: {e}"))?;
        if spec.schema != SCHEMA {
            return Err(format!("semantic spec: schema {:?}, expected {SCHEMA}", spec.schema));
        }
        for (id, k) in spec.kinds.iter_mut() {
            for p in k.props.values_mut() {
                p.owner = id.clone();
            }
            for s in k.signals.values_mut() {
                s.owner = id.clone();
            }
        }
        for (tag, kind) in &spec.tags {
            if !spec.kinds.contains_key(kind) {
                return Err(format!("semantic spec: tag {tag} names unknown kind {kind}"));
            }
        }
        Ok(spec)
    }
}
