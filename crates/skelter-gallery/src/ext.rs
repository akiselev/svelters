//! What a host such as `cadatomic-gallery` adds: story roots, kinds, per-mount setup and token sets.

use crate::model::{Cell, Story, Theme};
use skelter_core::{Document, KindRegistry};
use skelter_widgets::theme::TokenMap;
use std::path::PathBuf;
use std::rc::Rc;
use svelters_skelter::SkelterBackend;

/// Registers a host's kinds on the gallery's registry.
pub type RegisterKinds = fn(&mut KindRegistry) -> Result<(), String>;

/// A host's per-mount setup ([`Extensions::setup`]).
pub type SetupHook = Rc<dyn Fn(&mut SetupCx<'_>)>;

/// A host's additions. `Extensions::default()` is svelters' own gallery: the `gallery/` root of this
/// repository (when the CLI finds it), the built-in kinds and the four VS Code themes.
#[derive(Default)]
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
