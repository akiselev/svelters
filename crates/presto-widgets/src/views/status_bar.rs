//! `status-bar` (ported from svelters-egui `views/status_bar.rs`; QStatusBar): a row whose children
//! are sections placed by their `status-section`:
//!
//! - `start`: permanent, first;
//! - `temporary` (the default): after the start sections, hidden while a message shows (Qt's
//!   `addWidget`);
//! - `end`: permanent, pushed to the far end (Qt's `addPermanentWidget`).
//!
//! Only layout sees the section order ([`order_sections`]); the first end section gets
//! `margin-left: auto`, so the free space between the start and end sections is the message area.
//! The message is `message`, `show-message(text, timeout)` and `clear-message`. A status tip wins over
//! it: the `status-tip` of the innermost hovered node, else of the focused one, gathered per frame by
//! [`StatusTips`] and shown by every status bar of the window.

use super::common;
use crate::interact::StateBits;
use crate::kind_props;
use crate::paint::{self, Align, PaintCx, Ua, UaRule, UaValue::*};
use crate::shim::core::{A11yCx, Event, EventCx, Handled, Kind, KindSpec, NameFrom, PropError, PropId, RecordCx, SignalSpec, Value, ValueType};
use presto_base::accesskit;
use presto_base::geom::Rect;
use presto_base::time::{Span, Time};
use std::sync::Arc;

/// The bar's UA sheet: the panel surface with a hairline on top, the theme's status-bar height and
/// small text (VS Code's status bar); `:open` while a message or tip shows.
pub const UA: Ua = Ua(&[UaRule::base(&[
    ("background-color", Token("panel-bg")),
    ("border-top-width", Token("stroke")),
    ("border-top-color", Token("border")),
    ("padding-left", Px(4.0)),
    ("padding-right", Px(4.0)),
    ("font-size", Token("font-size-sm")),
    ("color", Token("fg")),
])]);

/// `::part(message)`'s UA sheet: the message text, cut at the end.
pub const MESSAGE_UA: Ua = Ua(&[UaRule::base(&[("text-overflow", Keyword("ellipsis"))])]);

/// A section's place in the bar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Section {
    Start = 0,
    #[default]
    Temporary = 1,
    End = 2,
}

impl Section {
    pub fn parse(s: &str) -> Section {
        match s {
            "start" => Section::Start,
            "end" => Section::End,
            _ => Section::Temporary,
        }
    }
}

/// The bar's layout order (indices into `sections`: start, temporary, end, stable within each) and
/// the index of the section that gets `margin-left: auto` (the first end one).
pub fn order_sections(sections: &[Section]) -> (Vec<usize>, Option<usize>) {
    let mut idx: Vec<usize> = (0..sections.len()).collect();
    idx.sort_by_key(|i| sections[*i]); // stable
    let pushed = idx.iter().copied().find(|i| sections[*i] == Section::End);
    (idx, pushed)
}

kind_props! {
    /// The bar's props.
    pub struct StatusBarProps;
    /// The message (until changed; `show-message` gives it a timeout).
    message "message": String = Str("") ; Str, Layout, bindable;
}

/// The signals, in spec order.
pub const SIGNALS: &[SignalSpec] = &[SignalSpec::new("changed", &[("message", ValueType::Str)], "The message changed by `show-message`, `clear-message` or its timeout.")];

pub static SPEC: KindSpec = KindSpec {
    tag: "status-bar",
    props: StatusBarProps::SPECS,
    signals: SIGNALS,
    slots: &[],
    children: true,
    role: accesskit::Role::Status,
    name_from: NameFrom::Prop("message"),
    text_prop: None,
    focusable: false,
};

/// The `status-bar` kind.
#[derive(Clone, Debug, Default)]
pub struct StatusBar {
    pub props: StatusBarProps,
    /// When the message times out.
    until: Option<Time>,
    /// The status tip shown this frame (from [`StatusTips::shown`]).
    tip: Option<Arc<str>>,
    /// The message area (local x range) between the last start and the first end section.
    area: (f32, f32),
}

impl StatusBar {
    /// `show-message(text, timeout)`: the message until changed, or for `timeout` (`None` or zero:
    /// no timeout). Announced with `changed`, as a method call is (a prop write is not).
    pub fn show_message(&mut self, cx: &mut EventCx<'_>, text: &str, timeout: Option<Span>) {
        let changed = text != self.props.message;
        self.props.message = text.to_string();
        self.until = timeout.filter(|t| *t > Span::ZERO && !self.props.message.is_empty()).map(|t| cx.now + t);
        if let Some(t) = self.until {
            cx.wake_at(t);
        }
        if changed {
            cx.prop_changed(P::message.id());
            common::emit(cx, &SPEC, "changed", vec![Value::str(&self.props.message)]);
        }
    }

    /// `clear-message()`.
    pub fn clear_message(&mut self, cx: &mut EventCx<'_>) {
        self.show_message(cx, "", None);
    }

    /// The window's status tip this frame.
    pub fn set_tip(&mut self, tip: Option<Arc<str>>) -> bool {
        if tip == self.tip {
            return false;
        }
        self.tip = tip;
        true
    }

    /// What the bar shows: the tip, else the message.
    pub fn text(&self) -> Option<&str> {
        self.tip.as_deref().or((!self.props.message.is_empty()).then_some(self.props.message.as_str()))
    }

    /// The temporary sections are hidden (a message or a tip shows): `:open` on the bar.
    pub fn showing(&self) -> bool {
        self.text().is_some()
    }

    /// The layout's message area: from the right edge of the last start section to the left edge of
    /// the first end one (local x).
    pub fn set_message_area(&mut self, left: f32, right: f32) {
        self.area = (left, right.max(left));
    }
}

impl Kind for StatusBar {
    fn parts() -> &'static [&'static str] {
        &["message"]
    }
    fn spec() -> &'static KindSpec {
        &SPEC
    }
    fn kind_spec(&self) -> &'static KindSpec {
        &SPEC
    }
    fn set_prop(&mut self, prop: PropId, v: &Value) -> Result<(), PropError> {
        self.props.set(prop, v)?;
        if prop == P::message.id() {
            self.until = None;
        }
        Ok(())
    }
    fn get_prop(&self, prop: PropId) -> Option<Value> {
        self.props.get(prop)
    }

    fn record(&self, cx: &mut RecordCx) {
        let mut style = cx.style.state;
        style.set(StateBits::OPEN, self.showing());
        let host = crate::paint::StyleView::new(cx.style.computed, UA, style, cx.style.dark);
        let msg_v = host.part(cx.style.part_css("message"), MESSAGE_UA);
        let rect = cx.rect();
        let mut pc = PaintCx::of(cx);
        let geom = paint::paint_box(&mut pc, &host, rect);
        if let Some(t) = self.text() {
            let (l, r) = if self.area.1 > self.area.0 { (geom.content.min.x.max(self.area.0), self.area.1.min(geom.content.max.x)) } else { (geom.content.min.x, geom.content.max.x) };
            let width = (r - l).max(0.0);
            let shown = paint::elide(pc.text, &msg_v, t, paint::Elide::of_style(&msg_v).unwrap_or(paint::Elide::End), width);
            let layout = paint::text_layout(pc.text, &msg_v, &shown, None);
            let area = Rect::new(l, geom.content.min.y, r, geom.content.max.y);
            paint::paint_text(&mut pc, &msg_v, &layout, area, Align::Start);
        }
    }

    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        cx.state.set(StateBits::OPEN, self.showing());
        if let Event::Timer = ev
            && self.until.is_some_and(|u| cx.now >= u)
        {
            self.until = None;
            self.props.message.clear();
            cx.prop_changed(P::message.id());
            cx.state.set(StateBits::OPEN, self.showing());
            common::emit(cx, &SPEC, "changed", vec![Value::str("")]);
            return Handled::Yes;
        }
        Handled::No
    }

    fn a11y(&self, cx: &mut A11yCx) {
        // a polite live status region carrying the message
        cx.node.set_role(accesskit::Role::Status);
        cx.node.set_live(accesskit::Live::Polite);
        if let Some(t) = self.text() {
            cx.node.set_label(t);
        }
    }
}

/// The status tips of one window: during a frame's dispatch the innermost hovered node's tip and the
/// focused node's are offered; [`StatusTips::end_frame`] picks the one the status bars show.
#[derive(Clone, Debug, Default)]
pub struct StatusTips {
    hovered: Option<(usize, Arc<str>)>,
    focused: Option<Arc<str>>,
    shown: Option<Arc<str>>,
}

impl StatusTips {
    /// A hovered node at tree depth `depth` with a `status-tip` (the deepest wins).
    pub fn offer_hovered(&mut self, depth: usize, tip: Arc<str>) {
        if self.hovered.as_ref().is_none_or(|(d, _)| depth >= *d) {
            self.hovered = Some((depth, tip));
        }
    }
    /// The focused node's `status-tip`.
    pub fn offer_focused(&mut self, tip: Arc<str>) {
        self.focused = Some(tip);
    }
    /// Resolves the frame's tip; returns whether it changed (the bars then take it with
    /// [`StatusBar::set_tip`]).
    pub fn end_frame(&mut self) -> bool {
        let now = self.hovered.take().map(|(_, t)| t).or_else(|| self.focused.take());
        self.focused = None;
        let changed = now != self.shown;
        self.shown = now;
        changed
    }
    /// The tip of the last complete frame (cadatomic's own status bar reads it here).
    pub fn shown(&self) -> Option<Arc<str>> {
        self.shown.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::testkit::{self, Driver};
    use presto_base::geom::Size;

    #[test]
    fn sections_order_and_the_pushed_end() {
        use Section::*;
        let (order, pushed) = order_sections(&[End, Temporary, Start, End, Start]);
        assert_eq!(order, vec![2, 4, 1, 0, 3]);
        assert_eq!(pushed, Some(0));
        assert_eq!(order_sections(&[Temporary]).1, None);
    }

    #[test]
    fn a_message_times_out() {
        let mut d = Driver::new(StatusBar::default(), Size::new(400.0, 22.0));
        let t0 = d.now;
        {
            let policy = d.policy;
            let mut state = StateBits::NONE;
            let mut cx = EventCx::new(t0, d.size, &mut state, &policy);
            d.kind.show_message(&mut cx, "Saved", Some(Span::from_millis(2000)));
            assert_eq!(cx.wake, Some(t0 + Span::from_millis(2000)));
            assert_eq!(cx.emitted.len(), 1);
            assert_eq!(cx.changed, vec![P::message.id()]);
        }
        assert_eq!(d.kind.text(), Some("Saved"));
        assert_eq!(d.get("message"), Some(Value::str("Saved")));
        d.advance(1000);
        assert_eq!(d.send(Event::Timer), Handled::No);
        d.advance(1000);
        assert_eq!(d.send(Event::Timer), Handled::Yes);
        assert_eq!(d.kind.text(), None);
        assert_eq!(d.signals.last().cloned(), Some(("changed", vec![Value::str("")])));
        assert_eq!(d.changed, ["message"]);
        assert!(!d.state.contains(StateBits::OPEN));
    }

    #[test]
    fn records_surface_hairline_and_message() {
        let mut bar = StatusBar::default();
        bar.props.message = "Ready".into();
        let (list, res) = testkit::record(&bar, Size::new(400.0, 22.0), StateBits::NONE);
        assert_eq!(testkit::kinds(&list), ["quad", "quad", "glyphs"], "surface, top hairline, message");
        assert_eq!(res.runs.len(), 1);
        let n = testkit::a11y(&bar, StateBits::NONE, Size::new(400.0, 22.0));
        assert_eq!(n.role(), accesskit::Role::Status);
        assert_eq!(n.label(), Some("Ready"));
    }

    #[test]
    fn a_tip_wins_over_the_message() {
        let mut tips = StatusTips::default();
        let mut bar = StatusBar::default();
        bar.props.message = "Ready".into();
        tips.offer_focused("Focused field".into());
        tips.offer_hovered(1, "Outer".into());
        tips.offer_hovered(3, "Inner button".into());
        assert!(tips.end_frame());
        bar.set_tip(tips.shown());
        assert_eq!(bar.text(), Some("Inner button"));
        // nothing hovered: the focused node's tip
        tips.offer_focused("Focused field".into());
        assert!(tips.end_frame());
        bar.set_tip(tips.shown());
        assert_eq!(bar.text(), Some("Focused field"));
        // neither: the message again
        assert!(tips.end_frame());
        bar.set_tip(tips.shown());
        assert_eq!(bar.text(), Some("Ready"));
        assert!(!tips.end_frame(), "no change");
    }
}
