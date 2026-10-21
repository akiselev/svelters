//! Types of the generated kind tables (`generated/tables.rs`, written by
//! `tools/metadata/appkit/gen_backend.py` from `metadata/appkit/*.json` and
//! `metadata/overrides/appkit/*.yaml`). Ids are 1-based indexes into the tables; 0 is "none".

/// A property's value type (neutral refs from the metadata).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ty {
    Bool,
    I32,
    I64,
    U32,
    U64,
    F32,
    F64,
    Str,
    /// An object of this class (or any object for `""`): a handle, or `null`.
    Object(&'static str),
    Rect,
    Size,
    Point,
    Insets,
    Range,
    /// An `NS_ENUM`; index into `ENUMS`.
    Enum(u16),
    /// An `NS_OPTIONS`; index into `ENUMS`.
    Flags(u16),
    /// `NSArray`/`NSSet` (read-only lists).
    List,
}

/// How `create` makes the object.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ctor {
    /// `+new` (`-init`).
    New,
    /// `-initWithContentRect:{0,0,width,height} styleMask: backing:NSBackingStoreBuffered defer:NO`.
    Window { width: f64, height: f64, style_mask: u64 },
}

pub struct KindInfo {
    pub class: &'static str,
    /// Element tag; empty for a base class that is not an element.
    pub tag: &'static str,
    /// Kind id of the superclass (0 at the top of the ingested chain).
    pub parent: u16,
    pub toplevel: bool,
    pub ctor: Ctor,
}

pub struct PropInfo {
    pub kind: u16,
    /// JS name (kebab-case of the property).
    pub name: &'static str,
    /// The ObjC property name (the KVC key).
    pub key: &'static str,
    pub ty: Ty,
    pub writable: bool,
    /// A setter-only "prop" (`setButtonType:`): set by sending this selector; not readable.
    pub setter: &'static str,
}

pub struct EnumInfo {
    pub name: &'static str,
    pub flags: bool,
    /// JS name -> native value.
    pub members: &'static [(&'static str, i64)],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Via {
    /// The control's `target`/`action` pair.
    TargetAction,
    /// An `NSNotification` (name in `native`) observed on the default center with the object as sender.
    Notification,
    /// A method of the delegate the backend installs (selector in `native`).
    Delegate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncResult {
    /// Posted (async).
    None,
    /// `call_sync`; a `true` result (`e.preventDefault()`) makes the delegate answer `NO`.
    VetoBool,
}

pub struct SignalInfo {
    pub kind: u16,
    pub name: &'static str,
    pub via: Via,
    pub native: &'static str,
    pub sync: SyncResult,
    /// Props of the emitter read and passed as the event's arguments.
    pub args: &'static [&'static str],
}

pub struct MethodInfo {
    pub kind: u16,
    pub name: &'static str,
    pub selector: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clear {
    Nil,
    /// Install a fresh `NSView` (window and box content views must stay non-nil).
    EmptyView,
}

/// A child slot. `many == false`: `set`/`get`/`clear`. `many == true`: `insert_at` (index from
/// `list`), `insert_relative` (`addSubview:positioned:NSWindowBelow relativeTo:before`) or
/// `append`; `remove` selectors go to the parent with the child, `detach` ones to the child.
pub struct SlotInfo {
    pub kind: u16,
    pub name: &'static str,
    pub default: bool,
    pub many: bool,
    pub set: &'static str,
    pub get: &'static str,
    pub clear: Clear,
    pub append: &'static str,
    pub insert_at: &'static str,
    pub insert_relative: &'static str,
    pub list: &'static str,
    pub remove: &'static [&'static str],
    pub detach: &'static [&'static str],
}

/// The observe class of a prop (`metadata/observe/appkit.json`, REACTIVITY.md 4.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObsClass {
    Constant,
    SetOnly,
    Exact,
    Partial,
    None,
}

/// ABI v2.6 `observe` facts of one prop of one class (the class the fact is written on; lookup walks the
/// element's chain, so `NSTextField:string-value` wins over `NSControl:string-value` for a text field).
pub struct ObserveInfo {
    pub kind: u16,
    /// JS name of the prop.
    pub name: &'static str,
    /// The KVC key KVO observes (`notify`); empty when the prop has no KVO mechanism.
    pub kvo: &'static str,
    pub class: ObsClass,
    /// `NSNotification` names posted with the object as sender (`NSControlTextDidChangeNotification`).
    pub notifications: &'static [&'static str],
    /// The control's target-action pair reports a change (user edits of sliders, buttons).
    pub action: bool,
    /// `value_cost: proportional`: not reported while our own `set_prop` writes it (REACTIVITY.md 5.4).
    pub proportional: bool,
}
