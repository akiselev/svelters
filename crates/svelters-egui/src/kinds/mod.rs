//! The kind table: every element, property, signal and method the egui backend knows.
//!
//! This is the single source of the egui vocabulary (plans/research/EGUI.md §6, "Rust kind table as
//! the single source"). `resolve`/`describe` answer from it, the scene stores one typed field per
//! [`P`], and [`crate::spec`] turns it into `metadata/semantics/egui.json` for the compiler, the
//! TestHost and the docs (checked by `tests/spec.rs`).
//!
//! Names follow N1 where egui has a type (`label`, `button`, `checkbox`, `text-edit`, `slider`:
//! kebab-case of `egui::Label` etc.) and GTK-familiar names for the kinds we build (`window`, `box`).
//! Layout properties are CSS names because taffy's are (D2, D3): they are setters in the style
//! table, so `<style>` can use them on egui.
//!
//! Files (ARCH-EMBED §9): this one holds the table types, the id scheme and [`KindRegistry`];
//! `builtin.rs` the built-in table [`KINDS`] with the `Widget` base and the leaves; `containers.rs`
//! the `window` and `box` tables; WG1's families `controls.rs`, `decor.rs`, `layouts.rs`; WG2b's
//! `inputs.rs`, `panes.rs`, `media.rs`; WG2a's `menus.rs`, `popups.rs`; WG4a's `views.rs`; WG4b's
//! `pickers.rs` (`date-picker`, `color-picker`) and `dnd.rs` (drag and drop on any node);
//! `registry.rs` [`KindRegistry`] with the provider kinds of
//! `#[svelters::widget]` (RA1). Writer lanes: WIDGETS (tables, one file per new family), RUSTAPI
//! (`registry.rs`).

mod builtin;
mod containers;
pub mod dnd;
pub mod pickers;
mod registry;
// WG1 families
mod controls;
mod decor;
mod layouts;
// WG2a families (plans/milestones/WG2a-menus-popups.md)
mod menus;
mod popups;
// WG2b families (plans/milestones/WG2b-tier0-controls.md)
mod inputs;
mod media;
mod panes;
// WG4a (plans/milestones/WG4a-tier1-views.md)
mod views;

pub use builtin::{ALIGN, ALIGN_SELF, DIRECTION, FOCUS_SCOPE, JUSTIFY, KINDS, POSITION, WRAP, WRAP_MODE};
pub use decor::ORIENTATION;
pub use layouts::{JUSTIFY_ITEMS, SCROLL_DIRECTION};
pub use menus::attaches_to;
pub use popups::PLACEMENT;
pub use media::FIT;
pub use panes::{FRAME_STYLE, SIDE};
pub use views::{CELL_ALIGN, SORT, TOAST_CORNER, TOAST_LEVEL};
pub use registry::{KindRegistry, PROVIDER_PACK, ProviderKind, kind};

/// The kinds a slot of kind `id` accepts (spec `children.slots.*.accepts`): WG4a's `table` takes
/// `table-column`s, WG2a's menu containers menu items, every other container any `Widget`.
pub fn slot_accepts(id: &str) -> &'static [&'static str] {
    views::slot_accepts(id).unwrap_or_else(|| menus::slot_accepts(id))
}

/// A property's value type (also its ABI tag).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ty {
    Bool,
    F64,
    I64,
    Str,
    /// A string from a fixed list.
    Enum(&'static [&'static str]),
    /// WG1: a list of strings (`string[]`; the ABI's `SVL_LIST` of `SVL_STR`).
    StrList,
    /// RA1: a provider kind's prop or event payload, typed by its Rust type's `JsType` description
    /// ([`KindRegistry::provider_type`]).
    Provider,
}

/// A property's default (JSON in the spec).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Def {
    Null,
    Bool(bool),
    F64(f64),
    I64(i64),
    Str(&'static str),
}

/// Every property the scene stores, by identity (one typed field each, see `scene/node.rs`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum P {
    // Widget (every element inside a window)
    Enabled,
    Visible,
    Tooltip,
    AccessibleName,
    /// EM1: a stable name inside the root (ARCH-EMBED §4.4): salts the node's egui id and is its
    /// AccessKit author id.
    Anchor,
    Width,
    Height,
    MinWidth,
    MinHeight,
    MaxWidth,
    MaxHeight,
    FlexGrow,
    FlexShrink,
    FlexBasis,
    AlignSelf,
    Margin,
    // SG4: out-of-flow positioning
    Position,
    Left,
    Top,
    Right,
    Bottom,
    // EV3: focus
    Autofocus,
    // Box (and the window's content area)
    FlexDirection,
    FlexWrap,
    Gap,
    RowGap,
    ColumnGap,
    Padding,
    AlignItems,
    JustifyContent,
    /// EV3: the box's focus scope.
    FocusScope,
    // Window
    Title,
    DefaultWidth,
    DefaultHeight,
    // Leaves
    Text,
    WrapMode,
    Selectable,
    Selected,
    Frame,
    Small,
    Checked,
    HintText,
    Multiline,
    Password,
    CharLimit,
    Value,
    Min,
    Max,
    Step,
    ShowValue,
    // WG1 (plans/milestones/WG1-sc1-widgets.md)
    /// Widget: drawn and announced as invalid (a red outline, AccessKit `invalid`).
    Invalid,
    /// Text kinds and `icon`: a CSS weight, mapped to a registered font family (`crate::fonts`).
    FontWeight,
    /// grid
    GridTemplateColumns,
    JustifyItems,
    /// collapsing-header
    Open,
    /// scroll-area
    ScrollDirection,
    ScrollX,
    ScrollY,
    /// combo-box
    Options,
    SelectedIndex,
    /// toggle-button
    Radio,
    /// separator
    Orientation,
    /// icon
    Name,
    Glyph,
    Family,
    Size,
    // WG2a (plans/milestones/WG2a-menus-popups.md)
    /// action
    Shortcut,
    Checkable,
    Group,
    /// menu-button
    Active,
    /// popup (`placement`: SG4's `position` is the CSS one)
    Placement,
    Autohide,
    /// modal
    Dismissable,
    /// inner-window
    DefaultX,
    DefaultY,
    Closable,
    Collapsible,
    Resizable,
    // WG2b (plans/milestones/WG2b-tier0-controls.md); `panel` and `spinner` reuse `Size`
    /// hyperlink
    Url,
    NewTab,
    /// radio-button: its `value`, and the group's chosen value
    Choice,
    // `group` (radio-button), `active` (switch) and `resizable` (panel) reuse WG2a's keys
    /// range-slider
    Low,
    High,
    /// drag-value
    Speed,
    Decimals,
    Prefix,
    Suffix,
    /// panel
    Side,
    ShowSeparator,
    /// frame
    FrameStyle,
    /// stack
    VisibleChildName,
    VisibleIndex,
    Homogeneous,
    /// image
    Source,
    Fit,
    /// progress-bar
    Fraction,
    ShowPercentage,
    Animate,
    /// spinner
    Spinning,
    /// canvas
    Ops,
    ContentWidth,
    ContentHeight,
    // WG4a (plans/milestones/WG4a-tier1-views.md; `kinds/views.rs`). Also reused there: `Title`
    // (tab, table-column), `SelectedIndex` (tab-view, table), `Orientation` (split-view), and
    // WG2a's `Resizable` (table-column, split-view), `Closable` (tab).
    /// table
    RowCount,
    RowHeight,
    StickyColumns,
    Striped,
    Overscan,
    /// table-column
    CellAlign,
    Sort,
    /// split-view (`divider-position`: SG4's `position` is the CSS one)
    DividerPosition,
    MinSize,
    /// toast-host
    Corner,
    Duration,
    // WG4b (plans/milestones/WG4b-dialogs-pickers-dnd.md)
    /// Widget: drag and drop (`kinds/dnd.rs`).
    DragType,
    DragData,
    DropTypes,
    /// date-picker
    Format,
    CalendarWeek,
    /// color-picker
    Alpha,
    /// EV3: a button's Enter and Escape roles.
    Default,
    Cancel,
    /// RA1: prop `i` of a provider kind (a field of its widget struct).
    Ext(u16),
}

/// How a property can change without the host setting it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Native {
    /// Only `set_prop` changes it. Observing it is still exact: the scene never changes it alone.
    Never,
    /// The widget changes it from user input during the pass (`prop_changed` when observed).
    Widget,
}

pub struct PropDef {
    pub name: &'static str,
    pub key: P,
    pub ty: Ty,
    pub default: Def,
    pub nullable: bool,
    pub native: Native,
    /// A layout property: a setter in the style table (D3), taffy reads it.
    pub layout: bool,
    pub doc: &'static str,
}

pub struct SignalDef {
    pub name: &'static str,
    pub args: &'static [(&'static str, Ty)],
    /// Delivered through `call_sync` before the pass; the handler's `true` stops the default.
    pub sync: bool,
    pub doc: &'static str,
}

pub struct MethodDef {
    pub name: &'static str,
    pub doc: &'static str,
}

pub struct KindDef {
    /// The kind's id in the spec (`kinds` keys) and in `resolve(SVL_KIND)`.
    pub id: &'static str,
    /// The element tag; `None` for abstract kinds.
    pub tag: Option<&'static str>,
    /// What it renders with.
    pub native: &'static str,
    pub base: Option<&'static str>,
    pub toplevel: bool,
    /// Accepts children (default slot `slot`).
    pub slot: Option<&'static str>,
    pub text_content: Option<&'static str>,
    pub present: Option<&'static str>,
    pub props: &'static [PropDef],
    pub signals: &'static [SignalDef],
    pub methods: &'static [MethodDef],
    pub doc: &'static str,
}

const fn prop(name: &'static str, key: P, ty: Ty, default: Def, doc: &'static str) -> PropDef {
    PropDef { name, key, ty, default, nullable: false, native: Native::Never, layout: false, doc }
}

const fn length(name: &'static str, key: P, doc: &'static str) -> PropDef {
    PropDef { name, key, ty: Ty::F64, default: Def::Null, nullable: true, native: Native::Never, layout: true, doc }
}

const fn layout(name: &'static str, key: P, ty: Ty, default: Def, doc: &'static str) -> PropDef {
    PropDef { name, key, ty, default, nullable: false, native: Native::Never, layout: true, doc }
}

const fn input(name: &'static str, key: P, ty: Ty, default: Def, doc: &'static str) -> PropDef {
    PropDef { name, key, ty, default, nullable: false, native: Native::Widget, layout: false, doc }
}

/// A nullable prop that is not a layout setter (null = the kind's own default, e.g. the body size).
const fn optional(name: &'static str, key: P, ty: Ty, doc: &'static str) -> PropDef {
    PropDef { name, key, ty, default: Def::Null, nullable: true, native: Native::Never, layout: false, doc }
}

const fn signal(name: &'static str, args: &'static [(&'static str, Ty)], doc: &'static str) -> SignalDef {
    SignalDef { name, args, sync: false, doc }
}

// ---- the module's functions: the global registry's answers ----

/// Index of a kind by id (built-in or provider).
pub fn kind_index(id: &str) -> Option<usize> {
    KindRegistry::global().kind_index(id)
}

/// Index of a kind by tag (built-in or provider).
pub fn tag_index(tag: &str) -> Option<usize> {
    KindRegistry::global().tag_index(tag)
}

/// The kind and its bases, most derived first.
pub fn chain(kind: usize) -> Vec<usize> {
    KindRegistry::global().chain(kind)
}

pub fn kind_id(kind: usize) -> u32 {
    KindRegistry::global().kind_id(kind)
}

pub fn kind_of_id(id: u32) -> Option<usize> {
    KindRegistry::global().kind_of_id(id)
}

/// A property of `kind` (or a base) by name: its id and definition.
pub fn find_prop(kind: usize, name: &str) -> Option<(u32, &'static PropDef)> {
    KindRegistry::global().find_prop(kind, name)
}

/// The property definition behind an id, if `kind` has it.
pub fn prop_def(kind: usize, id: u32) -> Option<&'static PropDef> {
    KindRegistry::global().prop_def(kind, id)
}

pub fn find_signal(kind: usize, name: &str) -> Option<(u32, &'static SignalDef)> {
    KindRegistry::global().find_signal(kind, name)
}

pub fn signal_def(kind: usize, id: u32) -> Option<&'static SignalDef> {
    KindRegistry::global().signal_def(kind, id)
}

pub fn find_method(kind: usize, name: &str) -> Option<(u32, &'static MethodDef)> {
    KindRegistry::global().find_method(kind, name)
}

pub fn method_def(kind: usize, id: u32) -> Option<&'static MethodDef> {
    KindRegistry::global().method_def(kind, id)
}

/// The default slot of a container kind (slot id 1; 0 is "default" on the ABI).
pub fn slot_of(kind: usize) -> Option<&'static str> {
    KindRegistry::global().slot_of(kind)
}

/// The signal id of `name` on `kind`, or 0.
pub fn signal_id(kind: usize, name: &str) -> u32 {
    KindRegistry::global().signal_id(kind, name)
}

/// The property id of `key` on `kind`, or 0.
pub fn prop_id(kind: usize, key: P) -> u32 {
    KindRegistry::global().prop_id(kind, key)
}
