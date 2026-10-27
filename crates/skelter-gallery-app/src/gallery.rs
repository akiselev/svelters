//! [`Gallery`]: the interactive gallery's state and its window, built from Rust on skelter.
//!
//! The chrome is Rust on skelter, not Svelte: each preview cell must be a Svelte root of its own in a
//! host-owned slot (so a fault, a reload and a theme stay per cell), and slots are host nodes the host
//! places in its own tree; the source pane is quire's `code-editor`, a Rust kind. Rust owns the
//! frame around the roots, as cadatomic's shell does.
//!
//! Layout: a controls bar on top (themes, states, density, zoom, args, inspect, copy), the sidebar on
//! the left (filter, families → stories → variants, kinds with no story), the canvas in the middle
//! (rows of variant × args, columns of state × theme, each cell a themed root), and on the right the
//! source pane (the selected variant's file in quire) over the inspect pane.

use crate::live::{CellMount, LiveStory, LiveVariant, SourceError};
use crate::theme::{self, Themes};
use crate::ui::{Ui, w};
use quire_core::EditorConfig;
use skelter_base::geom::Point;
use skelter_base::ids::{NodeId, WindowId};
use skelter_core::{Document, JsEvent, StateMask, Value};
use skelter_gallery::{Axes, Catalog, Cell, Extensions, Matrix, State, Story, Theme as GTheme};
use skelter_quire::kind::QuireHost;
use skelter_widgets::theme::Density;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use svelters_skelter::kinds::Kinds;

/// What the user picked on the axes.
#[derive(Clone, Debug)]
pub struct Settings {
    pub themes: Vec<String>,
    /// `None`: the story's own states.
    pub states: Option<Vec<State>>,
    pub density: Density,
    pub zoom: f32,
    /// Fixed args (`None` for a key: every value).
    pub args: BTreeMap<String, serde_json::Value>,
    /// Clicks in a cell pick the node under the pointer instead of reaching it.
    pub inspect: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { themes: theme::BUILTIN.iter().map(|s| s.to_string()).collect(), states: None, density: Density::Default, zoom: 1.0, args: BTreeMap::new(), inspect: false }
    }
}

pub const ZOOMS: [f32; 5] = [1.0, 1.25, 1.5, 2.0, 0.75];

/// What a chrome node does when it raises its signal.
#[derive(Clone, Debug, PartialEq)]
enum Action {
    Story(String),
    Variant(String, String),
    Theme(String),
    State(State),
    Density,
    Zoom,
    Arg(String, Vec<serde_json::Value>),
    Inspect,
    Filter,
    Save,
    Revert,
    CopyShot,
    CopyInspect,
    Cell(usize),
}

/// The chrome's fixed nodes.
struct Nodes {
    root: NodeId,
    bar: NodeId,
    side: NodeId,
    filter: NodeId,
    list: NodeId,
    cells: NodeId,
    title: NodeId,
    file: NodeId,
    errors: NodeId,
    editor: NodeId,
    inspect: NodeId,
}

/// One cell on the canvas.
struct CellView {
    cell: Cell,
    frame: NodeId,
    error: NodeId,
    /// The frame's classes and inline style (its theme, its size).
    classes: Vec<String>,
    inline: String,
}

/// The source pane's buffer.
struct Buffer {
    file: PathBuf,
    doc: Rc<RefCell<quire_core::Document>>,
    /// The buffer's version when it last matched the disk.
    saved: quire_core::Version,
    /// The version last previewed.
    previewed: quire_core::Version,
    name: String,
}

/// What the inspect pane shows.
#[derive(Clone, Debug, Default)]
pub struct Inspected {
    pub cell: Option<Cell>,
    pub selector: Option<String>,
    pub json: Option<serde_json::Value>,
    pub command: Option<String>,
}

/// Inspects a node of a mounted cell: `(document, the cell's frame, the picked node, the cell,
/// the selector)` → the JSON `skelter-gallery inspect` prints.
pub type InspectFn = Box<dyn Fn(&Document, NodeId, NodeId, &Cell, &str) -> serde_json::Value>;

/// The gallery's state and window. See the module docs.
pub struct Gallery {
    doc: Rc<RefCell<Document>>,
    kinds: Arc<Kinds>,
    ext: Extensions,
    catalog: Catalog,
    load_errors: Vec<String>,
    coverage_missing: Vec<String>,
    themes: Themes,
    pub settings: Settings,
    filter: String,
    story: Option<String>,
    variant: Option<String>,
    live: Option<LiveStory>,
    generation: u64,
    nodes: Nodes,
    actions: HashMap<NodeId, Action>,
    cells: Vec<CellView>,
    picked: Option<usize>,
    buffer: Option<Buffer>,
    buffers: u64,
    pub inspected: Inspected,
    inspect_fn: Option<InspectFn>,
    cell_sheet: Option<svelters_style::SheetId>,
    clipboard: Option<Box<dyn skelter_platform::Clipboard>>,
    /// The last text put on the clipboard (tests read it; the clipboard may refuse).
    pub copied: Option<String>,
    /// A frame is needed (the chrome or the cells changed outside input).
    pub dirty: bool,
    /// The watcher's wake (the platform's waker), handed to each story's backend.
    wake: Option<Arc<dyn Fn() + Send + Sync>>,
    /// Start each story's file watcher (the app; tests drive changes themselves).
    pub watch: bool,
    /// The events each story's backend delivered at event time (its tap), for the chrome.
    tapped: Rc<RefCell<Vec<JsEvent>>>,
    /// skelter-widgets' `base.css`, read from its source file when there is one (edits show live).
    base: BaseCss,
    /// The host's kinds as a compiler spec fragment (a temporary file), when it has any.
    fragment: Option<PathBuf>,
}

impl Drop for Gallery {
    fn drop(&mut self) {
        if let Some(f) = &self.fragment {
            let _ = std::fs::remove_file(f);
        }
    }
}

/// The stock kinds' sheet: its source file, the text in use, and the theme sheet carrying it.
#[derive(Default)]
struct BaseCss {
    path: Option<PathBuf>,
    /// The file's (modified time, length) when last read.
    seen: Option<(std::time::SystemTime, u64)>,
    text: String,
    sheet: Option<svelters_style::SheetId>,
}

/// skelter-widgets' `base.css` source when the gallery runs inside the svelters checkout (as
/// `skelter-gallery` finds it): edits to it show without a rebuild.
pub fn base_css_source(roots: &[PathBuf]) -> Option<PathBuf> {
    roots.iter().filter_map(|r| r.canonicalize().ok()).find_map(|r| r.ancestors().map(|d| d.join("crates/skelter-widgets/src/theme/base.css")).find(|p| p.is_file()))
}

fn stamp(p: &Path) -> Option<(std::time::SystemTime, u64)> {
    let m = std::fs::metadata(p).ok()?;
    Some((m.modified().ok()?, m.len()))
}

/// The kinds the gallery's document registers and JS sees: svelters' built-ins (what the app runner
/// registers), the host's (`Kinds::with_app`, as `skelter_gallery::Renderer` builds them), and
/// quire's `code-editor` for the source pane.
pub fn kinds(ext: &Extensions) -> Result<Arc<Kinds>, String> {
    let failed: RefCell<Option<String>> = RefCell::new(None);
    let kinds = Kinds::with_app(&svelters_skelter::surface::builtin(), |reg| {
        for f in &ext.register_kinds {
            if let Err(e) = f(reg) {
                *failed.borrow_mut() = Some(e);
                break;
            }
        }
        if reg.by_name("code-editor").is_none() {
            reg.register::<skelter_quire::kind::CodeEditorKind>()?;
        }
        Ok(())
    })
    .map_err(|e| format!("the kinds don't register: {e:?}"))?;
    if let Some(e) = failed.into_inner() {
        return Err(format!("a host's kinds don't register: {e}"));
    }
    Ok(Arc::new(kinds))
}

/// The gallery's own kind: the source pane's editor, which no story needs to cover.
const CHROME_KINDS: &[&str] = &["code-editor"];

/// Every story that parses under `roots`, and the errors of the rest: one bad `story.toml` hides
/// only itself (`skelter_gallery::discover` refuses the whole catalog, as the CLI's `check` wants).
pub fn discover(roots: &[PathBuf], families: &[String]) -> (Catalog, Vec<String>) {
    match skelter_gallery::discover_with(roots, families) {
        Ok(c) => (c, Vec::new()),
        Err(errs) => {
            let mut stories = Vec::new();
            for root in roots.iter().filter_map(|r| r.canonicalize().ok()) {
                let mut manifests: Vec<PathBuf> = std::fs::read_dir(root.join("stories"))
                    .into_iter()
                    .flatten()
                    .flatten()
                    .flat_map(|fam| std::fs::read_dir(fam.path()).into_iter().flatten().flatten())
                    .map(|id| id.path().join("story.toml"))
                    .filter(|m| m.is_file())
                    .collect();
                manifests.sort();
                for m in manifests {
                    if let Ok(s) = skelter_gallery::catalog::parse_story(&root, &m, families) {
                        stories.push(s);
                    }
                }
            }
            let shown = |e: &skelter_gallery::StoryError| {
                let mut t = e.to_string();
                for r in roots.iter().filter_map(|r| r.canonicalize().ok()) {
                    t = t.replace(&format!("{}/", r.display()), "");
                }
                t
            };
            (Catalog::from_stories(stories), errs.iter().map(shown).collect())
        }
    }
}

/// The gallery roots: the host's, else this repository's `gallery/` (from the working directory
/// up, then next to this crate).
pub fn story_roots(ext: &Extensions) -> Vec<PathBuf> {
    if !ext.story_roots.is_empty() {
        return ext.story_roots.clone();
    }
    let here = std::env::current_dir().ok();
    let found = here.as_deref().and_then(|d| d.ancestors().map(|a| a.join("gallery")).find(|g| g.join("package.json").is_file()));
    let fallback = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../gallery");
    match found {
        Some(g) => vec![g],
        None if fallback.join("package.json").is_file() => vec![fallback.canonicalize().unwrap_or(fallback)],
        None => Vec::new(),
    }
}

impl Gallery {
    /// The gallery over `doc` (built on [`registry`]), its window's root built and the stories found.
    pub fn new(doc: Rc<RefCell<Document>>, kinds: Arc<Kinds>, fonts: Arc<skelter_text::FontStore>, mut ext: Extensions) -> Gallery {
        let roots = story_roots(&ext);
        // host hooks and the CLI commands it builds name the same roots
        ext.story_roots = roots.clone();
        let (catalog, load_errors) = discover(&roots, &ext.families);
        let mut coverage_missing = catalog.coverage(doc.borrow().registry()).missing;
        coverage_missing.retain(|k| !CHROME_KINDS.contains(&k.as_str()));
        let themes = Themes { extra: ext.themes.clone() };
        let base_path = base_css_source(&roots);
        let nodes = {
            let mut d = doc.borrow_mut();
            // the chrome's classes as author rules (the theme sheet is installed below)
            d.add_sheet(theme::CHROME_CSS);
            let mut q = QuireHost::new(fonts);
            q.theme = skelter_quire::EditorTheme::default();
            d.set_service(q);
            build(&mut Ui::new(&mut d))
        };
        let mut g = Gallery {
            doc,
            kinds,
            ext,
            catalog,
            load_errors,
            coverage_missing,
            themes,
            settings: Settings::default(),
            filter: String::new(),
            story: None,
            variant: None,
            live: None,
            generation: 0,
            nodes,
            actions: HashMap::new(),
            cells: Vec::new(),
            picked: None,
            buffer: None,
            buffers: 0,
            inspected: Inspected::default(),
            inspect_fn: None,
            cell_sheet: None,
            clipboard: None,
            copied: None,
            dirty: true,
            wake: None,
            watch: false,
            tapped: Rc::default(),
            base: BaseCss { path: base_path, ..BaseCss::default() },
            fragment: None,
        };
        if !g.ext.register_kinds.is_empty() {
            let dir = std::env::temp_dir().join(format!("skelter-gallery-app-{}", std::process::id()));
            static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let f = dir.join(format!("kinds-{}.json", N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
            match std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&f, svelters_skelter::spec::fragment_text(&g.kinds))) {
                Ok(()) => g.fragment = Some(f),
                Err(e) => g.load_errors.push(format!("{}: {e}", f.display())),
            }
        }
        g.load_base_css();
        g.wire();
        g.install_cell_sheet();
        g.rebuild_bar();
        g.rebuild_sidebar();
        g.rebuild_inspect();
        g
    }

    pub fn root(&self) -> NodeId {
        self.nodes.root
    }

    pub fn document(&self) -> &Rc<RefCell<Document>> {
        &self.doc
    }

    /// Why stories did not load (each names its file).
    pub fn load_errors(&self) -> &[String] {
        &self.load_errors
    }

    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    pub fn live(&self) -> Option<&LiveStory> {
        self.live.as_ref()
    }

    /// The selected story and variant.
    pub fn selection(&self) -> (Option<&str>, Option<&str>) {
        (self.story.as_deref(), self.variant.as_deref())
    }

    /// The cells on the canvas, in order.
    pub fn cells(&self) -> impl Iterator<Item = (&Cell, NodeId)> {
        self.cells.iter().map(|c| (&c.cell, c.frame))
    }

    /// The source pane's file and text.
    pub fn source(&self) -> Option<(PathBuf, String)> {
        self.buffer.as_ref().map(|b| (b.file.clone(), b.doc.borrow().text().to_string()))
    }

    /// The source pane's quire document (tests type into it as the editor would).
    pub fn source_document(&self) -> Option<Rc<RefCell<quire_core::Document>>> {
        self.buffer.as_ref().map(|b| b.doc.clone())
    }

    pub fn set_inspector(&mut self, f: InspectFn) {
        self.inspect_fn = Some(f);
    }

    pub fn set_clipboard(&mut self, c: Box<dyn skelter_platform::Clipboard>) {
        self.clipboard = Some(c);
    }

    /// The loop's waker: the story's file watcher and the `base.css` watch wake it.
    pub fn set_wake(&mut self, wake: Arc<dyn Fn() + Send + Sync>) {
        if let Some(l) = &self.live {
            l.backend().wake_state().set_notify(Some(wake.clone()));
        }
        if let Some(p) = self.base.path.clone() {
            // base.css is not a module of any story, so the host's watcher does not see it: a
            // small thread stats it and wakes the loop, which reinstalls it (`poll_base_css`)
            let w = wake.clone();
            let _ = std::thread::Builder::new().name("gallery-base-css".into()).spawn(move || {
                let mut seen = stamp(&p);
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(250));
                    let now = stamp(&p);
                    if now != seen {
                        seen = now;
                        w();
                    }
                }
            });
        }
        self.wake = Some(wake);
    }

    /// The chrome's share of what the last dispatch delivered at event time.
    pub fn take_tapped(&self) -> Vec<JsEvent> {
        std::mem::take(&mut *self.tapped.borrow_mut())
    }

    /// The backend of the selected story (input goes through it so JS gets its events).
    pub fn backend(&self) -> Option<&svelters_skelter::SkelterBackend> {
        self.live.as_ref().map(|l| l.backend())
    }

    // ---- the chrome ----

    /// The file `base.css` is read from (`None`: the copy built into skelter-widgets).
    pub fn base_css_path(&self) -> Option<&Path> {
        self.base.path.as_deref()
    }

    /// Reads `base.css` from `path` from now on (`None`: the built-in copy).
    pub fn set_base_css_path(&mut self, path: Option<PathBuf>) {
        self.base.path = path;
        self.base.seen = None;
        self.load_base_css();
    }

    /// Reads `base.css` (its file, else the built-in copy) and installs the theme sheet: the
    /// chrome's tokens and base.css at the theme origin, as skelter_gallery's renderer installs
    /// them (a story's own rules win over them).
    fn load_base_css(&mut self) {
        let text = match &self.base.path {
            Some(p) => match std::fs::read_to_string(p) {
                Ok(t) => t,
                Err(e) => {
                    self.load_errors.push(format!("{}: {e}", p.display()));
                    skelter_widgets::theme::BASE_CSS.to_string()
                }
            },
            None => skelter_widgets::theme::BASE_CSS.to_string(),
        };
        self.base.seen = self.base.path.as_deref().and_then(stamp);
        if self.base.sheet.is_some() && text == self.base.text {
            return;
        }
        self.base.text = text;
        let chrome = theme::builtin(theme::CHROME, Density::Default).expect("the chrome theme");
        let css = format!("{}\n{}", chrome.css(), self.base.text);
        let mut d = self.doc.borrow_mut();
        if let Some(old) = self.base.sheet.take() {
            d.remove_sheet(old);
        }
        self.base.sheet = Some(d.add_sheet_with_origin(&css, svelters_style::Origin::Theme));
        drop(d);
        self.dirty = true;
    }

    /// `base.css` changed on disk: install it again. Returns whether it did.
    pub fn poll_base_css(&mut self) -> bool {
        let Some(p) = &self.base.path else { return false };
        if stamp(p) == self.base.seen {
            return false;
        }
        let before = self.base.sheet;
        self.load_base_css();
        self.base.sheet != before
    }

    fn install_cell_sheet(&mut self) {
        let css = self.themes.cell_sheet(self.settings.density);
        let mut d = self.doc.borrow_mut();
        if let Some(old) = self.cell_sheet.take() {
            d.remove_sheet(old);
        }
        self.cell_sheet = Some(d.add_sheet_with_origin(&css, svelters_style::Origin::Theme));
    }

    fn act(&mut self, n: NodeId, a: Action) -> NodeId {
        self.actions.insert(n, a);
        n
    }

    fn forget(&mut self, under: NodeId) {
        let d = self.doc.borrow();
        let mut gone = Vec::new();
        for n in self.actions.keys() {
            let mut x = Some(*n);
            while let Some(y) = x {
                if y == under {
                    gone.push(*n);
                    break;
                }
                x = d.parent(y);
            }
            if !d.contains(*n) {
                gone.push(*n);
            }
        }
        drop(d);
        for n in gone {
            self.actions.remove(&n);
        }
    }

    fn rebuild_bar(&mut self) {
        let bar = self.nodes.bar;
        self.forget(bar);
        let story = self.current_story().cloned();
        let names = self.themes.names();
        let mut made = Vec::new();
        {
            let mut d = self.doc.borrow_mut();
            let mut ui = Ui::new(&mut d);
            ui.clear(bar);
            ui.label(bar, "Themes", &["gal-heading"]);
            for t in &names {
                let n = ui.checkbox(bar, t, self.settings.themes.contains(t));
                ui.anchor(n, &format!("theme:{t}"));
                made.push((n, Action::Theme(t.clone())));
            }
            if let Some(s) = &story {
                ui.label(bar, "States", &["gal-heading"]);
                for st in &s.states {
                    let on = self.settings.states.as_ref().is_none_or(|v| v.contains(st));
                    let n = ui.checkbox(bar, st.name(), on);
                    ui.anchor(n, &format!("state:{st}"));
                    made.push((n, Action::State(*st)));
                }
            }
            let densities: Vec<String> = Density::ALL.iter().map(|d| d.name().to_string()).collect();
            let di = Density::ALL.iter().position(|d| *d == self.settings.density).unwrap_or(1);
            let n = ui.select(bar, "Density", &densities, di);
            ui.anchor(n, "density");
            made.push((n, Action::Density));
            let zooms: Vec<String> = ZOOMS.iter().map(|z| format!("zoom {z}")).collect();
            let zi = ZOOMS.iter().position(|z| *z == self.settings.zoom).unwrap_or(0);
            let n = ui.select(bar, "Zoom", &zooms, zi);
            ui.anchor(n, "zoom");
            made.push((n, Action::Zoom));
            if let Some(s) = &story {
                let mut keys: BTreeMap<String, Vec<serde_json::Value>> = BTreeMap::new();
                for v in &s.variants {
                    for (k, vals) in &v.args {
                        let e = keys.entry(k.clone()).or_default();
                        for x in vals {
                            if !e.contains(x) {
                                e.push(x.clone());
                            }
                        }
                    }
                }
                for (k, vals) in keys {
                    let mut opts = vec![format!("{k}: all")];
                    opts.extend(vals.iter().map(|v| format!("{k}={}", skelter_gallery::model::value_label(v))));
                    let sel = self.settings.args.get(&k).and_then(|v| vals.iter().position(|x| x == v)).map_or(0, |i| i + 1);
                    let n = ui.select(bar, &format!("Arg {k}"), &opts, sel);
                    ui.anchor(n, &format!("arg:{k}"));
                    made.push((n, Action::Arg(k, vals)));
                }
            }
            let n = ui.checkbox(bar, "Inspect", self.settings.inspect);
            ui.anchor(n, "inspect-toggle");
            made.push((n, Action::Inspect));
            let n = ui.button(bar, "Copy shot command");
            ui.anchor(n, "copy-shot");
            made.push((n, Action::CopyShot));
            let n = ui.button(bar, "Copy inspect command");
            ui.anchor(n, "copy-inspect");
            made.push((n, Action::CopyInspect));
        }
        for (n, a) in made {
            self.act(n, a);
        }
        self.dirty = true;
    }

    fn rebuild_sidebar(&mut self) {
        let list = self.nodes.list;
        self.forget(list);
        let filter = self.filter.to_lowercase();
        let mut made = Vec::new();
        {
            let mut d = self.doc.borrow_mut();
            let mut ui = Ui::new(&mut d);
            ui.clear(list);
            for e in &self.load_errors {
                ui.label(list, e, &["gal-error"]);
            }
            let mut families: Vec<&str> = skelter_gallery::FAMILIES.to_vec();
            families.extend(self.ext.families.iter().map(String::as_str));
            for fam in families {
                let stories: Vec<&Story> = self
                    .catalog
                    .family(fam)
                    .filter(|s| filter.is_empty() || s.id.0.contains(&filter) || s.title.to_lowercase().contains(&filter) || s.kinds.iter().any(|k| k.contains(&filter)))
                    .collect();
                if stories.is_empty() {
                    continue;
                }
                let h = ui.boxed(list, &["gal-family"], "");
                ui.label(h, fam, &[]);
                for s in stories {
                    let on = self.story.as_deref() == Some(s.id.as_str());
                    let row = ui.boxed(list, if on && self.variant.is_none() { &["gal-item", "gal-item-on"] } else { &["gal-item"] }, "padding-left: 16px");
                    let n = ui.link(row, &s.title, &[], "");
                    ui.anchor(n, &format!("story:{}", s.id));
                    made.push((n, Action::Story(s.id.0.clone())));
                    if on {
                        for v in &s.variants {
                            let von = self.variant.as_deref() == Some(v.id.as_str());
                            let row = ui.boxed(list, if von { &["gal-item", "gal-item-on"] } else { &["gal-item"] }, "padding-left: 32px");
                            let n = ui.link(row, &v.title, &[], "");
                            ui.anchor(n, &format!("variant:{}/{}", s.id, v.id));
                            made.push((n, Action::Variant(s.id.0.clone(), v.id.0.clone())));
                        }
                    }
                }
            }
            let missing: Vec<&String> = self.coverage_missing.iter().filter(|k| filter.is_empty() || k.contains(&filter)).collect();
            if !missing.is_empty() {
                let h = ui.boxed(list, &["gal-family"], "");
                ui.label(h, &format!("No story yet ({})", missing.len()), &[]);
                for k in missing {
                    let row = ui.boxed(list, &["gal-item"], "padding-left: 16px");
                    ui.label(row, k, &["gal-gap"]);
                }
            }
        }
        for (n, a) in made {
            self.act(n, a);
        }
        self.dirty = true;
    }

    fn current_story(&self) -> Option<&Story> {
        self.story.as_deref().and_then(|s| self.catalog.get(s))
    }

    /// The cells the canvas shows for the current selection and settings.
    pub fn matrix(&self) -> Option<Result<Vec<Cell>, String>> {
        let story = self.current_story()?;
        let themes: Vec<GTheme> = self.settings.themes.iter().filter_map(|t| self.ext.theme_named(t)).collect();
        let axes = Axes {
            variants: self.variant.as_ref().map(|v| vec![skelter_gallery::VariantId(v.clone())]),
            args: BTreeMap::new(),
            states: self.settings.states.clone(),
            themes,
            densities: vec![self.settings.density],
            zooms: vec![self.settings.zoom],
        };
        Some(Matrix::new(story, &axes).map(|m| m.cells.into_iter().filter(|c| self.settings.args.iter().all(|(k, v)| c.args.get(k).is_none_or(|x| x == v))).collect()))
    }

    /// Selects story `id` (and `variant`): a fresh live story, the canvas and the source pane.
    pub fn select(&mut self, id: &str, variant: Option<&str>) {
        let Some(story) = self.catalog.get(id).cloned() else { return };
        let same = self.story.as_deref() == Some(id);
        if !same {
            self.settings.args.clear();
            self.settings.states = None;
        }
        self.story = Some(id.to_string());
        self.variant = variant.map(str::to_string);
        if !same || self.live.is_none() {
            self.reload_story(&story);
        }
        let file = match &self.variant {
            Some(v) => story.variant(v).map(|v| story.path(&v.file)),
            None => story.variants.first().map(|v| story.path(&v.file)),
        };
        if let Some(f) = file
            && self.buffer.as_ref().is_none_or(|b| b.file != f)
        {
            self.open_source(&f);
        }
        self.picked = None;
        self.inspected = Inspected::default();
        self.rebuild_bar();
        self.rebuild_sidebar();
        self.rebuild_canvas();
        self.rebuild_inspect();
    }

    /// A fresh embed for `story`.
    fn reload_story(&mut self, story: &Story) {
        if let Some(old) = self.live.take() {
            old.close();
        }
        self.generation += 1;
        let variants: Vec<LiveVariant> = story.variants.iter().enumerate().map(|(i, v)| LiveVariant { export: format!("V{i}"), file: story.path(&v.file) }).collect();
        let fragments: Vec<PathBuf> = self.fragment.iter().cloned().collect();
        match LiveStory::load(self.doc.clone(), self.kinds.clone(), &fragments, variants, self.generation) {
            Ok(l) => {
                let tapped = self.tapped.clone();
                l.backend().add_event_tap(move |_doc, _input, events| tapped.borrow_mut().extend_from_slice(events));
                if let Some(w) = &self.wake {
                    l.backend().wake_state().set_notify(Some(w.clone()));
                }
                if self.watch
                    && let Err(e) = l.watch()
                {
                    eprintln!("skelter-gallery-app: watch: {e}");
                }
                self.live = Some(l);
            }
            Err(e) => self.load_errors = vec![format!("{}: {e}", story.id)],
        }
    }

    fn rebuild_canvas(&mut self) {
        let cells_node = self.nodes.cells;
        self.forget(cells_node);
        // the old roots go first: their slots are nodes of the canvas
        if let Some(l) = &mut self.live {
            l.mount(Vec::new());
            l.logic();
        }
        self.cells.clear();
        let Some(story) = self.current_story().cloned() else {
            let mut d = self.doc.borrow_mut();
            let mut ui = Ui::new(&mut d);
            ui.clear(cells_node);
            ui.label(cells_node, "Pick a story on the left.", &["gal-heading"]);
            return;
        };
        let cells = match self.matrix() {
            Some(Ok(c)) => c,
            Some(Err(e)) => {
                let mut d = self.doc.borrow_mut();
                let mut ui = Ui::new(&mut d);
                ui.clear(cells_node);
                ui.label(cells_node, &e, &["gal-error"]);
                return;
            }
            None => Vec::new(),
        };
        let mut mounts = Vec::new();
        let mut views = Vec::new();
        let mut made = Vec::new();
        {
            let mut d = self.doc.borrow_mut();
            let mut ui = Ui::new(&mut d);
            ui.clear(cells_node);
            ui.label(cells_node, &format!("{} — {}", story.title, story.notes.clone().unwrap_or_default()), &["gal-heading"]);
            let mut row: Option<(String, NodeId)> = None;
            for (i, cell) in cells.into_iter().enumerate() {
                let Some(variant) = story.variant(cell.variant.as_str()) else { continue };
                let key = format!("{} {}", cell.variant, cell.args_label());
                if row.as_ref().is_none_or(|(k, _)| *k != key) {
                    let title = if cell.args.is_empty() { variant.title.clone() } else { format!("{} · {}", variant.title, cell.args_label()) };
                    ui.label(cells_node, &title, &["gal-heading"]);
                    let r = ui.boxed(cells_node, &["gal-row"], "");
                    row = Some((key, r));
                }
                let r = row.as_ref().expect("a row").1;
                let c = ui.boxed(r, &["gal-cell"], "");
                let disabled_prop = cell.state == State::Disabled && variant.takes_disabled();
                let caption = format!("{} · {}{}", cell.state, cell.theme, if cell.state == State::Disabled && !disabled_prop { " (forced :disabled)" } else { "" });
                let cap = ui.link(c, &caption, &["gal-caption"], "");
                made.push((cap, Action::Cell(i)));
                let classes = vec!["gal-frame".to_string(), theme::class(cell.theme.name())];
                let inline = format!("width: {}px; height: {}px", story.size[0], story.size[1]);
                let frame = ui.boxed(c, &classes.iter().map(String::as_str).collect::<Vec<_>>(), &inline);
                ui.anchor(frame, &format!("cell:{i}"));
                let error = ui.label(c, "", &["gal-error"]);
                let mut props = serde_json::Map::new();
                for (k, v) in &cell.args {
                    props.insert(k.clone(), v.clone());
                }
                let mut force = cell.state.pseudo_class().and_then(svelters_skelter::style::pseudo_state).unwrap_or(StateMask::NONE);
                if disabled_prop {
                    props.insert("disabled".into(), serde_json::Value::Bool(true));
                    force = StateMask::NONE;
                }
                let vi = story.variants.iter().position(|v| v.id == cell.variant).unwrap_or(0);
                mounts.push(CellMount { key: format!("cell:{i}"), frame, variant: vi, props: serde_json::Value::Object(props), force });
                views.push(CellView { cell, frame, error, classes, inline });
            }
        }
        for (n, a) in made {
            self.act(n, a);
        }
        self.cells = views;
        if let Some(l) = &mut self.live {
            // the host's per-mount state (cadatomic's grid sources, its pick board), as the CLI runs it
            for v in &self.cells {
                self.ext.run_setup(&v.cell, &story, l.backend());
            }
            l.mount(mounts);
            l.logic();
        }
        self.show_errors();
        self.dirty = true;
    }

    /// Shows the live story's errors in the cells of the files they name and in the source pane.
    fn show_errors(&mut self) {
        let Some(story) = self.current_story().cloned() else { return };
        let errors: Vec<SourceError> = self.live.as_ref().map(|l| l.errors().to_vec()).unwrap_or_default();
        let root = story.root.clone();
        let mut d = self.doc.borrow_mut();
        for v in &self.cells {
            let file = story.variant(v.cell.variant.as_str()).map(|x| story.path(&x.file));
            let text: Vec<String> = errors.iter().filter(|e| Some(&e.file) == file.as_ref()).map(|e| e.display(&root)).collect();
            let _ = d.set_prop(v.error, w::LABEL_TEXT, Value::Str(text.join("\n").into()));
        }
        if let Some(b) = &self.buffer {
            let mine: Vec<&SourceError> = errors.iter().filter(|e| e.file == b.file).collect();
            let text: Vec<String> = mine.iter().map(|e| e.display(&root)).collect();
            let _ = d.set_prop(self.nodes.errors, w::LABEL_TEXT, Value::Str(text.join("\n").into()));
            // mark the lines in the editor
            let mut doc = b.doc.borrow_mut();
            let version = doc.version();
            let diags: Vec<quire_core::diagnostics::Diagnostic> = mine
                .iter()
                .filter(|e| e.line > 0)
                .map(|e| {
                    let t = doc.text();
                    let line = (e.line as usize - 1).min(t.len_lines().saturating_sub(1));
                    let start = t.line_to_byte(line);
                    let end = if line + 1 < t.len_lines() { t.line_to_byte(line + 1) } else { t.len_bytes() };
                    let at = (start + (e.column as usize).saturating_sub(1)).min(end);
                    let range = if at < end { at..end } else { start..end };
                    quire_core::diagnostics::Diagnostic::new(range, quire_core::diagnostics::Severity::Error, e.message.clone())
                })
                .collect();
            let _ = doc.set_diagnostics(quire_core::SourceId::new("svelters"), version, diags);
        }
        drop(d);
        self.dirty = true;
    }

    fn open_source(&mut self, file: &Path) {
        let text = match (self.live.as_ref().and_then(|l| l.provider().overlay().get(file)), std::fs::read_to_string(file)) {
            (Some(t), _) => t,
            (None, Ok(t)) => t,
            (None, Err(e)) => format!("<!-- {}: {e} -->", file.display()),
        };
        self.buffers += 1;
        let name = format!("source:{}", self.buffers);
        let qdoc = quire_core::Document::new(&text, EditorConfig::default());
        let v = qdoc.version();
        let rc = {
            let mut d = self.doc.borrow_mut();
            let host = d.service::<QuireHost>().expect("the quire host");
            if let Some(b) = &self.buffer {
                host.buffers.remove(&b.name);
            }
            host.insert_buffer(name.clone(), qdoc)
        };
        self.buffer = Some(Buffer { file: file.to_path_buf(), doc: rc, saved: v, previewed: v, name: name.clone() });
        let root = self.current_story().map(|s| s.root.clone()).unwrap_or_default();
        let mut d = self.doc.borrow_mut();
        let _ = d.set_prop(self.nodes.editor, w::EDITOR_BUFFER, Value::Str(name.into()));
        let rel = crate::live::rel(file, &root);
        let _ = d.set_prop(self.nodes.file, w::LABEL_TEXT, Value::Str(rel.into()));
        drop(d);
        self.dirty = true;
        self.show_errors();
    }

    fn rebuild_inspect(&mut self) {
        let pane = self.nodes.inspect;
        self.forget(pane);
        let i = self.inspected.clone();
        let mut d = self.doc.borrow_mut();
        let mut ui = Ui::new(&mut d);
        ui.clear(pane);
        ui.label(pane, "Inspect", &["gal-heading"]);
        match (&i.cell, &i.json) {
            (Some(cell), json) => {
                ui.label(pane, &format!("{}/{} {}", cell.story, cell.label(), i.selector.clone().unwrap_or_default()), &[]);
                if let Some(cmd) = &i.command {
                    let n = ui.label(pane, cmd, &[]);
                    ui.set(n, w::LABEL_SELECTABLE, Value::Bool(true));
                    ui.anchor(n, "inspect-command");
                }
                if let Some(j) = json {
                    for line in serde_json::to_string_pretty(j).unwrap_or_default().lines() {
                        ui.label(pane, line, &[]);
                    }
                }
            }
            _ => {
                ui.label(pane, if self.settings.inspect { "Click a node in a cell." } else { "Turn on Inspect, then click a node in a cell." }, &[]);
            }
        }
        drop(d);
        self.dirty = true;
    }

    // ---- events ----

    /// Chrome events from one dispatch (or a frame): acts on the ones on the chrome's nodes.
    pub fn handle(&mut self, events: &[JsEvent]) {
        for ev in events {
            let Some(a) = self.actions.get(&ev.target).cloned() else { continue };
            if !ev.path.iter().any(|p| p.node == ev.target) && !ev.path.is_empty() {
                continue;
            }
            self.on(a, ev);
        }
    }

    fn on(&mut self, a: Action, ev: &JsEvent) {
        let b = |ev: &JsEvent| matches!(ev.arg, Some(Value::Bool(true)));
        let int = |ev: &JsEvent| match ev.arg {
            Some(Value::I64(i)) => i as usize,
            Some(Value::F64(f)) => f as usize,
            _ => 0,
        };
        match a {
            Action::Story(id) => self.select(&id, None),
            Action::Variant(s, v) => self.select(&s, Some(&v)),
            Action::Theme(t) => {
                let on = b(ev);
                self.settings.themes.retain(|x| *x != t);
                if on {
                    self.settings.themes.push(t);
                }
                let order = self.themes.names();
                self.settings.themes.sort_by_key(|x| order.iter().position(|o| o == x));
                self.rebuild_canvas();
            }
            Action::State(s) => {
                let all = self.current_story().map(|x| x.states.clone()).unwrap_or_default();
                let mut cur = self.settings.states.clone().unwrap_or_else(|| all.clone());
                cur.retain(|x| *x != s);
                if b(ev) {
                    cur.push(s);
                }
                cur.sort_by_key(|x| all.iter().position(|o| o == x));
                self.settings.states = Some(cur);
                self.rebuild_canvas();
            }
            Action::Density => {
                self.settings.density = Density::ALL.get(int(ev)).copied().unwrap_or_default();
                self.install_cell_sheet();
                self.rebuild_canvas();
            }
            Action::Zoom => {
                self.settings.zoom = ZOOMS.get(int(ev)).copied().unwrap_or(1.0);
                self.dirty = true;
            }
            Action::Arg(k, vals) => {
                match int(ev) {
                    0 => self.settings.args.remove(&k),
                    i => self.settings.args.insert(k, vals.get(i - 1).cloned().unwrap_or_default()),
                };
                self.rebuild_canvas();
            }
            Action::Inspect => {
                self.settings.inspect = b(ev);
                self.rebuild_inspect();
            }
            Action::Filter => {
                if let Some(Value::Str(s)) = &ev.arg {
                    self.filter = s.to_string();
                    self.rebuild_sidebar();
                }
            }
            Action::Save => self.save(),
            Action::Revert => self.revert(),
            Action::CopyShot => {
                if let Some(c) = self.picked_cell().cloned() {
                    let cmd = shot_command(&c);
                    self.copy(&cmd);
                }
            }
            Action::CopyInspect => {
                if let Some(c) = self.picked_cell().cloned() {
                    let sel = self.inspected.selector.clone().unwrap_or_else(|| "#subject".into());
                    let cmd = inspect_command(&c, &sel);
                    self.copy(&cmd);
                }
            }
            Action::Cell(i) => self.pick_cell(i),
        }
    }

    /// The picked cell, else the first one.
    pub fn picked_cell(&self) -> Option<&Cell> {
        self.picked.and_then(|i| self.cells.get(i)).or(self.cells.first()).map(|c| &c.cell)
    }

    /// Picks cell `i`: its variant's file in the source pane, its commands ready to copy.
    pub fn pick_cell(&mut self, i: usize) {
        let Some(c) = self.cells.get(i) else { return };
        let cell = c.cell.clone();
        {
            let mut d = self.doc.borrow_mut();
            let mut ui = Ui::new(&mut d);
            for (j, v) in self.cells.iter().enumerate() {
                let was = self.picked == Some(j);
                if was != (j == i) {
                    let mut classes: Vec<&str> = v.classes.iter().map(String::as_str).collect();
                    if j == i {
                        classes.push("gal-frame-picked");
                    }
                    ui.style(v.frame, &classes, &v.inline);
                }
            }
        }
        self.picked = Some(i);
        if let Some(story) = self.current_story().cloned()
            && let Some(v) = story.variant(cell.variant.as_str())
        {
            let f = story.path(&v.file);
            if self.buffer.as_ref().is_none_or(|b| b.file != f) {
                self.open_source(&f);
            }
        }
        self.inspected = Inspected { cell: Some(cell.clone()), selector: None, json: None, command: Some(shot_command(&cell)) };
        self.rebuild_inspect();
    }

    fn copy(&mut self, text: &str) {
        if let Some(c) = &mut self.clipboard {
            c.set_text(skelter_platform::ClipboardKind::Clipboard, text);
        }
        self.copied = Some(text.to_string());
        self.inspected.command = Some(text.to_string());
        self.rebuild_inspect();
    }

    /// Inspect mode: a press at `pos` in `window` picks the node under it in a cell. Returns
    /// whether the press was over a cell (and so taken).
    pub fn inspect_at(&mut self, window: WindowId, pos: Point) -> bool {
        let target = {
            let d = self.doc.borrow();
            d.hit_test(window, pos)
        };
        let Some(target) = target else { return false };
        let Some(i) = ({
            let d = self.doc.borrow();
            self.cells.iter().position(|c| is_within(&d, target, c.frame))
        }) else {
            return false;
        };
        self.pick_cell(i);
        let frame = self.cells[i].frame;
        let cell = self.cells[i].cell.clone();
        let (selector, node) = {
            let d = self.doc.borrow();
            selector_for(&d, target, frame)
        };
        let json = match &self.inspect_fn {
            Some(f) => Some(f(&self.doc.borrow(), frame, node, &cell, &selector)),
            None => Some(self.inspect_live(i, &selector)),
        };
        self.inspected = Inspected { command: Some(inspect_command(&cell, &selector)), cell: Some(cell), selector: Some(selector), json };
        self.rebuild_inspect();
        true
    }

    /// Inspects cell `i` with `selector` (as a click on the node it names would).
    pub fn inspect_cell(&mut self, i: usize, selector: &str) -> Result<(), String> {
        let cell = self.cells.get(i).map(|c| c.cell.clone()).ok_or_else(|| format!("no cell {i} (there are {})", self.cells.len()))?;
        self.pick_cell(i);
        let json = self.inspect_live(i, selector);
        self.inspected = Inspected { command: Some(inspect_command(&cell, selector)), cell: Some(cell), selector: Some(selector.to_string()), json: Some(json) };
        self.rebuild_inspect();
        Ok(())
    }

    /// What `skelter-gallery inspect` prints for the nodes `selector` picks in the live cell `i`
    /// (its `nodes` built by skelter-gallery's own `inspect::node_json`), unsaved edits included.
    /// `accesskit` is `null`: the app publishes no AccessKit snapshot per cell (the CLI does).
    pub fn inspect_live(&self, i: usize, selector: &str) -> serde_json::Value {
        let (Some(c), Some(l)) = (self.cells.get(i), &self.live) else { return serde_json::json!({ "error": "no such cell" }) };
        let b = l.backend();
        let window = self.doc.borrow().window_of(c.frame);
        let nodes: Vec<serde_json::Value> = skelter_gallery::inspect::select_in(b, c.frame, selector).into_iter().filter_map(|h| skelter_gallery::inspect::node_json(b, window, h, None)).collect();
        if nodes.is_empty() {
            return serde_json::json!({ "error": format!("no node matches {selector:?} in {} (a selector is #anchor or a kind name)", c.cell.label()) });
        }
        serde_json::json!({ "cell": c.cell, "label": c.cell.label(), "selector": selector, "source": "live", "nodes": nodes })
    }

    // ---- the source pane ----

    /// The source pane has an edit not previewed yet.
    pub fn edit_pending(&self) -> bool {
        self.buffer.as_ref().is_some_and(|b| b.doc.borrow().version() != b.previewed)
    }

    /// The buffer changed: preview it through the overlay (no save).
    pub fn sync_buffer(&mut self) {
        let Some(b) = &mut self.buffer else { return };
        let v = b.doc.borrow().version();
        if v == b.previewed {
            return;
        }
        b.previewed = v;
        let text = b.doc.borrow().text().to_string();
        let file = b.file.clone();
        if let Some(l) = &mut self.live {
            if v == b.saved {
                l.clear_preview(&file);
                l.apply(&file);
            } else {
                l.preview(&file, &text);
            }
        }
        self.after_change(&file);
    }

    fn after_change(&mut self, file: &Path) {
        if self.live.as_ref().is_some_and(|l| l.needs_reload(file)) {
            // a variant that did not compile at load compiles now: load the story again
            if let Some(s) = self.current_story().cloned() {
                self.reload_story(&s);
                self.rebuild_canvas();
            }
        }
        self.show_errors();
    }

    /// Writes the buffer to its file (Ctrl+S). The overlay is dropped: the file is the source now.
    pub fn save(&mut self) {
        let Some(b) = &mut self.buffer else { return };
        let text = b.doc.borrow().text().to_string();
        if let Err(e) = std::fs::write(&b.file, &text) {
            let msg = format!("{}: {e}", b.file.display());
            let _ = self.doc.borrow_mut().set_prop(self.nodes.errors, w::LABEL_TEXT, Value::Str(msg.into()));
            return;
        }
        let v = b.doc.borrow().version();
        b.saved = v;
        b.previewed = v;
        let file = b.file.clone();
        if let Some(l) = &mut self.live {
            l.clear_preview(&file);
            l.apply(&file);
        }
        self.after_change(&file);
    }

    /// Drops the unsaved buffer: the file on disk again.
    pub fn revert(&mut self) {
        let Some(file) = self.buffer.as_ref().map(|b| b.file.clone()) else { return };
        if let Some(l) = &mut self.live {
            l.clear_preview(&file);
            l.apply(&file);
        }
        self.open_source(&file);
        self.after_change(&file);
    }

    /// The buffer has edits not on disk.
    pub fn unsaved(&self) -> bool {
        self.buffer.as_ref().is_some_and(|b| b.doc.borrow().version() != b.saved)
    }

    /// Once per frame, before the document's frame: the editor's events and edits, the live
    /// story's JS, reload reports.
    pub fn before_frame(&mut self) {
        self.poll_base_css();
        let events: Vec<_> = self.doc.borrow_mut().service::<QuireHost>().map(|h| std::mem::take(&mut h.events)).unwrap_or_default();
        for (_, e) in events {
            match e {
                skelter_quire::EditorEvent::SaveRequested => self.save(),
                skelter_quire::EditorEvent::CopyText { text } => {
                    if let Some(c) = &mut self.clipboard {
                        c.set_text(skelter_platform::ClipboardKind::Clipboard, &text);
                    }
                }
                skelter_quire::EditorEvent::PasteRequested => {}
                _ => {}
            }
        }
        self.sync_buffer();
        let before = self.live.as_ref().map(|l| l.errors().to_vec());
        if let Some(l) = &mut self.live {
            l.logic();
            let r = l.take_report();
            if r.applied {
                self.dirty = true;
            }
        }
        let after = self.live.as_ref().map(|l| l.errors().to_vec());
        if before != after {
            self.show_errors();
        }
        // the shown file changed on disk (an agent's edit) and the buffer has no edits: show it
        if let Some(b) = &self.buffer
            && b.doc.borrow().version() == b.saved
            && let Ok(disk) = std::fs::metadata(&b.file).and_then(|_| std::fs::read_to_string(&b.file))
            && disk != b.doc.borrow().text().to_string()
            && self.live.as_ref().is_none_or(|l| l.provider().overlay().get(&b.file).is_none())
        {
            let f = b.file.clone();
            self.open_source(&f);
        }
    }
}

/// The chrome's fixed nodes.
fn build(ui: &mut Ui<'_>) -> Nodes {
    let root = ui.node(None, w::BOX, &["gal-root"], "");
    ui.anchor(root, "gallery");
    let bar = ui.boxed(root, &["gal-bar"], "");
    ui.anchor(bar, "controls");
    let main = ui.boxed(root, &["gal-main"], "");
    let side = ui.boxed(main, &["gal-side"], "");
    let filter = ui.node(Some(side), w::TEXT_EDIT, &[], "margin: 6px");
    ui.set(filter, w::TEXT_HINT, Value::Str("Filter stories".into()));
    ui.aria_label(filter, "Filter stories");
    ui.anchor(filter, "filter");
    ui.listen(filter, w::TEXT_CHANGED);
    let (_, list) = ui.scroll(side, "flex-grow: 1");
    ui.anchor(list, "stories");
    let canvas = ui.boxed(main, &["gal-canvas"], "");
    let (_, cells) = ui.scroll(canvas, "flex-grow: 1");
    ui.style(cells, &["gal-cells"], "");
    ui.anchor(cells, "canvas");
    let right = ui.boxed(main, &["gal-right"], "");
    let head = ui.boxed(right, &["gal-bar"], "");
    let title = ui.label(head, "Source", &["gal-heading"]);
    let file = ui.label(head, "", &[]);
    ui.anchor(file, "source-file");
    let save = ui.button(head, "Save");
    ui.anchor(save, "save");
    let revert = ui.button(head, "Revert");
    ui.anchor(revert, "revert");
    let errors = ui.label(right, "", &["gal-error"]);
    ui.anchor(errors, "source-errors");
    let editor = ui.node(Some(right), w::CODE_EDITOR, &[], "flex-grow: 3; flex-shrink: 1; flex-basis: 0; min-height: 120px");
    ui.aria_label(editor, "Source");
    ui.anchor(editor, "source");
    let (_, inspect) = ui.scroll(right, "flex-grow: 2; flex-shrink: 1; flex-basis: 0; min-height: 120px; border-top: 1px solid var(--vscode-panel-border)");
    ui.anchor(inspect, "inspect");
    // The save/revert/filter actions are registered by the caller through `Gallery::new`'s
    // `adopt` below (their nodes are fixed).
    let _ = (save, revert, filter);
    Nodes { root, bar, side, filter, list, cells, title, file, errors, editor, inspect }
}

/// `node` is `frame` or under it.
fn is_within(d: &Document, node: NodeId, frame: NodeId) -> bool {
    let mut n = Some(node);
    while let Some(x) = n {
        if x == frame {
            return true;
        }
        n = d.parent(x);
    }
    false
}

/// The selector `skelter-gallery inspect` takes for `node` in the cell `frame`: the nearest anchor
/// at or above it (`#name`), else its kind name. Returns the node the selector names.
pub fn selector_for(d: &Document, node: NodeId, frame: NodeId) -> (String, NodeId) {
    let mut n = Some(node);
    while let Some(x) = n {
        if x == frame {
            break;
        }
        if let Some(Value::Str(a)) = d.get_prop(x, skelter_core::common::ANCHOR)
            && !a.is_empty()
        {
            return (format!("#{a}"), x);
        }
        n = d.parent(x);
    }
    let name = d.kind_of(node).and_then(|k| d.registry().get(k).map(|e| e.spec.name.to_string())).unwrap_or_else(|| "box".into());
    (name, node)
}

/// The cell flags of the CLI (`--args`, `--state`, `--theme`, `--density`, `--zoom`).
pub fn cell_flags(c: &Cell) -> String {
    let mut s = String::new();
    let a = c.args.iter().map(|(k, v)| format!("{k}={}", skelter_gallery::model::value_label(v))).collect::<Vec<_>>().join(",");
    if !a.is_empty() {
        s.push_str(&format!(" --args {}", shell_word(&a)));
    }
    s.push_str(&format!(" --state {} --theme {} --density {} --zoom {}", c.state, c.theme, c.density.name(), c.zoom));
    s
}

/// `skelter-gallery shot` for cell `c`.
pub fn shot_command(c: &Cell) -> String {
    format!("skelter-gallery shot {}/{}{} -o target/gallery/{}-{}.png", c.story, c.variant, cell_flags(c), c.story, c.variant)
}

/// `skelter-gallery inspect` for cell `c` and `selector`.
pub fn inspect_command(c: &Cell, selector: &str) -> String {
    format!("skelter-gallery inspect {}/{} {}{} --json", c.story, c.variant, shell_word(selector), cell_flags(c))
}

/// Quoted for a POSIX shell when it needs it.
fn shell_word(s: &str) -> String {
    if s.chars().all(|c| c.is_ascii_alphanumeric() || "-_=,./:".contains(c)) { s.to_string() } else { format!("'{}'", s.replace('\'', "'\\''")) }
}

impl Gallery {
    /// Registers the fixed nodes' actions (filter, save, revert).
    pub fn wire(&mut self) {
        let (filter, side) = (self.nodes.filter, self.nodes.side);
        self.act(filter, Action::Filter);
        let _ = side;
        let d = self.doc.borrow();
        let save = d.find_anchor_in(self.nodes.root, "save");
        let revert = d.find_anchor_in(self.nodes.root, "revert");
        drop(d);
        if let Some(s) = save {
            self.act(s, Action::Save);
        }
        if let Some(r) = revert {
            self.act(r, Action::Revert);
        }
        let _ = self.nodes.title;
    }
}

trait FindAnchor {
    fn find_anchor_in(&self, root: NodeId, anchor: &str) -> Option<NodeId>;
}

impl FindAnchor for Document {
    fn find_anchor_in(&self, root: NodeId, anchor: &str) -> Option<NodeId> {
        self.query().within(root).anchor(anchor).first()
    }
}
