//! The JS-facing kinds ([`crate::surface`]) on skelter-core's kinds, and the wire ids the ABI and
//! `skelter.json` speak.
//!
//! **Kinds.** A surface kind whose widget skelter-widgets has written (the 18 Tier 0 controls and
//! containers, and the inline runs a `label` folds; not yet `window`) is registered through
//! `skelter_widgets::host::register_tier0`, `views::register_all`, the table's, tabs' and pickers'
//! `register` (`text-edit` directly: it is a `Kind` of its own); every other concrete kind gets a core kind synthesized
//! from its surface spec ([`Placeholder`]: the props, signals and slots are real tree members, the
//! node draws nothing) until its widget lands. Abstract bases (`Widget`, `Run`) have no core kind.
//!
//! **Members.** Each surface member maps to its core member by name:
//! - a prop the core kind declares is [`PropTarget::Native`]; the `Widget` props that are skelter-core's
//!   common props go to those (coordinator Q2: egui's JS names onto core's DOM names: `enabled` is
//!   `!disabled`, `visible` is `!hidden`, `accessible-name` is `aria-label`, `tooltip` is `title`);
//! - a prop the core kind lacks (a skelter-widgets kind that does not take a `Widget` layout prop
//!   yet) is [`PropTarget::Side`]: the backend stores it, nothing native reads it;
//! - a signal is the core kind's of that name, else the common one (`click`, `keydown`), else none
//!   (it never fires: `drag-begin` until skelter's drag model lands).
//!
//! **Wire ids** (coordinator Q1: skelter-core's FNV-1a ids, no table): a member's id is skelter-core's
//! hash of (declaring kind, member, type), the declaring kind being the tag (`button`), `*` for the
//! `Widget` base (core's common pseudo-kind) and the lowercased name for other bases. So a kind's own
//! native member has its core id on the wire, an inherited one has one id on every kind, and a type
//! change changes the id (the spec hash then forces a recompile). Kinds are `KindId::of(tag)`, kept
//! out of the host's app-export pack ([`kind_id_wire`]).

use crate::surface::{self, DefaultValue, KindSpec, MethodSpec, PropSpec, SignalSpec, SlotSpec, ValueType};
use skelter_base::accesskit::Role;
use skelter_core::{self as core, KindRegistry, RegistryError};
use std::collections::HashMap;
use std::sync::Arc;
use svelters_abi::svl_id;

/// Where a surface prop lives in the tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropTarget {
    /// The core prop, value as is.
    Native(core::PropId),
    /// A core bool prop holding the negation (`enabled` ↔ `disabled`).
    Inverted(core::PropId),
    /// No core prop: the backend keeps the value.
    Side,
}

#[derive(Debug)]
pub struct PropMap {
    pub spec: &'static PropSpec,
    /// The kind that declares it (`Widget` for `enabled`).
    pub decl: &'static KindSpec,
    pub wire: svl_id,
    pub target: PropTarget,
}

#[derive(Debug)]
pub struct SignalMap {
    pub spec: &'static SignalSpec,
    pub decl: &'static KindSpec,
    pub wire: svl_id,
    /// The core signal that fires it; `None`: skelter does not produce it yet.
    pub native: Option<core::SignalId>,
}

#[derive(Debug)]
pub struct MethodMap {
    pub spec: &'static MethodSpec,
    pub decl: &'static KindSpec,
    pub wire: svl_id,
}

#[derive(Debug)]
pub struct SlotMap {
    pub spec: &'static SlotSpec,
    pub decl: &'static KindSpec,
    pub wire: svl_id,
    /// The core slot (`None`: the default slot, the first surface slot).
    pub native: Option<core::SlotId>,
}

/// One surface kind and its members, nearest declaration first.
#[derive(Debug)]
pub struct KindMap {
    pub surface: &'static KindSpec,
    /// `None` for an abstract base.
    pub core: Option<core::KindId>,
    pub wire: svl_id,
    /// Its widget is skelter-widgets' or the app's (not a [`Placeholder`]).
    pub hosted: bool,
    /// The app registered its core kind ([`Kinds::with_app`]).
    pub app: bool,
    pub props: Vec<PropMap>,
    pub signals: Vec<SignalMap>,
    pub methods: Vec<MethodMap>,
    pub slots: Vec<SlotMap>,
}

impl KindMap {
    pub fn tag(&self) -> &'static str {
        self.surface.tag.unwrap_or(self.surface.name)
    }
    pub fn prop(&self, wire: svl_id) -> Option<&PropMap> {
        self.props.iter().find(|p| p.wire == wire)
    }
    pub fn prop_named(&self, name: &str) -> Option<&PropMap> {
        self.props.iter().find(|p| p.spec.name == name)
    }
    pub fn signal(&self, wire: svl_id) -> Option<&SignalMap> {
        self.signals.iter().find(|s| s.wire == wire)
    }
    pub fn signal_named(&self, name: &str) -> Option<&SignalMap> {
        self.signals.iter().find(|s| s.spec.name == name)
    }
    pub fn method(&self, wire: svl_id) -> Option<&MethodMap> {
        self.methods.iter().find(|m| m.wire == wire)
    }
    /// Wire slot id → core slot (`Some(None)`: the default slot). 0 is the default slot.
    pub fn slot(&self, wire: svl_id) -> Option<Option<core::SlotId>> {
        if wire == 0 {
            return Some(None);
        }
        self.slots.iter().find(|s| s.wire == wire).map(|s| s.native)
    }
    /// The surface signal a core signal fires.
    pub fn of_native_signal(&self, s: core::SignalId) -> Option<&SignalMap> {
        self.signals.iter().find(|m| m.native == Some(s))
    }
    /// The surface props a core prop holds (an inverted alias and a same-named prop can share one).
    pub fn of_native_prop(&self, p: core::PropId) -> impl Iterator<Item = &PropMap> {
        self.props.iter().filter(move |m| matches!(m.target, PropTarget::Native(x) | PropTarget::Inverted(x) if x == p))
    }
}

/// The `Widget` props that are skelter-core's common props: (JS name, core name, inverted).
pub const COMMON_ALIASES: &[(&str, &str, bool)] = &[
    ("enabled", "disabled", true),
    ("visible", "hidden", true),
    ("accessible-name", "aria-label", false),
    ("tooltip", "title", false),
    ("autofocus", "autofocus", false),
];

/// Surface tags whose core kind has another tag (A9): egui's `combo-box` is skelter-widgets'
/// `select`, its `action` a `menu-item`. CSS type selectors match the core tag.
pub const CORE_TAGS: &[(&str, &str)] = &[("combo-box", "select"), ("action", "menu-item")];

/// The core kind's tag for surface tag `tag`.
pub fn core_tag(tag: &str) -> &str {
    CORE_TAGS.iter().find(|(s, _)| *s == tag).map_or(tag, |(_, c)| c)
}

/// A10: registers svelters-skelter's built-in kinds ([`surface::builtin`]: skelter-widgets' where it
/// has them, placeholders otherwise) into a host's own `reg`, so a host that builds its registry
/// (its own kinds first) can hand its `Document` to [`crate::SkelterBackend::shared`] with
/// [`crate::builtin_kinds`]: the kinds, member ids and tags are the same as the built-in registry's.
/// A host kind with a built-in kind's tag is an error ([`RegistryError::DuplicateKind`]).
pub fn register_builtin(reg: &mut KindRegistry) -> Result<(), RegistryError> {
    register_hosted(reg)?;
    for k in surface::builtin() {
        if let Some(t) = k.tag
            && reg.by_name(core_tag(t)).is_none()
        {
            reg.register_with(synthesize(k), crate::window::factory(t).unwrap_or(placeholder))?;
        }
    }
    Ok(())
}

/// The kinds skelter-widgets has written, registered through its host bridge.
fn register_hosted(reg: &mut KindRegistry) -> Result<Vec<core::KindId>, RegistryError> {
    let before: Vec<core::KindId> = reg.iter().map(|e| e.id).collect();
    skelter_widgets::host::register_tier0(reg)?;
    // every view (scroll area, panels, splits, lists, trees, popups, menus, select, tooltip, toasts,
    // modal, inner window, status bar, dialog buttons; `surface::tier1`), the table, tabs and pickers
    skelter_widgets::host::register::<skelter_widgets::controls::CollapsingHeader>(reg)?;
    skelter_widgets::views::register_all(reg)?;
    skelter_widgets::table::kinds::register(reg)?;
    skelter_widgets::tabs::kinds::register(reg)?;
    skelter_widgets::pickers::kinds::register(reg)?;
    // a `Kind` of its own (IME, the text-input client: not the host bridge)
    reg.register::<skelter_widgets::text::TextEdit>()?;
    Ok(reg.iter().map(|e| e.id).filter(|k| !before.contains(k)).collect())
}

/// Whether surface type `t` can live in core prop `c`: the same core type and, for an enum, every
/// surface name among core's values (the index is remapped by name, [`enum_to_core`]). A surface
/// integer lives in a core float (egui's `indent: int` is the tree's `indent: float`, A9b): the
/// value is converted both ways ([`num_to_core`], [`num_from_core`]).
fn fits(t: ValueType, c: &core::PropSpec) -> bool {
    t != ValueType::Any
        && (c.ty == core_type(t) || (t == ValueType::I64 && c.ty == core::ValueType::F64))
        && match t {
            ValueType::Enum(names) => names.iter().all(|n| c.enum_values.contains(n)),
            _ => true,
        }
}

/// A surface value for core prop `c` of type `t` (an integer for a float prop becomes a float).
pub fn num_to_core(t: ValueType, c: &core::PropSpec, v: core::Value) -> core::Value {
    match (t, c.ty, v) {
        (ValueType::I64, core::ValueType::F64, core::Value::I64(i)) => core::Value::F64(i as f64),
        (_, _, v) => v,
    }
}

/// Core prop `c`'s value as surface type `t` (a float prop read as an integer is rounded).
pub fn num_from_core(t: ValueType, c: &core::PropSpec, v: core::Value) -> core::Value {
    match (t, c.ty, v) {
        (ValueType::I64, core::ValueType::F64, core::Value::F64(f)) => core::Value::I64(f.round() as i64),
        (_, _, v) => v,
    }
}

/// A surface enum index as core prop `c`'s index (by name).
pub fn enum_to_core(t: ValueType, c: &core::PropSpec, i: u16) -> Option<u32> {
    let ValueType::Enum(names) = t else { return Some(i as u32) };
    let name = names.get(i as usize)?;
    c.enum_values.iter().position(|v| v == name).map(|p| p as u32)
}

/// Core prop `c`'s enum index as the surface index (by name).
pub fn enum_from_core(t: ValueType, c: &core::PropSpec, i: u32) -> Option<u16> {
    let ValueType::Enum(names) = t else { return Some(i as u16) };
    let name = c.enum_values.get(i as usize)?;
    names.iter().position(|v| v == name).map(|p| p as u16)
}

/// The core spec of prop `p` on core kind `k` (its own, else a common one).
pub fn core_prop(reg: &KindRegistry, k: core::KindId, p: core::PropId) -> Option<&'static core::PropSpec> {
    reg.get(k).and_then(|e| e.prop(p)).or_else(|| core::common::prop(p))
}

/// The core type of a surface type (lists are lists, colours colours).
pub fn core_type(t: ValueType) -> core::ValueType {
    match t {
        ValueType::Bool => core::ValueType::Bool,
        ValueType::I64 => core::ValueType::I64,
        ValueType::F64 => core::ValueType::F64,
        ValueType::Str => core::ValueType::Str,
        ValueType::Enum(_) => core::ValueType::Enum,
        ValueType::Color => core::ValueType::Color,
        ValueType::StrList | ValueType::NumList | ValueType::IdList => core::ValueType::List,
        // stored by the backend ([`PropTarget::Side`]): never a core prop
        ValueType::Record | ValueType::Any => core::ValueType::Record,
    }
}

/// A signal's one core argument for its surface arguments: none, the one, or a record of them.
pub fn signal_arg_type(args: &[(&str, ValueType)]) -> Option<core::ValueType> {
    match args {
        [] => None,
        [(_, t)] => Some(core_type(*t)),
        _ => Some(core::ValueType::Record),
    }
}

/// The kind name member ids hash with (see the module docs).
pub fn decl_key(k: &KindSpec) -> String {
    match k.tag {
        Some(t) => t.to_string(),
        None if k.name == "Widget" => "*".to_string(),
        None => k.name.to_ascii_lowercase(),
    }
}

/// Wire ids live in the host's id space: the host splits the top bits of a signal id off as listen
/// flags (`SVL_SIGNAL_FLAGS_SHIFT`), so every wire id is skelter-core's fingerprint masked to
/// `SVL_SIGNAL_ID_MASK` (A18; the spec test fails on a collision after masking).
pub const WIRE_MASK: svl_id = svelters_abi::SVL_SIGNAL_ID_MASK;

/// The host's id pack of app exports' kinds and types (svelters-host `exports::APP_PACK`, bits
/// 27..22 all set, CUSTOM-WIDGETS.md §3.2): the host sends a kind id there to the app schema, so a
/// built-in kind's wire id stays out of it.
pub const APP_PACK: svl_id = 0x3F << 22;

/// The wire id of a kind whose core id is `core` (a skelter-core [`core::KindId`] hash): masked to
/// the host's id space ([`WIRE_MASK`]) and moved out of the app pack (bit 22 cleared; `modal` and
/// `toast-host` hash into it).
pub fn kind_id_wire(core: u32) -> svl_id {
    let w = core & WIRE_MASK;
    if w & APP_PACK == APP_PACK { w & !(1 << 22) } else { w }
}

pub fn kind_wire(k: &KindSpec) -> svl_id {
    match k.tag {
        Some(t) => kind_id_wire(core::KindId::of(t).0),
        None => kind_id_wire(core::KindId::of(&decl_key(k)).0),
    }
}

pub fn prop_wire(decl: &KindSpec, p: &PropSpec) -> svl_id {
    core::PropId::of(&decl_key(decl), p.name, core_type(p.ty).name()).0 & WIRE_MASK
}

pub fn signal_wire(decl: &KindSpec, s: &SignalSpec) -> svl_id {
    core::SignalId::of(&decl_key(decl), s.name, signal_arg_type(s.args).map_or("void", core::ValueType::name)).0 & WIRE_MASK
}

pub fn method_wire(decl: &KindSpec, m: &MethodSpec) -> svl_id {
    core::MethodId::of(&decl_key(decl), m.name, m.ret.map_or("void", |t| core_type(t).name())).0 & WIRE_MASK
}

pub fn slot_wire(decl: &KindSpec, s: &SlotSpec) -> svl_id {
    core::SlotId::of(&decl_key(decl), s.name, "slot").0 & WIRE_MASK
}

/// The surface kind that owns `k`'s slots (the nearest in its chain with any).
fn slot_owner(k: &'static KindSpec) -> Option<&'static KindSpec> {
    k.chain().find(|c| !c.slots.is_empty())
}

/// A surface prop that is a common prop: its core name and whether it is inverted.
fn common_alias(decl: &KindSpec, p: &PropSpec) -> Option<(core::PropId, bool)> {
    // the abstract bases' props (`Widget`'s, and `Run`'s `accessible-name`)
    if decl.tag.is_some() {
        return None;
    }
    if let Some(&(_, native, inv)) = COMMON_ALIASES.iter().find(|a| a.0 == p.name) {
        return core::common::COMMON.prop_named(native).map(|id| (id, inv));
    }
    // a common prop of the same name and a type it can hold (`anchor`, `drag-type`, `dialog-role`)
    core::common::COMMON.prop_named(p.name).filter(|&id| core::common::prop(id).is_some_and(|c| fits(p.ty, c))).map(|id| (id, false))
}

fn role_of(name: &str) -> Role {
    match name {
        "Button" => Role::Button,
        "CheckBox" => Role::CheckBox,
        "Image" => Role::Image,
        "Label" => Role::Label,
        "Link" => Role::Link,
        "ProgressIndicator" => Role::ProgressIndicator,
        "RadioButton" => Role::RadioButton,
        "Slider" => Role::Slider,
        "Switch" => Role::Switch,
        "TextInput" => Role::TextInput,
        "Window" => Role::Window,
        _ => Role::GenericContainer,
    }
}

fn core_default(p: &PropSpec) -> core::PropDefault {
    match p.default {
        DefaultValue::Null => core::PropDefault::Unset,
        DefaultValue::Bool(b) => core::PropDefault::Bool(b),
        DefaultValue::I64(i) => core::PropDefault::I64(i),
        DefaultValue::F64(f) => core::PropDefault::F64(f),
        DefaultValue::Str(s) => core::PropDefault::Str(s),
        DefaultValue::Enum(i) => core::PropDefault::Enum(i as u32),
    }
}

/// skelter-core's spec for a surface kind with no skelter-widgets widget yet: every member of its
/// chain (nearest first) that is not a common one, its named slots, its role.
fn synthesize(k: &'static KindSpec) -> &'static core::KindSpec {
    let mut props: Vec<core::PropSpec> = Vec::new();
    for decl in k.chain() {
        for p in decl.props {
            if common_alias(decl, p).is_some() || p.ty == ValueType::Any || props.iter().any(|q| q.name == p.name) {
                continue;
            }
            let mut c = core::PropSpec::new(p.name, core_type(p.ty), p.change).default(core_default(p));
            if let ValueType::Enum(names) = p.ty {
                c = c.enum_values(names);
            }
            if p.native {
                c = c.native();
            }
            if p.nullable {
                c = c.nullable();
            }
            props.push(c.doc(p.doc));
        }
    }
    let mut signals: Vec<core::SignalSpec> = Vec::new();
    for decl in k.chain() {
        for s in decl.signals {
            if core::common::COMMON.signal_named(s.name).is_some() || signals.iter().any(|q| q.name == s.name) {
                continue;
            }
            let mut f = core::SignalFlags::NONE;
            for (on, flag) in [(s.passive, core::SignalFlags::PASSIVE), (s.cancelable, core::SignalFlags::CANCELABLE), (s.sync, core::SignalFlags::SYNC), (s.bubbles, core::SignalFlags::BUBBLES)] {
                if on {
                    f = f.union(flag);
                }
            }
            signals.push(core::SignalSpec::new(s.name, signal_arg_type(s.args), f).args(core_args(s.args)).doc(s.doc));
        }
    }
    let mut methods: Vec<core::MethodSpec> = Vec::new();
    for decl in k.chain() {
        for m in decl.methods {
            if !methods.iter().any(|q| q.name == m.name) {
                let ret = m.ret.map(|t| core::ArgSpec::new("return", core_type(t)));
                methods.push(core::MethodSpec::new(m.name, core_args(m.params), ret).doc(m.doc));
            }
        }
    }
    let owner = slot_owner(k);
    let slots: Vec<core::SlotSpec> = owner.map_or(&[][..], |o| &o.slots[1..]).iter().map(|s| core::SlotSpec { name: s.name, single: !s.many }).collect();
    let text_prop = k.text_content.filter(|t| props.iter().any(|p| p.name == *t));
    // egui's name sources, as far as they name a source (descriptions like "tooltip (when …)" do not)
    let a11y = k.chain().find(|c| !c.a11y.name_from.is_empty()).map_or(&[][..], |c| c.a11y.name_from);
    let mut name_from: Vec<core::NameSource> = Vec::new();
    for src in a11y {
        let n = match *src {
            "accessible-name" => core::NameSource::AriaLabel,
            t if Some(t) == text_prop => core::NameSource::Text,
            t if props.iter().any(|p| p.name == t) => core::NameSource::Prop(props.iter().find(|p| p.name == t).expect("found").name),
            _ => continue,
        };
        name_from.push(n);
    }
    Box::leak(Box::new(core::KindSpec {
        name: k.tag.expect("concrete kind"),
        props: Box::leak(props.into_boxed_slice()),
        signals: Box::leak(signals.into_boxed_slice()),
        slots: Box::leak(slots.into_boxed_slice()),
        role: role_of(k.a11y.role),
        text_prop,
        focusable: k.signals.iter().any(|s| s.name == "clicked") || matches!(k.a11y.role, "TextInput" | "Slider"),
        container: owner.is_some(),
        methods: Box::leak(methods.into_boxed_slice()),
        name_from: if name_from.is_empty() { core::kind::DEFAULT_NAME_FROM } else { Box::leak(name_from.into_boxed_slice()) },
        doc: k.doc,
        // what a placeholder does not have (folded or placed children, parts, prop states) stays core's default
        ..core::KindSpec::new("")
    }))
}

/// Named, typed arguments in core's form.
fn core_args(args: &[(&'static str, ValueType)]) -> &'static [core::ArgSpec] {
    if args.is_empty() {
        return &[];
    }
    Box::leak(args.iter().map(|(n, t)| core::ArgSpec::new(n, core_type(*t))).collect::<Vec<_>>().into_boxed_slice())
}

/// The surface type of a core type (a list is a list of strings; an enum's names are the prop's).
pub(crate) fn surface_type(t: core::ValueType, names: &'static [&'static str]) -> ValueType {
    match t {
        core::ValueType::Bool => ValueType::Bool,
        core::ValueType::I64 => ValueType::I64,
        core::ValueType::F64 => ValueType::F64,
        core::ValueType::Str => ValueType::Str,
        core::ValueType::Enum => ValueType::Enum(names),
        core::ValueType::Color => ValueType::Color,
        core::ValueType::List => ValueType::StrList,
        core::ValueType::Record => ValueType::Record,
    }
}

fn surface_args(args: &[core::ArgSpec]) -> &'static [(&'static str, ValueType)] {
    if args.is_empty() {
        return &[];
    }
    Box::leak(args.iter().map(|a| (a.name, surface_type(a.ty, &[]))).collect::<Vec<_>>().into_boxed_slice())
}

/// `property-grid` → `PropertyGrid`.
fn kind_name(tag: &str) -> &'static str {
    let mut name = String::new();
    for w in tag.split(['-', '_']) {
        let mut c = w.chars();
        if let Some(f) = c.next() {
            name.push(f.to_ascii_uppercase());
            name.extend(c);
        }
    }
    Box::leak(name.into_boxed_str())
}

/// The JS-facing spec of an app's core kind (A1): its props (an `Unset` default reads as `null`, so
/// such a prop is nullable), signals (an unnamed argument is `value`), methods, slots (a container
/// has the default `children` slot before its named ones), role, name sources, text prop and docs,
/// on the `Widget` base. Its own members' wire ids are the core kind's ids ([`prop_wire`] hashes
/// under the tag, as skelter-core does).
pub fn surface_of(k: &'static core::KindSpec) -> &'static KindSpec {
    let props: Vec<PropSpec> = k
        .props
        .iter()
        .map(|p| PropSpec {
            name: p.name,
            ty: surface_type(p.ty, p.enum_values),
            default: match p.default {
                core::PropDefault::Unset | core::PropDefault::Color(_) => DefaultValue::Null,
                core::PropDefault::Bool(b) => DefaultValue::Bool(b),
                core::PropDefault::I64(i) => DefaultValue::I64(i),
                core::PropDefault::F64(f) => DefaultValue::F64(f),
                core::PropDefault::Str(s) => DefaultValue::Str(s),
                core::PropDefault::Enum(i) => DefaultValue::Enum(i as u16),
            },
            nullable: p.nullable || matches!(p.default, core::PropDefault::Unset),
            change: p.change,
            native: p.native_owned,
            doc: p.doc,
        })
        .collect();
    let signals: Vec<SignalSpec> = k
        .signals
        .iter()
        .map(|s| SignalSpec {
            name: s.name,
            args: match (s.args.is_empty(), s.arg) {
                (false, _) => surface_args(s.args),
                (true, Some(t)) => Box::leak(Box::new([("value", surface_type(t, &[]))])),
                (true, None) => &[],
            },
            passive: s.flags.contains(core::SignalFlags::PASSIVE),
            cancelable: s.flags.contains(core::SignalFlags::CANCELABLE),
            sync: s.flags.contains(core::SignalFlags::SYNC),
            bubbles: s.flags.contains(core::SignalFlags::BUBBLES),
            doc: s.doc,
        })
        .collect();
    let methods: Vec<MethodSpec> = k.methods.iter().map(|m| MethodSpec { name: m.name, params: surface_args(m.params), ret: m.ret.map(|r| surface_type(r.ty, &[])), doc: m.doc }).collect();
    let mut slots: Vec<SlotSpec> = Vec::new();
    if k.container || !k.slots.is_empty() {
        slots.push(SlotSpec { name: "children", accepts: &["Widget"], many: true });
        slots.extend(k.slots.iter().map(|s| SlotSpec { name: s.name, accepts: &["Widget"], many: !s.single }));
    }
    let name_from: Vec<&'static str> = k
        .name_from
        .iter()
        .filter_map(|n| match *n {
            core::NameSource::AriaLabel => Some("accessible-name"),
            core::NameSource::Text => k.text_prop,
            core::NameSource::Prop(p) => Some(p),
            core::NameSource::Children => Some("children"),
        })
        .collect();
    let role: &'static str = match k.role {
        Role::GenericContainer => "",
        r => Box::leak(format!("{r:?}").into_boxed_str()),
    };
    let native_state: Vec<&'static str> = k.props.iter().filter(|p| p.native_owned).map(|p| p.name).collect();
    Box::leak(Box::new(KindSpec {
        name: kind_name(k.name),
        tag: Some(k.name),
        extends: Some(&surface::tier0::WIDGET),
        doc: k.doc,
        props: Box::leak(props.into_boxed_slice()),
        signals: Box::leak(signals.into_boxed_slice()),
        methods: Box::leak(methods.into_boxed_slice()),
        slots: Box::leak(slots.into_boxed_slice()),
        a11y: surface::A11ySpec { role, name_from: Box::leak(name_from.into_boxed_slice()), name_required: false },
        text_content: k.text_prop,
        toplevel: false,
        native_state: Box::leak(native_state.into_boxed_slice()),
    }))
}

/// The node value of a synthesized kind: lays out by the default rules, draws nothing.
#[derive(Default)]
pub struct Placeholder;

static PLACEHOLDER: core::KindSpec = core::KindSpec::new("placeholder");

impl core::Kind for Placeholder {
    fn spec() -> &'static core::KindSpec {
        &PLACEHOLDER
    }
    fn record(&self, _cx: &mut core::RecordCx) {}
}

fn placeholder() -> Box<dyn core::Kind> {
    Box::new(Placeholder)
}

/// The surface kinds over one skelter-core registry.
pub struct Kinds {
    core: Arc<KindRegistry>,
    list: Vec<KindMap>,
    by_wire: HashMap<svl_id, usize>,
    by_tag: HashMap<&'static str, usize>,
    by_core: HashMap<core::KindId, usize>,
}

impl Kinds {
    /// The Tier 0 kinds: skelter-widgets' where it has them, synthesized otherwise.
    pub fn builtin() -> Result<Kinds, RegistryError> {
        Kinds::new(&surface::builtin())
    }

    /// The kinds of `surface` (bases included).
    pub fn new(surface: &[&'static KindSpec]) -> Result<Kinds, RegistryError> {
        Kinds::with_app(surface, |_| Ok(()))
    }

    /// The kinds of `surface` plus the app's (A1): `app` registers its core kinds (its own widgets)
    /// on the registry after skelter-widgets' and before any placeholder is synthesized. An app kind
    /// gets its JS-facing spec from its core `KindSpec` ([`surface_of`]: docs, methods, signal
    /// arguments, nullable props) unless `surface` has a spec with its tag: that one is used instead
    /// (the override, for names the core spec does not carry, such as egui's JS names). A surface
    /// kind that would be a placeholder is the app's real kind when the app registers its tag.
    /// `surface` must hold [`surface::tier0::WIDGET`], the base of derived kinds.
    pub fn with_app(surface: &[&'static KindSpec], app: impl FnOnce(&mut KindRegistry) -> Result<(), RegistryError>) -> Result<Kinds, RegistryError> {
        let mut reg = KindRegistry::new();
        let mut hosted = register_hosted(&mut reg)?;
        let before: Vec<core::KindId> = reg.iter().map(|e| e.id).collect();
        app(&mut reg)?;
        let added: Vec<core::KindId> = reg.iter().map(|e| e.id).filter(|k| !before.contains(k)).collect();
        hosted.extend(added.iter().copied());
        let mut all: Vec<&'static KindSpec> = surface.to_vec();
        for id in &added {
            let spec = reg.get(*id).expect("registered").spec;
            if !surface.iter().any(|k| k.tag == Some(spec.name)) {
                all.push(surface_of(spec));
            }
        }
        for k in &all {
            if let Some(t) = k.tag
                && reg.by_name(core_tag(t)).is_none()
            {
                reg.register_with(synthesize(k), crate::window::factory(t).unwrap_or(placeholder))?;
            }
        }
        let reg = Arc::new(reg);
        let mut list = Vec::new();
        for &k in &all {
            let core = k.tag.and_then(|t| reg.by_name(core_tag(t)));
            let mut m = Kinds::map(k, core, hosted.iter().any(|h| Some(*h) == core.map(|e| e.id)));
            m.app = m.core.is_some_and(|c| added.contains(&c));
            list.push(m);
        }
        let by_wire = list.iter().enumerate().map(|(i, m)| (m.wire, i)).collect();
        let by_tag = list.iter().enumerate().filter_map(|(i, m)| Some((m.surface.tag?, i))).collect();
        let by_core = list.iter().enumerate().filter_map(|(i, m)| Some((m.core?, i))).collect();
        Ok(Kinds { core: reg, list, by_wire, by_tag, by_core })
    }

    fn map(k: &'static KindSpec, entry: Option<&core::KindEntry>, hosted: bool) -> KindMap {
        let mut props: Vec<PropMap> = Vec::new();
        let mut signals: Vec<SignalMap> = Vec::new();
        let mut methods: Vec<MethodMap> = Vec::new();
        for decl in k.chain() {
            for p in decl.props {
                if props.iter().any(|m| m.spec.name == p.name) {
                    continue;
                }
                let target = match (common_alias(decl, p), entry) {
                    (Some((id, true)), _) => PropTarget::Inverted(id),
                    (Some((id, false)), _) => PropTarget::Native(id),
                    // the kind's own prop, else a common one of that name and type (`focus-scope`)
                    (None, Some(e)) => e
                        .spec
                        .prop_named(p.name)
                        .filter(|&id| e.prop(id).is_some_and(|s| fits(p.ty, s)))
                        .or_else(|| core::common::COMMON.prop_named(p.name).filter(|&id| core::common::prop(id).is_some_and(|s| fits(p.ty, s))))
                        .map_or(PropTarget::Side, PropTarget::Native),
                    (None, None) => PropTarget::Side,
                };
                props.push(PropMap { spec: p, decl, wire: prop_wire(decl, p), target });
            }
            for s in decl.signals {
                if signals.iter().any(|m| m.spec.name == s.name) {
                    continue;
                }
                let native = entry.and_then(|e| e.spec.signal_named(s.name)).or_else(|| core::common::COMMON.signal_named(s.name));
                signals.push(SignalMap { spec: s, decl, wire: signal_wire(decl, s), native: if entry.is_some() { native } else { None } });
            }
            for m in decl.methods {
                if !methods.iter().any(|x| x.spec.name == m.name) {
                    methods.push(MethodMap { spec: m, decl, wire: method_wire(decl, m) });
                }
            }
        }
        let slots = match slot_owner(k) {
            Some(o) => o
                .slots
                .iter()
                .enumerate()
                .map(|(i, s)| SlotMap { spec: s, decl: o, wire: slot_wire(o, s), native: if i == 0 { None } else { entry.and_then(|e| e.spec.slot_named(s.name)) } })
                .collect(),
            None => Vec::new(),
        };
        KindMap { surface: k, core: entry.map(|e| e.id), wire: kind_wire(k), hosted, app: false, props, signals, methods, slots }
    }

    pub fn registry(&self) -> &Arc<KindRegistry> {
        &self.core
    }

    pub fn iter(&self) -> impl Iterator<Item = &KindMap> {
        self.list.iter()
    }

    pub fn by_wire(&self, wire: svl_id) -> Option<&KindMap> {
        self.by_wire.get(&wire).map(|&i| &self.list[i])
    }

    pub fn by_tag(&self, tag: &str) -> Option<&KindMap> {
        self.by_tag.get(tag).map(|&i| &self.list[i])
    }

    pub fn by_core(&self, k: core::KindId) -> Option<&KindMap> {
        self.by_core.get(&k).map(|&i| &self.list[i])
    }

    /// The surface specs, bases first.
    pub fn surface(&self) -> impl Iterator<Item = &'static KindSpec> + '_ {
        self.list.iter().map(|m| m.surface)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_kinds_register_and_map() {
        let k = Kinds::builtin().unwrap();
        let button = k.by_tag("button").unwrap();
        assert!(button.hosted, "button is skelter-widgets'");
        assert_eq!(button.wire, core::KindId::of("button").0 & WIRE_MASK);
        // Q2: egui's names onto core's common props
        assert_eq!(button.prop_named("enabled").unwrap().target, PropTarget::Inverted(core::common::DISABLED));
        assert_eq!(button.prop_named("accessible-name").unwrap().target, PropTarget::Native(core::common::ARIA_LABEL));
        // Q1: a kind's own native member has its core id on the wire
        let text = button.prop_named("text").unwrap();
        assert_eq!(text.target, PropTarget::Native(core::PropId::of("button", "text", "string")));
        assert_eq!(text.wire, core::PropId::of("button", "text", "string").0 & WIRE_MASK);
        assert_eq!(button.signal_named("keydown").unwrap().native, Some(core::common::KEYDOWN));
        // inherited members have one id on every kind
        let label = k.by_tag("label").unwrap();
        assert_eq!(label.prop_named("enabled").unwrap().wire, button.prop_named("enabled").unwrap().wire);
        assert!(label.hosted);
        assert!(k.by_tag("slider").unwrap().hosted);
        assert!(k.by_tag("text-edit").unwrap().hosted, "text-edit is skelter-widgets' (its AccessKit text runs, qc-a11y §3.2)");
        assert!(k.by_tag("window").unwrap().core.is_some());
    }
}
