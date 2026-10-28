//! The theme: one token map, resolved from DESIGN.md's three layers, ported from svelters-egui
//! `theme/mod.rs` (DS1; plans/research/DESIGN.md §4) with skelter's colour type. Its outputs are the
//! `:root` token sheet for the cascade ([`Theme::css`], with [`BASE_CSS`] giving the built-in kinds their
//! default look) and typed getters ([`Theme::colors`], [`Theme::components`], [`Theme::metrics`],
//! [`Theme::elevation`], [`Theme::fonts`], and by name [`Theme::color`], [`Theme::px`],
//! [`Theme::shadow`]); the kinds' UA tables resolve their tokens through it (`paint::Ua`).
//! svelters-egui's third output, an `egui::Style`, stays with the egui backend (and the egui island).
//!
//! | Layer | Names | Content |
//! |---|---|---|
//! | L1 primitive | `--vscode-<key, . → ->` | the VS Code colour keys of the base set (also the names VS Code gives webviews) |
//! | L2 semantic | `--sv-<role>` | the roles of DESIGN.md §1.5 ([`ROLES`], [`Colors`]), shape and type ([`Metrics`], [`Fonts`]), density, elevation |
//! | L3 component | `--sv-<kind>-<part>[-<state>]` | per kind, where a kind departs from the roles ([`Components`], [`COMPONENT_COLORS`]) |
//!
//! Resolution ([`Theme`]): the built-in VS Code set for (scheme, contrast) → `--vscode-*`
//! overrides → roles from their VS Code keys → the shape tokens, the density set, the set's own
//! role values (elevation, and roles VS Code leaves null) → the affordance overlay (on by default)
//! → `--sv-<role>` overrides → components from their roles → component overrides. Other custom
//! properties in the overrides (`--cad-*`) are passed through.
//!
//! Built in ([`Theme::vscode`]): VS Code's Dark Modern and Light Modern (the defaults) and the two High
//! Contrast themes, with three densities ([`Density`]). The data is `theme/data/*.json`, taken from VS
//! Code 1.133.0's theme files and colour registry (Code - OSS, MIT) as DESIGN.md records them.

use skelter_base::color::Color;
use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};
pub use svelters_style::ColorScheme;

/// The base sheet for the stock-painted kinds (`@layer sv.base`, theme origin; DESIGN.md §4.3).
pub const BASE_CSS: &str = include_str!("base.css");

/// The cascade layers, lowest first: at theme origin the tokens, the base look and DS3's
/// affordances; at author origin `components` and `utilities` (U3, UTILITY-CSS §4.6: generated
/// utility rules sit below every unlayered component `<style>`). Declaring them here fixes the order
/// before any sheet loads (layer ranks are global, by first appearance), so the first generated
/// utility appends without a rebuild. Author rules (unlayered or not, at author origin) beat every
/// theme layer; a host's unlayered theme-origin sheet (`set_tokens`) beats them too.
pub const LAYERS: &str = "@layer sv.tokens, sv.base, sv.affordance, components, utilities;";

/// Contrast: `High` selects VS Code's High Contrast sets and `prefers-contrast: more`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Contrast {
    #[default]
    Normal,
    High,
}

impl Contrast {
    /// The media feature it sets.
    pub fn media(self) -> svelters_style::Contrast {
        match self {
            Contrast::Normal => svelters_style::Contrast::NoPreference,
            Contrast::High => svelters_style::Contrast::More,
        }
    }
}

/// Density (DESIGN.md §2.6): the metric set of rows, controls, tabs, headers, icons and button
/// padding.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Density {
    /// Data-heavy tables, as a per-panel opt-in (rows 20px).
    Compact,
    /// VS Code's own metrics: the app default (rows 22px, controls 26px).
    #[default]
    Default,
    /// Pen, touch and presentations (rows 28px).
    Comfortable,
}

impl Density {
    pub const ALL: [Density; 3] = [Density::Compact, Density::Default, Density::Comfortable];

    /// Its name in the data (`compact`, `default`, `comfortable`).
    pub fn name(self) -> &'static str {
        match self {
            Density::Compact => "compact",
            Density::Default => "default",
            Density::Comfortable => "comfortable",
        }
    }
}

// ---- the token map ----

/// Resolved tokens by full custom-property name (`--sv-bg`, `--vscode-list-hoverBackground`,
/// `--cad-panel`), values as CSS text.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TokenMap(BTreeMap<String, String>);

impl TokenMap {
    pub fn new() -> TokenMap {
        TokenMap::default()
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.0.get(name).map(String::as_str)
    }

    pub fn insert(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.0.insert(name.into(), value.into());
    }

    pub fn remove(&mut self, name: &str) -> Option<String> {
        self.0.remove(name)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// In name order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> + '_ {
        self.0.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// The token as a colour (hex, `rgb()`/`rgba()`, `transparent`).
    pub fn color(&self, name: &str) -> Option<Color> {
        parse_color(self.get(name)?)
    }

    /// The token as a length in px (`13px`, `13`).
    pub fn px(&self, name: &str) -> Option<f32> {
        parse_px(self.get(name)?)
    }

    /// The token as a shadow (`none`, or `<x> <y> <blur> [<spread>] <color>`).
    pub fn shadow(&self, name: &str) -> Option<Shadow> {
        parse_shadow(self.get(name)?)
    }

    /// The `:root { ... }` rule with every token.
    pub fn root_rule(&self) -> String {
        let mut s = String::from(":root {\n");
        for (k, v) in self.iter() {
            s.push_str("  ");
            s.push_str(k);
            s.push_str(": ");
            s.push_str(v);
            s.push_str(";\n");
        }
        s.push_str("}\n");
        s
    }
}

impl<K: Into<String>, V: Into<String>> FromIterator<(K, V)> for TokenMap {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        TokenMap(iter.into_iter().map(|(k, v)| (k.into(), v.into())).collect())
    }
}

/// The custom property of a VS Code colour key: `list.hoverBackground` →
/// `--vscode-list-hoverBackground` (the name VS Code gives webviews).
pub fn vscode_var(key: &str) -> String {
    format!("--vscode-{}", key.replace('.', "-"))
}

/// The VS Code key of a `--vscode-*` name (VS Code keys have no `-`).
fn vscode_key(name: &str) -> Option<String> {
    name.strip_prefix("--vscode-").map(|k| k.replace('-', "."))
}

/// `bg` → `--sv-bg`; a full custom-property name is kept.
fn full_name(name: &str) -> std::borrow::Cow<'_, str> {
    if name.starts_with("--") { name.into() } else { format!("--sv-{name}").into() }
}

// ---- the typed views ----

macro_rules! color_set {
    (@src) => { None };
    (@src $s:literal) => { Some($s) };
    ($(#[$m:meta])* $ty:ident, $table:ident, $tdoc:literal, { $( $(#[doc = $fd:literal])* $field:ident = $name:literal $(<- $src:literal)?, )* }) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug, PartialEq)]
        pub struct $ty { $( $(#[doc = $fd])* pub $field: Color, )* }

        #[doc = $tdoc]
        pub const $table: &[(&str, Option<&str>)] = &[ $( ($name, color_set!(@src $($src)?)) ),* ];

        impl $ty {
            /// Each token's doc comment (SV71), by bare name; empty where it has none.
            const DOCS: &'static [(&'static str, &'static str)] = &[ $( ($name, concat!($($fd, "\n",)* "")) ),* ];

            fn from_tokens(t: &TokenMap, problems: &mut Vec<String>) -> $ty {
                $ty { $( $field: color_of(t, concat!("--sv-", $name), problems), )* }
            }
        }
    };
}

color_set! {
    /// The semantic colour roles (L2, `--sv-<role>`; DESIGN.md §1.5), resolved.
    Colors, ROLES, "Every colour role (L2) and the VS Code key it comes from (DESIGN.md §1.5); `None`: no key, the sets give the value.", {
        /// Chrome: side bar, panel, title, status, activity bar.
        bg = "bg" <- "sideBar.background",
        /// Content: editor, document, panel bodies.
        surface = "surface" <- "editor.background",
        /// Menus, popovers, hovers, dialogs.
        surface_raised = "surface-raised" <- "editorWidget.background",
        /// The hairline between regions.
        border = "border" <- "sideBar.border",
        /// Popover and menu outline.
        border_widget = "border-widget" <- "widget.border",
        /// Menu outline and separators (a theme without `menu.border` takes `widget.border`).
        border_menu = "border-menu" <- "menu.border",
        shadow_color = "shadow-color" <- "widget.shadow",
        scroll_shadow = "scroll-shadow" <- "scrollbar.shadow",
        fg = "fg" <- "foreground",
        /// Headers, the active tab.
        fg_strong = "fg-strong" <- "settings.headerForeground",
        muted = "muted" <- "descriptionForeground",
        disabled_fg = "disabled-fg" <- "disabledForeground",
        placeholder = "placeholder" <- "input.placeholderForeground",
        icon = "icon" <- "icon.foreground",
        link = "link" <- "textLink.foreground",
        /// Primary button, active markers.
        accent = "accent" <- "button.background",
        accent_hover = "accent-hover" <- "button.hoverBackground",
        accent_fg = "accent-fg" <- "button.foreground",
        focus = "focus" <- "focusBorder",
        button_border = "button-border" <- "button.border",
        /// The secondary (default) button.
        button2_bg = "button2-bg" <- "button.secondaryBackground",
        button2_hover = "button2-hover" <- "button.secondaryHoverBackground",
        button2_fg = "button2-fg" <- "button.secondaryForeground",
        /// Field fill.
        control_bg = "control-bg" <- "input.background",
        control_border = "control-border" <- "input.border",
        control_fg = "control-fg" <- "input.foreground",
        toggle_on_bg = "toggle-on-bg" <- "inputOption.activeBackground",
        toggle_on_border = "toggle-on-border" <- "inputOption.activeBorder",
        toolbar_hover = "toolbar-hover" <- "toolbar.hoverBackground",
        toolbar_active = "toolbar-active" <- "toolbar.activeBackground",
        invalid_border = "invalid-border" <- "inputValidation.errorBorder",
        invalid_bg = "invalid-bg" <- "inputValidation.errorBackground",
        list_hover = "list-hover" <- "list.hoverBackground",
        list_sel = "list-sel" <- "list.activeSelectionBackground",
        list_sel_fg = "list-sel-fg" <- "list.activeSelectionForeground",
        list_sel_inactive = "list-sel-inactive" <- "list.inactiveSelectionBackground",
        list_focus_outline = "list-focus-outline" <- "list.focusOutline",
        list_drop = "list-drop" <- "list.dropBackground",
        indent_guide = "indent-guide" <- "tree.indentGuidesStroke",
        /// Text selection.
        selection = "selection" <- "editor.selectionBackground",
        caret = "caret" <- "editorCursor.foreground",
        tab_bg = "tab-bg" <- "tab.inactiveBackground",
        tab_active_bg = "tab-active-bg" <- "tab.activeBackground",
        tab_fg = "tab-fg" <- "tab.inactiveForeground",
        tab_active_fg = "tab-active-fg" <- "tab.activeForeground",
        tab_active_marker = "tab-active-marker" <- "tab.activeBorderTop",
        menu_sel = "menu-sel" <- "menu.selectionBackground",
        menu_sel_fg = "menu-sel-fg" <- "menu.selectionForeground",
        scrollbar = "scrollbar" <- "scrollbarSlider.background",
        scrollbar_hover = "scrollbar-hover" <- "scrollbarSlider.hoverBackground",
        scrollbar_active = "scrollbar-active" <- "scrollbarSlider.activeBackground",
        badge = "badge" <- "badge.background",
        badge_fg = "badge-fg" <- "badge.foreground",
        progress = "progress" <- "progressBar.background",
        danger = "danger" <- "errorForeground",
        error = "error" <- "editorError.foreground",
        warning = "warning" <- "editorWarning.foreground",
        info = "info" <- "editorInfo.foreground",
        success = "success" <- "testing.iconPassed",
    }
}

color_set! {
    /// The component colours (L3, `--sv-<kind>-<part>[-<state>]`), resolved. The base sheet and the
    /// UA tables both read these.
    Components, COMPONENT_COLORS, "Every component colour (L3) and the role it defaults to.", {
        /// Push button fill, at rest, hovered, pressed.
        button_bg = "button-bg" <- "button2-bg",
        button_bg_hover = "button-bg-hover" <- "button2-hover",
        button_bg_active = "button-bg-active" <- "toolbar-active",
        button_fg = "button-fg" <- "button2-fg",
        button_border_color = "button-border-color" <- "control-border",
        /// Check box, radio and slider handle fill.
        checkbox_bg = "checkbox-bg" <- "control-bg",
        checkbox_border_color = "checkbox-border-color" <- "control-border",
        /// Text field fill.
        input_bg = "input-bg" <- "control-bg",
        input_border_color = "input-border-color" <- "control-border",
        input_border_color_focus = "input-border-color-focus" <- "focus",
        input_fg = "input-fg" <- "control-fg",
        /// Windows, menus and popups.
        menu_bg = "menu-bg" <- "surface-raised",
        menu_border_color = "menu-border-color" <- "border-menu",
        tooltip_bg = "tooltip-bg" <- "surface-raised",
        /// Side and bottom panels, the status bar.
        panel_bg = "panel-bg" <- "bg",
        tab_marker = "tab-marker" <- "tab-active-marker",
    }
}

macro_rules! metric_set {
    ($(#[$m:meta])* $ty:ident, $table:ident, { $( $(#[doc = $fd:literal])* $field:ident = $name:literal $(<- $src:literal)?, )* }) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug, PartialEq)]
        pub struct $ty { $( $(#[doc = $fd])* pub $field: f32, )* }

        /// The component metrics (L3) and the token they default to.
        pub const $table: &[(&str, &str)] = &[ $( $( ($name, $src), )? )* ];

        impl $ty {
            /// Each token's doc comment (SV71), by bare name; empty where it has none.
            const DOCS: &'static [(&'static str, &'static str)] = &[ $( ($name, concat!($($fd, "\n",)* "")) ),* ];

            fn from_tokens(t: &TokenMap, problems: &mut Vec<String>) -> $ty {
                $ty { $( $field: px_of(t, concat!("--sv-", $name), problems), )* }
            }
        }
    };
}

metric_set! {
    /// Shape, type sizes and metrics in px (scheme-independent but for density; DESIGN.md §1.3,
    /// §1.5, §2.6).
    Metrics, COMPONENT_METRICS, {
        font_size = "font-size",
        font_size_sm = "font-size-sm",
        font_size_xs = "font-size-xs",
        font_size_h1 = "font-size-h1",
        font_size_h2 = "font-size-h2",
        font_size_h3 = "font-size-h3",
        font_size_label1 = "font-size-label1",
        font_size_label2 = "font-size-label2",
        font_size_label3 = "font-size-label3",
        /// Controls (`cornerRadius.small`).
        radius = "radius",
        radius_xs = "radius-xs",
        radius_md = "radius-md",
        radius_lg = "radius-lg",
        radius_xl = "radius-xl",
        space_0_5 = "space-0-5",
        space_1 = "space-1",
        space_1_5 = "space-1-5",
        space_2 = "space-2",
        space_3 = "space-3",
        space_4 = "space-4",
        space_5 = "space-5",
        space_6 = "space-6",
        space_7 = "space-7",
        space_8 = "space-8",
        stroke = "stroke",
        /// List and tree rows (density).
        row_h = "row-h",
        /// Buttons, fields (density).
        control_h = "control-h",
        /// Editor and dock tabs (density).
        tab_h = "tab-h",
        /// Pane and section headers (density).
        header_h = "header-h",
        statusbar_h = "statusbar-h",
        titlebar_h = "titlebar-h",
        activitybar_w = "activitybar-w",
        /// Tree indent per level.
        indent = "indent",
        twistie_w = "twistie-w",
        /// Icon size (density).
        icon_size = "icon-size",
        sash_w = "sash-w",
        scrollbar_w = "scrollbar-w",
        /// Button padding (density).
        button_pad_x = "button-pad-x",
        button_pad_y = "button-pad-y",
        button_radius = "button-radius" <- "radius",
        input_radius = "input-radius" <- "radius",
        menu_radius = "menu-radius" <- "radius-md",
        window_radius = "window-radius" <- "radius-lg",
    }
}

/// A theme shadow (`elev-*`, `popup-shadow`) in CSS terms: offsets, blur radius and spread in px.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shadow {
    pub x: f32,
    pub y: f32,
    pub blur: f32,
    pub spread: f32,
    pub color: Color,
}

impl Shadow {
    pub const NONE: Shadow = Shadow { x: 0.0, y: 0.0, blur: 0.0, spread: 0.0, color: Color::TRANSPARENT };
}

/// The elevation scale (DESIGN.md §2.5); all `NONE` in High Contrast (R7).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Elevation {
    /// `elev-0` (rows, regions, text) to `elev-4` (dialogs, floating windows).
    pub levels: [Shadow; 5],
    /// Menus, tooltips, popovers (`--sv-popup-shadow`, `elev-3` by default).
    pub popup: Shadow,
    /// Dialogs and windows (`--sv-window-shadow`, `elev-4` by default).
    pub window: Shadow,
}

/// The component shadows (L3) and the elevation they default to.
pub const COMPONENT_SHADOWS: &[(&str, &str)] = &[("popup-shadow", "elev-3"), ("window-shadow", "elev-4")];

/// Font families (CSS lists) and motion.
#[derive(Clone, Debug, PartialEq)]
pub struct Fonts {
    pub family: String,
    pub mono: String,
}

/// Motion tokens (modernUI): durations in seconds and the easing curve as CSS.
#[derive(Clone, Debug, PartialEq)]
pub struct Motion {
    pub fast: f32,
    pub normal: f32,
    pub ease_out: String,
}

/// Every L3 component token and its default source token (bare names).
pub fn components() -> impl Iterator<Item = (&'static str, &'static str)> {
    COMPONENT_COLORS
        .iter()
        .map(|&(n, s)| (n, s.unwrap_or_default()))
        .chain(COMPONENT_METRICS.iter().copied())
        .chain(COMPONENT_SHADOWS.iter().copied())
}

fn color_of(t: &TokenMap, name: &str, problems: &mut Vec<String>) -> Color {
    match t.get(name) {
        Some(v) => parse_color(v).unwrap_or_else(|| {
            problems.push(format!("{name}: `{v}` is not a colour"));
            Color::from_rgba8(255, 0, 255, 255)
        }),
        None => {
            problems.push(format!("{name}: missing"));
            Color::from_rgba8(255, 0, 255, 255)
        }
    }
}

fn px_of(t: &TokenMap, name: &str, problems: &mut Vec<String>) -> f32 {
    match t.get(name) {
        Some(v) => parse_px(v).unwrap_or_else(|| {
            problems.push(format!("{name}: `{v}` is not a px length"));
            0.0
        }),
        None => {
            problems.push(format!("{name}: missing"));
            0.0
        }
    }
}

fn shadow_of(t: &TokenMap, name: &str, problems: &mut Vec<String>) -> Shadow {
    match t.get(name) {
        Some(v) => parse_shadow(v).unwrap_or_else(|| {
            problems.push(format!("{name}: `{v}` is not a shadow"));
            Shadow::NONE
        }),
        None => {
            problems.push(format!("{name}: missing"));
            Shadow::NONE
        }
    }
}

/// Where a token's value comes from ([`Theme::explain`]).
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum TokenOrigin {
    /// An override ([`Theme::with`]), with the host's source tag when it gave one
    /// ([`Theme::with_source`]).
    Override { source: Option<Arc<str>> },
    /// It follows another token (by full name): a role its `--vscode-*` key, a component token
    /// its role.
    Follows(String),
    /// The built-in data: the colour set, or the shape, density or affordance tables.
    Builtin,
}

/// A token's doc comment (SV71), by bare name, from the role, component and metric tables; empty
/// for the set's own tokens without one.
pub fn token_doc(name: &str) -> String {
    let name = name.strip_prefix("--sv-").unwrap_or(name);
    let raw = Colors::DOCS.iter().chain(Components::DOCS).chain(Metrics::DOCS).find(|(n, _)| *n == name).map(|(_, d)| *d).unwrap_or("");
    let doc = raw.lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" ");
    if !doc.is_empty() {
        return doc;
    }
    let level = name.strip_prefix("elev-").and_then(|l| l.parse::<u8>().ok());
    match (name, level) {
        (_, Some(l)) => format!("Elevation level {l}: the shadow of surfaces at that height (0: rows and text, 4: dialogs and windows)."),
        ("popup-shadow", _) => "The shadow of menus, tooltips and popovers.".into(),
        ("window-shadow", _) => "The shadow of dialogs and floating windows.".into(),
        ("font-family", _) => "The body font family (a CSS list).".into(),
        ("font-mono", _) => "The monospace font family (a CSS list).".into(),
        _ => String::new(),
    }
}

// ---- the built-in data ----

struct Set {
    name: String,
    colors: Vec<(String, String)>,
    tokens: Vec<(String, String)>,
}

struct Data {
    /// Dark, light, HC dark, HC light.
    sets: [Set; 4],
    shape: Vec<(String, String)>,
    density: [Vec<(String, String)>; 3],
    affordance: [Vec<(String, String)>; 4],
}

fn pairs(v: &serde_json::Value) -> Vec<(String, String)> {
    v.as_object().map(|o| o.iter().filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string()))).collect()).unwrap_or_default()
}

fn data() -> &'static Data {
    static DATA: OnceLock<Data> = OnceLock::new();
    DATA.get_or_init(|| {
        let json = |t: &str| serde_json::from_str::<serde_json::Value>(t).expect("theme data is valid JSON");
        let set = |t: &str| {
            let v = json(t);
            Set { name: v["name"].as_str().unwrap_or_default().to_string(), colors: pairs(&v["colors"]), tokens: pairs(&v["tokens"]) }
        };
        let density = json(include_str!("data/density.json"));
        let aff = json(include_str!("data/affordance.json"));
        Data {
            sets: [
                set(include_str!("data/vscode-dark-modern.json")),
                set(include_str!("data/vscode-light-modern.json")),
                set(include_str!("data/vscode-hc-dark.json")),
                set(include_str!("data/vscode-hc-light.json")),
            ],
            shape: pairs(&json(include_str!("data/shape.json"))["tokens"]),
            density: Density::ALL.map(|d| pairs(&density[d.name()])),
            affordance: ["dark", "light", "hc-dark", "hc-light"].map(|k| pairs(&aff[k])),
        }
    })
}

fn set_index(scheme: ColorScheme, contrast: Contrast) -> usize {
    match (contrast, scheme) {
        (Contrast::Normal, ColorScheme::Dark) => 0,
        (Contrast::Normal, ColorScheme::Light) => 1,
        (Contrast::High, ColorScheme::Dark) => 2,
        (Contrast::High, ColorScheme::Light) => 3,
    }
}

// ---- the theme ----

/// A resolved theme: its recipe (scheme, contrast, density, affordances, overrides), the token
/// map, and typed views of it. Cheap to clone. See the module docs.
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    scheme: ColorScheme,
    contrast: Contrast,
    density: Density,
    affordances: bool,
    overrides: TokenMap,
    /// The host's source tag of each override that has one (SV71), by full name.
    sources: BTreeMap<String, Arc<str>>,
    /// The token each derived token follows ([`Theme::explain`]), by full name.
    follows: BTreeMap<String, String>,
    name: String,
    tokens: TokenMap,
    /// What the token sheet declares: `tokens`, with each component token that follows its source
    /// as `var()` of it (SV68).
    sheet: TokenMap,
    colors: Colors,
    components: Components,
    metrics: Metrics,
    elevation: Elevation,
    fonts: Fonts,
    motion: Motion,
    problems: Vec<String>,
}

/// SV83: the body text size the density tables' metrics are for (VS Code's 13 px).
pub const BASE_FONT_SIZE: f32 = 13.0;

impl Default for Theme {
    /// Dark Modern, default density, affordances on.
    fn default() -> Theme {
        Theme::vscode(ColorScheme::Dark, Contrast::Normal)
    }
}

impl Theme {
    /// A built-in VS Code theme: Dark Modern, Light Modern, or High Contrast dark or light;
    /// default density, affordances on, no overrides.
    pub fn vscode(scheme: ColorScheme, contrast: Contrast) -> Theme {
        Theme::resolve(scheme, contrast, Density::Default, true, TokenMap::new(), BTreeMap::new())
    }

    pub fn dark_modern() -> Theme {
        Theme::vscode(ColorScheme::Dark, Contrast::Normal)
    }

    pub fn light_modern() -> Theme {
        Theme::vscode(ColorScheme::Light, Contrast::Normal)
    }

    pub fn hc_dark() -> Theme {
        Theme::vscode(ColorScheme::Dark, Contrast::High)
    }

    pub fn hc_light() -> Theme {
        Theme::vscode(ColorScheme::Light, Contrast::High)
    }

    /// The same recipe in the other scheme (overrides kept).
    pub fn with_scheme(&self, scheme: ColorScheme) -> Theme {
        Theme::resolve(scheme, self.contrast, self.density, self.affordances, self.overrides.clone(), self.sources.clone())
    }

    pub fn with_contrast(&self, contrast: Contrast) -> Theme {
        Theme::resolve(self.scheme, contrast, self.density, self.affordances, self.overrides.clone(), self.sources.clone())
    }

    pub fn with_density(&self, density: Density) -> Theme {
        Theme::resolve(self.scheme, self.contrast, density, self.affordances, self.overrides.clone(), self.sources.clone())
    }

    /// The affordance overlay's token values (DESIGN.md §2; on by default). Off gives VS Code's
    /// exact colours (VIS1's base). DS3's affordance sheet follows the same switch.
    pub fn with_affordances(&self, on: bool) -> Theme {
        Theme::resolve(self.scheme, self.contrast, self.density, on, self.overrides.clone(), self.sources.clone())
    }

    /// Overrides on top of the recipe's (later wins): `--vscode-<key>` (L1; the roles follow),
    /// `--sv-<role>` (L2; the components follow), `--sv-<kind>-<part>` (L3), and any other custom
    /// property (`--cad-*`), passed through. A bare name is `--sv-<name>`.
    pub fn with<K: AsRef<str>, V: Into<String>>(&self, overrides: impl IntoIterator<Item = (K, V)>) -> Theme {
        self.with_tagged(None, overrides)
    }

    /// [`Theme::with`], tagging each override with `source`, a name the host chooses (a settings
    /// layer, a theme file, a preview): [`Theme::override_source`] and [`Theme::explain`] report
    /// it (SV71). A later override of the same token replaces the tag (an untagged one clears it).
    pub fn with_source<K: AsRef<str>, V: Into<String>>(&self, source: &str, overrides: impl IntoIterator<Item = (K, V)>) -> Theme {
        self.with_tagged(Some(Arc::from(source)), overrides)
    }

    fn with_tagged<K: AsRef<str>, V: Into<String>>(&self, source: Option<Arc<str>>, overrides: impl IntoIterator<Item = (K, V)>) -> Theme {
        let mut o = self.overrides.clone();
        let mut sources = self.sources.clone();
        for (k, v) in overrides {
            let name = full_name(k.as_ref()).into_owned();
            match &source {
                Some(s) => sources.insert(name.clone(), s.clone()),
                None => sources.remove(&name),
            };
            o.insert(name, v);
        }
        Theme::resolve(self.scheme, self.contrast, self.density, self.affordances, o, sources)
    }

    /// The source tag of the override of token `name` ([`Theme::with_source`]); `None` when it has
    /// no override or an untagged one.
    pub fn override_source(&self, name: &str) -> Option<&str> {
        self.sources.get(&*full_name(name)).map(|s| &**s)
    }

    /// Where token `name`'s value comes from (SV71), one step: an override (with its source tag),
    /// another token it follows (a role its `--vscode-*` key, a component its role), or the built-in
    /// data (the set, shape, density or affordance tables). `None` for a token the theme lacks.
    /// Follow [`TokenOrigin::Follows`] for the chain.
    pub fn explain(&self, name: &str) -> Option<TokenOrigin> {
        let full = full_name(name);
        if self.overrides.get(&full).is_some() {
            return Some(TokenOrigin::Override { source: self.sources.get(&*full).cloned() });
        }
        if let Some(f) = self.follows.get(&*full) {
            return Some(TokenOrigin::Follows(f.clone()));
        }
        self.tokens.get(&full).map(|_| TokenOrigin::Builtin)
    }

    fn resolve(scheme: ColorScheme, contrast: Contrast, density: Density, affordances: bool, overrides: TokenMap, sources: BTreeMap<String, Arc<str>>) -> Theme {
        let d = data();
        let k = set_index(scheme, contrast);
        let set = &d.sets[k];
        // L1
        let mut vs: BTreeMap<String, String> = set.colors.iter().cloned().collect();
        for (name, v) in overrides.iter() {
            if let Some(key) = vscode_key(name) {
                vs.insert(key, v.to_string());
            }
        }
        // L2
        let mut sv: BTreeMap<String, String> = BTreeMap::new();
        // what a token follows (SV71's `explain`): a role its VS Code key, a component its source
        let mut follows: BTreeMap<String, String> = BTreeMap::new();
        for &(role, key) in ROLES {
            if let Some((key, v)) = key.and_then(|key| Some((key, vs.get(key)?))) {
                sv.insert(role.to_string(), v.clone());
                follows.insert(format!("--sv-{role}"), vscode_var(key));
            }
        }
        // POL3-FINISH: VS Code's menus have their own outline (Dark Modern `#454545` over
        // `widget.border` `#313131`); a theme that names none keeps the widget outline
        if !sv.contains_key("border-menu")
            && let Some(v) = sv.get("border-widget").cloned()
        {
            sv.insert("border-menu".into(), v);
            follows.insert("--sv-border-menu".into(), vscode_var("widget.border"));
        }
        let layers = [&d.shape, &d.density[density as usize], &set.tokens];
        for (name, v) in layers.into_iter().flatten() {
            sv.insert(name.clone(), v.clone());
            follows.remove(&format!("--sv-{name}"));
        }
        if affordances {
            for (name, v) in &d.affordance[k] {
                sv.insert(name.clone(), v.clone());
                follows.remove(&format!("--sv-{name}"));
            }
        }
        let is_component = |n: &str| components().any(|(c, _)| c == n);
        let mut extra = TokenMap::new();
        for (name, v) in overrides.iter() {
            match name.strip_prefix("--sv-") {
                Some(n) if !is_component(n) => {
                    sv.insert(n.to_string(), v.to_string());
                    follows.remove(name);
                }
                Some(_) => {}
                None if name.starts_with("--vscode-") => {}
                None => extra.insert(name, v),
            }
        }
        // L3: resolved here for the typed views; the sheet keeps each one that follows its source
        // as `var()` of it (SV68), so an override of the source in the cascade reaches it
        let mut aliases: BTreeMap<String, String> = BTreeMap::new();
        for (c, src) in components() {
            if let Some(v) = sv.get(src).cloned() {
                sv.insert(c.to_string(), v);
                aliases.insert(format!("--sv-{c}"), format!("var(--sv-{src})"));
                follows.insert(format!("--sv-{c}"), format!("--sv-{src}"));
            }
        }
        for (name, v) in overrides.iter() {
            if let Some(n) = name.strip_prefix("--sv-")
                && is_component(n)
            {
                sv.insert(n.to_string(), v.to_string());
                aliases.remove(name);
                follows.remove(name);
            }
        }
        let mut tokens = extra;
        for (key, v) in vs {
            tokens.insert(vscode_var(&key), v);
        }
        for (n, v) in sv {
            tokens.insert(format!("--sv-{n}"), v);
        }
        let mut problems = Vec::new();
        let colors = Colors::from_tokens(&tokens, &mut problems);
        let components = Components::from_tokens(&tokens, &mut problems);
        let metrics = Metrics::from_tokens(&tokens, &mut problems);
        let levels = [0, 1, 2, 3, 4].map(|i| shadow_of(&tokens, &format!("--sv-elev-{i}"), &mut problems));
        let elevation = Elevation { levels, popup: shadow_of(&tokens, "--sv-popup-shadow", &mut problems), window: shadow_of(&tokens, "--sv-window-shadow", &mut problems) };
        let text = |n: &str| tokens.get(n).unwrap_or_default().to_string();
        let fonts = Fonts { family: text("--sv-font-family"), mono: text("--sv-font-mono") };
        let secs = |n: &str| tokens.get(n).and_then(parse_duration).unwrap_or(0.0);
        let motion = Motion { fast: secs("--sv-motion-fast"), normal: secs("--sv-motion"), ease_out: text("--sv-ease-out") };
        let mut sheet = tokens.clone();
        for (name, v) in aliases {
            sheet.insert(name, v);
        }
        Theme { scheme, contrast, density, affordances, overrides, sources, follows, name: set.name.clone(), tokens, sheet, colors, components, metrics, elevation, fonts, motion, problems }
    }

    pub fn scheme(&self) -> ColorScheme {
        self.scheme
    }

    pub fn is_dark(&self) -> bool {
        self.scheme == ColorScheme::Dark
    }

    pub fn contrast(&self) -> Contrast {
        self.contrast
    }

    pub fn density(&self) -> Density {
        self.density
    }

    pub fn affordances(&self) -> bool {
        self.affordances
    }

    /// The base set's name (`Dark Modern`).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The overrides given with [`Theme::with`].
    pub fn overrides(&self) -> &TokenMap {
        &self.overrides
    }

    /// Every resolved token (L1, L2, L3 and passed-through overrides).
    pub fn tokens(&self) -> &TokenMap {
        &self.tokens
    }

    /// Every token as the token sheet declares it ([`Theme::css`]): [`Theme::tokens`], except that
    /// each L3 component token that follows its source is `var(--sv-<source>)` (SV68), so an
    /// override of the source in the cascade reaches the component too. The typed views and
    /// [`Theme::tokens`] keep the resolved values.
    pub fn sheet_tokens(&self) -> &TokenMap {
        &self.sheet
    }

    /// The edits that turn `old`'s token sheet into this one's, for
    /// `Document::set_root_tokens` on the sheet [`Theme::css`] made: changed and new tokens with
    /// their value, removed ones with `None`.
    pub fn sheet_changes(&self, old: &Theme) -> Vec<(String, Option<String>)> {
        let mut out: Vec<(String, Option<String>)> = Vec::new();
        for (k, v) in self.sheet.iter() {
            if old.sheet.get(k) != Some(v) {
                out.push((k.to_string(), Some(v.to_string())));
            }
        }
        for (k, _) in old.sheet.iter() {
            if self.sheet.get(k).is_none() {
                out.push((k.to_string(), None));
            }
        }
        out
    }

    /// One token's CSS text (`bg`, `--sv-bg`, `--vscode-foreground`).
    pub fn token(&self, name: &str) -> Option<&str> {
        self.tokens.get(&full_name(name))
    }

    /// One token as a colour, by name (`list-hover`, `--vscode-focusBorder`, `--cad-panel`).
    pub fn color(&self, name: &str) -> Option<Color> {
        self.tokens.color(&full_name(name))
    }

    /// One token as px, by name (`row-h`).
    pub fn px(&self, name: &str) -> Option<f32> {
        self.tokens.px(&full_name(name))
    }

    /// One token as a shadow, by name (`elev-3`).
    pub fn shadow(&self, name: &str) -> Option<Shadow> {
        self.tokens.shadow(&full_name(name))
    }

    pub fn colors(&self) -> &Colors {
        &self.colors
    }

    pub fn components(&self) -> &Components {
        &self.components
    }

    pub fn metrics(&self) -> &Metrics {
        &self.metrics
    }

    /// SV83: the height of a list or tree row holding text of `font_size` points: the density's
    /// `row-h`, grown in proportion as the text grows past [`BASE_FONT_SIZE`] (a larger text size
    /// setting), never shorter than `row-h`.
    pub fn row_height_for(&self, font_size: f32) -> f32 {
        let h = self.metrics.row_h;
        (h * (font_size / BASE_FONT_SIZE).max(1.0)).round()
    }

    pub fn elevation(&self) -> &Elevation {
        &self.elevation
    }

    pub fn fonts(&self) -> &Fonts {
        &self.fonts
    }

    pub fn motion(&self) -> &Motion {
        &self.motion
    }

    /// Tokens the typed views could not read (missing, or a value they do not parse): empty for
    /// the built-in sets; an importer (DS4) reports them.
    pub fn problems(&self) -> &[String] {
        &self.problems
    }

    /// U3: the names and types of this theme's `--sv-*` tokens (L2 roles, L3 components, shape,
    /// density and set tokens, and `--sv-*` overrides such as cadatomic's `--sv-cad-panel`): what
    /// utility values resolve against, so `bg-cad-panel` is a utility once the theme has that
    /// colour. The types are the token manifest's ([`token_manifest`]).
    pub fn utility_tokens(&self) -> svelters_style::UtilityTokens {
        self.tokens.iter().filter_map(|(k, v)| k.strip_prefix("--sv-").map(|n| (n, token_type(n, v)))).collect()
    }

    /// The `:root` sheet of every token, theme origin in `@layer sv.tokens`; component tokens that
    /// follow their source are `var()` of it ([`Theme::sheet_tokens`]).
    pub fn css(&self) -> String {
        format!("{LAYERS}\n@layer sv.tokens {{\n{}}}\n", self.sheet.root_rule())
    }
}

/// The token sheet while no theme is set: the default theme in both schemes, under
/// `prefers-color-scheme`.
pub fn default_tokens_css() -> &'static str {
    static CSS: OnceLock<String> = OnceLock::new();
    CSS.get_or_init(|| {
        let mut s = format!("{LAYERS}\n@layer sv.tokens {{\n");
        for (t, q) in [(Theme::dark_modern(), "dark"), (Theme::light_modern(), "light")] {
            s.push_str(&format!("@media (prefers-color-scheme: {q}) {{\n{}}}\n", t.sheet.root_rule()));
        }
        s.push_str("}\n");
        s
    })
}

/// U3: the token names utilities resolve against while no theme is set: the default theme's
/// (both schemes, as [`default_tokens_css`] installs them).
pub fn default_utility_tokens() -> &'static svelters_style::UtilityTokens {
    static T: OnceLock<svelters_style::UtilityTokens> = OnceLock::new();
    T.get_or_init(|| {
        let mut t = Theme::light_modern().utility_tokens();
        for (k, v) in Theme::dark_modern().tokens().iter() {
            if let Some(n) = k.strip_prefix("--sv-") {
                t.insert(n, token_type(n, v));
            }
        }
        t
    })
}

// ---- U0: the token manifest ----

/// U0 (UTILITY-CSS §3.4): one L2/L3 token (`--sv-<name>`) as the spec's `theme.tokens` lists it:
/// for the utility generator (colour, scale and metric names), the docs and the editors. The
/// role and component tables above stay the single source; this only reads them.
#[derive(Clone, Debug, PartialEq)]
pub struct TokenInfo {
    /// The bare name (`list-hover`; the property is `--sv-list-hover`).
    pub name: String,
    /// `role` (L2 colour role, [`ROLES`]), `component` (L3, [`components`]), `density` (one value
    /// per [`Density`]), `shape` (scheme-independent) or `set` (per colour set, elevation).
    pub layer: &'static str,
    /// `color`, `length`, `shadow`, `duration`, `easing`, `font-family` or `raw`.
    pub ty: &'static str,
    /// A role's VS Code colour key (`list.hoverBackground`).
    pub vscode: Option<&'static str>,
    /// A component token's default source token (`button-bg` ← `button2-bg`).
    pub from: Option<&'static str>,
    /// Its value in each built-in set (`dark`, `light`, `hc-dark`, `hc-light`), at the default
    /// density with the affordance overlay on.
    pub values: [(&'static str, String); 4],
    /// A density token's value per density (`compact`, `default`, `comfortable`); empty otherwise.
    pub density: Vec<(&'static str, String)>,
    /// What the token is for (SV71, [`token_doc`]); empty where the tables give no doc.
    pub doc: String,
}

/// The built-in sets by name, in [`set_index`] order.
pub const SET_NAMES: [&str; 4] = ["dark", "light", "hc-dark", "hc-light"];

/// U0: every L2/L3 token of the built-in themes (the `--sv-*` names; no L1 `--vscode-*` keys), in
/// name order. See [`TokenInfo`].
pub fn token_manifest() -> Vec<TokenInfo> {
    let d = data();
    let themes = [Theme::dark_modern(), Theme::light_modern(), Theme::hc_dark(), Theme::hc_light()];
    let mut names: Vec<String> = Vec::new();
    for t in &themes {
        for (k, _) in t.tokens().iter() {
            if let Some(n) = k.strip_prefix("--sv-")
                && !names.iter().any(|x| x == n)
            {
                names.push(n.to_string());
            }
        }
    }
    names.sort();
    let in_density = |n: &str| d.density.iter().any(|set| set.iter().any(|(k, _)| k == n));
    let in_shape = |n: &str| d.shape.iter().any(|(k, _)| k == n);
    names
        .into_iter()
        .map(|name| {
            let role = ROLES.iter().find(|(r, _)| *r == name);
            let component = components().find(|(c, _)| *c == name);
            let layer = if role.is_some() {
                "role"
            } else if component.is_some() {
                "component"
            } else if in_density(&name) {
                "density"
            } else if in_shape(&name) {
                "shape"
            } else {
                "set"
            };
            let values = [0, 1, 2, 3].map(|i| (SET_NAMES[i], themes[i].token(&name).unwrap_or_default().to_string()));
            let ty = token_type(&name, &values[0].1);
            let density = if layer == "density" {
                Density::ALL.iter().map(|&dn| (dn.name(), themes[0].with_density(dn).token(&name).unwrap_or_default().to_string())).collect()
            } else {
                Vec::new()
            };
            TokenInfo { layer, ty, vscode: role.and_then(|r| r.1), from: component.map(|c| c.1).filter(|s| !s.is_empty()), values, density, doc: token_doc(&name), name }
        })
        .collect()
}

/// A token's value type, from the tables first (a colour role, an elevation), else from its value.
pub fn token_type(name: &str, value: &str) -> &'static str {
    let colors = ROLES.iter().chain(COMPONENT_COLORS).any(|(n, _)| *n == name);
    if colors {
        return "color";
    }
    if name.starts_with("elev-") || COMPONENT_SHADOWS.iter().any(|(n, _)| *n == name) {
        return "shadow";
    }
    let v = value.trim();
    if parse_px(v).is_some() {
        "length"
    } else if parse_duration(v).is_some() {
        "duration"
    } else if ["cubic-bezier(", "steps(", "linear", "ease"].iter().any(|p| v.starts_with(p)) {
        "easing"
    } else if parse_color(v).is_some() {
        "color"
    } else if name.starts_with("font-") {
        "font-family"
    } else {
        "raw"
    }
}

// ---- value parsing ----

/// `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, `rgb()`/`rgba()`, `transparent`.
pub fn parse_color(v: &str) -> Option<Color> {
    let v = v.trim();
    if v.eq_ignore_ascii_case("transparent") {
        return Some(Color::TRANSPARENT);
    }
    if let Some(h) = v.strip_prefix('#') {
        let n = |s: &str| u8::from_str_radix(s, 16).ok();
        let d = |i: usize| n(&h[i..i + 1]).map(|x| x * 17);
        let [r, g, b, a] = match h.len() {
            3 => [d(0)?, d(1)?, d(2)?, 255],
            4 => [d(0)?, d(1)?, d(2)?, d(3)?],
            6 => [n(&h[0..2])?, n(&h[2..4])?, n(&h[4..6])?, 255],
            8 => [n(&h[0..2])?, n(&h[2..4])?, n(&h[4..6])?, n(&h[6..8])?],
            _ => return None,
        };
        return Some(Color::from_rgba8(r, g, b, a));
    }
    let inner = v.strip_prefix("rgba(").or_else(|| v.strip_prefix("rgb("))?.strip_suffix(')')?;
    let parts: Vec<&str> = inner.split([',', ' ', '/']).filter(|s| !s.is_empty()).collect();
    if parts.len() < 3 || parts.len() > 4 {
        return None;
    }
    let ch = |s: &str| -> Option<u8> {
        Some(match s.strip_suffix('%') {
            Some(p) => (p.parse::<f32>().ok()? * 2.55).round().clamp(0.0, 255.0) as u8,
            None => s.parse::<f32>().ok()?.round().clamp(0.0, 255.0) as u8,
        })
    };
    let a = match parts.get(3) {
        Some(s) => match s.strip_suffix('%') {
            Some(p) => p.parse::<f32>().ok()? / 100.0,
            None => s.parse::<f32>().ok()?,
        },
        None => 1.0,
    };
    Some(Color::from_rgba8(ch(parts[0])?, ch(parts[1])?, ch(parts[2])?, (a.clamp(0.0, 1.0) * 255.0).round() as u8))
}

/// `13px` or `13`.
pub fn parse_px(v: &str) -> Option<f32> {
    let v = v.trim();
    v.strip_suffix("px").unwrap_or(v).trim().parse().ok()
}

/// `100ms` or `0.1s`, in seconds.
fn parse_duration(v: &str) -> Option<f32> {
    let v = v.trim();
    if let Some(ms) = v.strip_suffix("ms") {
        return ms.trim().parse::<f32>().ok().map(|x| x / 1000.0);
    }
    v.strip_suffix('s')?.trim().parse().ok()
}

/// `none`, or one outer shadow `<x> <y> [<blur> [<spread>]] <color>` (px lengths, colour last).
pub fn parse_shadow(v: &str) -> Option<Shadow> {
    let v = v.trim();
    if v == "none" {
        return Some(Shadow::NONE);
    }
    let parts: Vec<&str> = v.split_whitespace().collect();
    let (color, lens) = parts.split_last()?;
    let color = parse_color(color)?;
    let l: Vec<f32> = lens.iter().map(|s| parse_px(s)).collect::<Option<_>>()?;
    if l.len() < 2 || l.len() > 4 {
        return None;
    }
    let get = |i: usize| l.get(i).copied().unwrap_or(0.0);
    Some(Shadow { x: get(0), y: get(1), blur: get(2).max(0.0), spread: get(3), color })
}

/// WCAG relative luminance.
fn luminance(c: Color) -> f32 {
    let lin = |s: f32| {
        if s <= 0.04045 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b)
}

/// WCAG contrast ratio of two opaque colours.
pub fn contrast_ratio(a: Color, b: Color) -> f32 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every built-in set, in every density, with and without affordances, resolves every role,
    /// component, metric and elevation (nothing missing, everything parses).
    #[test]
    fn builtin_themes_resolve_every_token() {
        for scheme in [ColorScheme::Dark, ColorScheme::Light] {
            for contrast in [Contrast::Normal, Contrast::High] {
                for density in Density::ALL {
                    for aff in [true, false] {
                        let t = Theme::vscode(scheme, contrast).with_density(density).with_affordances(aff);
                        assert!(t.problems().is_empty(), "{} {density:?} aff={aff}: {:?}", t.name(), t.problems());
                    }
                }
            }
        }
        let d = Theme::dark_modern();
        assert_eq!(d.colors().bg, Color::from_rgba8(0x18, 0x18, 0x18, 255));
        assert_eq!(d.token("--vscode-list-hoverBackground"), Some("#2A2D2E"), "the L1 alias layer");
        assert_eq!(d.components().button_bg, Color::from_rgba8(0x31, 0x31, 0x31, 255), "affordance overlay");
        assert_eq!(d.with_affordances(false).components().button_bg, Color::TRANSPARENT, "exact VS Code");
        assert_eq!(Theme::light_modern().colors().muted, Color::from_rgba8(0x61, 0x61, 0x61, 255));
        assert_eq!(d.metrics().control_h, 26.0);
        assert_eq!(d.with_density(Density::Compact).metrics().row_h, 20.0);
        assert_eq!(d.motion().normal, 0.15);
    }

    /// SV68: the sheet keeps a component token that follows its role as `var()` of it; the typed
    /// views and `tokens()` keep the resolved value; an override of the component is a literal.
    #[test]
    fn component_tokens_stay_aliases_in_the_sheet() {
        let t = Theme::dark_modern();
        assert!(t.css().contains("--sv-button-bg: var(--sv-button2-bg);"));
        assert_eq!(t.token("button-bg"), t.token("button2-bg"));
        assert_eq!(t.components().button_bg, t.colors().button2_bg);
        let o = t.with([("button-bg", "#ff0000")]);
        assert!(o.css().contains("--sv-button-bg: #ff0000;"));
        assert_eq!(o.sheet_tokens().get("--sv-checkbox-bg"), Some("var(--sv-control-bg)"));
        // the edits between two themes' sheets
        let edits = o.sheet_changes(&t);
        assert_eq!(edits, vec![("--sv-button-bg".to_string(), Some("#ff0000".to_string()))]);
    }

    /// SV71: overrides carry the host's source tag; `explain` follows the chain; the manifest has
    /// docs.
    #[test]
    fn overrides_carry_their_source() {
        let t = Theme::dark_modern().with_source("user", [("accent", "#ff0000")]).with([("bg", "#000000")]).with_source("preview", [("--vscode-textLink-foreground", "#00ff00")]);
        assert_eq!(t.override_source("accent"), Some("user"));
        assert_eq!(t.override_source("bg"), None);
        assert_eq!(t.explain("accent"), Some(TokenOrigin::Override { source: Some(Arc::from("user")) }));
        assert_eq!(t.explain("bg"), Some(TokenOrigin::Override { source: None }));
        assert_eq!(t.explain("link"), Some(TokenOrigin::Follows("--vscode-textLink-foreground".into())));
        assert_eq!(t.token("link"), Some("#00ff00"));
        assert_eq!(t.explain("--vscode-textLink-foreground"), Some(TokenOrigin::Override { source: Some(Arc::from("preview")) }));
        assert_eq!(t.explain("button-bg"), Some(TokenOrigin::Follows("--sv-button2-bg".into())));
        assert_eq!(t.explain("radius"), Some(TokenOrigin::Builtin));
        assert_eq!(t.explain("no-such-token"), None);
        // a later untagged override clears the tag; the tag survives a scheme change
        assert_eq!(t.with([("accent", "#0000ff")]).override_source("accent"), None);
        assert_eq!(t.with_scheme(ColorScheme::Light).override_source("accent"), Some("user"));
        let m = token_manifest();
        let bg = m.iter().find(|i| i.name == "bg").unwrap();
        assert_eq!(bg.doc, "Chrome: side bar, panel, title, status, activity bar.");
        assert!(m.iter().find(|i| i.name == "elev-1").is_some_and(|i| !i.doc.is_empty()));
    }

    #[test]
    fn overrides_flow_through_the_layers() {
        let t = Theme::dark_modern().with_affordances(false).with([("--vscode-input-border", "#FF0000"), ("--cad-panel", "#101010")]);
        assert_eq!(t.colors().control_border, Color::from_rgba8(255, 0, 0, 255), "L1 → L2");
        assert_eq!(t.components().input_border_color, Color::from_rgba8(255, 0, 0, 255), "L2 → L3");
        assert_eq!(t.token("--cad-panel"), Some("#101010"), "passed through");
        let t = t.with([("control-border", "#00FF00"), ("checkbox-border-color", "#0000FF")]);
        assert_eq!(t.components().input_border_color, Color::from_rgba8(0, 255, 0, 255));
        assert_eq!(t.components().checkbox_border_color, Color::from_rgba8(0, 0, 255, 255), "L3 override wins");
        let light = t.with_scheme(ColorScheme::Light);
        assert_eq!(light.components().checkbox_border_color, Color::from_rgba8(0, 0, 255, 255), "overrides survive a scheme switch");
        assert!(t.css().contains("--sv-button-bg: "));
    }

    #[test]
    fn values_parse() {
        assert_eq!(parse_color("#0000005C"), Some(Color::from_rgba8(0, 0, 0, 0x5C)));
        assert_eq!(parse_color("#fff"), Some(Color::WHITE));
        assert_eq!(parse_color("rgba(0, 0, 0, .5)"), Some(Color::from_rgba8(0, 0, 0, 128)));
        assert_eq!(parse_shadow("0 2px 8px #0000005C"), Some(Shadow { x: 0.0, y: 2.0, blur: 8.0, spread: 0.0, color: Color::from_rgba8(0, 0, 0, 0x5C) }));
        assert_eq!(parse_shadow("none"), Some(Shadow::NONE));
        assert_eq!(parse_shadow("inset 0 1px 2px #000"), None);
        assert_eq!(vscode_var("list.hoverBackground"), "--vscode-list-hoverBackground");
        assert!(contrast_ratio(Color::WHITE, Color::BLACK) > 20.9);
    }
}
