//! The chrome's nodes: skelter-widgets kinds built from Rust, by id (the same ids cadatomic's
//! native panes use; `tests/chrome.rs` checks them against the registry).

use skelter_base::ids::NodeId;
use skelter_core::common::{ANCHOR, ARIA_LABEL};
use skelter_core::{Document, KindId, ListenFlags, PropId, SignalId, StyleInput, Value};

pub mod w {
    use skelter_core::{KindId, PropId, SignalId};

    pub const BOX: KindId = KindId::of("box");
    pub const LABEL: KindId = KindId::of("label");
    pub const BUTTON: KindId = KindId::of("button");
    pub const CHECKBOX: KindId = KindId::of("checkbox");
    pub const TEXT_EDIT: KindId = KindId::of("text-edit");
    pub const SCROLL: KindId = KindId::of("scroll-area");
    pub const SELECT: KindId = KindId::of("select");
    pub const CODE_EDITOR: KindId = KindId::of("code-editor");

    pub const LABEL_TEXT: PropId = PropId::of("label", "text", "string");
    pub const LABEL_CLICKABLE: PropId = PropId::of("label", "clickable", "bool");
    pub const LABEL_SELECTABLE: PropId = PropId::of("label", "selectable", "bool");
    pub const BUTTON_TEXT: PropId = PropId::of("button", "text", "string");
    pub const BUTTON_FRAME: PropId = PropId::of("button", "frame", "bool");
    pub const BUTTON_SMALL: PropId = PropId::of("button", "small", "bool");
    pub const BUTTON_SELECTED: PropId = PropId::of("button", "selected", "bool");
    pub const CHECKBOX_TEXT: PropId = PropId::of("checkbox", "text", "string");
    pub const CHECKBOX_CHECKED: PropId = PropId::of("checkbox", "checked", "bool");
    pub const TEXT: PropId = PropId::of("text-edit", "text", "string");
    pub const TEXT_HINT: PropId = PropId::of("text-edit", "hint-text", "string");
    pub const SCROLL_DIRECTION: PropId = PropId::of("scroll-area", "direction", "enum");
    pub const SELECT_OPTIONS: PropId = PropId::of("select", "options", "list");
    pub const SELECT_SELECTED: PropId = PropId::of("select", "selected", "int");
    pub const EDITOR_BUFFER: PropId = PropId::of("code-editor", "buffer", "string");

    pub const CLICKED: SignalId = SignalId::of("button", "clicked", "void");
    pub const LABEL_CLICKED: SignalId = SignalId::of("label", "clicked", "void");
    pub const CHECKBOX_CHANGED: SignalId = SignalId::of("checkbox", "changed", "bool");
    pub const TEXT_CHANGED: SignalId = SignalId::of("text-edit", "changed", "string");
    pub const SELECT_CHANGED: SignalId = SignalId::of("select", "changed", "int");
}

/// Builds nodes into a document.
pub struct Ui<'d> {
    pub doc: &'d mut Document,
}

impl<'d> Ui<'d> {
    pub fn new(doc: &'d mut Document) -> Self {
        Ui { doc }
    }

    /// A node of `kind` under `parent` (appended), with classes and inline style.
    pub fn node(&mut self, parent: Option<NodeId>, kind: KindId, classes: &[&str], inline: &str) -> NodeId {
        let n = self.doc.create(kind);
        self.style(n, classes, inline);
        if let Some(p) = parent {
            let _ = self.doc.insert(p, n, None, None);
        }
        n
    }

    pub fn style(&mut self, n: NodeId, classes: &[&str], inline: &str) {
        self.doc.set_style(
            n,
            StyleInput { classes: classes.iter().map(|c| (*c).into()).collect(), inline: (!inline.is_empty()).then(|| inline.into()), ..Default::default() },
        );
    }

    pub fn set(&mut self, n: NodeId, prop: PropId, v: Value) {
        if let Err(e) = self.doc.set_prop(n, prop, v) {
            eprintln!("skelter-gallery-app: {prop:?}: {e:?}");
        }
    }

    pub fn listen(&mut self, n: NodeId, signal: SignalId) {
        self.doc.listen(n, signal, ListenFlags::default());
    }

    pub fn anchor(&mut self, n: NodeId, anchor: &str) {
        self.set(n, ANCHOR, Value::Str(anchor.into()));
    }

    pub fn aria_label(&mut self, n: NodeId, text: &str) {
        self.set(n, ARIA_LABEL, Value::Str(text.into()));
    }

    pub fn boxed(&mut self, parent: NodeId, classes: &[&str], inline: &str) -> NodeId {
        self.node(Some(parent), w::BOX, classes, inline)
    }

    pub fn label(&mut self, parent: NodeId, text: &str, classes: &[&str]) -> NodeId {
        let n = self.node(Some(parent), w::LABEL, classes, "");
        self.set(n, w::LABEL_TEXT, Value::Str(text.into()));
        n
    }

    /// A label raising `clicked`.
    pub fn link(&mut self, parent: NodeId, text: &str, classes: &[&str], inline: &str) -> NodeId {
        let n = self.node(Some(parent), w::LABEL, classes, inline);
        self.set(n, w::LABEL_TEXT, Value::Str(text.into()));
        self.set(n, w::LABEL_CLICKABLE, Value::Bool(true));
        self.listen(n, w::LABEL_CLICKED);
        n
    }

    pub fn button(&mut self, parent: NodeId, text: &str) -> NodeId {
        let n = self.node(Some(parent), w::BUTTON, &[], "");
        self.set(n, w::BUTTON_TEXT, Value::Str(text.into()));
        self.listen(n, w::CLICKED);
        n
    }

    /// A frameless toggle button (a list row) showing `on`.
    pub fn selectable(&mut self, parent: NodeId, text: &str, on: bool, inline: &str) -> NodeId {
        let n = self.node(Some(parent), w::BUTTON, &[], inline);
        self.set(n, w::BUTTON_TEXT, Value::Str(text.into()));
        self.set(n, w::BUTTON_FRAME, Value::Bool(false));
        self.set(n, w::BUTTON_SELECTED, Value::Bool(on));
        self.listen(n, w::CLICKED);
        n
    }

    pub fn checkbox(&mut self, parent: NodeId, text: &str, on: bool) -> NodeId {
        let n = self.node(Some(parent), w::CHECKBOX, &[], "");
        self.set(n, w::CHECKBOX_TEXT, Value::Str(text.into()));
        self.set(n, w::CHECKBOX_CHECKED, Value::Bool(on));
        self.listen(n, w::CHECKBOX_CHANGED);
        n
    }

    /// A select over `options` showing `selected`, named `label` for assistive technology.
    pub fn select(&mut self, parent: NodeId, label: &str, options: &[String], selected: usize) -> NodeId {
        let n = self.node(Some(parent), w::SELECT, &[], "");
        self.set(n, w::SELECT_OPTIONS, Value::List(options.iter().map(|o| Value::Str(o.as_str().into())).collect::<Vec<_>>().into()));
        self.set(n, w::SELECT_SELECTED, Value::I64(selected as i64));
        self.aria_label(n, label);
        self.listen(n, w::SELECT_CHANGED);
        n
    }

    /// A vertical scroll area under `parent`; children go into the returned content box.
    pub fn scroll(&mut self, parent: NodeId, inline: &str) -> (NodeId, NodeId) {
        let s = self.node(Some(parent), w::SCROLL, &[], inline);
        let content = self.node(Some(s), w::BOX, &[], "display: flex; flex-direction: column");
        (s, content)
    }

    /// Destroys `n`'s children.
    pub fn clear(&mut self, n: NodeId) {
        for c in self.doc.children(n).to_vec() {
            let _ = self.doc.remove(n, c);
            self.doc.destroy(c);
        }
    }
}
