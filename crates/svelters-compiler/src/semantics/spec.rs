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
    /// Optional (RT2, egui): inline text. In a text-content element whose slot takes `kind`, text
    /// mixed with child elements lowers to elements of `kind` (the egui runs) instead of being
    /// `text_content_children`; text alone still folds into the text-content prop.
    #[serde(default)]
    pub inline_text: Option<TextNodeSpec>,
    /// Optional: parse templates with HTML rules (void elements, implicit closing), i.e.
    /// `ParseOptions.native == false`. Only the DOM conformance backend sets it.
    #[serde(default)]
    pub html_syntax: bool,
    /// CX3 (CADATOMIC.md SV3): the bind-target lowering table. A `bind:` whose bound expression
    /// matches an entry lowers as it says (`link`: a store field tied to the prop in the host;
    /// `draft`: calls to the source's `get`/`set`), so providers extend `bind:` with data, never code.
    /// Fragments append to it.
    #[serde(default)]
    pub bind_sources: Vec<BindSourceSpec>,
}

/// CX3: one row of the bind-target lowering table.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BindSourceSpec {
    /// The import source of the bound expression's root identifier: exact (`svelters:app/task`,
    /// `#drafts`), or a prefix ending in `/*` (`svelters:app/*`).
    pub module: String,
    /// The imported name: the marker function of a `draft` source (`slot`), `*` for any export.
    pub export: String,
    pub lowering: BindSourceLowering,
    /// `draft`: the export of `module` the node's value is read with, `get(...args)`.
    #[serde(default)]
    pub get: Option<String>,
    /// `draft`: the export of `module` a new value goes to, `set(...args, value)`.
    #[serde(default)]
    pub set: Option<String>,
    #[serde(default)]
    pub doc: String,
}

/// CX3: how a bind source lowers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BindSourceLowering {
    /// `bind:prop={store.field}` with `store` an import of the module: `$n.bind_link(node, prop,
    /// () => store, "field", fallback)`, a native link in the host (no JS per change); `fallback` is
    /// the ordinary binding, for a host that refuses the link.
    Link,
    /// `bind:prop={marker(...args)}` with `marker` the source's export: a draft binding whose value is
    /// `get(...args)` and whose changes (or commits, with `|commit`) call `set(...args, value)`.
    Draft,
}

impl BindSourceSpec {
    /// Whether the import (`module`, `export`) is this source.
    pub fn matches(&self, module: &str, export: &str) -> bool {
        let m = match self.module.strip_suffix('*') {
            Some(prefix) => module.starts_with(prefix),
            None => module == self.module,
        };
        m && (self.export == "*" || self.export == export)
    }

    fn check(&self, what: &str) -> Result<(), String> {
        if self.lowering == BindSourceLowering::Draft && (self.get.is_none() || self.set.is_none() || self.export == "*") {
            return Err(format!("{what}: draft bind source {}#{} needs `get`, `set` and a named export", self.module, self.export));
        }
        Ok(())
    }
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
    /// A kind inserted into any element that `is_a` this kind or class, whatever that element's
    /// slots (GTK event controllers and context menus, the Qt context `menu`): `host.insert` with
    /// no slot and no position. None for every other kind.
    #[serde(default)]
    pub attaches_to: Option<String>,
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
    /// Whether and how a native change of the property can be observed on this platform (milestone
    /// R0, `metadata/observe/<tk>.json`). `None`: no fact (unknown), as in specs from before R0.
    #[serde(default)]
    pub observe: Option<ObserveSpec>,
    /// Kind id that declares the property (filled in at load).
    #[serde(skip)]
    pub owner: String,
}

/// Observability of a property on one platform (plans/research/REACTIVITY.md 4.1-4.3, decisions 13).
/// The compiler reads the class, the missed causes and the cost; the mechanism string is for
/// messages only (the mechanisms themselves go to the backend tables).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ObserveSpec {
    pub class: ObsClass,
    /// `partial` only: what the mechanisms miss (`user_edit`, `programmatic`, `cross_prop`, `parent`,
    /// `layout`, `wm`, `async`).
    #[serde(default)]
    pub missed: Vec<String>,
    /// One notifier shared by several properties (Qt `QAction::changed()`).
    #[serde(default)]
    pub coarse: bool,
    #[serde(default)]
    pub value_cost: ValueCost,
    #[serde(default)]
    pub timing: ObsTiming,
    /// `notify`, `notify+notification:NSControlTextDidChangeNotification`, `via:buffer.text`, ...
    #[serde(default)]
    pub mechanism: String,
    /// How strongly the fact is verified in the ledger (milestone OV1). Absent (specs from before
    /// OV1) reads as [`ObsVerified::None`].
    #[serde(default)]
    pub verified: ObsVerified,
    /// The fact's name for messages: `<Ns>:<Type>:<prop>` of `metadata/observe/<tk>.json`.
    #[serde(default)]
    pub fact: String,
}

/// Verification strength of an observe fact (`tools/metadata/common/observe.py` `verification`).
/// `bind:` lowers onto observation only for an `exact` fact that is [`ObsVerified::Delivery`]
/// (decision 13 #8 as implemented in OV1).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObsVerified {
    /// A generated row changed the property natively on the real backend and the change was
    /// announced (or, for a missed cause, was not).
    Delivery,
    /// A row only installed the mechanism (`connect`) or checked the backend's refusal.
    Weak,
    /// No passing row.
    #[default]
    None,
}

impl ObsVerified {
    /// The spec's word (`delivery`, `weak`, `none`).
    pub fn name(self) -> &'static str {
        match self {
            ObsVerified::Delivery => "delivery",
            ObsVerified::Weak => "weak",
            ObsVerified::None => "none",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObsClass {
    /// Never changes after construction (construct-only, Qt `CONSTANT`): a plain read.
    Constant,
    /// Only our setters change it: no native observation, setters invalidate locally; two-way
    /// `bind:` is an error (decision 6).
    SetOnly,
    /// Every native change is announced by the mechanisms: `bind:` lowers onto observation (decision 8).
    Exact,
    /// Some causes (`missed`) are not announced: allowed with a warning (decision 5).
    Partial,
    /// Native changes are not announced: a snapshot read; `bind:` is an error. No polling (decision 9).
    None,
}

impl ObsClass {
    /// A subscription is worth installing (the reactive getter observes).
    pub fn observable(self) -> bool {
        matches!(self, ObsClass::Exact | ObsClass::Partial)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueCost {
    #[default]
    Cheap,
    /// Whole-text properties: the host delivers an invalidation without reading and comparing.
    Proportional,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObsTiming {
    #[default]
    Sync,
    /// Announced later than the change (Qt Resize/Move of a hidden widget at `show()`).
    Deferred,
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
    /// CX3: the kind's commit event for `bind:<prop>|commit` (Enter, blur, release), carrying the
    /// committed value; absent when the kind has none (`|commit` is then `bind_commit_unsupported`).
    #[serde(default)]
    pub commit: Option<CommitSpec>,
}

/// CX3: where `bind:x|commit` takes its value: `args[arg]` of the commit event `event`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CommitSpec {
    pub event: String,
    pub arg: usize,
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
    /// `backend`: the backend runs the Rust CSS engine itself (egui, CS5e) and gets the sheets as
    /// CSS, so `<style>` rules are not lowered to setters at compile time (D9 on egui).
    #[serde(default)]
    pub engine: String,
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
        for b in &spec.bind_sources {
            b.check("semantic spec")?;
        }
        Ok(spec)
    }

    /// RA1: merges a spec fragment (`kinds` and `tags` only, e.g. the `kinds.json` an egui app binary
    /// dumps for its Rust-provided kinds, ARCH-EMBED §8.3) into this spec. A kind id or tag that is
    /// already there is an error, never a shadow; every tag must name a kind and every chain entry a
    /// known kind.
    pub fn merge_fragment(&mut self, json: &str) -> Result<(), String> {
        #[derive(Deserialize)]
        struct Fragment {
            schema: String,
            #[serde(default)]
            backend: Option<String>,
            #[serde(default)]
            tags: BTreeMap<String, String>,
            #[serde(default)]
            kinds: BTreeMap<String, KindSpec>,
            #[serde(default)]
            bind_sources: Vec<BindSourceSpec>,
        }
        let f: Fragment = serde_json::from_str(json).map_err(|e| format!("spec fragment: {e}"))?;
        if f.schema != SCHEMA {
            return Err(format!("spec fragment: schema {:?}, expected {SCHEMA}", f.schema));
        }
        if let Some(b) = &f.backend
            && *b != self.backend
        {
            return Err(format!("spec fragment for backend {b:?} cannot extend the {} spec", self.backend));
        }
        let added: Vec<String> = f.kinds.keys().cloned().collect();
        for (id, mut k) in f.kinds {
            if self.kinds.contains_key(&id) {
                return Err(format!("spec fragment: kind {id} is already defined"));
            }
            for p in k.props.values_mut() {
                p.owner = id.clone();
            }
            for s in k.signals.values_mut() {
                s.owner = id.clone();
            }
            self.kinds.insert(id, k);
        }
        for (tag, kind) in f.tags {
            if self.tags.contains_key(&tag) {
                return Err(format!("spec fragment: tag <{tag}> is already defined"));
            }
            if !self.kinds.contains_key(&kind) {
                return Err(format!("spec fragment: tag {tag} names unknown kind {kind}"));
            }
            self.tags.insert(tag, kind);
        }
        for id in &added {
            if let Some(c) = self.kinds[id].chain.iter().find(|c| !self.kinds.contains_key(*c)) {
                return Err(format!("spec fragment: kind {id} names unknown base {c}"));
            }
        }
        // CX3: bind sources append; the same (module, export, lowering) twice is an error
        for b in f.bind_sources {
            b.check("spec fragment")?;
            if self.bind_sources.iter().any(|x| x.module == b.module && x.export == b.export && x.lowering == b.lowering) {
                return Err(format!("spec fragment: bind source {}#{} is already declared", b.module, b.export));
            }
            self.bind_sources.push(b);
        }
        Ok(())
    }
}
