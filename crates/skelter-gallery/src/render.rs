//! The headless renderer: one QuickJS engine, one skelter document and one headless window
//! (skelter-test's `Harness` with the lavapipe renderer) for every cell. A cell change remounts the
//! variant into the window's slot; the engine, the module cache, the document and the GPU device
//! stay. Each variant file is its own embed entry (loaded once per run), so a file that doesn't
//! compile fails its cells only.

use crate::catalog::{Catalog, StoryError, discover_with};
use crate::ext::{Extensions, SetupCx};
use crate::model::{Cell, State, Story, Theme, Variant};
use skelter_base::event::WindowEvent;
use skelter_base::geom::{DeviceSize, Scale, Size};
use skelter_core::{MediaPreferences, Origin};
use skelter_test::{Harness, Image};
use skelter_widgets::theme::{Density, TokenMap};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use svelters_abi::{Backend, svl_handle};
use svelters_host::modules::{Entry, SourceConfig, SourceProvider};
use svelters_skelter::SkelterBackend;
use svelters_skelter::app::{SkelterDocument, install_widget_services};
use svelters_skelter::embed::{Budgets, Embed, EmbedConfig, EmbedEvent, EntryId, MountSpec, Props, RootId, RootState, SlotId, SlotOptions};

thread_local! {
    /// Numbers the spec fragments of the renderers on this thread.
    static FRAGMENTS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// The export name each variant's entry gives its component.
const EXPORT: &str = "Variant";
/// The slot's own style: it fills the window, on the theme's background.
const SLOT_STYLE: &str = "flex-grow: 1; background-color: var(--sv-bg); color: var(--sv-fg)";
/// Steps (vsyncs on the virtual clock) a cell may take to settle before it is drawn anyway.
const MAX_STEPS: u32 = 120;

/// Why a cell could not be rendered.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum Error {
    /// The stories could not be read.
    Stories(Vec<StoryError>),
    /// A bad request: an unknown story, variant, theme or arg.
    Request(String),
    /// The variant failed to load: a compile error or an exception while evaluating it.
    Load { file: PathBuf, message: String },
    /// The variant threw while mounting or rendering.
    Runtime { file: PathBuf, message: String },
    /// Setting up the engine or the document failed.
    Setup(String),
    /// No pixels: no GPU adapter, or the draw failed.
    Render(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Stories(errs) => {
                for (i, e) in errs.iter().enumerate() {
                    if i > 0 {
                        writeln!(f)?;
                    }
                    write!(f, "{e}")?;
                }
                Ok(())
            }
            Error::Request(m) | Error::Setup(m) | Error::Render(m) => f.write_str(m),
            Error::Load { file, message } if message.starts_with(&*file.to_string_lossy()) => write!(f, "does not load: {message}"),
            Error::Load { file, message } => write!(f, "{}: does not load: {message}", file.display()),
            Error::Runtime { file, message } => write!(f, "{}: {message}", file.display()),
        }
    }
}

impl std::error::Error for Error {}

/// A rendered cell.
#[derive(Clone, Debug)]
pub struct Shot {
    /// Premultiplied RGBA8, (story size × zoom) device pixels.
    pub image: Image,
    /// What the cell logged or reported without failing: console warnings and errors, handler
    /// exceptions, "not settled".
    pub warnings: Vec<String>,
    /// How the state was applied, for the cell's label: `forced :hover on 1 subject`,
    /// `disabled prop`, `forced :disabled (the variant has no disabled prop)`.
    pub applied: Option<String>,
}

/// What [`Renderer::reload`] did.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct Reload {
    /// A `story.toml` changed and the stories were read again.
    pub stories: bool,
    /// The `asset://` refs registered again.
    pub assets: Vec<String>,
    /// The variant files whose module graph had a changed file: they load again at their next mount.
    pub variants: Vec<PathBuf>,
    /// The JS runtime was replaced (a loaded variant was affected).
    pub restarted: bool,
}

/// What [`Renderer::mount`] left in the window.
#[derive(Clone, Debug)]
pub struct Mounted {
    pub warnings: Vec<String>,
    pub applied: Option<String>,
    /// The handles the state was forced on (the `subject` anchors, else the root's nodes).
    pub subjects: Vec<svl_handle>,
    /// Whether any node carries `anchor="subject"`.
    pub has_subject: bool,
    /// The nodes the root mounted under the slot (top level).
    pub top: Vec<svl_handle>,
    pub steps: u32,
    pub settled: bool,
}

impl Drop for Renderer {
    fn drop(&mut self) {
        if let Some(f) = &self.fragment {
            let _ = std::fs::remove_file(f);
        }
    }
}

/// The renderer (see the module docs). `!Send`: one per thread.
pub struct Renderer {
    catalog: Catalog,
    fonts: Arc<skelter_text::FontStore>,
    /// The host's kinds as a compiler spec fragment (a temporary file), when it has any.
    fragment: Option<PathBuf>,
    families: Vec<String>,
    roots: Vec<PathBuf>,
    setup: Vec<crate::ext::SetupHook>,
    themes: Vec<(String, TokenMap)>,
    backend: SkelterBackend,
    /// `None` only while [`Renderer::reload`] swaps it.
    embed: Option<Embed>,
    harness: Harness<SkelterDocument>,
    slot: SlotId,
    /// The `window` node the slot sits in.
    window_node: svl_handle,
    entries: HashMap<PathBuf, Result<EntryId, String>>,
    /// Each variant file's provider (its module graph and assets), loaded or not.
    providers: HashMap<PathBuf, Rc<SourceProvider>>,
    root: Option<RootId>,
    mounts: u64,
    /// The theme, density and sheet installed now.
    theme: Option<(Theme, Density, svelters_style::SheetId)>,
    base_css: String,
    /// (size in points, zoom) of the window now.
    window: (Size, f32),
    /// The cell mounted now.
    current: Option<Cell>,
}

impl Renderer {
    /// A renderer over `ext`'s story roots, kinds, setup hooks and token sets.
    pub fn new(ext: &Extensions) -> Result<Renderer, Error> {
        let catalog = discover_with(&ext.story_roots, &ext.families).map_err(Error::Stories)?;
        let fonts = crate::fonts::store(ext.fonts);
        crate::icons::register_demo();
        // the host's kinds after the built-ins (`Kinds::with_app`); their errors are strings
        let failed: RefCell<Option<String>> = RefCell::new(None);
        let kinds = svelters_skelter::Kinds::with_app(&svelters_skelter::surface::builtin(), |reg| {
            for f in &ext.register_kinds {
                if let Err(e) = f(reg) {
                    *failed.borrow_mut() = Some(e);
                    break;
                }
            }
            Ok(())
        })
        .map_err(|e| Error::Setup(format!("the kinds don't register: {e}")))?;
        if let Some(e) = failed.into_inner() {
            return Err(Error::Setup(format!("a host's kinds don't register: {e}")));
        }
        let kinds = Arc::new(kinds);
        // a host's kinds reach the compiler as a spec fragment (svelters_skelter::spec::fragment)
        let fragment = if ext.register_kinds.is_empty() {
            None
        } else {
            let dir = std::env::temp_dir().join(format!("skelter-gallery-{}", std::process::id()));
            let f = dir.join(format!("kinds-{}.json", FRAGMENTS.with(|n| n.replace(n.get() + 1))));
            std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&f, svelters_skelter::spec::fragment_text(&kinds))).map_err(|e| Error::Setup(format!("{}: {e}", f.display())))?;
            Some(f)
        };
        let backend = SkelterBackend::with_text(kinds, Box::new(skelter_text::TextSystem::new(fonts.clone())));
        install_widget_services(&backend, skelter_text::TextSystem::new(fonts.clone()));
        // the window: a `window` node the harness's window attaches, the cell's slot in it
        let window = backend.create_borrowed("window").map_err(Error::Setup)?;
        let (embed, slot) = attach_embed(&backend, window)?;
        let mut doc = SkelterDocument::new(backend.clone());
        doc.opening(window);
        let size = Size::new(320.0, 96.0);
        let harness = Harness::<SkelterDocument>::builder()
            .with_size(size)
            .with_glyphs(Arc::new(skelter_text::FontRasterizer::new(fonts.clone())))
            .with_accessibility_check(false)
            .with_max_steps(u64::from(MAX_STEPS))
            .build_unsettled(doc);
        if backend.window_id(window).is_none() {
            return Err(Error::Setup("the harness window did not attach the gallery's window node".into()));
        }
        let mut r = Renderer {
            catalog,
            fonts: fonts.clone(),
            fragment,
            families: ext.families.clone(),
            roots: ext.story_roots.clone(),
            setup: ext.setup.clone(),
            themes: ext.themes.clone(),
            backend,
            embed: Some(embed),
            harness,
            slot,
            window_node: window,
            entries: HashMap::new(),
            providers: HashMap::new(),
            root: None,
            mounts: 0,
            theme: None,
            base_css: skelter_widgets::theme::BASE_CSS.to_string(),
            window: (size, 1.0),
            current: None,
        };
        r.apply_theme(&Theme::DarkModern, Density::Default)?;
        let _ = r.settle();
        Ok(r)
    }

    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    /// Reads the story roots again (after an edit to a story.toml). Loaded variant files stay
    /// cached for this renderer's life: an edited `.svelte` file needs a new renderer.
    pub fn rediscover(&mut self) -> Result<(), Vec<StoryError>> {
        self.catalog = discover_with(&self.roots, &self.families)?;
        Ok(())
    }

    /// Takes edits on disk into account without a new renderer (the app's live loop, or an agent's
    /// long-running renderer): `paths` are the files that changed.
    ///
    /// - a `story.toml` re-reads the stories ([`Renderer::rediscover`]);
    /// - an image file is registered again under its `asset://` ref;
    /// - a variant file that failed to load is retried at its next mount;
    /// - a file in the module graph of a loaded variant (the variant, a component or module it
    ///   imports, a manifest) replaces the JS runtime: a fresh engine on the same document, window
    ///   and GPU device, and every variant loads again at its next mount. JS state is not kept
    ///   (each cell mounts fresh anyway).
    ///
    /// The mounted cell is unmounted when the runtime is replaced; the next `shot`, `sheet` or
    /// `mount` renders from the new files. A path the renderer never loaded changes nothing.
    pub fn reload(&mut self, paths: &[PathBuf]) -> Result<Reload, Error> {
        let paths: Vec<PathBuf> = paths.iter().map(|p| crate::model::canonicalize(p).unwrap_or_else(|_| p.clone())).collect();
        let mut out = Reload::default();
        if paths.iter().any(|p| p.file_name().is_some_and(|n| n == "story.toml")) {
            self.rediscover().map_err(Error::Stories)?;
            out.stories = true;
        }
        // images: registered again for every provider that serves them (an asset's importers don't re-run)
        let mut code: Vec<&PathBuf> = paths.iter().collect();
        for p in &paths {
            let image = p.extension().and_then(|e| e.to_str()).is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "svg" | "png"));
            if !image {
                continue;
            }
            if let Some(provider) = self.providers.values().find(|pr| pr.assets().serves(p))
                && let Some(dir) = p.parent()
            {
                code.retain(|c| *c != p);
                let (ok, _) = crate::icons::register_assets(provider.assets(), dir);
                for r in ok {
                    if !out.assets.contains(&r) {
                        out.assets.push(r);
                    }
                }
            }
        }
        // entries: a failed one is retried; a loaded one whose graph has a changed file restarts JS
        let mut restart = false;
        for (file, provider) in &self.providers {
            let in_graph = code.contains(&file) || {
                let watched = svelters_host::ModuleProvider::watch_files(provider.as_ref());
                code.iter().any(|p| watched.contains(p))
            };
            if !in_graph {
                continue;
            }
            out.variants.push(file.clone());
            if matches!(self.entries.get(file), Some(Ok(_))) {
                restart = true;
            }
        }
        out.variants.sort();
        if restart {
            self.restart()?;
            out.restarted = true;
        } else {
            for f in &out.variants {
                self.entries.remove(f);
                self.providers.remove(f);
            }
        }
        Ok(out)
    }

    /// Replaces the JS runtime (see [`Renderer::reload`]).
    fn restart(&mut self) -> Result<(), Error> {
        self.drain();
        if let Some(r) = self.root.take() {
            self.embed().unmount(r);
        }
        self.embed().destroy_slot(self.slot);
        self.embed().logic();
        let _ = self.settle();
        self.current = None;
        // the old host detaches from the backend when dropped: drop it before attaching the new one
        self.embed = None;
        self.entries.clear();
        self.providers.clear();
        let (embed, slot) = attach_embed(&self.backend, self.window_node)?;
        self.embed = Some(embed);
        self.slot = slot;
        // the window's size and scale are the harness's; the new slot takes them at the next mount
        self.window = (Size::new(0.0, 0.0), 0.0);
        let _ = self.settle();
        Ok(())
    }

    /// The base sheet the kinds' default look comes from (skelter-widgets' `base.css` as built, by
    /// default; the CLI reads the source file when it finds it, so an edit shows without a rebuild).
    pub fn set_base_css(&mut self, css: impl Into<String>) -> Result<(), Error> {
        self.base_css = css.into();
        if let Some((t, d, _)) = self.theme.clone() {
            self.theme_force(&t, d)?;
        }
        Ok(())
    }

    /// The fonts every cell draws with ([`crate::fonts`]; hermetic unless `--system-fonts`).
    pub fn fonts(&self) -> &Arc<skelter_text::FontStore> {
        &self.fonts
    }

    pub fn backend(&self) -> &SkelterBackend {
        &self.backend
    }

    pub fn embed(&self) -> &Embed {
        self.embed.as_ref().expect("the embed is swapped only inside Renderer::reload")
    }

    pub fn harness(&mut self) -> &mut Harness<SkelterDocument> {
        &mut self.harness
    }

    /// The cell mounted now.
    pub fn current(&self) -> Option<&Cell> {
        self.current.as_ref()
    }

    /// The slot's node (the mounted root's parent).
    pub fn slot_node(&self) -> Option<skelter_base::ids::NodeId> {
        self.embed().slot_node(self.slot)
    }

    /// The theme a cell's theme name resolves to.
    pub fn resolve_theme(&self, theme: &Theme, density: Density) -> Result<skelter_widgets::theme::Theme, Error> {
        use skelter_widgets::theme::Theme as T;
        let base = match theme.recipe() {
            Some((scheme, contrast)) => T::vscode(scheme, contrast),
            None => {
                let (_, tokens) = self.themes.iter().find(|(n, _)| n == theme.name()).ok_or_else(|| Error::Request(format!("no theme {theme}")))?;
                T::dark_modern().with(tokens.iter())
            }
        };
        Ok(base.with_density(density))
    }

    fn apply_theme(&mut self, theme: &Theme, density: Density) -> Result<(), Error> {
        if self.theme.as_ref().is_some_and(|(t, d, _)| t == theme && *d == density) {
            return Ok(());
        }
        self.theme_force(theme, density)
    }

    fn theme_force(&mut self, theme: &Theme, density: Density) -> Result<(), Error> {
        let t = self.resolve_theme(theme, density)?;
        let css = format!("{}\n{}", t.css(), self.base_css);
        let old = self.theme.take().map(|(_, _, s)| s);
        let sheet = self.backend.with_doc(|d| {
            if let Some(s) = old {
                d.remove_sheet(s);
            }
            let s = d.add_sheet_with_origin(&css, Origin::Theme);
            d.set_sheet_info(s, Some(format!("gallery theme {} + base.css", t.name())), None);
            // reduced motion: transitions jump, so a cell settles at its end state
            d.set_media_preferences(MediaPreferences::new(true, t.contrast().media()));
            skelter_widgets::host::set_theme(d, t.clone());
            s
        });
        self.theme = Some((theme.clone(), density, sheet));
        Ok(())
    }

    /// `cell`'s story and variant.
    pub fn lookup(&self, cell: &Cell) -> Result<(Story, Variant), Error> {
        let story = self.catalog.get(cell.story.as_str()).ok_or_else(|| Error::Request(format!("no story {}", cell.story)))?;
        let variant = story.variant(cell.variant.as_str()).ok_or_else(|| Error::Request(format!("story {} has no variant {}", cell.story, cell.variant)))?;
        Ok((story.clone(), variant.clone()))
    }

    fn entry(&mut self, story: &Story, variant: &Variant) -> Result<EntryId, Error> {
        let file = story.path(&variant.file);
        if let Some(r) = self.entries.get(&file) {
            return r.clone().map_err(|message| Error::Load { file, message });
        }
        let r = self.load_entry(story, &file);
        self.entries.insert(file.clone(), r.clone());
        r.map_err(|message| Error::Load { file, message })
    }

    fn load_entry(&mut self, story: &Story, file: &Path) -> Result<EntryId, String> {
        let mut cfg = SourceConfig::for_path(&story.root)?;
        cfg.entry = Entry::Embed(vec![(EXPORT.into(), file.to_path_buf())]);
        cfg.backend = "skelter".into();
        cfg.dev = true;
        cfg.spec_fragments.extend(self.fragment.clone());
        let provider = Rc::new(SourceProvider::new(cfg)?);
        // the story's images under the `asset://` refs its imports evaluate to
        if let Some(dir) = file.parent() {
            crate::icons::register_assets(provider.assets(), dir);
        }
        self.providers.insert(file.to_path_buf(), provider.clone());
        self.embed().load_from(provider.clone()).map_err(|e| {
            // the engine shortens module-load messages: the compiler's own errors read in full
            let c = provider.check(file);
            let errors: Vec<String> = c
                .diagnostics
                .iter()
                .filter(|d| d.severity == svelters_host::modules::Severity::Error)
                .map(|d| format!("{}:{}:{}: {} [{}]", file.display(), d.line, d.column, d.message, d.code))
                .collect();
            if errors.is_empty() { c.error.unwrap_or(e.message) } else { errors.join("\n") }
        })
    }

    /// Mounts `cell` (its variant with its args, theme, density and zoom), forces its state and
    /// runs to settled on the virtual clock. Does not draw.
    pub fn mount(&mut self, cell: &Cell) -> Result<Mounted, Error> {
        let (story, variant) = self.lookup(cell)?;
        let file = story.path(&variant.file);
        for k in cell.args.keys() {
            if !variant.args.contains_key(k) && !variant.props.contains(k) {
                return Err(Error::Request(format!("variant {}/{} has no arg or prop {k}", story.id, variant.id)));
            }
        }
        let entry = self.entry(&story, &variant)?;
        self.drain();
        if let Some(r) = self.root.take() {
            self.embed().unmount(r);
            self.embed().logic();
        }
        self.current = None;
        self.apply_theme(&cell.theme, cell.density)?;
        // the window: the story's size in points at the cell's zoom
        let size = Size::new(story.size[0] as f32, story.size[1] as f32);
        if self.window != (size, cell.zoom) {
            let device = DeviceSize { w: (size.w * cell.zoom).round() as u32, h: (size.h * cell.zoom).round() as u32 };
            self.harness.window_event(WindowEvent::Resized { size: device, scale: Scale(cell.zoom) });
            self.window = (size, cell.zoom);
        }
        let wid = self.harness.window();
        self.embed().set_slot_layout(self.slot, wid, [size.w, size.h], cell.zoom);
        for f in self.setup.clone() {
            f(&mut SetupCx { cell, story: &story, backend: &self.backend });
        }
        let mut props = Props::json(serde_json::Value::Object(cell.args.clone().into_iter().collect()));
        let mut applied = None;
        let mut force_disabled = false;
        if cell.state == State::Disabled {
            if variant.takes_disabled() {
                props.set("disabled", true);
                applied = Some("disabled prop".to_string());
            } else {
                force_disabled = true;
            }
        }
        self.mounts += 1;
        let key = format!("gallery.{}", self.mounts);
        let root = self.embed().mount(MountSpec::new(entry, EXPORT, self.slot, key).props(props)).map_err(|e| Error::Setup(e.to_string()))?;
        self.root = Some(root);
        let (mut steps, mut settled) = self.settle();
        let mut warnings = self.drain();
        if let Some(info) = self.embed().root_info(root)
            && let RootState::Faulted(e) = info.state
        {
            return Err(Error::Runtime { file, message: e.error.message });
        }
        let top: Vec<svl_handle> = self.top_level();
        let anchored: Vec<svl_handle> = self.backend.scene().read(|s| s.nodes().filter(|n| n.anchor().as_deref() == Some("subject")).map(|n| n.id()).collect());
        let has_subject = !anchored.is_empty();
        let subjects = if has_subject { anchored } else { top.clone() };
        let pseudo = match cell.state {
            State::Disabled if !force_disabled => None,
            s => s.pseudo_class(),
        };
        if let Some(p) = pseudo {
            let bits = self.backend.pseudo_state(p).ok_or_else(|| Error::Request(format!("the style engine has no state bit for :{p}")))?;
            for &h in &subjects {
                self.backend.force_state(h, bits).map_err(Error::Setup)?;
            }
            let on = if has_subject { format!("{} subject{}", subjects.len(), if subjects.len() == 1 { "" } else { "s" }) } else { "the root (no subject anchor)".to_string() };
            applied = Some(if force_disabled { format!("forced :disabled on {on} (the variant has no disabled prop)") } else { format!("forced :{p} on {on}") });
            let (s2, ok) = self.settle();
            steps += s2;
            settled = ok;
            warnings.extend(self.drain());
        }
        if !settled {
            warnings.push(format!("not settled after {steps} steps: {}", self.busy().unwrap_or_default()));
        }
        self.current = Some(cell.clone());
        Ok(Mounted { warnings, applied, subjects, has_subject, top, steps, settled })
    }

    /// The handles of the nodes the root mounted under the slot.
    pub fn top_level(&self) -> Vec<svl_handle> {
        let Some(slot) = self.embed().slot_node(self.slot) else { return Vec::new() };
        let kids = self.backend.with_doc(|d| d.children(slot).to_vec());
        kids.into_iter().filter_map(|n| self.backend.handle(n)).collect()
    }

    /// Mounts `cell` and draws it.
    pub fn shot(&mut self, cell: &Cell) -> Result<Shot, Error> {
        let m = self.mount(cell)?;
        let image = self.harness.render().map_err(|e| Error::Render(e.to_string()))?;
        Ok(Shot { image, warnings: m.warnings, applied: m.applied })
    }

    /// Why the embed or the harness is not settled.
    fn busy(&self) -> Option<String> {
        let s = self.embed().settled();
        if !s.is_settled() {
            return Some(format!("embed busy: {:?}", s.busy));
        }
        self.harness.busy()
    }

    /// Steps until the embed and the harness are settled, at most [`MAX_STEPS`]: (steps, settled).
    fn settle(&mut self) -> (u32, bool) {
        for i in 1..=MAX_STEPS {
            self.embed().logic();
            self.harness.step();
            if self.busy().is_none() {
                return (i, true);
            }
        }
        (MAX_STEPS, false)
    }

    /// What JS reported since the last call: errors, handler exceptions, console warnings.
    fn drain(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        for ev in self.embed().take_events() {
            match ev {
                EmbedEvent::Error { error, .. } => out.push(format!("error: {}", error.message)),
                EmbedEvent::Faulted { error, .. } => out.push(format!("faulted: {}", error.error.message)),
                EmbedEvent::RuntimeFaulted(e) => out.push(format!("runtime faulted: {}", e.message)),
                _ => {}
            }
        }
        for e in self.embed().host().take_errors() {
            out.push(format!("error: {}", e.message));
        }
        for l in self.embed().host().take_console() {
            if l.level == "warn" || l.level == "error" {
                out.push(format!("console.{}: {}", l.level, l.text));
            }
        }
        out
    }
}

/// A JS runtime for `backend` and the cell's slot in `window`, filling it on the theme's background.
fn attach_embed(backend: &SkelterBackend, window: svl_handle) -> Result<(Embed, SlotId), Error> {
    let host = svelters_host::HostConfig { echo_console: false, echo_errors: false, quit_on_last_window: false, locale: Some("en-US".into()), ..svelters_host::HostConfig::default() };
    let budgets = Budgets { warn: Duration::from_secs(1), interrupt: Duration::from_secs(5), mount: Duration::from_secs(10) };
    let embed = Embed::with_backend(backend.clone(), EmbedConfig { host, budgets, ..EmbedConfig::default() }).map_err(|e| Error::Setup(e.to_string()))?;
    let slot = embed.create_slot("gallery.cell", SlotOptions::default().label("cell"));
    backend.insert(window, slot.handle(), None, 0).map_err(|e| Error::Setup(format!("{e:?}")))?;
    // the cell fills the window on the theme's background (the window kind paints none headless)
    if let Some(n) = embed.slot_node(slot) {
        backend.with_doc(|d| d.set_style(n, skelter_core::StyleInput { inline: Some(SLOT_STYLE.into()), ..Default::default() }));
    }
    Ok((embed, slot))
}

/// The state bits pseudo-class `name` tests (`hover`, `focus-visible`, ...).
pub fn pseudo_bits(name: &str) -> Option<svelters_style::StateMask> {
    svelters_skelter::style::pseudo_state(name)
}

/// `image` as PNG bytes (straight alpha).
pub fn png(image: &Image) -> Vec<u8> {
    let rgba = crate::diff::straight(image).rgba;
    let mut out = Vec::new();
    let enc = image::codecs::png::PngEncoder::new(&mut out);
    image::ImageEncoder::write_image(enc, &rgba, image.width, image.height, image::ExtendedColorType::Rgba8).expect("encoding to memory");
    out
}

/// Writes `image` to `path` as PNG, creating its directory.
pub fn save_png(image: &Image, path: &Path) -> std::io::Result<()> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    std::fs::write(path, png(image))
}
