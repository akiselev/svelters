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
    /// Element tag -> kind id.
    pub tags: BTreeMap<String, String>,
    pub kinds: BTreeMap<String, KindSpec>,
    pub style: StyleSpec,
    /// Tags this backend lacks, with advice (diagnostics only).
    #[serde(default)]
    pub element_hints: BTreeMap<String, String>,
    #[serde(default)]
    pub source: Value,
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
    #[serde(default)]
    pub methods: Vec<String>,
    #[serde(default)]
    pub provenance: BTreeMap<String, String>,
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
    /// Object: kind id; list: item type.
    #[serde(default)]
    pub of: Option<String>,
    #[serde(default)]
    pub range: Option<Range>,
    #[serde(default, rename = "virtual")]
    pub is_virtual: bool,
    /// The native change notification (GTK `notify::x`, Qt NOTIFY signature), if any. Its
    /// presence does not mean the backend exposes it as a signal.
    #[serde(default)]
    pub notify: Option<String>,
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

#[derive(Debug, Clone, Deserialize)]
pub struct BindingSpec {
    pub event: String,
    pub value: BindingValue,
    pub echo: Echo,
    #[serde(default)]
    pub provenance: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlotOrder {
    AppendOnly,
    BeforeSibling,
    NativeIndex,
    None,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SlotSpec {
    pub many: bool,
    pub accepts: Vec<String>,
    pub route_by_kind: bool,
    pub order: SlotOrder,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChildrenSpec {
    pub policy: String,
    pub default_slot: Option<String>,
    pub slots: BTreeMap<String, SlotSpec>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StyleSpec {
    pub units: BTreeMap<String, UnitSpec>,
    pub declarations: BTreeMap<String, DeclarationSpec>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UnitSpec {
    pub scale: f64,
    #[serde(default)]
    pub provenance: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Contract {
    /// Same meaning on every backend; lowers to one property.
    Common,
    /// A design intent each backend maps to its own convention (or rejects).
    NativeIntent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueKind {
    Enum,
    Length,
    Bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum IntentAction {
    Set { prop: String, value: Value },
    AddToken { prop: String, token: String },
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeclarationSpec {
    /// Kind id or class the element must be (`is_a`).
    pub applies_to: String,
    pub contract: Contract,
    #[serde(default)]
    pub provenance: String,
    /// Common: target property (JS name).
    #[serde(default)]
    pub prop: Option<String>,
    #[serde(default)]
    pub value: Option<ValueKind>,
    /// Native intent: intent name -> actions (empty = leave the native default).
    #[serde(default)]
    pub intents: BTreeMap<String, Vec<IntentAction>>,
    /// Native intent names this backend knowingly cannot express, with the reason.
    #[serde(default)]
    pub unsupported: BTreeMap<String, String>,
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
