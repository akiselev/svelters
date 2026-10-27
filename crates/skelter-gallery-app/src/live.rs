//! One story, live: its variant files compiled from source by svelters' module loader into one
//! embed entry (an export per variant), and each preview cell a root of that entry mounted into
//! the cell's frame node.
//!
//! Each story gets a fresh embed (its own QuickJS runtime, over the gallery's document) when it is
//! selected, so its modules are the only ones the host's reloader watches. Edits then go through
//! the host's in-place update (H1, `dev` + `hmr`): an unsaved buffer is set in the provider's
//! overlay and applied with `Host::apply_changes`; a saved file, an agent's edit or a `base.css`
//! change comes from the host's watcher. A compile or load error leaves the previous version
//! running (the last good render stays) and is reported with its file and line.

use skelter_base::ids::NodeId;
use skelter_core::{Document, StateMask};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use svelters_host::modules::{Diagnostic, Entry, Severity, SourceConfig, SourceProvider};
use svelters_skelter::SkelterBackend;
use svelters_skelter::embed::{Embed, EmbedConfig, EmbedEvent, MountSpec, Props, RootId, SlotId};
use svelters_skelter::kinds::Kinds;

/// A variant as the live story loads it.
#[derive(Clone, Debug)]
pub struct LiveVariant {
    /// The entry's export name (`V0`, `V1`, …).
    pub export: String,
    pub file: PathBuf,
}

/// A cell to mount: which export, with what props, into which node, with which states forced.
#[derive(Clone, Debug)]
pub struct CellMount {
    pub key: String,
    pub frame: NodeId,
    pub variant: usize,
    pub props: serde_json::Value,
    /// Forced on the `anchor="subject"` nodes (or the root's first node when none has it).
    pub force: StateMask,
}

/// An error to show: where and what.
#[derive(Clone, Debug, PartialEq)]
pub struct SourceError {
    pub file: PathBuf,
    /// 1-based; 0 when unknown.
    pub line: u32,
    pub column: u32,
    pub message: String,
}

impl SourceError {
    /// `file:line:col: message` (the file relative to `root` when inside it).
    pub fn display(&self, root: &Path) -> String {
        let f = rel(&self.file, root);
        if self.line > 0 { format!("{f}:{}:{}: {}", self.line, self.column, self.message) } else { format!("{f}: {}", self.message) }
    }

    /// From a host error's message (`/path/file.svelte:2:23: error[...]: text`, as the host maps
    /// load errors), else attributed to `fallback`.
    pub fn parse(message: &str, fallback: &Path) -> SourceError {
        // the path ends at the first `:<digits>:<digits>:`
        let mut search = 0;
        while let Some(i) = message[search..].find(':').map(|i| i + search) {
            let rest = &message[i + 1..];
            let mut parts = rest.splitn(3, ':');
            if let (Some(l), Some(c), Some(msg)) = (parts.next(), parts.next(), parts.next())
                && let (Ok(line), Ok(column)) = (l.trim().parse::<u32>(), c.trim().parse::<u32>())
            {
                return SourceError { file: PathBuf::from(&message[..i]), line, column, message: msg.trim().to_string() };
            }
            search = i + 1;
        }
        SourceError { file: fallback.to_path_buf(), line: 0, column: 0, message: message.to_string() }
    }

    fn from_diagnostic(file: &Path, d: &Diagnostic) -> SourceError {
        SourceError { file: file.to_path_buf(), line: d.line, column: d.column, message: format!("error[{}]: {}", d.code, d.message) }
    }
}

/// What happened to the story since the last [`LiveStory::take_report`].
#[derive(Clone, Debug, Default)]
pub struct Report {
    /// The story's current errors (empty: everything compiled and mounted).
    pub errors: Vec<SourceError>,
    /// A reload or an edit was applied (the cells changed).
    pub applied: bool,
}

/// The selected story's embed and cells.
pub struct LiveStory {
    pub root: PathBuf,
    pub variants: Vec<LiveVariant>,
    backend: SkelterBackend,
    embed: Embed,
    provider: Rc<SourceProvider>,
    entry: Option<svelters_skelter::embed::EntryId>,
    /// Variants left out of the entry because they did not compile (their cells show the error).
    broken: Vec<usize>,
    cells: Vec<(CellMount, SlotId, Option<RootId>)>,
    errors: Vec<SourceError>,
    /// Shared with the host's reload hook.
    reloads: Rc<RefCell<Vec<svelters_host::ReloadReport>>>,
    applied: bool,
    generation: u64,
}

impl LiveStory {
    /// Loads `variants` (files under the gallery root that holds `package.json`) into a fresh embed
    /// over `doc`. A variant that does not compile is left out and reported; the rest load.
    pub fn load(doc: Rc<RefCell<Document>>, kinds: Arc<Kinds>, variants: Vec<LiveVariant>, generation: u64) -> Result<LiveStory, String> {
        let first = variants.first().ok_or("the story has no variants")?.file.clone();
        let backend = SkelterBackend::shared(kinds, doc)?;
        // as skelter_gallery's renderer: the same locale, so formatted text matches the CLI's shots
        let host = svelters_host::HostConfig { echo_console: false, echo_errors: false, quit_on_last_window: false, locale: Some("en-US".into()), ..Default::default() };
        let embed = Embed::with_backend(backend.clone(), EmbedConfig { host, ..EmbedConfig::default() }).map_err(|e| e.to_string())?;
        let mut cfg = SourceConfig::for_path(&first)?;
        cfg.backend = "skelter".into();
        cfg.dev = true;
        cfg.hmr = true;
        let root = cfg.root.clone();
        // one provider for the story's lifetime: its overlay holds the unsaved buffers
        let mut errors = Vec::new();
        let mut broken = Vec::new();
        let probe = SourceProvider::new(cfg.clone())?;
        for (i, v) in variants.iter().enumerate() {
            let check = probe.check(&v.file);
            if let Some(e) = check.error {
                errors.push(SourceError::parse(&e, &v.file));
                broken.push(i);
            } else if let Some(d) = check.diagnostics.iter().find(|d| d.severity == Severity::Error) {
                errors.push(SourceError::from_diagnostic(&v.file, d));
                broken.push(i);
            }
        }
        cfg.entry = Entry::Embed(variants.iter().enumerate().filter(|(i, _)| !broken.contains(i)).map(|(_, v)| (v.export.clone(), v.file.clone())).collect());
        let provider = Rc::new(SourceProvider::new(cfg)?);
        let mut story = LiveStory {
            root,
            variants,
            backend,
            embed,
            provider,
            entry: None,
            broken,
            cells: Vec::new(),
            errors,
            reloads: Rc::default(),
            applied: false,
            generation,
        };
        if story.broken.len() < story.variants.len() {
            match story.embed.load_from(story.provider.clone()) {
                Ok(e) => story.entry = Some(e),
                Err(e) => story.errors.push(SourceError::parse(&e.message, &first)),
            }
        }
        let reloads = story.reloads.clone();
        story.embed.host().on_reload(move |r| reloads.borrow_mut().push(r.clone()));
        Ok(story)
    }

    pub fn backend(&self) -> &SkelterBackend {
        &self.backend
    }

    pub fn embed(&self) -> &Embed {
        &self.embed
    }

    pub fn provider(&self) -> &Rc<SourceProvider> {
        &self.provider
    }

    /// Starts the host's file watcher (needs a loop that delivers the backend's wakes).
    pub fn watch(&self) -> Result<(), String> {
        if self.entry.is_none() {
            return Err("nothing loaded to watch".into());
        }
        self.embed.host().watch().map_err(|e| e.message)
    }

    /// Mounts `cells` (replacing the cells mounted before). A cell of a variant that did not
    /// compile gets no root.
    pub fn mount(&mut self, cells: Vec<CellMount>) {
        self.unmount_all();
        for c in cells {
            let slot = match self.embed.adopt_slot(format!("{}#{}", c.key, self.generation), c.frame) {
                Ok(s) => s,
                Err(e) => {
                    self.errors.push(SourceError { file: self.root.clone(), line: 0, column: 0, message: format!("cell {}: {e}", c.key) });
                    continue;
                }
            };
            let root = match (self.entry, self.broken.contains(&c.variant)) {
                (Some(entry), false) => {
                    let export = self.variants[c.variant].export.clone();
                    let spec = MountSpec::new(entry, export, slot, format!("{}#{}", c.key, self.generation)).props(Props::json(c.props.clone()));
                    self.embed.mount(spec).ok()
                }
                _ => None,
            };
            self.cells.push((c, slot, root));
        }
        self.applied = true;
    }

    fn unmount_all(&mut self) {
        for (_, slot, _) in self.cells.drain(..) {
            self.embed.destroy_slot(slot);
        }
    }

    /// Runs the embed's logic (the only place its JS runs outside input), then reapplies the
    /// cells' forced states to whatever the roots render now.
    pub fn logic(&mut self) {
        self.embed.logic();
        for ev in self.embed.take_events() {
            match ev {
                EmbedEvent::Faulted { root, error, .. } => {
                    let file = self.cells.iter().find(|(_, _, r)| *r == Some(root)).map(|(c, _, _)| self.variants[c.variant].file.clone()).unwrap_or_else(|| self.root.clone());
                    let e = SourceError::parse(&error.error.message, &file);
                    if !self.errors.contains(&e) {
                        self.errors.push(e);
                    }
                }
                EmbedEvent::Error { error, .. } => {
                    let e = SourceError::parse(&error.message, &self.root);
                    if !self.errors.contains(&e) {
                        self.errors.push(e);
                    }
                }
                _ => {}
            }
        }
        let reports: Vec<_> = self.reloads.borrow_mut().drain(..).collect();
        for r in reports {
            self.note_reload(&r);
        }
        self.force_states();
    }

    fn note_reload(&mut self, r: &svelters_host::ReloadReport) {
        if r.ok {
            // what compiled now replaces what failed before, for the files it swapped
            let swapped: Vec<PathBuf> = r.hot.iter().flatten().map(PathBuf::from).collect();
            self.errors.retain(|e| !swapped.contains(&e.file));
            self.applied = true;
        } else if let Some(e) = &r.error {
            let e = SourceError::parse(&e.message, &self.root);
            self.errors.retain(|x| x.file != e.file);
            self.errors.push(e);
        }
    }

    /// Previews `text` as the content of `file` without saving: the overlay holds it and the
    /// host swaps the changed module in place. A compile error keeps the last good render.
    pub fn preview(&mut self, file: &Path, text: &str) {
        self.provider.overlay().set(file, text);
        self.apply(file);
    }

    /// Back to the file on disk (after a save, or to drop an unsaved buffer).
    pub fn clear_preview(&mut self, file: &Path) {
        self.provider.overlay().clear(file);
    }

    /// Applies a change to `file` now (as the watcher would after its debounce).
    pub fn apply(&mut self, file: &Path) {
        let i = self.variants.iter().position(|v| v.file == file);
        let check = self.provider.check(file);
        let compile_error = check.error.map(|e| SourceError::parse(&e, file)).or_else(|| check.diagnostics.iter().find(|d| d.severity == Severity::Error).map(|d| SourceError::from_diagnostic(file, d)));
        self.errors.retain(|e| e.file != file);
        if let Some(e) = compile_error {
            self.errors.push(e);
            return;
        }
        if i.is_some_and(|i| self.broken.contains(&i)) {
            // it compiles now but is not in the entry: the caller reloads the story
            self.errors.push(SourceError { file: file.to_path_buf(), line: 0, column: 0, message: "fixed: reloading the story".into() });
            return;
        }
        if let Some(r) = self.embed.host().apply_changes(vec![file.to_path_buf()]) {
            self.note_reload(&r);
        }
        self.force_states();
    }

    /// The variant was left out of the entry because it did not compile when the story loaded.
    pub fn needs_reload(&self, file: &Path) -> bool {
        self.variants.iter().position(|v| v.file == file).is_some_and(|i| self.broken.contains(&i)) && !self.errors.iter().any(|e| e.file == file && e.line > 0)
    }

    pub fn errors(&self) -> &[SourceError] {
        &self.errors
    }

    /// Errors of `file`.
    pub fn errors_of(&self, file: &Path) -> Vec<&SourceError> {
        self.errors.iter().filter(|e| e.file == file).collect()
    }

    pub fn take_report(&mut self) -> Report {
        Report { errors: self.errors.clone(), applied: std::mem::take(&mut self.applied) }
    }

    /// The cells as mounted: their spec and root.
    pub fn cells(&self) -> impl Iterator<Item = (&CellMount, Option<RootId>)> {
        self.cells.iter().map(|(c, _, r)| (c, *r))
    }

    /// The cell whose frame holds `node`.
    pub fn cell_of(&self, doc: &Document, node: NodeId) -> Option<usize> {
        let mut n = Some(node);
        while let Some(x) = n {
            if let Some(i) = self.cells.iter().position(|(c, _, _)| c.frame == x) {
                return Some(i);
            }
            n = doc.parent(x);
        }
        None
    }

    fn force_states(&self) {
        let doc = self.backend.document();
        let mut doc = doc.borrow_mut();
        for (c, _, _) in &self.cells {
            let subjects = subjects(&doc, c.frame);
            for n in subjects {
                if doc.forced_state(n) != c.force {
                    doc.force_state(n, c.force);
                }
            }
        }
    }

    /// Unmounts every cell and lets the embed run its teardown.
    pub fn close(mut self) {
        self.unmount_all();
        self.embed.logic();
    }
}

/// The nodes states are forced on: those anchored `subject` under `frame`, else the frame's first
/// child (the variant's root node).
pub fn subjects(doc: &Document, frame: NodeId) -> Vec<NodeId> {
    let out = doc.query().within(frame).anchor("subject").collect();
    if out.is_empty() {
        return doc.children(frame).first().copied().into_iter().collect();
    }
    out
}

/// `file` relative to `root` when inside it, with `/` separators on every OS.
pub fn rel(file: &Path, root: &Path) -> String {
    let r = file.strip_prefix(root).unwrap_or(file);
    r.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/")
}
