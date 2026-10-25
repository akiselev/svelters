//! `icon` (ARCH-WIDGETS §6.1, ported from svelters-egui `widgets/image.rs` and `paint/icon.rs`): an
//! icon registry icon, an icon-font glyph, or an image `source` (tinted with `color` when `tint`).
//!
//! What it draws, in order:
//! - `source` set: that image (`controls::image`'s sources) fitted in a square of `size` (else
//!   `font-size`) points; tinted, its alpha is a coverage mask drawn in `color` (a template image);
//!   untinted, its own pixels. A source that does not load paints an outlined square.
//! - `name` an icon registry key (family `icons`, or `family` empty): the registry icon
//!   (`paint::paint_icon_sized`: optical-size master, `color` tint, `--icon-*` slots) in a square of
//!   `size` (else 16) points.
//! - else a glyph: `name` from the icon font `family`'s table, else `glyph`, else `name` (a ligature),
//!   at `size` (else `font-size`) points and `font-weight`. An empty `family` is the body font. A
//!   family that is not a registered icon font, or a name not in its table, paints an outlined
//!   square; no name and no glyph draws nothing (a blank square of the size).
//!
//! With `frame` it is an icon button (the button's UA table: R1–R3 fills, R8 ring, R9 disabled, R10
//! pointer; focusable, role `Button`); without, an image (role `Image`, or `Button` while a click
//! signal is listened, [`Kind::listen`]). An unframed icon's drawn rect and hit area are its content
//! at the start of its box, centred vertically. Named by `name`.

use crate::controls::button;
use crate::controls::common::{self, Units};
use crate::controls::image::{self as images, PLACEHOLDER_UA, clicks, device, paint_placeholder};
use crate::interact::{FocusParams, Focusable, Hoverable, Intents, PressParams, Pressable, Sense, StateBits, Unit};
use crate::kind_props;
use crate::paint::icon::registry;
use crate::paint::{self, IconKey, PaintCx, StyleView, Ua, UaRule, UaValue::*};
use crate::shim::core::{A11yCx, Event, EventCx, Handled, HitCx, HitResult, Kind, KindSpec, MeasureCx, NameFrom, PrepareCx, PropError, PropId, RecordCx, SignalId, SignalSpec, Value};
use crate::shim::text::{RichText, TextLayout, TextShaper};
use presto_base::accesskit;
use presto_base::color::Color;
use presto_base::display::{Icon as IconPrim, Prim};
use presto_base::geom::{Point, Rect, Size, Vec2};
use presto_base::key::Key;
use std::sync::Arc;

/// An unframed icon: its tint (the theme's `icon` colour) and the disabled look (R9).
pub const ICON_UA: Ua = Ua(&[UaRule::base(&[("color", Token("icon"))]), UaRule::when(StateBits::DISABLED, &[("opacity", Number(0.4)), ("cursor", Keyword("not-allowed"))])]);

kind_props! {
    /// The icon's props.
    pub struct IconProps;
    /// An icon registry key, or a name in the family's table.
    name "name": String = Str("") ; Str, Layout;
    /// The glyph itself, when `name` is empty or not in the family's table.
    glyph "glyph": String = Str("") ; Str, Layout;
    /// The registered icon font (empty: the registry, then the body font).
    family "family": String = Str("") ; Str, Layout;
    /// Points (else 16 for a registry icon, the CSS `font-size` for the rest).
    size "size": Option<f64> = None ; F64, Layout;
    font_weight "font-weight": i64 = I64(400) ; I64, Layout;
    /// An image instead of a glyph (empty: none).
    source "source": String = Str("") ; Str, Layout;
    /// With `source`: drawn in the colour.
    tint "tint": bool = Bool(true) ; Bool, Paint;
    /// An icon button.
    frame "frame": bool = Bool(false) ; Bool, Layout;
    /// With `frame`: the selected (toggle) look.
    selected "selected": bool = Bool(false) ; Bool, Paint;
    disabled "disabled": bool = Bool(false) ; Bool, Paint;
}

/// The signals, in spec order.
pub const SIGNALS: &[SignalSpec] = &[
    SignalSpec::new("clicked", &[], "Pressed and released (framed, or while listened); Space, Enter, AccessKit `Click`."),
    SignalSpec::new("double-clicked", &[], ""),
    SignalSpec::new("secondary-clicked", &[], ""),
];

pub static SPEC: KindSpec = KindSpec { tag: "icon", props: IconProps::SPECS, signals: SIGNALS, slots: &[], children: false, role: accesskit::Role::Image, name_from: NameFrom::Prop("name"), text_prop: None, focusable: false };

/// What an icon draws, with its size.
enum Content {
    Source,
    Registry(IconKey),
    Glyph(Arc<TextLayout>),
    /// No name and no glyph.
    Blank,
    /// A family, name or glyph that cannot be drawn.
    Placeholder,
}

/// An icon registry icon, an icon-font glyph or a small image (an icon button with `frame`).
#[derive(Debug, Default)]
pub struct Icon {
    pub props: IconProps,
    units: Units,
    hover: Hoverable,
    focus: Focusable,
    press: Pressable,
    /// Which click signals are listened (bits by signal index).
    listened: u32,
    /// An unframed icon's content size at the last prepare (its hit area).
    content: Option<Vec2>,
}

impl Icon {
    /// It takes clicks: framed, or a click signal is listened.
    fn clickable(&self) -> bool {
        self.props.frame || self.listened != 0
    }
    fn focus_params(&self) -> FocusParams {
        FocusParams { tab_stop: self.props.frame, ..FocusParams::default() }
    }
    fn ua(&self) -> Ua {
        if self.props.frame { button::UA } else { ICON_UA }
    }
    fn mirror(&self, state: &mut StateBits) {
        state.set(StateBits::SELECTED, self.props.frame && self.props.selected);
        state.set(StateBits::DISABLED, self.props.disabled);
    }
    fn size_prop(&self) -> Option<f32> {
        self.props.size.filter(|v| v.is_finite() && *v > 0.0).map(|v| v as f32)
    }
    /// The registry key `name` is, when it draws from the icon registry.
    fn registry_key(&self) -> Option<IconKey> {
        if !self.props.source.is_empty() || self.props.name.is_empty() {
            return None;
        }
        // an empty family is the registry's
        let family = if self.props.family.is_empty() { IconKey::REGISTRY_FAMILY } else { self.props.family.as_str() };
        let key = IconKey::new(family, self.props.name.clone());
        paint::is_registered(&key).then_some(key)
    }
    /// A registry icon's side: `size`, else 16 (ICON-STYLE's default chrome size).
    fn registry_side(&self) -> f32 {
        self.size_prop().unwrap_or(registry::DEFAULT_SIZE)
    }
    /// What it draws under `s`, and its size.
    fn content(&self, shaper: &mut dyn TextShaper, s: &StyleView) -> (Vec2, Content) {
        let pt = self.size_prop().unwrap_or_else(|| s.font_size());
        let square = Vec2::new(pt, pt);
        if !self.props.source.is_empty() {
            return (square, Content::Source);
        }
        if let Some(key) = self.registry_key() {
            return (paint::icon_size_sized(shaper, s, &key, Some(self.registry_side())), Content::Registry(key));
        }
        let family = self.props.family.as_str();
        if !family.is_empty() && !registry::has_font(family) {
            return (square, Content::Placeholder);
        }
        let Some(text) = registry::icon_text(family, &self.props.name, &self.props.glyph) else { return (square, Content::Placeholder) };
        if text.is_empty() {
            return (square, Content::Blank);
        }
        let mut style = paint::text_style(s);
        style.size = pt;
        style.weight = self.props.font_weight.clamp(1, 1000) as u16;
        if !family.is_empty() {
            style.family = Arc::from(family);
        }
        let l = shaper.layout(&RichText::plain(&text, style), None);
        (Vec2::new(l.size().w, l.size().h), Content::Glyph(l))
    }
    /// An unframed icon's content rect in a box of `size`: at the start, centred vertically (the
    /// whole box when the content is empty).
    fn content_rect(size: Size, c: Vec2) -> Rect {
        if c.x <= 0.0 || c.y <= 0.0 {
            return Rect::from_origin_size(Point::ZERO, size);
        }
        Rect::from_origin_size(Point::new(0.0, size.h / 2.0 - c.y / 2.0), Size::new(c.x, c.y))
    }
}

impl Kind for Icon {
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
    fn listen(&mut self, signal: SignalId, listened: bool) {
        if signal.0 < SIGNALS.len() as u32 {
            let bit = 1 << signal.0;
            if listened { self.listened |= bit } else { self.listened &= !bit }
        }
    }

    fn measure(&self, cx: &mut MeasureCx, _known: taffy::Size<Option<f32>>, _avail: taffy::Size<taffy::AvailableSpace>) -> taffy::Size<f32> {
        let s = common::host_view(&cx.style, self.ua());
        let (c, _) = self.content(cx.text, &s);
        let c = if self.props.frame { c + paint::box_extra(&s) } else { c };
        taffy::Size { width: c.x, height: c.y }
    }

    fn prepare(&mut self, cx: &mut PrepareCx) {
        let s = common::host_view(&cx.style, self.ua());
        self.content = (!self.props.frame).then(|| self.content(cx.text, &s).0);
    }

    fn hit_test(&self, cx: &HitCx, p: Point) -> HitResult {
        match self.content {
            Some(c) if !self.props.frame => {
                if Self::content_rect(cx.size, c).contains(p) { HitResult::Hit } else { HitResult::Miss }
            }
            _ => cx.box_hit(p),
        }
    }

    fn record(&self, cx: &mut RecordCx) {
        let ua = self.ua();
        let host = common::host_view(&cx.style, ua);
        let rect = cx.rect();
        let size = cx.size;
        let mut pc = PaintCx::of(cx);
        let (csize, content) = self.content(pc.text, &host);
        let area = if self.props.frame { paint::paint_box(&mut pc, &host, rect).content } else { Self::content_rect(size, csize) };
        let color = host.color("color").unwrap_or(Color::BLACK);
        let center = Point::new((area.min.x + area.max.x) / 2.0, (area.min.y + area.max.y) / 2.0);
        let (w, h) = (csize.x.min(area.width().max(0.0)).max(1.0), csize.y.min(area.height().max(0.0)).max(1.0));
        let r = Rect::from_origin_size(Point::new(center.x - w / 2.0, center.y - h / 2.0), Size::new(w, h));
        match content {
            Content::Source => {
                let src = self.props.source.as_str();
                let mut drawn = false;
                if let Ok(l) = images::load(src) {
                    let dest = paint::fit_rect("contain", l.size, r);
                    let px = device(dest.size(), pc.scale);
                    if self.props.tint {
                        if let Some((id, _)) = l.image(pc.res, src, px, true) {
                            pc.push(Prim::Icon(IconPrim { image: id, rect: dest, color: Some(color) }), dest);
                            drawn = true;
                        }
                    } else if let Some((id, pixels)) = l.image(pc.res, src, px, false) {
                        let plain = StyleView::ua_only(Ua::NONE, host.state(), host.is_dark());
                        paint::paint_image_pixels(&mut pc, &plain, "contain", id, l.size, pixels, r);
                        drawn = true;
                    }
                }
                if !drawn {
                    let pv = StyleView::ua_only(PLACEHOLDER_UA, host.state(), host.is_dark());
                    paint_placeholder(&mut pc, r, r.width().min(r.height()), pv.color("color").unwrap_or(color), false);
                }
            }
            Content::Registry(key) => paint::paint_icon_sized(&mut pc, &host, &key, r, Some(self.registry_side())),
            Content::Glyph(l) => {
                let pos = pc.snap_point(Point::new(center.x - l.size().w / 2.0, center.y - l.size().h / 2.0));
                paint::paint_runs(&mut pc, &l, pos, &|_| color);
            }
            Content::Blank => {}
            Content::Placeholder => paint_placeholder(&mut pc, r, csize.x, color, false),
        }
        if self.props.frame {
            paint::paint_outline(&mut pc, &host, rect);
        }
    }

    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        self.mirror(cx.state);
        cx.enabled = !self.props.disabled;
        let clickable = self.clickable();
        let fp = self.focus_params();
        let pp = PressParams::default();
        let sense = if clickable { Sense::HOVER.union(self.focus.sense(&fp)).union(self.press.sense(&pp)) } else { Sense::HOVER };
        let mut fired = Intents::default();
        let (hover, focus, press) = (&mut self.hover, &mut self.focus, &mut self.press);
        let pass = self.units.run(cx, ev, sense, |icx| {
            hover.input(&(), icx, &mut Intents::default());
            if clickable {
                focus.input(&fp, icx, &mut Intents::default());
                press.input(&pp, icx, &mut fired);
            }
        });
        if clickable {
            let (c, d, s) = clicks(&fired);
            for (on, name) in [(c, "clicked"), (d, "double-clicked"), (s, "secondary-clicked")] {
                if on {
                    common::emit(cx, &SPEC, name, Vec::new());
                }
            }
            common::cursor(cx, &self.ua());
        }
        if pass.resp.hovered || pass.resp.down_on || !fired.is_empty() { Handled::Yes } else { Handled::No }
    }

    fn a11y(&self, cx: &mut A11yCx) {
        let clickable = self.clickable();
        cx.node.set_role(if clickable { accesskit::Role::Button } else { accesskit::Role::Image });
        if !self.props.name.is_empty() {
            cx.node.set_label(self.props.name.clone());
        }
        if self.props.disabled {
            cx.node.set_disabled();
        }
        if self.props.frame && self.props.selected {
            cx.node.set_selected(true);
        }
        if clickable {
            if self.props.frame {
                self.focus.a11y(&self.focus_params(), cx.node);
            }
            self.press.a11y(&PressParams::default(), cx.node);
        }
    }

    fn claims_key(&self, key: Key, m: presto_base::event::Modifiers) -> bool {
        self.props.frame && self.press.claims(&PressParams::default(), key, crate::interact::Mods::of(m, false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::image::tests::png_2x1;
    use crate::controls::testkit::{self, Driver};
    use crate::paint::icon::registry::{Icon as RegIcon, IconRegistry, Owner};
    use crate::shim::testing::MonoShaper;
    use presto_base::change::ChangeClass;
    use presto_base::commit::ImageFormat;

    const SQUARE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><rect width="16" height="16" fill="currentColor"/></svg>"#;

    /// Registers `key` (unique per test: the registry is the process's) as a square SVG icon.
    fn register(key: &str) {
        IconRegistry::global().register(Owner::Host, key, RegIcon::svg(16, SQUARE).master(24, SQUARE)).unwrap();
    }

    fn icon(name: &str) -> Icon {
        let mut i = Icon::default();
        i.props.name = name.into();
        i
    }

    #[test]
    fn spec() {
        let s = Icon::spec();
        assert_eq!(s.tag, "icon");
        assert_eq!(s.role, accesskit::Role::Image);
        assert_eq!(s.name_from, NameFrom::Prop("name"));
        for (p, c) in [("name", ChangeClass::Layout), ("size", ChangeClass::Layout), ("frame", ChangeClass::Layout), ("tint", ChangeClass::Paint), ("selected", ChangeClass::Paint)] {
            assert_eq!(s.prop_spec(s.prop(p).unwrap()).unwrap().change, c, "{p}");
        }
        assert_eq!(s.signals.iter().map(|s| s.name).collect::<Vec<_>>(), ["clicked", "double-clicked", "secondary-clicked"]);
        let mut i = Icon::default();
        assert_eq!((i.props.size, i.props.tint, i.props.font_weight), (None, true, 400));
        let size = s.prop("size").unwrap();
        i.set_prop(size, &Value::F64(24.0)).unwrap();
        assert_eq!(i.get_prop(size), Some(Value::F64(24.0)));
        i.set_prop(size, &Value::List(Vec::new())).unwrap();
        assert_eq!(i.props.size, None, "unset");
        assert!(i.set_prop(s.prop("frame").unwrap(), &Value::str("yes")).is_err());
    }

    #[test]
    fn registry_icon_measures_and_records() {
        register("icon-kind-test.save");
        let mut i = icon("icon-kind-test.save");
        assert_eq!(testkit::measure(&i, StateBits::NONE), Size::new(16.0, 16.0), "16pt by default");
        i.props.size = Some(24.0);
        assert_eq!(testkit::measure(&i, StateBits::NONE), Size::new(24.0, 24.0));
        i.props.size = None;
        // at the start of a wider box, centred vertically
        let (list, res) = testkit::record(&i, Size::new(40.0, 20.0), StateBits::NONE);
        assert_eq!(testkit::kinds(&list), ["icon"]);
        let Prim::Icon(p) = &list.items[0] else { unreachable!() };
        assert_eq!(p.rect, Rect::new(0.0, 2.0, 16.0, 18.0));
        assert_eq!(p.color, paint::ua_token(true, "icon"));
        let (_, data) = res.images.values().next().unwrap();
        assert_eq!((data.format, data.size.w), (ImageFormat::A8, 16));
        // the `icons:` family is the registry's too
        i.props.family = "icons".into();
        assert_eq!(testkit::kinds(&testkit::record(&i, Size::new(16.0, 16.0), StateBits::NONE).0), ["icon"]);
    }

    #[test]
    fn framed_is_an_icon_button() {
        register("icon-kind-test.framed");
        let mut i = icon("icon-kind-test.framed");
        i.props.frame = true;
        // the button's padding 4/8 and 1px border around 16
        assert_eq!(testkit::measure(&i, StateBits::NONE), Size::new(34.0, 26.0));
        let (list, _) = testkit::record(&i, Size::new(34.0, 26.0), StateBits::NONE);
        assert_eq!(testkit::kinds(&list), ["shadow", "quad", "icon"]);
        let Prim::Icon(p) = &list.items[2] else { unreachable!() };
        assert_eq!(p.rect, Rect::new(9.0, 5.0, 25.0, 21.0));
        let (list, _) = testkit::record(&i, Size::new(34.0, 26.0), StateBits::FOCUSED | StateBits::FOCUS_VISIBLE);
        assert_eq!(testkit::kinds(&list), ["shadow", "quad", "icon", "quad"], "the ring last");
        let n = testkit::a11y(&i, StateBits::NONE, Size::new(34.0, 26.0));
        assert_eq!(n.role(), accesskit::Role::Button);
        assert_eq!(n.label(), Some("icon-kind-test.framed"));
        assert!(n.supports_action(accesskit::Action::Click) && n.supports_action(accesskit::Action::Focus));
    }

    #[test]
    fn glyph_of_a_registered_font() {
        registry::register_icon_font("icon-kind-test-font");
        let mut i = icon("ab");
        i.props.family = "icon-kind-test-font".into();
        i.props.size = Some(10.0);
        let adv = MonoShaper::advance(10.0);
        // a ligature font: the name is the text
        assert_eq!(testkit::measure(&i, StateBits::NONE), Size::new(2.0 * adv, 13.0));
        let (list, _) = testkit::record(&i, Size::new(20.0, 13.0), StateBits::NONE);
        assert_eq!(testkit::kinds(&list), ["glyphs"]);
        let Prim::Glyphs(g) = &list.items[0] else { unreachable!() };
        assert_eq!(Some(g.color), paint::ua_token(true, "icon"));
    }

    #[test]
    fn unregistered_family_and_unknown_name_are_squares() {
        let mut i = icon("nope");
        i.props.family = "icon-kind-test-unbound".into();
        i.props.size = Some(10.0);
        assert_eq!(testkit::measure(&i, StateBits::NONE), Size::new(10.0, 10.0));
        let (list, _) = testkit::record(&i, Size::new(10.0, 10.0), StateBits::NONE);
        assert_eq!(testkit::kinds(&list), ["quad"]);
        let Prim::Quad(q) = &list.items[0] else { unreachable!() };
        assert_eq!((q.rect, q.border_widths.top), (Rect::new(0.0, 0.0, 10.0, 10.0), 1.0));
        registry::register_icon_names("icon-kind-test-table", [("check".to_string(), "x".to_string())]);
        i.props.family = "icon-kind-test-table".into();
        assert_eq!(testkit::kinds(&testkit::record(&i, Size::new(10.0, 10.0), StateBits::NONE).0), ["quad"], "not in the table");
        i.props.name = "check".into();
        assert_eq!(testkit::kinds(&testkit::record(&i, Size::new(10.0, 10.0), StateBits::NONE).0), ["glyphs"]);
        // no name, no glyph: nothing
        let mut blank = Icon::default();
        blank.props.size = Some(10.0);
        assert!(testkit::record(&blank, Size::new(10.0, 10.0), StateBits::NONE).0.items.is_empty());
        assert_eq!(testkit::measure(&blank, StateBits::NONE), Size::new(10.0, 10.0));
    }

    #[test]
    fn source_tinted_or_not() {
        images::register_image("icon-kind-test.png", &png_2x1()).unwrap();
        let mut i = Icon::default();
        i.props.source = "icon-kind-test.png".into();
        i.props.size = Some(10.0);
        let (list, res) = testkit::record(&i, Size::new(10.0, 10.0), StateBits::NONE);
        assert_eq!(testkit::kinds(&list), ["icon"]);
        let Prim::Icon(p) = &list.items[0] else { unreachable!() };
        // 2x1 fitted in 10x10
        assert_eq!((p.rect, p.color), (Rect::new(0.0, 2.5, 10.0, 7.5), paint::ua_token(true, "icon")));
        let (_, data) = res.images.values().next().unwrap();
        assert_eq!((data.format, &data.pixels[..]), (ImageFormat::A8, &[255u8, 0][..]), "the alpha as coverage");
        i.props.tint = false;
        let (list, res) = testkit::record(&i, Size::new(10.0, 10.0), StateBits::NONE);
        assert_eq!(testkit::kinds(&list), ["image"]);
        assert_eq!(res.images.values().next().unwrap().1.format, ImageFormat::Rgba8Premul);
        i.props.source = "/no/such/icon-kind-test.png".into();
        let (list, _) = testkit::record(&i, Size::new(10.0, 10.0), StateBits::NONE);
        assert_eq!(testkit::kinds(&list), ["quad"]);
        let Prim::Quad(q) = &list.items[0] else { unreachable!() };
        assert_eq!(Some(q.border_color), paint::ua_token(true, "border-widget"));
    }

    #[test]
    fn unframed_hit_area_is_the_content() {
        register("icon-kind-test.hit");
        let mut i = icon("icon-kind-test.hit");
        let mut text = MonoShaper;
        let style = crate::shim::core::Styles { computed: None, parts: &[], children: &[], state: StateBits::NONE, dark: true };
        i.prepare(&mut PrepareCx { size: Size::new(40.0, 20.0), style, text: &mut text });
        let hit = HitCx { size: Size::new(40.0, 20.0) };
        assert_eq!(i.hit_test(&hit, Point::new(8.0, 10.0)), HitResult::Hit);
        assert_eq!(i.hit_test(&hit, Point::new(30.0, 10.0)), HitResult::Miss);
        assert_eq!(i.hit_test(&hit, Point::new(8.0, 1.0)), HitResult::Miss);
    }

    #[test]
    fn framed_clicks_and_keys() {
        let mut i = Icon::default();
        i.props.frame = true;
        let mut d = Driver::new(i, Size::new(34.0, 26.0));
        d.click();
        assert_eq!(d.take(), ["clicked"]);
        d.click();
        assert_eq!(d.take(), ["clicked", "double-clicked"]);
        d.advance(1000);
        d.secondary_click(5.0, 5.0);
        assert_eq!(d.take(), ["secondary-clicked"]);
        d.focus(true);
        d.key(Key::Enter);
        assert_eq!(d.take(), ["clicked"]);
        assert!(d.kind.claims_key(Key::Space, Default::default()));
        d.set("selected", Value::Bool(true));
        d.hover(true);
        assert!(d.state.contains(StateBits::SELECTED));
    }

    #[test]
    fn unframed_clicks_only_while_listened() {
        let mut d = Driver::new(icon("x"), Size::new(16.0, 16.0));
        d.click();
        assert!(d.take().is_empty());
        assert!(!d.kind.claims_key(Key::Space, Default::default()));
        assert_eq!(testkit::a11y(&d.kind, StateBits::NONE, d.size).role(), accesskit::Role::Image);
        let clicked = d.kind.kind_spec().signal("clicked").unwrap();
        d.kind.listen(clicked, true);
        d.click();
        assert_eq!(d.take(), ["clicked"]);
        let n = testkit::a11y(&d.kind, StateBits::NONE, d.size);
        assert_eq!(n.role(), accesskit::Role::Button);
        assert!(n.supports_action(accesskit::Action::Click) && !n.supports_action(accesskit::Action::Focus));
        d.kind.listen(clicked, false);
        d.advance(1000);
        d.click();
        assert!(d.take().is_empty(), "no longer listened");
    }

    #[test]
    fn disabled_does_not_click() {
        let mut i = Icon::default();
        i.props.frame = true;
        let mut d = Driver::new(i, Size::new(34.0, 26.0));
        d.set("disabled", Value::Bool(true));
        d.click();
        assert!(d.take().is_empty());
        assert!(d.state.contains(StateBits::DISABLED));
        assert!(testkit::a11y(&d.kind, StateBits::DISABLED, d.size).is_disabled());
    }
}
