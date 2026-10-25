//! Stand-ins for APIs the views need that their owning crates do not have yet. Each item names the
//! API it stands in for; the consolidation replaces them with the real ones and deletes this module.
//!
//! - [`Kind`], [`KindSpec`], [`PropSpec`], [`SignalSpec`], [`Value`], [`EventCx`], [`RecordCx`],
//!   [`Handled`]: `presto_core::{Kind, KindSpec, Value, EventCx, RecordCx}` (architecture §4.2).
//!   The real `Kind` reads props from the tree; here a kind keeps them in its own fields and takes
//!   writes through [`Kind::set_prop`].
//! - [`Event`]: `presto_core`'s dispatched event (an input event in node-local points).
//! - [`Labels`]: shaping a label into glyph runs the compositor holds by `RunId`
//!   (`presto_text::TextSystem::layout` plus the run registration `presto-core` does at record).
//! - [`Policy`], [`Os`]: the platform behaviour table (svelters-egui `native::Policy`), which
//!   lane WIDGETS-BASIC ports into `interact`/`theme`.
//! - [`Palette`]: the theme tokens the views paint with (lane WIDGETS-BASIC's `theme`).

use presto_base::accesskit;
use presto_base::change::ChangeClass;
use presto_base::color::Color;
use presto_base::commit::FastCommit;
use presto_base::cursor::CursorIcon;
use presto_base::display::DisplayList;
use presto_base::event::{GestureEvent, KeyEvent, Modifiers, PointerEvent, WheelEvent};
use presto_base::geom::{Point, Size};
use presto_base::time::{Span, Time};

/// The type of a prop as the spec declares it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropType {
    Bool,
    I64,
    F64,
    Str,
    /// One of these names.
    Enum(&'static [&'static str]),
    Color,
    List,
    Record,
}

/// One prop of a kind: its name, type and change class (architecture §4.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PropSpec {
    pub name: &'static str,
    pub ty: PropType,
    pub change: ChangeClass,
}

/// One signal (event) a kind emits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SignalSpec {
    pub name: &'static str,
    /// Listeners never cancel it: native code does not wait for JS (architecture §11.5).
    pub passive: bool,
    /// A listener may `preventDefault()` it.
    pub cancelable: bool,
}

/// What a kind declares (architecture §4.2).
#[derive(Clone, Copy, Debug)]
pub struct KindSpec {
    /// The element name (`scroll-area`, `menu-item`).
    pub name: &'static str,
    pub props: &'static [PropSpec],
    pub signals: &'static [SignalSpec],
    /// Named child slots (`""` is the default slot).
    pub slots: &'static [&'static str],
    pub role: accesskit::Role,
}

impl KindSpec {
    pub fn prop(&self, name: &str) -> Option<&PropSpec> {
        self.props.iter().find(|p| p.name == name)
    }
    pub fn signal(&self, name: &str) -> Option<&SignalSpec> {
        self.signals.iter().find(|s| s.name == name)
    }
}

/// Shorthand for spec tables.
pub const fn prop(name: &'static str, ty: PropType, change: ChangeClass) -> PropSpec {
    PropSpec { name, ty, change }
}
pub const fn signal(name: &'static str, passive: bool, cancelable: bool) -> SignalSpec {
    SignalSpec { name, passive, cancelable }
}

/// A prop value.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    I64(i64),
    F64(f64),
    Str(String),
    Color(Color),
    List(Vec<Value>),
    Record(Vec<(String, Value)>),
}

impl Value {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::F64(v) => Some(*v),
            Value::I64(v) => Some(*v as f64),
            _ => None,
        }
    }
    pub fn as_f32(&self) -> Option<f32> {
        self.as_f64().map(|v| v as f32)
    }
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::I64(v) => Some(*v),
            Value::F64(v) if v.fract() == 0.0 => Some(*v as i64),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_list(&self) -> Option<&[Value]> {
        match self {
            Value::List(l) => Some(l),
            _ => None,
        }
    }
    /// A record field by name.
    pub fn field(&self, name: &str) -> Option<&Value> {
        match self {
            Value::Record(fs) => fs.iter().find(|(k, _)| k == name).map(|(_, v)| v),
            _ => None,
        }
    }
}

/// Why a prop write was refused.
#[derive(Clone, Debug, PartialEq)]
pub enum PropError {
    /// The kind has no prop of that name.
    Unknown,
    /// The value has the wrong type or is out of range.
    Invalid(&'static str),
    /// Native code changed this state since the event JS saw (architecture §4.5).
    Stale,
}

/// An input event as it reaches a node, positions in node-local points.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Pointer(PointerEvent),
    Wheel(WheelEvent),
    Key(KeyEvent),
    Gesture(GestureEvent),
    /// Keyboard focus entered (`true`) or left the node.
    Focus(bool),
    /// An AccessKit action addressed to the node.
    A11y(accesskit::ActionRequest),
    /// The deadline the kind asked for with [`EventCx::wake_at`] passed.
    Timer,
}

/// Whether a kind took an event (stops native propagation; JS listeners still run).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handled {
    No,
    Yes,
}

/// A signal a kind emitted while handling an event: delivered to JS in order.
#[derive(Clone, Debug, PartialEq)]
pub struct Emitted {
    pub signal: &'static str,
    pub detail: Value,
}

/// The operating system whose behaviour a kind follows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Os {
    Windows,
    MacOs,
    Linux,
}

impl Os {
    /// The OS this build runs on.
    pub fn current() -> Os {
        if cfg!(target_os = "windows") {
            Os::Windows
        } else if cfg!(target_os = "macos") {
            Os::MacOs
        } else {
            Os::Linux
        }
    }
}

/// How momentum scrolling is produced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kinetic {
    /// The OS sends momentum phases (macOS, Windows precision touchpads).
    Os,
    /// presto flings from the release velocity (Linux touchpads without OS momentum).
    Own,
    /// None.
    None,
}

/// What a primary press on a scroll bar's track does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrackClick {
    /// Scrolls one page towards the press, repeating while held.
    Page,
    /// Moves the thumb's centre to the press and drags it from there.
    Jump,
}

/// Scroll bars over the content (shown while scrolling) or beside it (always).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scrollbars {
    Overlay,
    Classic,
}

/// A platform's dialog button order (`dialog-buttons`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonOrder {
    /// `accept action reject destructive apply` (QDialogButtonBox WinLayout).
    Windows,
    /// `accept action apply destructive reject` (KdeLayout).
    Kde,
    /// `destructive action apply reject accept` (GnomeLayout).
    Gnome,
    /// `destructive apply action reject accept` (MacLayout).
    MacOs,
}

/// The platform behaviour the views read (svelters-egui `native::Policy`, the view fields only).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Policy {
    pub os: Os,
    /// Points the pointer moves before a press becomes a drag.
    pub drag_threshold: f32,
    pub double_click: Span,
    /// Hover time before a tooltip shows.
    pub tooltip_delay: Span,
    /// After a tooltip hid, how long another one shows at once.
    pub tooltip_follow_on: Span,
    /// Hover time on a submenu row before the submenu opens.
    pub submenu_delay: Span,
    /// A lone Alt press and release highlights the menu bar's first menu.
    pub alt_focuses_menu_bar: bool,
    /// A secondary click on an unselected row selects it before its context menu opens.
    pub context_click_selects: bool,
    /// How long a drag hovers over a collapsed tree row before it expands (`Span::ZERO`: never).
    pub drag_expand: Span,
    pub kinetic: Kinetic,
    pub scrollbars: Scrollbars,
    pub track_click: TrackClick,
    /// Shift+press on the track does the other [`TrackClick`].
    pub shift_track_click_inverts: bool,
    /// Points one arrow key scrolls.
    pub scroll_line: f32,
    /// Overlay bars stay this long after the last scroll or hover, then fade over `bar_fade`.
    pub bar_hold: Span,
    pub bar_fade: Span,
    pub button_order: ButtonOrder,
    /// `prefers-reduced-motion`: smooth curves become instant.
    pub reduced_motion: bool,
}

impl Policy {
    /// The defaults per OS (svelters-egui `native::{WINDOWS, MAC, GTK}`).
    pub fn for_os(os: Os) -> Policy {
        let ms = Span::from_millis;
        match os {
            Os::Windows => Policy {
                os,
                drag_threshold: 4.0,
                double_click: ms(500),
                tooltip_delay: ms(400),
                tooltip_follow_on: ms(2000),
                submenu_delay: ms(400),
                alt_focuses_menu_bar: true,
                context_click_selects: true,
                drag_expand: ms(500),
                kinetic: Kinetic::Os,
                scrollbars: Scrollbars::Overlay,
                track_click: TrackClick::Page,
                shift_track_click_inverts: false,
                scroll_line: 40.0,
                bar_hold: ms(500),
                bar_fade: ms(800),
                button_order: ButtonOrder::Windows,
                reduced_motion: false,
            },
            Os::MacOs => Policy {
                os,
                drag_threshold: 3.0,
                double_click: ms(500),
                tooltip_delay: ms(1000),
                tooltip_follow_on: ms(1000),
                submenu_delay: Span::ZERO,
                alt_focuses_menu_bar: false,
                context_click_selects: false,
                drag_expand: ms(500),
                kinetic: Kinetic::Os,
                scrollbars: Scrollbars::Overlay,
                track_click: TrackClick::Jump,
                shift_track_click_inverts: false,
                scroll_line: 40.0,
                bar_hold: ms(500),
                bar_fade: ms(800),
                button_order: ButtonOrder::MacOs,
                reduced_motion: false,
            },
            Os::Linux => Policy {
                os,
                drag_threshold: 8.0,
                double_click: ms(400),
                tooltip_delay: ms(500),
                tooltip_follow_on: ms(1000),
                submenu_delay: ms(225),
                alt_focuses_menu_bar: false,
                context_click_selects: true,
                drag_expand: ms(500),
                kinetic: Kinetic::Own,
                scrollbars: Scrollbars::Overlay,
                track_click: TrackClick::Jump,
                shift_track_click_inverts: true,
                scroll_line: 40.0,
                bar_hold: ms(500),
                bar_fade: ms(800),
                button_order: ButtonOrder::Gnome,
                reduced_motion: false,
            },
        }
    }
}

/// The colours and metrics the views paint with until the theme tokens exist.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub surface: Color,
    pub surface_raised: Color,
    pub border: Color,
    pub text: Color,
    pub text_muted: Color,
    pub accent: Color,
    pub on_accent: Color,
    pub hover: Color,
    pub selection: Color,
    pub scrollbar_thumb: Color,
    pub backdrop: Color,
    pub shadow: Color,
    pub radius: f32,
    pub font_size: f32,
    /// A list, menu or tree row's height.
    pub row_height: f32,
    pub scrollbar_width: f32,
}

impl Default for Palette {
    fn default() -> Self {
        Palette {
            surface: Color::from_rgba8(0x1e, 0x1e, 0x1e, 0xff),
            surface_raised: Color::from_rgba8(0x25, 0x25, 0x26, 0xff),
            border: Color::from_rgba8(0x45, 0x45, 0x45, 0xff),
            text: Color::from_rgba8(0xcc, 0xcc, 0xcc, 0xff),
            text_muted: Color::from_rgba8(0x9d, 0x9d, 0x9d, 0xff),
            accent: Color::from_rgba8(0x00, 0x78, 0xd4, 0xff),
            on_accent: Color::WHITE,
            hover: Color::from_rgba8(0x2a, 0x2d, 0x2e, 0xff),
            selection: Color::from_rgba8(0x04, 0x39, 0x5e, 0xff),
            scrollbar_thumb: Color::from_rgba8(0x79, 0x79, 0x79, 0x66),
            backdrop: Color::rgba(0.0, 0.0, 0.0, 0.4),
            shadow: Color::rgba(0.0, 0.0, 0.0, 0.36),
            radius: 4.0,
            font_size: 13.0,
            row_height: 22.0,
            scrollbar_width: 10.0,
        }
    }
}

/// Label shaping and recording (presto-text's `TextSystem::layout` plus run registration).
pub trait Labels {
    /// The size of `text` at `font_size` laid out at `max_width` (`None`: one line).
    fn measure(&mut self, text: &str, font_size: f32, max_width: Option<f32>) -> Size;
    /// Records `text` with its line box's top left at `origin`, ellipsised at `max_width`.
    fn record(&mut self, list: &mut DisplayList, text: &str, origin: Point, font_size: f32, color: Color, max_width: Option<f32>);
}

/// What [`Kind::record`] writes with.
pub struct RecordCx<'a> {
    /// The node's border box size (local coordinates start at 0,0).
    pub size: Size,
    pub list: &'a mut DisplayList,
    pub palette: &'a Palette,
    pub labels: &'a mut dyn Labels,
    pub now: Time,
}

/// What [`Kind::event`] runs with and reports through.
pub struct EventCx<'a> {
    pub now: Time,
    /// The node's border box size.
    pub size: Size,
    pub policy: &'a Policy,
    pub modifiers: Modifiers,
    /// Signals to deliver to JS, in order.
    pub emitted: Vec<Emitted>,
    /// The most expensive work this event caused (`None`: nothing changed).
    pub changed: Option<ChangeClass>,
    /// Fast commits of native-owned properties (architecture §7.3).
    pub fast: Vec<FastCommit>,
    /// The earliest time the kind wants an [`Event::Timer`].
    pub wake: Option<Time>,
    pub cursor: Option<CursorIcon>,
    /// Keep the pointer until release (`Some(false)`: release it).
    pub capture: Option<bool>,
    /// Ask for keyboard focus.
    pub focus: bool,
}

impl<'a> EventCx<'a> {
    pub fn new(now: Time, size: Size, policy: &'a Policy) -> Self {
        EventCx {
            now,
            size,
            policy,
            modifiers: Modifiers::default(),
            emitted: Vec::new(),
            changed: None,
            fast: Vec::new(),
            wake: None,
            cursor: None,
            capture: None,
            focus: false,
        }
    }
    pub fn emit(&mut self, signal: &'static str, detail: Value) {
        self.emitted.push(Emitted { signal, detail });
    }
    /// Marks the work this event caused (the costliest class wins).
    pub fn mark(&mut self, c: ChangeClass) {
        self.changed = Some(self.changed.map_or(c, |o| o.max(c)));
    }
    pub fn wake_at(&mut self, t: Time) {
        self.wake = Some(self.wake.map_or(t, |w| w.min(t)));
    }
    pub fn emitted(&self, signal: &str) -> impl Iterator<Item = &Emitted> {
        self.emitted.iter().filter(move |e| e.signal == signal)
    }
}

/// A retained kind (architecture §4.2, the subset the views use).
pub trait Kind: 'static {
    fn spec() -> &'static KindSpec
    where
        Self: Sized;
    /// A prop write from JS or Rust; returns the work it causes.
    fn set_prop(&mut self, name: &str, value: &Value) -> Result<ChangeClass, PropError>;
    /// Writes the node's display list (node-local coordinates).
    fn record(&self, cx: &mut RecordCx<'_>);
    /// Native defaults for input reaching the node.
    fn event(&mut self, _cx: &mut EventCx<'_>, _ev: &Event) -> Handled {
        Handled::No
    }
    /// The node's AccessKit properties.
    fn a11y(&self, node: &mut accesskit::Node);
}

/// A test [`Labels`] with a fixed advance per char (`0.6 em`) and line height (`1.25 em`), recording
/// one `Quad` per label so goldens can see where text went.
#[cfg(test)]
#[derive(Default)]
pub struct FixedLabels;

#[cfg(test)]
impl Labels for FixedLabels {
    fn measure(&mut self, text: &str, font_size: f32, max_width: Option<f32>) -> Size {
        let w = text.chars().count() as f32 * font_size * 0.6;
        Size::new(max_width.map_or(w, |m| w.min(m)), font_size * 1.25)
    }
    fn record(&mut self, list: &mut DisplayList, text: &str, origin: Point, font_size: f32, color: Color, max_width: Option<f32>) {
        let s = self.measure(text, font_size, max_width);
        let rect = presto_base::geom::Rect::from_origin_size(origin, s);
        list.push(
            presto_base::display::Prim::Quad(presto_base::display::Quad {
                rect,
                radii: presto_base::geom::Corners::ZERO,
                fill: presto_base::color::Fill::Solid(color.with_alpha(0.0)),
                border_widths: presto_base::geom::Insets::ZERO,
                border_color: color,
                border_style: Default::default(),
            }),
            rect,
        );
    }
}
