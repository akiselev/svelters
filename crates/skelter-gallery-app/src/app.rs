//! The platform handler and [`run`]: one window with the gallery, on skelter's app runner.

use crate::document::GalleryDocument;
use crate::gallery::{Gallery, kinds};
use skelter::seams::{CompositorApi, DocumentApi};
use skelter::{App, AppConfig, SpawnCompositor};
use skelter_base::event::{InputEnvelope, WindowEvent};
use skelter_base::geom::Size;
use skelter_base::ids::WindowId;
use skelter_base::time::{Clock, Span, Time};
use skelter_gallery::Extensions;
use skelter_platform::{A11yEvent, AppHandler, PlatformCx, WindowDesc};
use std::cell::RefCell;
use std::process::ExitCode;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

/// The gallery's document, its stories found and its chrome built, over `fonts`.
pub fn document(ext: Extensions, fonts: Arc<skelter_text::FontStore>) -> Result<GalleryDocument, String> {
    let kinds = kinds(&ext)?;
    let mut doc = skelter_core::Document::new(kinds.registry().clone(), Box::new(skelter_text::TextSystem::new(fonts.clone())));
    let policy = skelter_widgets::host::os_policy();
    let chrome = crate::theme::builtin(crate::theme::CHROME, skelter_widgets::theme::Density::Default).expect("the chrome theme");
    skelter_widgets::host::install(&mut doc, skelter_text::TextSystem::new(fonts.clone()), policy, chrome);
    let gallery = Gallery::new(Rc::new(RefCell::new(doc)), kinds, fonts, ext);
    Ok(GalleryDocument::new(gallery))
}

/// The platform's handler around the gallery's [`App`].
pub struct GalleryApp<C: CompositorApi + 'static> {
    app: App<GalleryDocument, C>,
    clock: Option<Arc<dyn Clock>>,
}

impl<C: CompositorApi + 'static> GalleryApp<C> {
    pub fn new(doc: GalleryDocument, spawn: SpawnCompositor<C>) -> Self {
        let cfg = AppConfig { windows: vec![WindowDesc::new("skelter gallery", Size::new(1500.0, 950.0))], ..AppConfig::default() };
        GalleryApp { app: App::new(cfg, doc, spawn), clock: None }
    }

    /// After every callback: frames for what changed outside input, the platform timer at the next
    /// JS or kind deadline.
    fn sync(&mut self, cx: &mut dyn PlatformCx) {
        let windows: Vec<WindowId> = self.app.windows().collect();
        if let Some(c) = self.app.compositor() {
            for w in windows {
                if self.app.doc().needs_frame(w) {
                    c.request_ui_frame(w);
                }
            }
        }
        let js = self.app.doc().gallery.backend().and_then(|b| b.deadline());
        let at = match (js, &self.clock) {
            (Some(d), Some(clock)) => Some(clock.now() + Span::from_micros(d.saturating_duration_since(Instant::now()).as_micros() as i64)),
            _ => None,
        };
        cx.set_timer(at);
    }
}

impl<C: CompositorApi + 'static> AppHandler for GalleryApp<C> {
    fn init(&mut self, cx: &mut dyn PlatformCx) {
        self.clock = Some(cx.clock());
        let waker = cx.waker();
        self.app.doc_mut().gallery.set_wake(Arc::new(move || waker.wake()));
        self.app.init(cx);
        self.sync(cx);
    }

    fn input(&mut self, cx: &mut dyn PlatformCx, ev: InputEnvelope) {
        self.app.input(cx, ev);
        self.sync(cx);
    }

    fn window_event(&mut self, cx: &mut dyn PlatformCx, window: WindowId, ev: WindowEvent) {
        self.app.window_event(cx, window, ev);
        self.sync(cx);
    }

    fn wake(&mut self, cx: &mut dyn PlatformCx) {
        if let Some(b) = self.app.doc().gallery.backend().cloned() {
            // the watcher's reload runs in the host's checkpoint
            b.poll_wake();
        }
        self.app.doc_mut().gallery.dirty = true;
        self.app.wake(cx);
        self.sync(cx);
    }

    fn timer(&mut self, cx: &mut dyn PlatformCx, now: Time) {
        if let Some(b) = self.app.doc().gallery.backend().cloned()
            && b.deadline().is_some_and(|d| d <= Instant::now())
        {
            b.fire_timer();
        }
        self.app.timer(cx, now);
        self.sync(cx);
    }

    fn a11y_event(&mut self, cx: &mut dyn PlatformCx, window: WindowId, ev: A11yEvent) {
        self.app.a11y_event(cx, window, ev);
        self.sync(cx);
    }

    fn exiting(&mut self, cx: &mut dyn PlatformCx) {
        self.app.exiting(cx);
    }
}

const USAGE: &str = "usage: skelter-gallery-app [<story>[/<variant>]] [--base-css <file>] [--shot <out.png> [--size <w>x<h>] [--pick <cell> [--inspect <selector>]]]";

/// The command line: what to open, and the headless shot.
#[derive(Debug, Default)]
struct Args {
    select: Option<String>,
    shot: Option<std::path::PathBuf>,
    size: Option<(f32, f32)>,
    pick: Option<usize>,
    inspect: Option<String>,
    base_css: Option<std::path::PathBuf>,
}

fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut a = Args::default();
    let mut it = args.iter();
    while let Some(x) = it.next() {
        match x.as_str() {
            "--shot" => a.shot = Some(it.next().ok_or("--shot needs a path")?.into()),
            "--size" => {
                let v = it.next().ok_or("--size needs <w>x<h>")?;
                let (w, h) = v.split_once('x').ok_or("--size needs <w>x<h>")?;
                a.size = Some((w.parse().map_err(|_| "bad --size")?, h.parse().map_err(|_| "bad --size")?));
            }
            "--inspect" => a.inspect = Some(it.next().ok_or("--inspect needs a selector")?.clone()),
            "--base-css" => a.base_css = Some(it.next().ok_or("--base-css needs a file")?.into()),
            "--pick" => a.pick = Some(it.next().ok_or("--pick needs a cell index")?.parse().map_err(|_| "bad --pick")?),
            s if s.starts_with('-') => return Err(format!("unknown flag {s}")),
            s => a.select = Some(s.to_string()),
        }
    }
    Ok(a)
}

/// Runs the gallery with a host's [`Extensions`] (`Extensions::default()`: svelters' own `gallery/`).
/// `skelter-gallery-app [<story>[/<variant>]]` opens on that story; `--shot out.png` renders the
/// window headlessly (lavapipe in a container) instead of opening it.
pub fn run(ext: Extensions) -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        eprintln!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let args = match parse_args(&args) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("skelter-gallery-app: {e}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let fonts = skelter_text::FontStore::new(skelter_text::FontConfig::default());
    let mut doc = match document(ext, fonts.clone()) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("skelter-gallery-app: {e}");
            return ExitCode::from(1);
        }
    };
    if let Some(css) = &args.base_css {
        doc.gallery.set_base_css_path(Some(css.clone()));
    }
    let first = args.select.clone().or_else(|| doc.gallery.catalog().stories().first().map(|s| s.id.0.clone()));
    if let Some(sel) = first {
        let (s, v) = sel.split_once('/').map_or((sel.as_str(), None), |(s, v)| (s, Some(v)));
        if doc.gallery.catalog().get(s).is_none() {
            eprintln!("skelter-gallery-app: no story {s}");
            for e in doc.gallery.load_errors() {
                eprintln!("  {e}");
            }
            return ExitCode::from(2);
        }
        doc.gallery.select(s, v);
    }
    if let Some(out) = &args.shot {
        if args.inspect.is_some() && args.pick.is_none() {
            eprintln!("skelter-gallery-app: --inspect needs --pick\n{USAGE}");
            return ExitCode::from(2);
        }
        return match shot(doc, fonts, args.size.unwrap_or((1500.0, 950.0)), args.pick.map(|p| (p, args.inspect.clone())), out) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("skelter-gallery-app: {e}");
                ExitCode::from(1)
            }
        };
    }
    doc.gallery.watch = true;
    if let Some(l) = doc.gallery.live()
        && let Err(e) = l.watch()
    {
        eprintln!("skelter-gallery-app: watch: {e}");
    }
    let glyphs: Arc<dyn skelter_base::text::GlyphRasterizer> = Arc::new(skelter_text::FontRasterizer::new(fonts));
    let spawn = skelter::compositor(|clock| skelter_compositor::CompositorConfig::new(clock, Span::from_micros(16_667)), glyphs);
    match skelter_platform::run(Box::new(GalleryApp::new(doc, spawn))) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("skelter-gallery-app: {e}");
            ExitCode::from(1)
        }
    }
}

/// Renders the gallery's window headlessly (skelter-test's harness, the environment's GPU adapter)
/// to `out` as a PNG, `size` in points, with cell `pick` picked and, with a selector, inspected.
pub fn shot(doc: GalleryDocument, fonts: Arc<skelter_text::FontStore>, size: (f32, f32), pick: Option<(usize, Option<String>)>, out: &std::path::Path) -> Result<(), String> {
    let glyphs: Arc<dyn skelter_base::text::GlyphRasterizer> = Arc::new(skelter_text::FontRasterizer::new(fonts));
    let mut h = skelter_test::Harness::<GalleryDocument>::builder().with_size(Size::new(size.0, size.1)).with_glyphs(glyphs).build(doc);
    h.run();
    if let Some((i, selector)) = pick {
        let g = &mut h.state_mut().gallery;
        g.pick_cell(i);
        if let Some(sel) = selector {
            g.inspect_cell(i, &sel)?;
        }
        h.run();
    }
    let img = h.render().map_err(|e| e.0)?;
    write_png(&img, out)
}

/// A rendered frame as a PNG (un-premultiplied).
pub fn write_png(img: &skelter_test::Image, out: &std::path::Path) -> Result<(), String> {
    let mut rgba = img.rgba.clone();
    for p in rgba.chunks_exact_mut(4) {
        let a = p[3] as u32;
        if a > 0 && a < 255 {
            for c in &mut p[..3] {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    if let Some(d) = out.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    image::save_buffer(out, &rgba, img.width, img.height, image::ExtendedColorType::Rgba8).map_err(|e| format!("{}: {e}", out.display()))
}
