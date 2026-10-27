//! What a host such as `cadatomic-gallery` adds: story roots, kinds, per-mount setup and token sets.

use crate::model::{Cell, Story, Theme};
use skelter_core::{Document, KindRegistry};
use skelter_widgets::theme::{ColorScheme, Contrast, Density, TokenMap};
use std::path::PathBuf;
use std::rc::Rc;
use svelters_skelter::SkelterBackend;

/// Registers a host's kinds on the gallery's registry.
pub type RegisterKinds = fn(&mut KindRegistry) -> Result<(), String>;

/// A host's per-mount setup ([`Extensions::setup`]).
pub type SetupHook = Rc<dyn Fn(&mut SetupCx<'_>)>;

/// A host's per-frame sync ([`Extensions::on_frame`]).
pub type FrameHook = Rc<dyn Fn(&mut FrameCx<'_>)>;

/// A host's busy probe ([`Extensions::busy`]): why the document is not settled, or `None`.
pub type BusyHook = Rc<dyn Fn(&Document) -> Option<String>>;

/// A host's once-per-backend setup ([`Extensions::on_backend`]).
pub type BackendHook = Rc<dyn Fn(&SkelterBackend)>;

/// A host's additions. `Extensions::default()` is svelters' own gallery: the `gallery/` root of this
/// repository (when the CLI finds it), the built-in kinds and the four VS Code themes.
#[derive(Clone, Default)]
#[non_exhaustive]
pub struct Extensions {
    /// Gallery roots (each holds `stories/<family>/<id>/story.toml`).
    pub story_roots: Vec<PathBuf>,
    /// Registers the host's kinds on the gallery's registry, after the built-in ones.
    pub register_kinds: Vec<RegisterKinds>,
    /// Runs before each mount (a cell's story and variant): host state a story reads (cadatomic's
    /// grid sources, its pick board).
    /// (`Rc`, not the brief's `Box`: the renderer keeps them.)
    pub setup: Vec<SetupHook>,
    /// Extra token sets, by theme name, selectable as `--theme <name>`: each is Dark Modern with
    /// these tokens as overrides (`--sv-*`, `--vscode-*`, or the host's own custom properties).
    pub themes: Vec<(String, TokenMap)>,
    /// Families a host's stories may use on top of [`crate::FAMILIES`].
    pub families: Vec<String>,
    /// The fonts cells draw with ([`crate::fonts::FontSet::Bundled`] unless `--system-fonts`).
    pub font_set: crate::fonts::FontSet,
    /// G4: the host's font files (TTF/OTF/TTC), added to the gallery's set: an icon font, the
    /// app's own families. Shots stay hermetic.
    pub font_files: Vec<Vec<u8>>,
    /// G1: runs before each frame (each harness step in the renderer, each frame in the app):
    /// the host's per-frame sync (highlighting, pick marks, behaviours).
    pub frame: Vec<FrameHook>,
    /// G1: why the host's part of the document is not settled; counted by the renderer's settle
    /// and reported in its "not settled" warning.
    pub busy: Vec<BusyHook>,
    /// G6: runs once per backend (a renderer, an app story) before the first variant loads.
    pub backend: Vec<BackendHook>,
    /// G5: the scheme and contrast of a host theme made with [`Extensions::theme_with`]; a theme
    /// not listed here is Dark Modern's.
    pub theme_bases: Vec<(String, ColorScheme, Contrast)>,
}

impl Extensions {
    pub fn new() -> Extensions {
        Extensions::default()
    }

    pub fn story_root(mut self, root: impl Into<PathBuf>) -> Extensions {
        self.story_roots.push(root.into());
        self
    }

    pub fn kinds(mut self, f: fn(&mut KindRegistry) -> Result<(), String>) -> Extensions {
        self.register_kinds.push(f);
        self
    }

    pub fn on_setup(mut self, f: impl Fn(&mut SetupCx<'_>) + 'static) -> Extensions {
        self.setup.push(Rc::new(f));
        self
    }

    pub fn theme(mut self, name: impl Into<String>, tokens: TokenMap) -> Extensions {
        self.themes.push((name.into(), tokens));
        self
    }

    pub fn family(mut self, name: impl Into<String>) -> Extensions {
        self.families.push(name.into());
        self
    }

    /// The fonts cells draw with (`--system-fonts` sets [`crate::fonts::FontSet::System`]).
    pub fn font_set(mut self, set: crate::fonts::FontSet) -> Extensions {
        self.font_set = set;
        self
    }

    /// G4: font files (TTF/OTF/TTC bytes) added to the gallery's bundled set.
    pub fn fonts(mut self, files: impl IntoIterator<Item = Vec<u8>>) -> Extensions {
        self.font_files.extend(files);
        self
    }

    /// G1: `f` runs before each frame with the frame's number and time.
    pub fn on_frame(mut self, f: impl Fn(&mut FrameCx<'_>) + 'static) -> Extensions {
        self.frame.push(Rc::new(f));
        self
    }

    /// G1: a busy probe: `Some(why)` while the host's state is still changing.
    pub fn busy(mut self, f: impl Fn(&Document) -> Option<String> + 'static) -> Extensions {
        self.busy.push(Rc::new(f));
        self
    }

    /// G6: `f` runs once per backend before the first variant loads (behaviours, `:state()`
    /// names, services, sheets).
    pub fn on_backend(mut self, f: impl Fn(&SkelterBackend) + 'static) -> Extensions {
        self.backend.push(Rc::new(f));
        self
    }

    /// G5: a host theme on its own base: `scheme` and `contrast` pick the VS Code theme its
    /// `tokens` override, and the theme service, `prefers-contrast` and `dark` follow them.
    pub fn theme_with(mut self, name: impl Into<String>, scheme: ColorScheme, contrast: Contrast, tokens: TokenMap) -> Extensions {
        let name = name.into();
        self.theme_bases.retain(|(n, _, _)| *n != name);
        self.theme_bases.push((name.clone(), scheme, contrast));
        self.themes.push((name, tokens));
        self
    }

    /// The scheme and contrast a host theme is built on (Dark Modern's unless
    /// [`Extensions::theme_with`] said otherwise).
    pub fn theme_base(&self, name: &str) -> (ColorScheme, Contrast) {
        self.theme_bases.iter().find(|(n, _, _)| n == name).map_or((ColorScheme::Dark, Contrast::Normal), |(_, s, c)| (*s, *c))
    }

    /// A theme by name at `density` (a built-in, or a host's on its base); `None` when unknown.
    pub fn resolve_theme(&self, theme: &Theme, density: Density) -> Option<skelter_widgets::theme::Theme> {
        use skelter_widgets::theme::Theme as T;
        let base = match theme.recipe() {
            Some((scheme, contrast)) => T::vscode(scheme, contrast),
            None => {
                let (_, tokens) = self.themes.iter().find(|(n, _)| n == theme.name())?;
                let (scheme, contrast) = self.theme_base(theme.name());
                T::vscode(scheme, contrast).with(tokens.iter())
            }
        };
        Some(base.with_density(density))
    }

    /// Runs every frame hook (what the renderer does before each harness step).
    pub fn run_frame(&self, backend: &SkelterBackend, frame: u64, now_ms: f64) {
        for f in &self.frame {
            f(&mut FrameCx { backend, frame, now_ms });
        }
    }

    /// The busy probes' reasons, joined; `None` when every probe is settled.
    pub fn busy_reason(&self, backend: &SkelterBackend) -> Option<String> {
        if self.busy.is_empty() {
            return None;
        }
        let why: Vec<String> = backend.with_doc(|d| self.busy.iter().filter_map(|f| f(d)).collect());
        (!why.is_empty()).then(|| why.join("; "))
    }

    /// Runs every backend hook (what the renderer does once, before the first variant loads).
    pub fn run_backend(&self, backend: &SkelterBackend) {
        for f in &self.backend {
            f(backend);
        }
    }

    /// A theme by name: one of the four, or a host's.
    pub fn theme_named(&self, name: &str) -> Option<Theme> {
        Theme::builtin(name).or_else(|| self.themes.iter().any(|(n, _)| n == name).then(|| Theme::Extra(name.to_string())))
    }

    /// Runs every setup hook for `cell` (what [`crate::Renderer`] does before each mount).
    pub fn run_setup(&self, cell: &Cell, story: &Story, backend: &SkelterBackend) {
        for f in &self.setup {
            f(&mut SetupCx::new(cell, story, backend));
        }
    }

    /// Every theme name, the four first.
    pub fn theme_names(&self) -> Vec<String> {
        Theme::BUILTIN.iter().map(|t| t.name().to_string()).chain(self.themes.iter().map(|(n, _)| n.clone())).collect()
    }
}

/// What a frame hook gets: the backend, the frame's number (from 1 per renderer or app) and its
/// time in milliseconds on the renderer's virtual clock (the app's frame time).
#[non_exhaustive]
pub struct FrameCx<'a> {
    pub backend: &'a SkelterBackend,
    pub frame: u64,
    pub now_ms: f64,
}

impl FrameCx<'_> {
    /// Runs `f` on the gallery's document.
    pub fn with_doc<R>(&self, f: impl FnOnce(&mut Document) -> R) -> R {
        self.backend.with_doc(f)
    }
}

/// What a setup hook gets: the cell about to mount and the document it mounts into.
#[non_exhaustive]
pub struct SetupCx<'a> {
    pub cell: &'a Cell,
    pub story: &'a Story,
    pub backend: &'a SkelterBackend,
}

impl<'a> SetupCx<'a> {
    /// The context for a host that mounts cells itself (the gallery app).
    pub fn new(cell: &'a Cell, story: &'a Story, backend: &'a SkelterBackend) -> SetupCx<'a> {
        SetupCx { cell, story, backend }
    }

    /// Runs `f` on the gallery's document (services, sheets, host nodes).
    pub fn with_doc<R>(&self, f: impl FnOnce(&mut Document) -> R) -> R {
        self.backend.with_doc(f)
    }
}
