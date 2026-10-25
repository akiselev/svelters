//! `hyperlink` (ARCH-WIDGETS §6.1, ported from svelters-egui `widgets/hyperlink.rs`): text;
//! Hoverable, Focusable, Pressable.
//!
//! Look (DESIGN R18, R10): the `link` colour, underlined while hovered, the pointer cursor, and the
//! keyboard-only focus ring (R8). A click sends `clicked` (cancelable) and then, with a `url`,
//! `open-url` with it: the host opens it (presto never opens anything itself), in a new tab or
//! window with `new-tab` or a Ctrl/Cmd-click.

use crate::controls::common::{self, Units};
use crate::interact::{Focusable, Hoverable, Intents, Press, PressParams, Pressable, Sense, StateBits, Unit};
use crate::kind_props;
use crate::paint::{self, Align, PaintCx, StyleView, Ua, UaRule, UaValue::*};
use crate::shim::core::{A11yCx, Event, EventCx, Handled, Kind, KindSpec, MeasureCx, NameFrom, PropError, PropId, RecordCx, SignalSpec, Value, ValueType};
use presto_base::accesskit;
use presto_base::color::Color;
use presto_base::cursor::CursorIcon;
use presto_base::display::{Line, LineCap, Prim};
use presto_base::event::MouseButton;
use presto_base::geom::{Point, Rect};
use presto_base::key::Key;

pub const UA: Ua = Ua(&[
    UaRule::base(&[
        ("color", Token("link")),
        ("outline-style", Keyword("none")),
        ("outline-color", Token("focus")),
        ("outline-width", Px(1.0)),
        ("outline-offset", Px(1.0)),
        ("text-decoration-line", Keyword("none")),
    ]),
    UaRule::when(StateBits::HOVERED, &[("text-decoration-line", Keyword("underline"))]),
    UaRule::when(StateBits::FOCUS_VISIBLE, &[("outline-style", Keyword("solid"))]),
    UaRule::when(StateBits::DISABLED, &[("color", Token("disabled-fg"))]),
]);

kind_props! {
    /// The hyperlink's props.
    pub struct HyperlinkProps;
    /// The shown text (the `url` when empty).
    text "text": String = Str("") ; Str, Layout;
    url "url": String = Str("") ; Str, Layout;
    /// Opens in a new tab or window where the host has them.
    new_tab "new-tab": bool = Bool(false) ; Bool, Property;
    disabled "disabled": bool = Bool(false) ; Bool, Paint;
}

/// The signals, in spec order.
pub const SIGNALS: &[SignalSpec] = &[
    SignalSpec::new("clicked", &[], "Activated (click, Space, Enter, AccessKit `Click`); preventing it keeps `open-url` from following.").cancelable(),
    SignalSpec::new("secondary-clicked", &[], ""),
    SignalSpec::new("open-url", &[("url", ValueType::Str), ("new-tab", ValueType::Bool)], "The default of `clicked` with a `url`: the host opens `url`."),
];

pub static SPEC: KindSpec = KindSpec { tag: "hyperlink", props: HyperlinkProps::SPECS, signals: SIGNALS, slots: &[], children: false, role: accesskit::Role::Link, name_from: NameFrom::Prop("text"), text_prop: Some("text"), focusable: true };

/// Paints the link's `text` at `rect` under `s`: the text at the start, its underline while
/// `text-decoration-line` says so (1 point at the bottom of the text), the focus ring last.
pub fn paint(pc: &mut PaintCx, s: &StyleView, text: &str, rect: Rect) {
    let l = paint::text_layout(pc.text, s, text, None);
    let drawn = paint::paint_text(pc, s, &l, rect, Align::Start);
    if s.keyword("text-decoration-line") == Some("underline") {
        let color = s.color("color").unwrap_or(Color::BLACK);
        let w = pc.snap_width(1.0);
        let y = drawn.max.y - 1.0;
        let (x0, x1) = (rect.min.x, rect.min.x + l.size().w);
        let line = Line { from: Point::new(x0, y), to: Point::new(x1, y), width: w, color, cap: LineCap::Butt, dash: None, wavy: false };
        pc.push(Prim::Line(line), Rect::new(x0, y - w / 2.0, x1, y + w / 2.0));
    }
    paint::paint_outline(pc, s, rect);
}

/// A clickable link.
#[derive(Debug, Default)]
pub struct Hyperlink {
    pub props: HyperlinkProps,
    units: Units,
    hover: Hoverable,
    focus: Focusable,
    press: Pressable,
}

impl Hyperlink {
    fn shown(&self) -> &str {
        if self.props.text.is_empty() { &self.props.url } else { &self.props.text }
    }

    fn view<'a>(&self, s: &crate::shim::core::Styles<'a>) -> StyleView<'a> {
        let mut state = s.state;
        if self.props.disabled {
            state |= StateBits::DISABLED;
        }
        StyleView::new(s.computed, UA, state, s.dark)
    }
}

impl Kind for Hyperlink {
    fn spec() -> &'static KindSpec {
        &SPEC
    }
    fn kind_spec(&self) -> &'static KindSpec {
        &SPEC
    }
    fn set_prop(&mut self, prop: PropId, v: &Value) -> Result<(), PropError> {
        self.props.set(prop, v)
    }
    fn get_prop(&self, prop: PropId) -> Option<Value> {
        self.props.get(prop)
    }

    fn measure(&self, cx: &mut MeasureCx, _known: taffy::Size<Option<f32>>, _avail: taffy::Size<taffy::AvailableSpace>) -> taffy::Size<f32> {
        let s = paint::measure_text(cx.text, &self.view(&cx.style), self.shown(), None);
        taffy::Size { width: s.w, height: s.h }
    }

    fn record(&self, cx: &mut RecordCx) {
        let s = self.view(&cx.style);
        let rect = cx.rect();
        paint(&mut PaintCx::of(cx), &s, self.shown(), rect);
    }

    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        cx.state.set(StateBits::DISABLED, self.props.disabled);
        cx.enabled = !self.props.disabled;
        let params = PressParams::default();
        let sense = Sense::HOVER.union(self.focus.sense(&Default::default())).union(self.press.sense(&params));
        let mut fired = Intents::default();
        let (hover, focus, press) = (&mut self.hover, &mut self.focus, &mut self.press);
        let pass = self.units.run(cx, ev, sense, |icx| {
            hover.input(&(), icx, &mut Intents::default());
            focus.input(&Default::default(), icx, &mut Intents::default());
            press.input(&params, icx, &mut fired);
        });
        for p in fired.iter() {
            match p {
                Press::Click { count: 1, button: MouseButton::Primary, mods } => {
                    // EV1: `clicked` first; `open-url` is its default, skipped when a listener
                    // prevented it
                    let prevented = common::emit_sync(cx, &SPEC, "clicked", Vec::new());
                    if !prevented && !self.props.url.is_empty() {
                        let new_tab = self.props.new_tab || mods.command;
                        common::emit(cx, &SPEC, "open-url", vec![Value::str(&self.props.url), Value::Bool(new_tab)]);
                    }
                }
                Press::Click { button: MouseButton::Secondary, .. } => common::emit(cx, &SPEC, "secondary-clicked", Vec::new()),
                _ => {}
            }
        }
        if cx.state.contains(StateBits::HOVERED) && cx.enabled {
            cx.cursor = Some(CursorIcon::Pointer);
        }
        if pass.resp.hovered || pass.resp.down_on || !fired.is_empty() { Handled::Yes } else { Handled::No }
    }

    fn a11y(&self, cx: &mut A11yCx) {
        cx.node.set_role(accesskit::Role::Link);
        let text = self.shown();
        if !text.is_empty() {
            cx.node.set_label(text);
        }
        if !self.props.url.is_empty() {
            cx.node.set_url(self.props.url.as_str());
        }
        if self.props.disabled {
            cx.node.set_disabled();
        }
        self.focus.a11y(&Default::default(), cx.node);
        self.press.a11y(&PressParams::default(), cx.node);
    }

    fn claims_key(&self, key: Key, m: presto_base::event::Modifiers) -> bool {
        let m = crate::interact::Mods::of(m, false);
        self.press.claims(&PressParams::default(), key, m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::testkit::{self, Driver};
    use crate::shim::testing::MonoShaper;
    use presto_base::change::ChangeClass;
    use presto_base::event::{Modifiers, PointerEvent, PointerPhase, PointerType};
    use presto_base::geom::{Size, Vec2};
    use smallvec::SmallVec;

    fn link(url: &str) -> Hyperlink {
        let mut h = Hyperlink::default();
        h.props.url = url.into();
        h
    }

    #[test]
    fn spec() {
        let s = Hyperlink::spec();
        assert_eq!((s.tag, s.role, s.focusable), ("hyperlink", accesskit::Role::Link, true));
        assert_eq!(s.prop_spec(s.prop("url").unwrap()).unwrap().change, ChangeClass::Layout);
        assert_eq!(s.prop_spec(s.prop("new-tab").unwrap()).unwrap().change, ChangeClass::Property);
        assert!(s.signals[s.signal("clicked").unwrap().0 as usize].cancelable);
        assert_eq!(s.signals[s.signal("open-url").unwrap().0 as usize].args.len(), 2);
    }

    #[test]
    fn shows_the_url_without_text() {
        let h = link("https://a.b");
        let adv = MonoShaper::advance(StyleView::ua_only(UA, StateBits::NONE, true).font_size());
        assert_eq!(testkit::measure(&h, StateBits::NONE).w, 11.0 * adv);
    }

    #[test]
    fn underlined_while_hovered_ring_on_keyboard_focus() {
        let mut h = link("https://a.b");
        h.props.text = "Docs".into();
        let size = Size::new(80.0, 20.0);
        let (list, _) = testkit::record(&h, size, StateBits::NONE);
        assert_eq!(testkit::kinds(&list), ["glyphs"]);
        let Prim::Glyphs(g) = &list.items[0] else { unreachable!() };
        assert_eq!(g.color, paint::ua_token(true, "link").unwrap());
        let (list, _) = testkit::record(&h, size, StateBits::HOVERED);
        assert_eq!(testkit::kinds(&list), ["glyphs", "line"]);
        let Prim::Line(l) = &list.items[1] else { unreachable!() };
        let adv = MonoShaper::advance(StyleView::ua_only(UA, StateBits::NONE, true).font_size());
        assert_eq!((l.from.x, l.to.x), (0.0, 4.0 * adv), "under the text");
        assert_eq!(l.from.y, l.to.y);
        let (list, _) = testkit::record(&h, size, StateBits::FOCUSED | StateBits::FOCUS_VISIBLE);
        assert_eq!(testkit::kinds(&list), ["glyphs", "quad"], "the ring last");
        let (list, _) = testkit::record(&h, size, StateBits::DISABLED);
        let Prim::Glyphs(g) = &list.items[0] else { unreachable!() };
        assert_eq!(g.color, paint::ua_token(true, "disabled-fg").unwrap());
    }

    #[test]
    fn click_asks_the_host_to_open_the_url() {
        let mut d = Driver::new(link("https://a.b"), Size::new(80.0, 20.0));
        d.click();
        assert_eq!(d.names(), ["clicked", "open-url"]);
        assert_eq!(d.signals[1].1, vec![Value::str("https://a.b"), Value::Bool(false)]);
        assert!(d.state.contains(StateBits::HOVERED));
        d.take();
        // no url: only `clicked`
        let mut d = Driver::new(Hyperlink::default(), Size::new(80.0, 20.0));
        d.click();
        assert_eq!(d.take(), ["clicked"]);
    }

    #[test]
    fn ctrl_click_opens_in_a_new_tab() {
        let mut d = Driver::new(link("https://a.b"), Size::new(80.0, 20.0));
        let ev = |phase, buttons| Event::Pointer(PointerEvent { phase, pointer: 0, kind: PointerType::Mouse, button: Some(MouseButton::Primary), buttons, pos: Point::new(5.0, 5.0), modifiers: Modifiers::CTRL, pressure: 0.0, tilt: Vec2::ZERO, history: SmallVec::new() });
        d.hover(true);
        d.send(ev(PointerPhase::Down, 1));
        d.advance(30);
        d.send(ev(PointerPhase::Up, 0));
        assert_eq!(d.signals.last().unwrap().1, vec![Value::str("https://a.b"), Value::Bool(true)]);
    }

    #[test]
    fn keyboard_secondary_and_disabled() {
        let mut d = Driver::new(link("u"), Size::new(80.0, 20.0));
        d.focus(true);
        assert!(d.state.contains(StateBits::FOCUS_VISIBLE));
        d.key(Key::Enter);
        assert_eq!(d.take(), ["clicked", "open-url"]);
        assert!(d.kind.claims_key(Key::Space, Default::default()));
        d.advance(1000);
        d.secondary_click(5.0, 5.0);
        assert_eq!(d.take(), ["secondary-clicked"]);
        d.advance(1000);
        d.set("disabled", Value::Bool(true));
        d.click();
        assert!(d.take().is_empty());
    }

    #[test]
    fn a11y_node() {
        let h = link("https://a.b");
        let n = testkit::a11y(&h, StateBits::NONE, Size::new(80.0, 20.0));
        assert_eq!(n.role(), accesskit::Role::Link);
        assert_eq!(n.url(), Some("https://a.b"));
        assert_eq!(n.label(), Some("https://a.b"));
        assert!(n.supports_action(accesskit::Action::Click) && n.supports_action(accesskit::Action::Focus));
    }
}
