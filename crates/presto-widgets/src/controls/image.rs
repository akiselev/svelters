//! `image` (ARCH-WIDGETS §6.1, ported from svelters-egui `widgets/image.rs`, `elements/images.rs`
//! and `elements/media.rs`): an image from a `source`, fitted by `fit`; and the image sources the
//! `icon` kind's `source` shares.
//!
//! A `source` is a name registered with [`register_image`] (PNG or SVG bytes the app hands over,
//! egui's `bytes://` images), else a file: `file://<path>` or a bare path ending in `.png` or `.svg`.
//! Files are read and decoded synchronously the first time a source is measured or recorded and
//! kept per source (a PNG through tiny-skia's decoder, an SVG parsed by usvg and rasterised at the
//! device size it is drawn at); [`forget_image`] drops one so it is read again. The pixels enter the
//! commit through [`Resources::image`](crate::shim::core::Resources::image).
//!
//! `fit` maps to `object-fit` (`contain`, `cover`, `fill`, `none`, `scale-down`; a CSS `object-fit`
//! on the node wins). A source that does not load (a missing file, an unsupported scheme or format)
//! paints a crossed frame in the placeholder colour (`border-widget`). The image takes clicks only
//! while `clicked` is listened ([`Kind::listen`]): then it is Hoverable and Pressable (not in the
//! Tab order, as egui's). Role `Image`.

use crate::controls::common::{self, Units};
use crate::interact::{Hoverable, Intents, Press, PressParams, Pressable, Sense, StateBits, Unit};
use crate::kind_props;
use crate::paint::icon::registry;
use crate::paint::{self, PaintCx, Ua, UaRule, UaValue::*};
use crate::shim::core::{A11yCx, Event, EventCx, Handled, Kind, KindSpec, MeasureCx, NameFrom, PropError, PropId, RecordCx, Resources, SignalId, SignalSpec, Value};
use presto_base::accesskit;
use presto_base::color::{Color, Fill};
use presto_base::commit::ImageData;
use presto_base::display::{BorderStyle, Line, LineCap, Prim, Quad};
use presto_base::event::MouseButton;
use presto_base::geom::{Corners, DeviceSize, Insets, Point, Rect, Size};
use presto_base::ids::ImageId;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};

/// The colour of a placeholder or a not-loaded image's frame (R9: disabled at .4).
pub const PLACEHOLDER_UA: Ua = Ua(&[UaRule::base(&[("color", Token("border-widget")), ("border-radius", Px(2.0))]), UaRule::when(StateBits::DISABLED, &[("opacity", Number(0.4))])]);

/// The side of the square an `image` whose source did not load is measured at (points).
pub const PLACEHOLDER: f32 = 16.0;

// ---- sources ----

/// A loaded source's pixels.
#[derive(Clone, Debug)]
pub enum Pixels {
    /// Decoded once (a PNG): premultiplied RGBA at its own size.
    Raster(Arc<ImageData>),
    /// An SVG document, rasterised at the size it is drawn at.
    Svg(Arc<[u8]>),
}

/// A loaded image source.
#[derive(Clone, Debug)]
pub struct Loaded {
    /// The natural size in points (a PNG's pixels, an SVG's own size).
    pub size: Size,
    pub pixels: Pixels,
    /// Which load this is: a source loaded again (re-registered, forgotten and re-read) gets new
    /// commit keys.
    generation: u64,
}

impl Loaded {
    /// The commit image that draws this source at `px` device pixels, and its pixel size: a PNG's own
    /// pixels, an SVG rasterised at `px`; as an `A8` coverage mask (its alpha) with `mask`, to be
    /// tinted. `None` when it does not rasterise.
    pub fn image(&self, res: &mut dyn Resources, source: &str, px: DeviceSize, mask: bool) -> Option<(ImageId, DeviceSize)> {
        let m = if mask { ":mask" } else { "" };
        match &self.pixels {
            Pixels::Raster(img) => {
                let key = format!("image:{source}#{}{m}", self.generation);
                let id = res.image(&key, &mut || Some(if mask { registry::coverage(img) } else { (**img).clone() }))?;
                Some((id, img.size))
            }
            Pixels::Svg(bytes) => {
                let px = DeviceSize { w: px.w.max(1), h: px.h.max(1) };
                let key = format!("image:{source}#{}@{}x{}{m}", self.generation, px.w, px.h);
                let render = |b: &[u8]| if mask { registry::render_svg_mask(b, px.w, px.h) } else { registry::render_svg(b, px.w, px.h) };
                let id = res.image(&key, &mut || render(bytes).ok())?;
                Some((id, px))
            }
        }
    }
}

type Entry = Result<Arc<Loaded>, String>;

static SOURCES: LazyLock<Mutex<HashMap<String, Entry>>> = LazyLock::new(Mutex::default);
static GENERATION: AtomicU64 = AtomicU64::new(0);

fn sources() -> MutexGuard<'static, HashMap<String, Entry>> {
    SOURCES.lock().unwrap_or_else(|p| p.into_inner())
}

/// Decodes `bytes`: a PNG (by its signature), else an SVG document.
pub fn decode(bytes: &[u8]) -> Result<Loaded, String> {
    let generation = GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
    if bytes.starts_with(b"\x89PNG") {
        let p = resvg::tiny_skia::Pixmap::decode_png(bytes).map_err(|e| e.to_string())?;
        let (w, h) = (p.width(), p.height());
        if w == 0 || h == 0 {
            return Err(format!("empty image ({w}x{h})"));
        }
        let img = ImageData { size: DeviceSize { w, h }, format: presto_base::commit::ImageFormat::Rgba8Premul, pixels: p.take() };
        return Ok(Loaded { size: Size::new(w as f32, h as f32), pixels: Pixels::Raster(Arc::new(img)), generation });
    }
    let size = registry::svg_size(bytes)?;
    Ok(Loaded { size, pixels: Pixels::Svg(Arc::from(bytes)), generation })
}

/// Registers `bytes` (PNG or SVG) as the image source `name`, replacing an earlier one; returns its
/// natural size, or why it does not decode (nothing is registered then).
pub fn register_image(name: &str, bytes: &[u8]) -> Result<Size, String> {
    let l = decode(bytes)?;
    let size = l.size;
    sources().insert(name.to_string(), Ok(Arc::new(l)));
    Ok(size)
}

/// Forgets source `name` (registered or read from a file): a file is read again when next drawn.
/// Whether it was loaded.
pub fn forget_image(name: &str) -> bool {
    sources().remove(name).is_some()
}

/// The file path a `source` names: `file://<path>`, or a bare path (no `scheme://`, not `data:`).
fn file_path(source: &str) -> Option<&str> {
    if let Some(p) = source.strip_prefix("file://") {
        return Some(p);
    }
    let data = source.get(..5).is_some_and(|p| p.eq_ignore_ascii_case("data:"));
    (!data && !source.contains("://")).then_some(source)
}

fn read(source: &str) -> Entry {
    let Some(path) = file_path(source) else {
        return Err(format!("{source}: only registered images and file paths load"));
    };
    let lower = path.to_ascii_lowercase();
    if !lower.ends_with(".png") && !lower.ends_with(".svg") {
        return Err(format!("{path}: only PNG and SVG files load"));
    }
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    decode(&bytes).map(Arc::new).map_err(|e| format!("{path}: {e}"))
}

/// Source `source` loaded (a registered image, else the file read and decoded once), or why it
/// does not load (kept too, until [`forget_image`]).
pub fn load(source: &str) -> Entry {
    if source.is_empty() {
        return Err("no source".into());
    }
    if let Some(e) = sources().get(source) {
        return e.clone();
    }
    let e = read(source);
    sources().insert(source.to_string(), e.clone());
    e
}

// ---- shared with `icon` ----

/// The clicks of a pressable host, as the button reports them: (clicked, double, secondary).
pub(crate) fn clicks(fired: &Intents<Press>) -> (bool, bool, bool) {
    let mut c = (false, false, false);
    for p in fired.iter() {
        match p {
            Press::Click { count: 1, button: MouseButton::Primary, .. } => c.0 = true,
            Press::Click { count: 2, button: MouseButton::Primary, .. } => c.1 = true,
            Press::Click { button: MouseButton::Secondary, .. } => c.2 = true,
            _ => {}
        }
    }
    c
}

/// An outlined square of `side` centred in `r` (an icon or image that cannot be drawn); with
/// `cross`, `r` itself outlined and crossed (an image that did not load).
pub(crate) fn paint_placeholder(pc: &mut PaintCx, r: Rect, side: f32, color: Color, cross: bool) {
    let c = Point::new((r.min.x + r.max.x) / 2.0, (r.min.y + r.max.y) / 2.0);
    let sq = if cross { r } else { Rect::from_origin_size(Point::new(c.x - side / 2.0, c.y - side / 2.0), Size::new(side, side)) };
    let sq = pc.snap_rect(sq);
    let w = pc.snap_width(1.0);
    pc.push(Prim::Quad(Quad { rect: sq, radii: Corners::all(1.0), fill: Fill::Solid(Color::TRANSPARENT), border_widths: Insets::all(w), border_color: color, border_style: BorderStyle::Solid }), sq);
    let x = sq.inset(Insets::all(3.0));
    if cross && !x.is_empty() {
        for (from, to) in [(x.min, x.max), (Point::new(x.max.x, x.min.y), Point::new(x.min.x, x.max.y))] {
            pc.push(Prim::Line(Line { from, to, width: w, color, cap: LineCap::Butt, dash: None, wavy: false }), x.inflate(w));
        }
    }
}

/// `px` points at `scale` as device pixels (at least 1).
pub(crate) fn device(size: Size, scale: f32) -> DeviceSize {
    DeviceSize { w: (size.w * scale).round().max(1.0) as u32, h: (size.h * scale).round().max(1.0) as u32 }
}

// ---- the kind ----

kind_props! {
    /// The image's props.
    pub struct ImageProps;
    /// A registered image name, `file://<path>` or a path (`.png`, `.svg`).
    source "source": String = Str("") ; Str, Layout;
    /// `object-fit` (a CSS `object-fit` on the node wins).
    fit "fit": String = Enum("contain") ; Enum(&["contain", "cover", "fill", "none", "scale-down"]), Paint;
    disabled "disabled": bool = Bool(false) ; Bool, Paint;
}

/// The signals, in spec order.
pub const SIGNALS: &[SignalSpec] = &[SignalSpec::new("clicked", &[], "Pressed and released on it (only while listened).")];

pub static SPEC: KindSpec = KindSpec { tag: "image", props: ImageProps::SPECS, signals: SIGNALS, slots: &[], children: false, role: accesskit::Role::Image, name_from: NameFrom::None, text_prop: None, focusable: false };

/// An image from a `source`, fitted by `fit`.
#[derive(Debug, Default)]
pub struct Image {
    pub props: ImageProps,
    units: Units,
    hover: Hoverable,
    press: Pressable,
    /// `clicked` is listened.
    listened: bool,
}

impl Image {
    fn clickable(&self) -> bool {
        self.listened
    }
}

impl Kind for Image {
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
        if SPEC.signals.get(signal.0 as usize).is_some_and(|s| s.name == "clicked") {
            self.listened = listened;
        }
    }

    fn measure(&self, _cx: &mut MeasureCx, _known: taffy::Size<Option<f32>>, _avail: taffy::Size<taffy::AvailableSpace>) -> taffy::Size<f32> {
        // not loaded (missing file, unknown source): the placeholder's size
        let s = match load(&self.props.source) {
            Ok(l) => l.size,
            Err(_) if self.props.source.is_empty() => Size::ZERO,
            Err(_) => Size::new(PLACEHOLDER, PLACEHOLDER),
        };
        taffy::Size { width: s.w, height: s.h }
    }

    fn record(&self, cx: &mut RecordCx) {
        if self.props.source.is_empty() {
            return;
        }
        let host = common::host_view(&cx.style, PLACEHOLDER_UA);
        let rect = cx.rect();
        let mut pc = PaintCx::of(cx);
        if let Ok(l) = load(&self.props.source) {
            let fit = host.keyword("object-fit").unwrap_or(self.props.fit.as_str());
            let dest = paint::fit_rect(fit, l.size, rect);
            if let Some((id, pixels)) = l.image(pc.res, &self.props.source, device(dest.size(), pc.scale), false) {
                paint::paint_image_pixels(&mut pc, &host, fit, id, l.size, pixels, rect);
                return;
            }
        }
        // not loaded (an error, or it does not rasterise): a crossed-out frame
        paint_placeholder(&mut pc, rect, 0.0, host.color("color").unwrap_or(Color::TRANSPARENT), true);
    }

    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        cx.state.set(StateBits::DISABLED, self.props.disabled);
        cx.enabled = !self.props.disabled;
        let clickable = self.clickable();
        let params = PressParams::default();
        let sense = if clickable { Sense::HOVER.union(self.press.sense(&params)) } else { Sense::HOVER };
        let mut fired = Intents::default();
        let (hover, press) = (&mut self.hover, &mut self.press);
        let pass = self.units.run(cx, ev, sense, |icx| {
            hover.input(&(), icx, &mut Intents::default());
            if clickable {
                press.input(&params, icx, &mut fired);
            }
        });
        if clickable && clicks(&fired).0 {
            common::emit(cx, &SPEC, "clicked", Vec::new());
        }
        if pass.resp.hovered || pass.resp.down_on || !fired.is_empty() { Handled::Yes } else { Handled::No }
    }

    fn a11y(&self, cx: &mut A11yCx) {
        cx.node.set_role(accesskit::Role::Image);
        if self.props.disabled {
            cx.node.set_disabled();
        }
        if self.clickable() {
            self.press.a11y(&PressParams::default(), cx.node);
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::controls::testkit::{self, Driver};
    use presto_base::change::ChangeClass;
    use presto_base::commit::ImageFormat;

    /// A 2x1 PNG: one opaque red pixel, one transparent (premultiplied: all zero).
    pub(crate) fn png_2x1() -> Vec<u8> {
        let mut p = resvg::tiny_skia::Pixmap::new(2, 1).unwrap();
        p.data_mut()[..4].copy_from_slice(&[255, 0, 0, 255]);
        p.encode_png().unwrap()
    }

    pub(crate) const SVG_20X10: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><rect width="20" height="10" fill="blue"/></svg>"#;

    #[test]
    fn spec() {
        let s = Image::spec();
        assert_eq!(s.tag, "image");
        assert_eq!(s.role, accesskit::Role::Image);
        assert!(!s.focusable);
        assert_eq!(s.prop_spec(s.prop("source").unwrap()).unwrap().change, ChangeClass::Layout);
        let fit = s.prop("fit").unwrap();
        assert_eq!(s.prop_spec(fit).unwrap().change, ChangeClass::Paint);
        assert_eq!(s.prop_spec(fit).unwrap().default, crate::shim::core::Default::Enum("contain"));
        assert_eq!(s.signals.iter().map(|s| s.name).collect::<Vec<_>>(), ["clicked"]);
        let mut i = Image::default();
        assert_eq!(i.props.fit, "contain");
        assert!(i.set_prop(fit, &Value::str("stretch")).is_err(), "not an object-fit");
        i.set_prop(fit, &Value::str("cover")).unwrap();
        assert_eq!(i.get_prop(fit), Some(Value::str("cover")));
    }

    #[test]
    fn decodes_png_and_svg() {
        let l = decode(&png_2x1()).unwrap();
        assert_eq!(l.size, Size::new(2.0, 1.0));
        let Pixels::Raster(img) = &l.pixels else { panic!() };
        assert_eq!((img.format, img.size), (ImageFormat::Rgba8Premul, DeviceSize { w: 2, h: 1 }));
        assert_eq!(img.pixels, [255, 0, 0, 255, 0, 0, 0, 0]);
        let s = decode(SVG_20X10).unwrap();
        assert_eq!(s.size, Size::new(20.0, 10.0));
        assert!(matches!(s.pixels, Pixels::Svg(_)));
        assert!(decode(b"neither").is_err());
    }

    #[test]
    fn sources_resolve() {
        assert_eq!(register_image("image-test.red", &png_2x1()), Ok(Size::new(2.0, 1.0)));
        assert!(load("image-test.red").is_ok());
        assert!(register_image("image-test.bad", b"nope").is_err());
        assert!(load("image-test.bad").is_err(), "a failed registration registers nothing (and it is no file)");
        assert!(load("https://example.com/a.png").unwrap_err().contains("only registered images"));
        assert!(load("data:image/png;base64,AAAA").is_err());
        assert!(load("/no/such/image-test.png").unwrap_err().contains("/no/such/image-test.png"));
        assert!(load("/no/such/image-test.jpg").unwrap_err().contains("only PNG and SVG"));
        assert!(forget_image("image-test.red"));
        assert!(load("image-test.red").is_err());
        assert_eq!(file_path("file:///a/b.png"), Some("/a/b.png"));
        assert_eq!(file_path("a/b.png"), Some("a/b.png"));
        assert_eq!(file_path("asset://x/b.png"), None);
    }

    #[test]
    fn reads_png_files() {
        let dir = std::env::temp_dir().join(format!("presto-widgets-image-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("red.png");
        std::fs::write(&path, png_2x1()).unwrap();
        let p = path.to_str().unwrap();
        assert_eq!(load(p).unwrap().size, Size::new(2.0, 1.0));
        assert_eq!(load(&format!("file://{p}")).unwrap().size, Size::new(2.0, 1.0));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn measures_loaded_placeholder_and_empty() {
        register_image("image-test.measure", SVG_20X10).unwrap();
        let mut i = Image::default();
        assert_eq!(testkit::measure(&i, StateBits::NONE), Size::ZERO);
        i.props.source = "image-test.measure".into();
        assert_eq!(testkit::measure(&i, StateBits::NONE), Size::new(20.0, 10.0));
        i.props.source = "image-test.missing.png".into();
        assert_eq!(testkit::measure(&i, StateBits::NONE), Size::new(PLACEHOLDER, PLACEHOLDER));
    }

    #[test]
    fn records_a_fitted_image() {
        register_image("image-test.svg", SVG_20X10).unwrap();
        let mut i = Image::default();
        i.props.source = "image-test.svg".into();
        // contain: 40x20 in the middle of 40x40, rasterised at that size
        let (list, res) = testkit::record(&i, Size::new(40.0, 40.0), StateBits::NONE);
        assert_eq!(testkit::kinds(&list), ["image"]);
        let Prim::Image(p) = &list.items[0] else { unreachable!() };
        assert_eq!((p.rect, p.src), (Rect::new(0.0, 10.0, 40.0, 30.0), None));
        let (_, data) = res.images.values().next().unwrap();
        assert_eq!((data.format, data.size), (ImageFormat::Rgba8Premul, DeviceSize { w: 40, h: 20 }));
        // cover: fills the box, cropped in the raster's pixels
        i.props.fit = "cover".into();
        let (list, res) = testkit::record(&i, Size::new(40.0, 40.0), StateBits::NONE);
        let Prim::Image(p) = &list.items[0] else { unreachable!() };
        assert_eq!(p.rect, Rect::new(0.0, 0.0, 40.0, 40.0));
        assert_eq!(p.src, Some(Rect::new(20.0, 0.0, 60.0, 40.0)));
        assert_eq!(res.images.values().next().unwrap().1.size, DeviceSize { w: 80, h: 40 });
    }

    #[test]
    fn records_png_pixels_as_they_are() {
        register_image("image-test.png", &png_2x1()).unwrap();
        let mut i = Image::default();
        i.props.source = "image-test.png".into();
        i.props.fit = "fill".into();
        let (list, res) = testkit::record(&i, Size::new(10.0, 10.0), StateBits::NONE);
        let Prim::Image(p) = &list.items[0] else { unreachable!() };
        assert_eq!(p.rect, Rect::new(0.0, 0.0, 10.0, 10.0));
        let (id, data) = res.images.values().next().unwrap();
        assert_eq!((*id, data.size, &data.pixels[..4]), (p.image, DeviceSize { w: 2, h: 1 }, &[255u8, 0, 0, 255][..]));
    }

    #[test]
    fn a_source_that_does_not_load_is_a_crossed_frame() {
        let mut i = Image::default();
        i.props.source = "/no/such/image-test-frame.png".into();
        let (list, res) = testkit::record(&i, Size::new(16.0, 16.0), StateBits::NONE);
        assert_eq!(testkit::kinds(&list), ["quad", "line", "line"]);
        let Prim::Quad(q) = &list.items[0] else { unreachable!() };
        assert_eq!((q.rect, q.border_widths.top, q.fill.clone()), (Rect::new(0.0, 0.0, 16.0, 16.0), 1.0, Fill::Solid(Color::TRANSPARENT)));
        assert_eq!(Some(q.border_color), paint::ua_token(true, "border-widget"));
        let Prim::Line(l) = &list.items[1] else { unreachable!() };
        assert_eq!((l.from, l.to), (Point::new(3.0, 3.0), Point::new(13.0, 13.0)));
        assert!(res.images.is_empty());
        // no source: nothing
        i.props.source.clear();
        assert!(testkit::record(&i, Size::new(16.0, 16.0), StateBits::NONE).0.items.is_empty());
    }

    #[test]
    fn clicks_only_while_listened() {
        let mut d = Driver::new(Image::default(), Size::new(20.0, 20.0));
        d.click();
        assert!(d.take().is_empty(), "nobody listens");
        assert!(!d.state.contains(StateBits::PRESSED));
        let n = testkit::a11y(&d.kind, StateBits::NONE, d.size);
        assert!(!n.supports_action(accesskit::Action::Click));
        d.kind.listen(d.kind.kind_spec().signal("clicked").unwrap(), true);
        d.click();
        assert_eq!(d.take(), ["clicked"]);
        let n = testkit::a11y(&d.kind, StateBits::NONE, d.size);
        assert_eq!(n.role(), accesskit::Role::Image);
        assert!(n.supports_action(accesskit::Action::Click));
        d.set("disabled", Value::Bool(true));
        d.click();
        assert!(d.take().is_empty(), "disabled");
        assert!(d.state.contains(StateBits::DISABLED));
    }
}
