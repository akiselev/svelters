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
//! LISTS' `virtuallist.rs` (`virtual-list`);
//! `registry.rs` [`KindRegistry`] with the provider kinds of
//! `#[svelters::widget]` (RA1). Writer lanes: WIDGETS (tables, one file per new family), RUSTAPI
//! (`registry.rs`).

/// EV1: the DOM-style input events every widget and window has, dispatched along the scene with
/// capture, target and bubble phases (`crate::events`). Sync signals: each listener gets the event
/// object first. Every one starts with `target` (the node the event is for, a handle number) and
/// `phase` (1 capture, 2 target, 3 bubble; +8 when a listener before prevented the default).
/// `$extra`: the kind's own signals, listed first.
macro_rules! dom_signals {
    ($($extra:expr),* $(,)?) => {
        &[
            $($extra,)*
            $crate::kinds::SignalDef { name: "pointerdown", args: $crate::kinds::POINTER_ARGS, sync: true, doc: "A pointer button went down over this node or a descendant (EV1: capture, target, bubble). `preventDefault` keeps the press from egui (no click, drag or focus)." },
            $crate::kinds::SignalDef { name: "pointerup", args: $crate::kinds::POINTER_ARGS, sync: true, doc: "A pointer button went up over this node or a descendant (EV1). `preventDefault` cancels the click." },
            $crate::kinds::SignalDef { name: "click", args: $crate::kinds::POINTER_ARGS, sync: true, doc: "The primary button was pressed and released; targets the innermost node that holds both points (EV1). `preventDefault` keeps the click from the widget." },
            $crate::kinds::SignalDef { name: "dblclick", args: $crate::kinds::POINTER_ARGS, sync: true, doc: "A second click on the same node in a short time, after its `click` (EV1)." },
            $crate::kinds::SignalDef { name: "contextmenu", args: $crate::kinds::POINTER_ARGS, sync: true, doc: "The secondary button went down (EV1), after `pointerdown`. `preventDefault` keeps the press from egui (no context menu)." },
            $crate::kinds::SignalDef { name: "wheel", args: $crate::kinds::WHEEL_ARGS, sync: true, doc: "The mouse wheel or a trackpad scrolled over this node (EV1); `dx`, `dy` in points. `preventDefault` keeps it from scrolling." },
            $crate::kinds::SignalDef { name: "keydown", args: $crate::kinds::KEY_ARGS, sync: true, doc: "A key went down while this node or a descendant had focus (EV1); `key` is egui's key name. `preventDefault` keeps the key (and the text it types) from the focused widget." },
            $crate::kinds::SignalDef { name: "keyup", args: $crate::kinds::KEY_ARGS, sync: true, doc: "A key went up while this node or a descendant had focus (EV1)." },
            $crate::kinds::SignalDef { name: "focusin", args: $crate::kinds::FOCUS_ARGS, sync: true, doc: "This node or a descendant took keyboard focus (EV1; after the frame that moved it). `related` is the node that lost it, 0 for none." },
            $crate::kinds::SignalDef { name: "focusout", args: $crate::kinds::FOCUS_ARGS, sync: true, doc: "This node or a descendant lost keyboard focus (EV1). `related` is the node that took it, 0 for none." },
        ]
    };
}

/// EV1: the arguments of the DOM-style events (`dom_signals!`).
pub(crate) const POINTER_ARGS: &[(&str, Ty)] = &[("target", Ty::I64), ("phase", Ty::I64), ("x", Ty::F64), ("y", Ty::F64), ("button", Ty::I64), ("modifiers", Ty::I64)];
pub(crate) const WHEEL_ARGS: &[(&str, Ty)] = &[("target", Ty::I64), ("phase", Ty::I64), ("x", Ty::F64), ("y", Ty::F64), ("dx", Ty::F64), ("dy", Ty::F64), ("modifiers", Ty::I64)];
pub(crate) const KEY_ARGS: &[(&str, Ty)] = &[("target", Ty::I64), ("phase", Ty::I64), ("key", Ty::Str), ("repeat", Ty::Bool), ("modifiers", Ty::I64)];
pub(crate) const FOCUS_ARGS: &[(&str, Ty)] = &[("target", Ty::I64), ("phase", Ty::I64), ("related", Ty::I64)];

/// EV1: the names of the events `dom_signals!` declares (they propagate along the scene).
pub const DOM_EVENTS: &[&str] = &["pointerdown", "pointerup", "click", "dblclick", "contextmenu", "wheel", "keydown", "keyup", "focusin", "focusout"];

/// EV1: whether signal `name` is one of the propagating DOM-style events.
pub fn is_dom_event(name: &str) -> bool {
    DOM_EVENTS.contains(&name)
}

mod builtin;
mod containers;
/// DOC1: category, states, accessibility and paint of each kind, for the docs.
pub mod docs;
pub mod dnd;
pub mod pickers;
mod registry;
pub(crate) mod virtuallist;
pub mod rich;
pub mod treeview;
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
pub use views::{CELL_ALIGN, COMMIT_MODE, SORT, TABLE_SELECTION, TOAST_CORNER, TOAST_LEVEL};
pub use registry::{KindRegistry, PROVIDER_PACK, ProviderKind, kind};
pub use virtuallist::{ListStats, VList};

/// The kinds a slot of kind `id` accepts (spec `children.slots.*.accepts`): WG4a's `table` takes
/// `table-column`s, WG2a's menu containers menu items, every other container any `Widget`.
pub fn slot_accepts(id: &str) -> &'static [&'static str] {
    views::slot_accepts(id).or_else(|| rich::slot_accepts(id)).unwrap_or_else(|| menus::slot_accepts(id))
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
    /// WG5a: a list of row ids (`number[]`; the ABI's `SVL_LIST` of `SVL_I64`): item-view selections.
    IdList,
    /// OW2: a list of numbers (`number[]`; the ABI's `SVL_LIST` of `SVL_F64`): split-view `sizes`.
    NumList,
    /// WG5a: a Svelte snippet (a function), kept by the JS runtime; the backend sees `true` while
    /// one is set (`table-column`'s `editor`).
    Snippet,
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
    /// icon (AS1)
    Tint,
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
    // `virtual-list` (LISTS LS1, `virtuallist.rs`)
    ListSelected,
    ListSelection,
    ListLayout,
    // WG5a (plans/milestones/WG5a-list-table-interaction.md): item-view selection and editing
    /// virtual-list, table: the selected row ids (`bind:selected` on table, `selected-ids` on the list)
    SelectedIds,
    /// table: the cursor row's id (-1: none)
    CurrentRow,
    /// table: the cursor column
    CurrentColumn,
    /// table-column: its cells can be edited
    Editable,
    /// table-column: a Svelte `editor` snippet is set
    Editor,
    // `row-height` reuses WG4a's `RowHeight`
    // `tree-view` (WG5b, `treeview.rs`)
    TreeIndent,
    RowDragType,
    RowDropTypes,
    /// EV3: a button's Enter and Escape roles.
    Default,
    Cancel,
    /// RT2: an `a` run's link target
    Href,
    /// OW2: split-view (N panes)
    Sizes,
    // `collapsible` reuses inner-window's `Collapsible`
    RestoreOnDoubleClick,
    CommitMode,
    /// OW2: scrollbar
    PageSize,
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

#[derive(Clone)]
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

#[derive(Clone)]
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

#[derive(Clone)]
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
