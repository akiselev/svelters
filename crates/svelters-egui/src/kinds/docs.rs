//! What the reference pages say about each built-in kind beyond its props, events and methods: its
//! category, which built-in states it gets, its accessibility and how CSS paints it (DOCS-PLAN §6.1).
//!
//! [`crate::spec`] writes these into `metadata/semantics/egui.json` as the per-kind fields
//! `category`, `states`, `a11y` and `paint`; `tools/docs/egui/gen_reference.py` renders them into
//! `docs/egui/reference/widgets/<tag>.md`. They describe the element's API (what a stylesheet, a
//! screen reader or a test sees), not how it is drawn today, so they stay true when a kind's
//! painting moves from an egui widget to svelters' own.
//!
//! Every kind of [`super::KINDS`] needs a row here (`tests/spec.rs`). Provider kinds
//! (`#[svelters::widget]`) get theirs from [`provider_doc`].

/// Where a kind is listed in the reference index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Inputs,
    Buttons,
    Display,
    Containers,
    Layout,
    Views,
    Menus,
    Overlays,
    Pickers,
    Media,
    /// Abstract bases (`Widget`, `MenuEntry`): documented, never written as a tag.
    Base,
    /// A provider kind (`#[svelters::widget]`).
    Custom,
}

impl Category {
    pub fn name(self) -> &'static str {
        match self {
            Category::Inputs => "inputs",
            Category::Buttons => "buttons",
            Category::Display => "display",
            Category::Containers => "containers",
            Category::Layout => "layout",
            Category::Views => "views",
            Category::Menus => "menus",
            Category::Overlays => "overlays",
            Category::Pickers => "pickers",
            Category::Media => "media",
            Category::Base => "base",
            Category::Custom => "custom",
        }
    }
}

/// How the CSS engine's paint properties reach a kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Paint {
    /// A themed control: `background-color`, `border-*`, `color` set its look per interaction state
    /// (normal, hovered, pressed); outer `box-shadow` and `opacity` apply.
    Stock,
    /// The engine paints a box at the element's rect (shadows, background, border) under its
    /// content; `opacity` applies; `color` sets the text colour of a leaf and is inherited.
    Box,
    /// A provider kind: the box under it as [`Paint::Box`], the rest is the kind's own `ui`.
    Provider,
    /// No paint property reaches the element itself (layout properties still do where it has
    /// them); its children are styled as usual.
    None,
}

impl Paint {
    pub fn name(self) -> &'static str {
        match self {
            Paint::Stock => "stock",
            Paint::Box => "box",
            Paint::Provider => "provider",
            Paint::None => "none",
        }
    }
}

/// Which built-in states the walk sets on a kind (`style.rs`: `before_leaf`, `before_container`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Walk {
    /// A leaf: hover, press and focus from its own interaction.
    Leaf(Focus),
    /// A container: hover and press from the pointer over its box.
    Container,
    /// Drawn out of flow or by its parent: no built-in state is set.
    None,
}

/// Whether a leaf's `:focus` can match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    Yes,
    /// Only while it takes clicks (an `onclicked` handler is set).
    Clickable,
    Never,
}

/// Who sets a state bit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// This frame's interaction with the element (pointer, keyboard focus).
    Response,
    /// A prop of the element.
    Prop,
    /// The scene: the focus chain, transitions.
    Scene,
    /// The kind's own code.
    Kind,
}

impl Source {
    pub fn name(self) -> &'static str {
        match self {
            Source::Response => "response",
            Source::Prop => "prop",
            Source::Scene => "scene",
            Source::Kind => "kind",
        }
    }
}

/// A built-in state a kind sets (beyond its [`Walk`]'s), by its `StateBits::BUILTIN` name.
pub struct StateDoc {
    pub name: &'static str,
    pub source: Source,
    pub when: &'static str,
}

/// What AccessKit gets for the element.
pub struct A11yDoc {
    /// Its role (AccessKit `Role` name); `None`: no node of its own, its content is announced in
    /// place.
    pub role: Option<&'static str>,
    /// Other roles and when they apply.
    pub variants: &'static [(&'static str, &'static str)],
    /// Where its accessible name comes from, first match wins (prop names, or a phrase).
    pub name_from: &'static [&'static str],
    /// What it announces as its value or state.
    pub value: Option<&'static str>,
    /// AccessKit actions assistive tools and tests can request.
    pub actions: &'static [&'static str],
    /// `anchor` becomes its AccessKit author id, and `invalid` its `invalid` flag.
    pub decorated: bool,
    pub note: &'static str,
}

pub struct KindDoc {
    pub id: &'static str,
    pub category: Category,
    pub paint: Paint,
    pub walk: Walk,
    /// States beyond the walk's (props such as `checked`).
    pub states: &'static [StateDoc],
    pub a11y: A11yDoc,
    /// One line on the CSS section (what CSS does not reach, a theme token to use instead).
    pub css_note: &'static str,
}

const fn a11y(role: Option<&'static str>, name_from: &'static [&'static str], value: Option<&'static str>, actions: &'static [&'static str]) -> A11yDoc {
    A11yDoc { role, variants: &[], name_from, value, actions, decorated: true, note: "" }
}

/// A container with no accessibility node of its own.
const NO_NODE: A11yDoc = A11yDoc { role: None, variants: &[], name_from: &[], value: None, actions: &[], decorated: false, note: "" };

const CLICK_FOCUS: &[&str] = &["Click", "Focus"];
const NAME_TEXT: &[&str] = &["accessible-name", "text", "tooltip (when `text` is empty)"];

const fn kind(id: &'static str, category: Category, paint: Paint, walk: Walk, a11y: A11yDoc) -> KindDoc {
    KindDoc { id, category, paint, walk, states: &[], a11y, css_note: "" }
}

const fn layout(id: &'static str) -> KindDoc {
    kind(id, Category::Layout, Paint::Box, Walk::Container, NO_NODE)
}

/// One row per built-in kind of [`super::KINDS`].
pub static KIND_DOCS: &[KindDoc] = &[
    KindDoc {
        id: "Widget",
        category: Category::Base,
        paint: Paint::Box,
        walk: Walk::Leaf(Focus::Yes),
        states: &[],
        a11y: A11yDoc {
            note: "Each element page says what its own node is. Containers that only lay out (`box`, `grid`, ...) have no node: `anchor` and `invalid` reach AccessKit only on elements that have one.",
            ..a11y(None, &["accessible-name"], None, &[])
        },
        css_note: "What a given element paints is on its page: themed controls take their look per interaction state, other elements get a box.",
    },
    KindDoc {
        id: "Window",
        category: Category::Containers,
        paint: Paint::Box,
        walk: Walk::Container,
        states: &[],
        a11y: A11yDoc { role: Some("Window"), note: "The root node of its viewport's accessibility tree.", ..NO_NODE },
        css_note: "The window's own background is the theme's panel colour; a `background-color` rule paints over it.",
    },
    layout("Box"),
    KindDoc {
        id: "Label",
        category: Category::Display,
        paint: Paint::Box,
        walk: Walk::Leaf(Focus::Clickable),
        states: &[],
        a11y: A11yDoc { actions: &["Click (with `onclicked`)", "Focus (with `onclicked`)"], ..a11y(Some("Label"), &["accessible-name", "text"], None, &[]) },
        css_note: "",
    },
    KindDoc {
        id: "Button",
        category: Category::Buttons,
        paint: Paint::Stock,
        walk: Walk::Leaf(Focus::Yes),
        states: &[StateDoc { name: "selected", source: Source::Prop, when: "`selected` is true" }],
        a11y: A11yDoc { value: Some("selected, when `selected` is true"), ..a11y(Some("Button"), NAME_TEXT, None, CLICK_FOCUS) },
        css_note: "",
    },
    KindDoc {
        id: "Checkbox",
        category: Category::Inputs,
        paint: Paint::Stock,
        walk: Walk::Leaf(Focus::Yes),
        states: &[StateDoc { name: "checked", source: Source::Prop, when: "`checked` is true" }],
        a11y: a11y(Some("CheckBox"), NAME_TEXT, Some("checked or not (`checked`)"), CLICK_FOCUS),
        css_note: "",
    },
    KindDoc {
        id: "TextEdit",
        category: Category::Inputs,
        paint: Paint::Stock,
        walk: Walk::Leaf(Focus::Yes),
        states: &[],
        a11y: A11yDoc {
            variants: &[("MultilineTextInput", "`multiline`"), ("PasswordInput", "`password`")],
            ..a11y(Some("TextInput"), &["accessible-name", "hint-text", "tooltip"], Some("the text"), &["Focus", "SetValue", "ReplaceSelectedText", "SetTextSelection"])
        },
        css_note: "The field's fill follows the `--sv-input-bg` token; prefer the input tokens to rules.",
    },
    KindDoc {
        id: "Slider",
        category: Category::Inputs,
        paint: Paint::Stock,
        walk: Walk::Leaf(Focus::Yes),
        states: &[],
        a11y: A11yDoc {
            note: "With `show-value`, its value box is a separate `SpinButton` node.",
            ..a11y(Some("Slider"), &["accessible-name", "text", "tooltip"], Some("`value`, within `min`..`max`"), &["Focus", "SetValue", "Increment", "Decrement"])
        },
        css_note: "",
    },
    KindDoc {
        id: "ToggleButton",
        category: Category::Buttons,
        paint: Paint::Box,
        walk: Walk::Leaf(Focus::Yes),
        states: &[],
        a11y: A11yDoc {
            variants: &[("RadioButton", "`radio`")],
            ..a11y(Some("Button"), NAME_TEXT, Some("selected while `checked`"), CLICK_FOCUS)
        },
        css_note: "`:checked` does not match it yet: style the pressed look through a class (`class:on={checked}`).",
    },
    KindDoc {
        id: "ComboBox",
        category: Category::Inputs,
        paint: Paint::Box,
        walk: Walk::Leaf(Focus::Never),
        states: &[],
        a11y: A11yDoc {
            note: "Its open list is a `ListBox`. It shows no caption of its own: name it with `accessible-name` (or a `tooltip`).",
            ..a11y(Some("ComboBox"), &["accessible-name", "tooltip"], Some("the selected option's text"), CLICK_FOCUS)
        },
        css_note: "`:focus` does not match it yet; use `:focus-within` on its container.",
    },
    KindDoc {
        id: "Separator",
        category: Category::Display,
        paint: Paint::Box,
        walk: Walk::Leaf(Focus::Never),
        states: &[],
        a11y: A11yDoc { decorated: true, ..NO_NODE },
        css_note: "The line itself follows the theme's separator colour.",
    },
    KindDoc {
        id: "Icon",
        category: Category::Display,
        paint: Paint::Box,
        walk: Walk::Leaf(Focus::Clickable),
        states: &[],
        a11y: A11yDoc {
            variants: &[("Button", "`frame`, or an `onclicked` handler")],
            ..a11y(Some("Image"), &["accessible-name", "tooltip", "name"], None, &["Click (when it is a button)", "Focus (when it is a button)"])
        },
        css_note: "The glyph takes `color`. With `frame`, it is drawn as a button and `:focus` matches.",
    },
    layout("Grid"),
    KindDoc {
        id: "CollapsingHeader",
        category: Category::Containers,
        paint: Paint::Box,
        walk: Walk::Container,
        states: &[],
        a11y: A11yDoc {
            note: "The node is the header row; the body's children are announced in place while it is open.",
            ..a11y(Some("DisclosureTriangle"), &["accessible-name", "text"], Some("expanded while `open`"), CLICK_FOCUS)
        },
        css_note: "`:open` and `:expanded` do not match it yet: use `class:open={open}` with `bind:open`.",
    },
    KindDoc {
        id: "ScrollArea",
        category: Category::Containers,
        paint: Paint::Box,
        walk: Walk::Container,
        states: &[],
        a11y: A11yDoc {
            note: "svelters' own (OW2): the node carries the scroll offsets and their ranges and takes ScrollUp/Down/Left/Right and SetScrollOffset; its bars are `ScrollBar` nodes with their value; ScrollIntoView on a descendant scrolls it into view.",
            decorated: false,
            ..a11y(Some("ScrollView"), &["accessible-name"], Some("`scroll-x`, `scroll-y` and their ranges"), &["ScrollUp", "ScrollDown", "ScrollLeft", "ScrollRight", "SetScrollOffset"])
        },
        css_note: "`overflow-x`/`overflow-y` (`hidden`, `scroll`, `auto`) override `direction` per axis; `overscroll-behavior: contain` keeps the wheel from chaining to an outer area; `scroll-margin` is the gap `scroll-into-view` and focus keep; `scrollbar-gutter: stable` (or `--sv-scrollbar-mode: classic`) gives classic bars in a gutter. Provider states `:state(sv.overflow-top)`, `-bottom`, `-left`, `-right` match while content lies beyond that edge; the theme draws R15's edge shadows as an inset `box-shadow` on `:state(sv.overflow-top)`/`(sv.overflow-left)`, painted over the content. The bars and thumbs are parts (`scrollbar-x`/`-y`, `thumb-x`/`-y`, `corner`, `shadow-*`), painted from VS Code's `scrollbarSlider.*` tokens until `::part()` reaches them.",
    },
    // WG2a: menus
    KindDoc {
        id: "MenuEntry",
        category: Category::Base,
        paint: Paint::None,
        walk: Walk::None,
        states: &[],
        a11y: A11yDoc { decorated: true, name_from: &["accessible-name"], ..NO_NODE },
        css_note: "Menu entries are drawn by the menu that shows them; CSS reaches the menu's own content only.",
    },
    KindDoc {
        id: "MenuBar",
        category: Category::Menus,
        paint: Paint::Box,
        walk: Walk::Leaf(Focus::Never),
        states: &[],
        a11y: A11yDoc { decorated: true, note: "Its menus are `Button` nodes.", ..NO_NODE },
        css_note: "",
    },
    KindDoc {
        id: "MenuButton",
        category: Category::Menus,
        paint: Paint::Box,
        walk: Walk::Leaf(Focus::Yes),
        states: &[],
        a11y: a11y(Some("Button"), &["accessible-name", "label"], None, CLICK_FOCUS),
        css_note: "The menu it opens is not styled by CSS.",
    },
    KindDoc {
        id: "Menu",
        category: Category::Menus,
        paint: Paint::None,
        walk: Walk::None,
        states: &[],
        a11y: a11y(Some("Button"), &["accessible-name", "label"], None, CLICK_FOCUS),
        css_note: "",
    },
    KindDoc {
        id: "Action",
        category: Category::Menus,
        paint: Paint::None,
        walk: Walk::None,
        states: &[],
        a11y: A11yDoc {
            variants: &[("CheckBox", "`checkable`"), ("RadioButton", "`group` is set")],
            note: "Its `shortcut` is announced as the keyboard shortcut.",
            ..a11y(Some("Button"), &["accessible-name", "label"], Some("checked or not, for a check or radio item"), CLICK_FOCUS)
        },
        css_note: "",
    },
    KindDoc {
        id: "MenuSeparator",
        category: Category::Menus,
        paint: Paint::None,
        walk: Walk::None,
        states: &[],
        a11y: NO_NODE,
        css_note: "",
    },
    KindDoc {
        id: "ContextMenu",
        category: Category::Menus,
        paint: Paint::None,
        walk: Walk::None,
        states: &[],
        a11y: NO_NODE,
        css_note: "",
    },
    // WG2a: overlays
    KindDoc {
        id: "Popup",
        category: Category::Overlays,
        paint: Paint::None,
        walk: Walk::None,
        states: &[],
        a11y: NO_NODE,
        css_note: "The popup's frame follows the theme; style its content through a child `box`.",
    },
    KindDoc {
        id: "Modal",
        category: Category::Overlays,
        paint: Paint::None,
        walk: Walk::None,
        states: &[],
        a11y: NO_NODE,
        css_note: "The dialog frame and backdrop follow the theme; style its content through a child `box`.",
    },
    KindDoc {
        id: "InnerWindow",
        category: Category::Overlays,
        paint: Paint::None,
        walk: Walk::None,
        states: &[],
        a11y: A11yDoc { role: Some("Window"), name_from: &["title"], note: "Its title bar's collapse and close buttons are `Button` nodes.", ..NO_NODE },
        css_note: "The window frame follows the theme; style its content through a child `box`.",
    },
    // WG2b
    KindDoc {
        id: "Hyperlink",
        category: Category::Buttons,
        paint: Paint::Box,
        walk: Walk::Leaf(Focus::Yes),
        states: &[],
        a11y: a11y(Some("Link"), &["accessible-name", "text"], None, CLICK_FOCUS),
        css_note: "",
    },
    KindDoc {
        id: "RadioButton",
        category: Category::Inputs,
        paint: Paint::Box,
        walk: Walk::Leaf(Focus::Yes),
        states: &[],
        a11y: a11y(Some("RadioButton"), NAME_TEXT, Some("checked or not (`checked`)"), CLICK_FOCUS),
        css_note: "`:checked` does not match it yet: use `class:on={checked}`.",
    },
    KindDoc {
        id: "Switch",
        category: Category::Inputs,
        paint: Paint::Box,
        walk: Walk::Leaf(Focus::Yes),
        states: &[],
        a11y: a11y(Some("Switch"), NAME_TEXT, Some("on or off (`active`)"), CLICK_FOCUS),
        css_note: "`:checked` does not match it yet: use `class:on={active}`.",
    },
    KindDoc {
        id: "RangeSlider",
        category: Category::Inputs,
        paint: Paint::Box,
        walk: Walk::Leaf(Focus::Yes),
        states: &[],
        a11y: A11yDoc {
            note: "Each handle is a `Slider` node of its own; with `show-value`, the two value boxes are `SpinButton` nodes.",
            ..a11y(Some("Slider"), &["accessible-name", "text", "tooltip"], Some("`low` and `high`, one per handle"), &["Focus", "SetValue", "Increment", "Decrement"])
        },
        css_note: "`:focus` follows its first focusable part (the low value box with `show-value`, else the rail).",
    },
    KindDoc {
        id: "DragValue",
        category: Category::Inputs,
        paint: Paint::Box,
        walk: Walk::Leaf(Focus::Yes),
        states: &[],
        a11y: a11y(Some("SpinButton"), &["accessible-name", "tooltip"], Some("`value`"), &["Focus", "SetValue", "Increment", "Decrement"]),
        css_note: "",
    },
    KindDoc {
        id: "Panel",
        category: Category::Containers,
        paint: Paint::Box,
        walk: Walk::Container,
        states: &[],
        a11y: A11yDoc {
            note: "svelters' own (OW2): a `Pane`; a `resizable` panel's inner edge is a `Splitter` node with its size and bounds (arrows and Home/End while focused).",
            decorated: false,
            ..a11y(Some("Pane"), &["accessible-name"], None, &[])
        },
        css_note: "svelters' own (OW2): its box is CSS (`background-color` defaults to `--sv-bg`); the inner-edge line (`::part(separator)`, `border`) and the sash's hover bar (`focus`) paint from the theme until `::part()` reaches them.",
    },
    layout("Sides"),
    KindDoc {
        id: "Frame",
        category: Category::Containers,
        paint: Paint::Box,
        walk: Walk::Container,
        states: &[],
        a11y: NO_NODE,
        css_note: "Its `frame-style` paints over the CSS box (a style with a fill hides `background-color`); with `frame-style=\"none\"` CSS alone draws it.",
    },
    layout("Stack"),
    KindDoc {
        id: "Image",
        category: Category::Media,
        paint: Paint::Box,
        walk: Walk::Leaf(Focus::Clickable),
        states: &[],
        a11y: A11yDoc { actions: &["Click (with `onclicked`)", "Focus (with `onclicked`)"], ..a11y(Some("Image"), &["accessible-name", "tooltip"], None, &[]) },
        css_note: "",
    },
    KindDoc {
        id: "ProgressBar",
        category: Category::Display,
        paint: Paint::Box,
        walk: Walk::Leaf(Focus::Never),
        states: &[],
        a11y: a11y(Some("ProgressIndicator"), &["accessible-name", "text", "tooltip"], Some("`fraction`"), &[]),
        css_note: "The fill follows the theme's selection colour.",
    },
    KindDoc {
        id: "Spinner",
        category: Category::Display,
        paint: Paint::Box,
        walk: Walk::Leaf(Focus::Never),
        states: &[],
        a11y: a11y(Some("ProgressIndicator"), &["accessible-name", "tooltip"], None, &[]),
        css_note: "",
    },
    KindDoc {
        id: "Canvas",
        category: Category::Media,
        paint: Paint::Box,
        walk: Walk::Leaf(Focus::Clickable),
        states: &[],
        a11y: A11yDoc { actions: &["Click (with `onclicked`)", "Focus (with `onclicked`)"], ..a11y(Some("Canvas"), &["accessible-name", "tooltip"], None, &[]) },
        css_note: "The box paints under the drawing; the drawing's colours are the ones in `ops`.",
    },
    // WG4a
    KindDoc {
        id: "Table",
        category: Category::Views,
        paint: Paint::Box,
        walk: Walk::Container,
        states: &[],
        a11y: A11yDoc {
            note: "Its keyboard part is a `Grid` node whose active descendant is the cursor cell; rows are `Row` nodes and cells `Cell` nodes. Multi-row selection marks it multiselectable.",
            ..a11y(Some("Table"), &["accessible-name"], Some("its row and column counts"), &["Focus"])
        },
        css_note: "Rows, cells and headers follow the theme; the box paints under the whole table.",
    },
    KindDoc {
        id: "TableColumn",
        category: Category::Views,
        paint: Paint::None,
        walk: Walk::None,
        states: &[],
        a11y: A11yDoc { role: Some("ColumnHeader"), name_from: &["title"], value: Some("the sort direction (`sort`)"), note: "The table draws the header; the column has no node of its own.", ..NO_NODE },
        css_note: "Data only: the table draws it.",
    },
    KindDoc {
        id: "TabView",
        category: Category::Containers,
        paint: Paint::Box,
        walk: Walk::Container,
        states: &[],
        a11y: A11yDoc { note: "Each tab in its bar is a `Tab` node, selected for the shown page.", ..NO_NODE },
        css_note: "The tab bar follows the theme; the shown page is a `tab`, styled as a box.",
    },
    KindDoc {
        id: "Tab",
        category: Category::Containers,
        paint: Paint::Box,
        walk: Walk::Container,
        states: &[],
        a11y: A11yDoc { role: Some("Tab"), name_from: &["title"], value: Some("selected while its page is shown"), actions: CLICK_FOCUS, note: "The node is its button in the parent's tab bar.", ..NO_NODE },
        css_note: "Rules on `tab` style the page, not its button in the bar.",
    },
    KindDoc {
        id: "SplitView",
        category: Category::Containers,
        paint: Paint::Box,
        walk: Walk::Container,
        states: &[],
        a11y: A11yDoc {
            note: "svelters' own (OW2): each sash between two panes is a `Splitter` node named by the split view (else `Resize pane <n>`), in the tab order, with the size of the pane before it, its bounds and step; arrows (Shift: ×5), Home/End and Enter (collapse) work while it is focused.",
            ..a11y(Some("Group"), &["accessible-name"], None, &[])
        },
        css_note: "The sashes (`::part(sash)`, `::part(grip)`) paint from the theme's `border` (the 1 px line), `sash-w` (the hit area and hover bar) and `focus` (R11's hover bar after 300 ms) until `::part()` reaches them.",
    },
    KindDoc {
        id: "ToastHost",
        category: Category::Overlays,
        paint: Paint::None,
        walk: Walk::Leaf(Focus::Never),
        states: &[],
        a11y: A11yDoc { note: "Each toast is an `Alert` node named by its text, with a `Dismiss` button.", decorated: false, ..NO_NODE },
        css_note: "The toasts are drawn over everything in the theme's look; CSS does not reach them.",
    },
    // WG4b
    KindDoc {
        id: "DatePicker",
        category: Category::Pickers,
        paint: Paint::Provider,
        walk: Walk::Leaf(Focus::Yes),
        states: &[StateDoc { name: "open", source: Source::Kind, when: "its calendar popup is shown" }, StateDoc { name: "read-only", source: Source::Kind, when: "`read-only` is set" }],
        a11y: A11yDoc { note: "A `Button` named by the shown date (or its accessible name), `expanded` while open, `has_popup` dialog. The popup holds the calendar's `Grid` (cells named by ISO date) and, with `commit-mode=\"confirm\"`, Save and Cancel buttons; Escape closes it and focus returns to the button.", ..a11y(Some("Button"), &["accessible-name", "the shown date"], None, CLICK_FOCUS) },
        css_note: "The button: R4's field look with R14's chevron (`::part(chevron)`); the popup is `::part(popup)` (OW3's surface) holding the `calendar` kind's parts (`header`, `prev`, `next`, `month`, `year`, `weekday`, `week-number`, `grid`, `day`) and `::part(save)`, `::part(cancel)`.",
    },
    KindDoc {
        id: "ColorPicker",
        category: Category::Pickers,
        paint: Paint::Provider,
        walk: Walk::Leaf(Focus::Yes),
        states: &[StateDoc { name: "open", source: Source::Kind, when: "its editor popup is shown" }],
        a11y: A11yDoc { note: "A `ColorWell` named \"Color\" (or its accessible name) with the hex as its value, `expanded` while open. The popup holds the `color-editor` kind's sliders, boxes and swatches and, with `commit-mode=\"confirm\"`, Save and Cancel; Escape closes it and focus returns.", ..a11y(Some("ColorWell"), &["accessible-name", "tooltip", "\"Color\""], Some("the colour"), CLICK_FOCUS) },
        css_note: "The button shows `::part(swatch)` (a checkerboard under alpha) and R14's `::part(chevron)`; the popup is `::part(popup)` holding the `color-editor` kind's parts and `::part(save)`, `::part(cancel)`.",
    },
    // LISTS
    KindDoc {
        id: "VirtualList",
        category: Category::Views,
        paint: Paint::None,
        walk: Walk::None,
        states: &[],
        a11y: A11yDoc {
            variants: &[("ListBox", "`selection` is not `none`")],
            note: "Rows are `ListItem` nodes (`ListBoxOption` when selectable) with their position in the set; the cursor row is the active descendant while the list has focus. Multi selection marks it multiselectable.",
            decorated: false,
            ..a11y(Some("List"), &["accessible-name"], Some("its row count (size of set)"), &["Focus"])
        },
        css_note: "Style the rows: each row is the element its row template renders.",
    },
    // WG5b: a `virtual-list` with a tree attached
    KindDoc {
        id: "TreeView",
        category: Category::Views,
        paint: Paint::None,
        walk: Walk::None,
        states: &[],
        a11y: A11yDoc {
            note: "Rows are `TreeItem` nodes with their level, `expanded` on expandable rows, `selected`, and their position and set size over the visible rows. The tree is one tab stop; the cursor row is its active descendant.",
            decorated: false,
            ..a11y(Some("Tree"), &["accessible-name"], None, &["Focus"])
        },
        css_note: "Style the rows: each row is the element its row template renders.",
    },
    // RT2: the inline runs (`rich.rs`)
    KindDoc {
        id: "Run",
        category: Category::Display,
        paint: Paint::None,
        walk: Walk::None,
        states: &[],
        a11y: A11yDoc { note: "Part of its label's text (the label's AccessKit value holds all its runs).", decorated: false, ..a11y(None, &[], None, &[]) },
        css_note: "Drawn as a section of its label's paragraph: `color`, `background-color`, `font-size`, `font-weight`, `font-style`, `font-family` (monospace when it names one) and `text-decoration-line` apply; box properties do not.",
    },
    KindDoc {
        id: "Span",
        category: Category::Display,
        paint: Paint::None,
        walk: Walk::None,
        states: &[],
        a11y: A11yDoc { note: "Part of its label's text (the label's AccessKit value holds all its runs).", decorated: false, ..a11y(None, &[], None, &[]) },
        css_note: "Drawn as a section of its label's paragraph: `color`, `background-color`, `font-size`, `font-weight`, `font-style`, `font-family` (monospace when it names one) and `text-decoration-line` apply; box properties do not.",
    },
    KindDoc {
        id: "Bold",
        category: Category::Display,
        paint: Paint::None,
        walk: Walk::None,
        states: &[],
        a11y: A11yDoc { note: "Part of its label's text (the label's AccessKit value holds all its runs).", decorated: false, ..a11y(None, &[], None, &[]) },
        css_note: "Drawn as a section of its label's paragraph: `color`, `background-color`, `font-size`, `font-weight`, `font-style`, `font-family` (monospace when it names one) and `text-decoration-line` apply; box properties do not.",
    },
    KindDoc {
        id: "Italic",
        category: Category::Display,
        paint: Paint::None,
        walk: Walk::None,
        states: &[],
        a11y: A11yDoc { note: "Part of its label's text (the label's AccessKit value holds all its runs).", decorated: false, ..a11y(None, &[], None, &[]) },
        css_note: "Drawn as a section of its label's paragraph: `color`, `background-color`, `font-size`, `font-weight`, `font-style`, `font-family` (monospace when it names one) and `text-decoration-line` apply; box properties do not.",
    },
    KindDoc {
        id: "Code",
        category: Category::Display,
        paint: Paint::None,
        walk: Walk::None,
        states: &[],
        a11y: A11yDoc { note: "Part of its label's text (the label's AccessKit value holds all its runs).", decorated: false, ..a11y(None, &[], None, &[]) },
        css_note: "Drawn as a section of its label's paragraph: `color`, `background-color`, `font-size`, `font-weight`, `font-style`, `font-family` (monospace when it names one) and `text-decoration-line` apply; box properties do not.",
    },
    KindDoc {
        id: "Anchor",
        category: Category::Display,
        paint: Paint::None,
        walk: Walk::None,
        states: &[StateDoc { name: "hovered", source: Source::Kind, when: "the pointer is over one of its glyphs" }, StateDoc { name: "focused", source: Source::Kind, when: "it has keyboard focus" }],
        a11y: A11yDoc { note: "A `Link` node at its glyphs' bounds, a child of the label's node; Tab reaches it, Enter or Space activates it.", decorated: false, ..a11y(Some("Link"), &["accessible-name", "text"], None, &["Click", "Focus"]) },
        css_note: "Drawn as a section of its label's paragraph: `color`, `background-color`, `font-size`, `font-weight`, `font-style`, `font-family` (monospace when it names one) and `text-decoration-line` apply; box properties do not.",
    },
    // OW2: a new kind, svelters' own only
    KindDoc {
        id: "Scrollbar",
        category: Category::Containers,
        paint: Paint::Provider,
        walk: Walk::Leaf(Focus::Never),
        states: &[],
        a11y: a11y(Some("ScrollBar"), &["accessible-name"], Some("`value` in `min`..`max`, with its step"), &["Increment", "Decrement", "ScrollUp", "ScrollDown"]),
        css_note: "Its track and thumb (`::part(scrollbar)`, `::part(thumb)`) paint from VS Code's `scrollbarSlider.*` tokens until `::part()` reaches them; its thickness is `scrollbar-w`.",
    },
    // OW6: new kinds, svelters' own only
    KindDoc {
        id: "Calendar",
        category: Category::Pickers,
        paint: Paint::Provider,
        walk: Walk::Leaf(Focus::Yes),
        states: &[StateDoc { name: "read-only", source: Source::Kind, when: "`read-only` is set" }],
        a11y: A11yDoc { note: "A `Grid` of six `Row`s of `GridCell`s, each named by its ISO date (`selected`, `disabled`, Click); previous/next are `Button`s. The grid is one tab stop and the arrow keys rove its days.", ..a11y(Some("Grid"), &["accessible-name"], Some("the picked date"), &["Focus"]) },
        css_note: "Parts: `header`, `prev`, `next`, `month`, `year`, `weekday`, `week-number`, `grid`, `day`. Day states `:hover`, `:selected`, `:current` (the keyboard cursor, ringed while `:focus-visible`), `:disabled`, and the part names `today`, `weekend`, `other-month` plus `day-parts`' names, painted from the UA tables until per-day part styles reach egui.",
    },
    KindDoc {
        id: "ColorEditor",
        category: Category::Pickers,
        paint: Paint::Provider,
        walk: Walk::Leaf(Focus::Yes),
        states: &[],
        a11y: A11yDoc { note: "A `Group` holding a `Slider` each for saturation/value, hue and alpha, a `SpinButton` per channel box, a `TextInput` for hex (SetValue), and a `Button` per swatch and custom colour. Each slider and box is a tab stop; arrows step it (Shift ×10).", ..a11y(Some("Group"), &["accessible-name"], Some("the colour (hex)"), &["Focus"]) },
        css_note: "Parts: `square`, `hue`, `alpha`, `handle` (`:dragging`, `:focus-visible`), `fields`, `field`, `swatches`, `swatch-cell`. The checkerboard under alpha is painted until `background-image` repeating gradients reach egui.",
    },
];

/// Paint properties a kind's own drawing covers, so a rule with them has no visible effect (the
/// panel's fill is drawn over the CSS box).
pub static CSS_HIDDEN: &[(&str, &[&str])] = &[("Panel", &["background-color", "border-color", "border-width", "border-radius"])];

/// [`CSS_HIDDEN`]'s entry for kind `id`.
pub fn css_hidden(id: &str) -> &'static [&'static str] {
    CSS_HIDDEN.iter().find(|(k, _)| *k == id).map_or(&[], |(_, v)| v)
}

/// U0 (UTILITY-CSS §3.4): the provider states (`:state(sv.<name>)`) a built-in kind sets on itself,
/// by kind id: the spec's `kinds[].provider_states`, for `state-[sv.<name>]:` utility variants and
/// the docs. `tests/spec.rs` checks that every `sv` provider state is claimed by a kind.
pub fn provider_states(id: &str) -> &'static [&'static crate::provider::StateBit] {
    use crate::interact::scroll::sv;
    static OVERFLOW: [&crate::provider::StateBit; 4] = [&sv::OVERFLOW_TOP, &sv::OVERFLOW_BOTTOM, &sv::OVERFLOW_LEFT, &sv::OVERFLOW_RIGHT];
    match id {
        // each runs a `Scrollable` unit, which sets the overflow states on its host (the lists
        // through `widgets::list_rows::ListBar`)
        "ScrollArea" | "Scrollbar" | "VirtualList" | "TreeView" => &OVERFLOW,
        _ => &[],
    }
}

/// The row of built-in kind `id`.
pub fn kind_doc(id: &str) -> Option<&'static KindDoc> {
    KIND_DOCS.iter().find(|d| d.id == id)
}

/// A provider kind's row (`#[svelters::widget]`): a leaf with the box under it; `:focus` when it
/// declares `Caps::FOCUSABLE`. Its role and value are what its `Widget::a11y` sets.
pub fn provider_doc(focusable: bool) -> KindDoc {
    KindDoc {
        id: "",
        category: Category::Custom,
        paint: Paint::Provider,
        walk: Walk::Leaf(if focusable { Focus::Yes } else { Focus::Never }),
        states: &[],
        a11y: A11yDoc { note: "Its role, value and actions are what the kind's `Widget::a11y` sets.", ..a11y(None, &["accessible-name", "tooltip"], None, &[]) },
        css_note: "Provider states match as `:state(<namespace>.<name>)`.",
    }
}
