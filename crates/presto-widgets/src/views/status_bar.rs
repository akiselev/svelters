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

use super::draw;
use super::legacy::{Event, EventCx, Handled, Kind, KindSpec, PropError, PropType, RecordCx, Value, prop, signal};
use presto_base::accesskit;
use presto_base::change::ChangeClass;
use presto_base::geom::{Point, Rect};
use presto_base::time::{Span, Time};
use std::sync::Arc;

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

/// The `status-bar` kind.
#[derive(Clone, Debug, Default)]
pub struct StatusBar {
    pub message: String,
    /// When the message times out.
    until: Option<Time>,
    /// The status tip shown this frame (from [`StatusTips::shown`]).
    tip: Option<Arc<str>>,
    /// The message area (local x range) between the last start and the first end section.
    area: (f32, f32),
}

static STATUS_BAR_SPEC: KindSpec = KindSpec {
    name: "status-bar",
    props: &[prop("message", PropType::Str, ChangeClass::Paint)],
    signals: &[signal("changed", true, false)],
    slots: &[""],
    role: accesskit::Role::Status,
};

impl StatusBar {
    /// `show-message(text, timeout)`: the message until changed, or for `timeout` (`None` or zero:
    /// no timeout). Announced with `changed`, as a method call is (a prop write is not).
    pub fn show_message(&mut self, cx: &mut EventCx<'_>, text: &str, timeout: Option<Span>) {
        let changed = text != self.message;
        self.message = text.to_string();
        self.until = timeout.filter(|t| *t > Span::ZERO && !self.message.is_empty()).map(|t| cx.now + t);
        if let Some(t) = self.until {
            cx.wake_at(t);
        }
        if changed {
            cx.mark(ChangeClass::Paint);
            cx.emit("changed", Value::Str(self.message.clone()));
        }
    }

    /// `clear-message()`.
    pub fn clear_message(&mut self, cx: &mut EventCx<'_>) {
        self.show_message(cx, "", None);
    }

    /// The window's status tip this frame.
    pub fn set_tip(&mut self, tip: Option<Arc<str>>) -> Option<ChangeClass> {
        if tip == self.tip {
            return None;
        }
        self.tip = tip;
        Some(ChangeClass::Layout)
    }

    /// What the bar shows: the tip, else the message.
    pub fn text(&self) -> Option<&str> {
        self.tip.as_deref().or((!self.message.is_empty()).then_some(self.message.as_str()))
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
    fn spec() -> &'static KindSpec {
        &STATUS_BAR_SPEC
    }

    fn set_prop(&mut self, name: &str, value: &Value) -> Result<ChangeClass, PropError> {
        match name {
            "message" => {
                self.message = match value {
                    Value::Null => String::new(),
                    v => v.as_str().ok_or(PropError::Invalid("a string"))?.to_string(),
                };
                self.until = None;
                Ok(ChangeClass::Layout)
            }
            _ => Err(PropError::Unknown),
        }
    }

    fn record(&self, cx: &mut RecordCx<'_>) {
        let pal = cx.palette;
        let size = cx.size;
        draw::fill(cx.list, Rect::from_origin_size(Point::ZERO, size), 0.0, pal.surface_raised);
        draw::line(cx.list, Point::new(0.0, 0.5), Point::new(size.w, 0.5), 1.0, pal.border);
        if let Some(t) = self.text() {
            let pad = 4.0;
            let (l, r) = (self.area.0 + pad, if self.area.1 > self.area.0 { self.area.1 } else { size.w });
            let h = cx.labels.measure(t, pal.font_size, None).h;
            let width = (r - l).max(0.0);
            cx.labels.record(cx.list, t, Point::new(l, (size.h - h) / 2.0), pal.font_size, pal.text, Some(width));
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, ev: &Event) -> Handled {
        if let Event::Timer = ev
            && self.until.is_some_and(|u| cx.now >= u)
        {
            self.until = None;
            self.message.clear();
            cx.mark(ChangeClass::Layout);
            cx.emit("changed", Value::Str(String::new()));
            return Handled::Yes;
        }
        Handled::No
    }

    fn a11y(&self, node: &mut accesskit::Node) {
        // a polite live status region carrying the message
        node.set_role(accesskit::Role::Status);
        node.set_live(accesskit::Live::Polite);
        if let Some(t) = self.text() {
            node.set_label(t);
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
    use super::super::legacy::{Os, Policy};
    use super::*;
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
        let pol = Policy::for_os(Os::Linux);
        let mut bar = StatusBar::default();
        let mut cx = EventCx::new(Time(0), Size::ZERO, &pol);
        bar.show_message(&mut cx, "Saved", Some(Span::from_millis(2000)));
        assert_eq!(bar.text(), Some("Saved"));
        assert_eq!(cx.wake, Some(Time(0) + Span::from_millis(2000)));
        assert_eq!(cx.emitted("changed").count(), 1);
        let mut cx = EventCx::new(Time(0) + Span::from_millis(1000), Size::ZERO, &pol);
        assert_eq!(bar.event(&mut cx, &Event::Timer), Handled::No);
        let mut cx = EventCx::new(Time(0) + Span::from_millis(2000), Size::ZERO, &pol);
        assert_eq!(bar.event(&mut cx, &Event::Timer), Handled::Yes);
        assert_eq!(bar.text(), None);
        assert_eq!(cx.emitted("changed").next().map(|e| e.detail.clone()), Some(Value::Str(String::new())));
    }

    #[test]
    fn a_tip_wins_over_the_message() {
        let mut tips = StatusTips::default();
        let mut bar = StatusBar { message: "Ready".into(), ..Default::default() };
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
