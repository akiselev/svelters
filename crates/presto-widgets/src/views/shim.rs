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

/// Stand-in for `presto_anim::scroll` (architecture §12.2; constants and sources in
/// `plans/research/presto/notes/round2/animation-scroll.md` §4): wheel deltas to points per OS, the
/// Chromium wheel smooth-scroll curve with its velocity-preserving retarget, the exponential fling,
/// and the release-velocity estimate for an own fling. Pure functions of time, as the compositor
/// will sample them.
pub mod scroll_physics {
    use super::Os;
    use presto_base::event::ScrollDelta;
    use presto_base::geom::{Size, Vec2};
    use presto_base::time::{Span, Time};

    /// Points one wheel detent scrolls: Windows 3 lines × 100/3 px (Chromium
    /// `web_input_event_builders_win.cc`), Linux 120 (Chromium `MouseWheelEvent::kWheelDelta`),
    /// macOS 40 per Cocoa tick (`kScrollbarPixelsPerCocoaTick`, the OS already accelerated it).
    pub fn points_per_detent(os: Os) -> f32 {
        match os {
            Os::Windows => 100.0,
            Os::Linux => 120.0,
            Os::MacOs => 40.0,
        }
    }

    /// A page step along one axis of a `len`-point viewport: `max(0.875·len, len − 40)` (Chromium
    /// `cc/input/scroll_utils.h`).
    pub fn page_step(len: f32) -> f32 {
        (0.875 * len).max(len - 40.0).max(1.0)
    }

    /// The offset change (content points, positive towards the end) a wheel delta asks for. The
    /// event's positive `y` scrolls the content up (the wheel away from the user), so it is negated.
    pub fn wheel_delta_points(os: Os, delta: ScrollDelta, viewport: Size) -> Vec2 {
        let v = match delta {
            ScrollDelta::Lines(v) => v * points_per_detent(os),
            ScrollDelta::Pixels(v) => v,
            ScrollDelta::Pages(v) => Vec2::new(v.x * page_step(viewport.w), v.y * page_step(viewport.h)),
        };
        Vec2::new(-v.x, -v.y)
    }

    /// Chromium's `kInverseDelta` wheel duration: 200 ms for deltas up to 120 px, falling linearly to
    /// 100 ms at 480 px and more (`cc/animation/scroll_offset_animation_curve.cc`).
    pub fn wheel_duration(delta: f32) -> Span {
        let d = delta.abs().clamp(120.0, 480.0);
        let ms = 200.0 - (d - 120.0) / (480.0 - 120.0) * 100.0;
        Span::from_secs_f64(ms as f64 / 1000.0)
    }

    /// A unit cubic Bézier `(0,0) (x1,y1) (x2,y2) (1,1)` as a timing function.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Bezier {
        pub x1: f32,
        pub y1: f32,
        pub x2: f32,
        pub y2: f32,
    }

    impl Bezier {
        fn coord(a: f32, b: f32, s: f32) -> f32 {
            let u = 1.0 - s;
            3.0 * u * u * s * a + 3.0 * u * s * s * b + s * s * s
        }
        fn dcoord(a: f32, b: f32, s: f32) -> f32 {
            let u = 1.0 - s;
            3.0 * u * u * a + 6.0 * u * s * (b - a) + 3.0 * s * s * (1.0 - b)
        }
        /// The curve's parameter at time `x` (Newton, then bisection when the slope is flat).
        fn solve(&self, x: f32) -> f32 {
            let mut s = x;
            for _ in 0..8 {
                let e = Self::coord(self.x1, self.x2, s) - x;
                if e.abs() < 1e-6 {
                    return s;
                }
                let d = Self::dcoord(self.x1, self.x2, s);
                if d.abs() < 1e-6 {
                    break;
                }
                s = (s - e / d).clamp(0.0, 1.0);
            }
            let (mut lo, mut hi) = (0.0f32, 1.0f32);
            s = x;
            for _ in 0..32 {
                let v = Self::coord(self.x1, self.x2, s);
                if (v - x).abs() < 1e-6 {
                    break;
                }
                if v < x { lo = s } else { hi = s }
                s = (lo + hi) / 2.0;
            }
            s
        }
        /// The progress at time fraction `x` in 0..=1.
        pub fn eval(&self, x: f32) -> f32 {
            if x <= 0.0 {
                return 0.0;
            }
            if x >= 1.0 {
                return 1.0;
            }
            Self::coord(self.y1, self.y2, self.solve(x))
        }
        /// d(progress)/d(time fraction) at `x`.
        pub fn slope(&self, x: f32) -> f32 {
            let s = self.solve(x.clamp(0.0, 1.0));
            let dx = Self::dcoord(self.x1, self.x2, s);
            if dx.abs() < 1e-6 { 0.0 } else { Self::dcoord(self.y1, self.y2, s) / dx }
        }
    }

    /// Chromium's `EaseInOutWithInitialSlope`: ease-in-out (0.42, 0, 0.58, 1) whose first control
    /// point is raised so the curve starts at `slope` (progress per time fraction).
    pub fn ease_in_out_with_slope(slope: f32) -> Bezier {
        let slope = slope.clamp(-1000.0, 1000.0);
        Bezier { x1: 0.42, y1: 0.42 * slope, x2: 0.58, y2: 1.0 }
    }

    /// A smooth scroll from `from` to `to` (Chromium's `ScrollOffsetAnimationCurve`, wheel flavour).
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct SmoothScroll {
        pub from: Vec2,
        pub to: Vec2,
        pub start: Time,
        pub duration: Span,
        pub curve: Bezier,
    }

    impl SmoothScroll {
        /// A wheel step starting at rest.
        pub fn wheel(now: Time, from: Vec2, to: Vec2) -> Self {
            let d = (to - from).length();
            SmoothScroll { from, to, start: now, duration: wheel_duration(d), curve: ease_in_out_with_slope(0.0) }
        }
        fn t(&self, now: Time) -> f32 {
            let total = self.duration.as_secs_f64();
            if total <= 0.0 {
                return 1.0;
            }
            ((now - self.start).as_secs_f64() / total).clamp(0.0, 1.0) as f32
        }
        pub fn offset_at(&self, now: Time) -> Vec2 {
            let p = self.curve.eval(self.t(now));
            self.from + (self.to - self.from) * p
        }
        /// Points per second at `now`.
        pub fn velocity_at(&self, now: Time) -> Vec2 {
            let total = self.duration.as_secs_f64() as f32;
            if total <= 0.0 || self.done_at(now) {
                return Vec2::ZERO;
            }
            (self.to - self.from) * (self.curve.slope(self.t(now)) / total)
        }
        pub fn done_at(&self, now: Time) -> bool {
            now >= self.start + self.duration
        }
        /// Heads for `to` from where the curve is at `now`, keeping its velocity (Chromium
        /// `UpdateTarget`): the new segment's duration is the wheel duration for its delta, bounded by
        /// `2.5 × remaining / velocity` (`VelocityBasedDurationBound`), and its curve starts at the
        /// current velocity.
        pub fn retarget(&mut self, now: Time, to: Vec2) {
            let at = self.offset_at(now);
            let v = self.velocity_at(now);
            let delta = to - at;
            let dist = delta.length();
            let mut dur = wheel_duration(dist).as_secs_f64() as f32;
            // the velocity along the new direction
            let along = if dist > 0.0 { (v.x * delta.x + v.y * delta.y) / dist } else { 0.0 };
            if along > 1.0 {
                dur = dur.min(2.5 * dist / along);
            }
            let slope = if dist > 0.0 && dur > 0.0 { along * dur / dist } else { 0.0 };
            *self = SmoothScroll { from: at, to, start: now, duration: Span::from_secs_f64(dur as f64), curve: ease_in_out_with_slope(slope) };
        }
    }

    /// The iOS-style exponential fling `v(t) = v₀·d^ms`, `x(t) = x₀ + v₀·(d^ms − 1)/ln d`, with the
    /// travel capped at three viewports (Chromium's `kDefaultBoundsMultiplier`) and the end where the
    /// speed drops below 0.5 pt per 16 ms frame.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Fling {
        pub from: Vec2,
        pub start: Time,
        /// Points per second at the start.
        pub velocity: Vec2,
        /// Velocity kept per millisecond (0.998 normal, 0.99 fast: `UIScrollView.decelerationRate`).
        pub rate_per_ms: f32,
        /// The longest travel per axis.
        pub max_travel: Vec2,
    }

    /// Below this speed (points per second) a fling stops: 0.5 pt per 16 ms frame.
    pub const FLING_STOP_SPEED: f32 = 0.5 / 0.016;

    impl Fling {
        pub fn new(now: Time, from: Vec2, velocity: Vec2, viewport: Size) -> Self {
            Fling { from, start: now, velocity, rate_per_ms: 0.998, max_travel: Vec2::new(viewport.w * 3.0, viewport.h * 3.0) }
        }
        fn ms(&self, now: Time) -> f32 {
            ((now - self.start).as_secs_f64() * 1000.0).max(0.0) as f32
        }
        pub fn velocity_at(&self, now: Time) -> Vec2 {
            self.velocity * self.rate_per_ms.powf(self.ms(now))
        }
        pub fn offset_at(&self, now: Time) -> Vec2 {
            let ln = self.rate_per_ms.ln();
            // v in points per ms for the integral
            let k = (self.rate_per_ms.powf(self.ms(now)) - 1.0) / ln / 1000.0;
            let mut d = self.velocity * k;
            d.x = d.x.clamp(-self.max_travel.x, self.max_travel.x);
            d.y = d.y.clamp(-self.max_travel.y, self.max_travel.y);
            self.from + d
        }
        pub fn done_at(&self, now: Time) -> bool {
            self.velocity_at(now).length() < FLING_STOP_SPEED
        }
        /// A fling does not retarget: a new wheel step replaces it with a smooth scroll from where it
        /// is, keeping its velocity.
        pub fn to_smooth(&self, now: Time, to: Vec2) -> SmoothScroll {
            let at = self.offset_at(now);
            let mut s = SmoothScroll { from: at, to: at, start: now, duration: Span::ZERO, curve: ease_in_out_with_slope(0.0) };
            // seed the velocity through a zero-length segment's retarget
            let v = self.velocity_at(now);
            let delta = to - at;
            let dist = delta.length();
            let mut dur = wheel_duration(dist).as_secs_f64() as f32;
            let along = if dist > 0.0 { (v.x * delta.x + v.y * delta.y) / dist } else { 0.0 };
            if along > 1.0 {
                dur = dur.min(2.5 * dist / along);
            }
            let slope = if dist > 0.0 && dur > 0.0 { along * dur / dist } else { 0.0 };
            s.to = to;
            s.duration = Span::from_secs_f64(dur as f64);
            s.curve = ease_in_out_with_slope(slope);
            s
        }
    }

    /// A running scroll animation (presto-anim's `ScrollAnimation`).
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum ScrollAnimation {
        Smooth(SmoothScroll),
        Fling(Fling),
    }

    impl ScrollAnimation {
        pub fn offset_at(&self, now: Time) -> Vec2 {
            match self {
                ScrollAnimation::Smooth(s) => s.offset_at(now),
                ScrollAnimation::Fling(f) => f.offset_at(now),
            }
        }
        pub fn done_at(&self, now: Time) -> bool {
            match self {
                ScrollAnimation::Smooth(s) => s.done_at(now),
                ScrollAnimation::Fling(f) => f.done_at(now),
            }
        }
        /// Heads for `to` keeping the velocity (a fling becomes a smooth scroll).
        pub fn retarget(&mut self, now: Time, to: Vec2) {
            match self {
                ScrollAnimation::Smooth(s) => s.retarget(now, to),
                ScrollAnimation::Fling(f) => *self = ScrollAnimation::Smooth(f.to_smooth(now, to)),
            }
        }
        /// Where it is heading (a fling's end where its speed runs out).
        pub fn target(&self) -> Vec2 {
            match self {
                ScrollAnimation::Smooth(s) => s.to,
                ScrollAnimation::Fling(f) => {
                    let ln = f.rate_per_ms.ln();
                    let mut d = f.velocity * (-1.0 / ln / 1000.0);
                    d.x = d.x.clamp(-f.max_travel.x, f.max_travel.x);
                    d.y = d.y.clamp(-f.max_travel.y, f.max_travel.y);
                    f.from + d
                }
            }
        }
    }

    /// The window of samples a release velocity is fitted over (GTK `SCROLL_CAPTURE_THRESHOLD_MS`).
    pub const VELOCITY_WINDOW: Span = Span::from_millis(150);
    /// No fling when the last sample is older than this at release (Chromium
    /// `kFlingStartTimeoutMs`).
    pub const FLING_START_TIMEOUT: Span = Span::from_millis(200);

    /// Touchpad deltas of one gesture, for the release velocity (least squares of the cumulative
    /// offset over the last [`VELOCITY_WINDOW`], as Chromium's Wayland `RegressScrollVelocity`).
    #[derive(Clone, Debug, Default)]
    pub struct VelocityTracker {
        samples: Vec<(Time, Vec2)>,
        total: Vec2,
    }

    impl VelocityTracker {
        pub fn reset(&mut self) {
            self.samples.clear();
            self.total = Vec2::ZERO;
        }
        /// Adds a delta (content points) taken at `t`.
        pub fn push(&mut self, t: Time, delta: Vec2) {
            self.total = self.total + delta;
            self.samples.push((t, self.total));
            let keep_from = t - VELOCITY_WINDOW;
            self.samples.retain(|(s, _)| *s >= keep_from);
        }
        /// Points per second at release time `now`; zero when the last sample is too old or there
        /// are fewer than two samples.
        pub fn velocity(&self, now: Time) -> Vec2 {
            let Some(&(last, _)) = self.samples.last() else { return Vec2::ZERO };
            if now - last > FLING_START_TIMEOUT || self.samples.len() < 2 {
                return Vec2::ZERO;
            }
            let t0 = self.samples[0].0;
            let n = self.samples.len() as f64;
            let (mut st, mut stt) = (0.0f64, 0.0f64);
            let (mut sx, mut sy, mut stx, mut sty) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
            for (t, p) in &self.samples {
                let t = (*t - t0).as_secs_f64();
                st += t;
                stt += t * t;
                sx += p.x as f64;
                sy += p.y as f64;
                stx += t * p.x as f64;
                sty += t * p.y as f64;
            }
            let den = n * stt - st * st;
            if den.abs() < 1e-12 {
                return Vec2::ZERO;
            }
            Vec2::new(((n * stx - st * sx) / den) as f32, ((n * sty - st * sy) / den) as f32)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn wheel_points_per_os_and_sign() {
            let v = Size::new(400.0, 600.0);
            assert_eq!(wheel_delta_points(Os::Windows, ScrollDelta::Lines(Vec2::new(0.0, 1.0)), v), Vec2::new(-0.0, -100.0));
            assert_eq!(wheel_delta_points(Os::Linux, ScrollDelta::Lines(Vec2::new(0.0, -1.0)), v), Vec2::new(-0.0, 120.0));
            assert_eq!(wheel_delta_points(Os::MacOs, ScrollDelta::Pixels(Vec2::new(3.0, -7.0)), v), Vec2::new(-3.0, 7.0));
            assert_eq!(wheel_delta_points(Os::Windows, ScrollDelta::Pages(Vec2::new(0.0, -1.0)), v).y, 560.0);
        }

        #[test]
        fn wheel_duration_falls_with_the_delta() {
            assert_eq!(wheel_duration(100.0), Span::from_millis(200));
            assert_eq!(wheel_duration(1000.0), Span::from_millis(100));
            let mid = wheel_duration(300.0).as_millis_f64();
            assert!((mid - 150.0).abs() < 0.01, "{mid}");
        }

        #[test]
        fn smooth_scroll_ends_at_its_target_and_retargets_with_velocity() {
            let t0 = Time(1_000_000_000);
            let mut s = SmoothScroll::wheel(t0, Vec2::ZERO, Vec2::new(0.0, 100.0));
            assert_eq!(s.offset_at(t0), Vec2::ZERO);
            assert!(s.done_at(t0 + Span::from_millis(200)));
            assert_eq!(s.offset_at(t0 + Span::from_millis(200)), Vec2::new(0.0, 100.0));
            let mid = t0 + Span::from_millis(100);
            let (at, v) = (s.offset_at(mid), s.velocity_at(mid));
            assert!(v.y > 0.0);
            s.retarget(mid, Vec2::new(0.0, 200.0));
            assert_eq!(s.offset_at(mid), at, "continuous position");
            let v2 = s.velocity_at(mid + Span::from_micros(1));
            assert!((v2.y - v.y).abs() / v.y < 0.05, "continuous velocity: {v:?} → {v2:?}");
            assert_eq!(s.offset_at(mid + s.duration), Vec2::new(0.0, 200.0));
        }

        #[test]
        fn fling_decays_and_stops() {
            let t0 = Time(0);
            let f = Fling::new(t0, Vec2::ZERO, Vec2::new(0.0, 1000.0), Size::new(100.0, 1000.0));
            // travel to the end is v0·0.4995 s for 0.998 per ms
            let end = ScrollAnimation::Fling(f).target();
            assert!((end.y - 499.5).abs() < 1.0, "{end:?}");
            assert!(!f.done_at(t0 + Span::from_millis(100)));
            assert!(f.done_at(t0 + Span::from_millis(5000)));
            let a = f.offset_at(t0 + Span::from_millis(100)).y;
            let b = f.offset_at(t0 + Span::from_millis(200)).y;
            assert!(a > 0.0 && b > a && b - a < a, "decelerates: {a} {b}");
        }

        #[test]
        fn velocity_is_a_regression_over_recent_samples() {
            let mut v = VelocityTracker::default();
            for i in 0..10 {
                v.push(Time(i * 10_000_000), Vec2::new(0.0, 5.0));
            }
            let now = Time(90_000_000);
            assert!((v.velocity(now).y - 500.0).abs() < 1.0, "{:?}", v.velocity(now));
            assert_eq!(v.velocity(now + Span::from_millis(250)), Vec2::ZERO, "released too late");
        }
    }
}
