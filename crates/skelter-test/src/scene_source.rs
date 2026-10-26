//! [`SkelterScene`]: skelter-native UI as a `svelters_automation::SceneSource` (TEST-V2 Q2), so
//! svelters-automation's selectors (`role=`, `label=`, `text=`, `anchor=`, `pos=`, `nth=`, CSS) and its
//! text snapshot run on a skelter document without svelters-egui.
//!
//! It reads the document through its AccessKit projection, the tree the app sends the platform adapter
//! ([`A11yTree`], folded by the harness; egui islands' subtrees included, under their graft nodes): roles
//! are AccessKit's mapped to lower-case ARIA names, names follow kittest's rule (a `Label` node's text,
//! else the label, else the `labelled_by` labels' text), anchors are author ids (skelter-core's
//! `anchor` prop), rects window points. CSS needs the skelter-core [`Document`]
//! ([`SkelterScene::with_document`]) and matches nodes of the document's own tree (not island
//! subtrees). The scene has one root, the window's, with no key (`root=` matches nothing).

use crate::a11y::{A11yTree, NodeInfo, NodeKey};
use skelter_base::accesskit::{Role, Toggled, TreeId};
use skelter_core::Document;
use std::borrow::Cow;
use std::collections::HashMap;
use svelters_automation::{Error, SceneSource, SourceRoot};

/// A skelter window's UI, read by svelters-automation (build it per query: it snapshots the tree).
pub struct SkelterScene<'a> {
    a11y: &'a A11yTree,
    doc: Option<&'a Document>,
    /// The walk, in tree order.
    walk: Vec<NodeInfo<'a>>,
    index: HashMap<NodeKey, usize>,
    children: HashMap<NodeKey, Vec<NodeKey>>,
}

impl<'a> SkelterScene<'a> {
    /// The tree in `a11y` (a harness's [`crate::Harness::a11y`]).
    pub fn new(a11y: &'a A11yTree) -> Self {
        let walk = a11y.walk();
        let index = walk.iter().enumerate().map(|(i, n)| (n.key(), i)).collect();
        let mut children: HashMap<NodeKey, Vec<NodeKey>> = HashMap::new();
        for n in &walk {
            if let Some(p) = n.parent {
                children.entry(p).or_default().push(n.key());
            }
        }
        SkelterScene { a11y, doc: None, walk, index, children }
    }

    /// CSS selectors match through `doc` (the document whose tree `a11y` projects).
    pub fn with_document(mut self, doc: &'a Document) -> Self {
        self.doc = Some(doc);
        self
    }

    fn info(&self, n: NodeKey) -> Option<&NodeInfo<'a>> {
        self.index.get(&n).map(|&i| &self.walk[i])
    }

    /// A text node's text: a `Label`'s value, else its label.
    fn own_text(&self, n: NodeKey) -> Option<&str> {
        self.info(n)?.match_label()
    }
}

/// AccessKit's role as the lower-case ARIA name svelters-automation's selectors use.
pub fn aria_role(role: Role) -> &'static str {
    match role {
        Role::Button | Role::DefaultButton | Role::DisclosureTriangle => "button",
        Role::CheckBox => "checkbox",
        Role::RadioButton => "radio",
        Role::Switch => "switch",
        Role::TextInput
        | Role::MultilineTextInput
        | Role::SearchInput
        | Role::EmailInput
        | Role::NumberInput
        | Role::PasswordInput
        | Role::PhoneNumberInput
        | Role::UrlInput
        | Role::DateInput
        | Role::DateTimeInput
        | Role::WeekInput
        | Role::MonthInput
        | Role::TimeInput => "textbox",
        Role::Label | Role::TextRun | Role::Paragraph => "label",
        Role::Link => "link",
        Role::Image | Role::Canvas => "img",
        Role::Slider => "slider",
        Role::SpinButton => "spinbutton",
        Role::ComboBox | Role::EditableComboBox => "combobox",
        Role::ProgressIndicator | Role::Meter => "progressbar",
        Role::ScrollBar => "scrollbar",
        Role::ScrollView => "scrollarea",
        Role::Tab => "tab",
        Role::TabList => "tablist",
        Role::TabPanel => "tabpanel",
        Role::List | Role::ListBox => "list",
        Role::ListItem | Role::ListBoxOption => "listitem",
        Role::Menu | Role::MenuListPopup => "menu",
        Role::MenuBar => "menubar",
        Role::MenuItem | Role::MenuItemCheckBox | Role::MenuItemRadio | Role::MenuListOption => "menuitem",
        Role::Table | Role::LayoutTable => "table",
        Role::Grid | Role::TreeGrid => "grid",
        Role::Row | Role::LayoutTableRow => "row",
        Role::Cell | Role::GridCell | Role::LayoutTableCell => "cell",
        Role::ColumnHeader => "columnheader",
        Role::RowHeader => "rowheader",
        Role::Tree => "tree",
        Role::TreeItem => "treeitem",
        Role::Window => "window",
        Role::Dialog => "dialog",
        Role::AlertDialog => "alertdialog",
        Role::Alert => "alert",
        Role::Status => "status",
        Role::Toolbar => "toolbar",
        Role::Tooltip => "tooltip",
        Role::Heading => "heading",
        Role::Group | Role::RadioGroup | Role::Pane | Role::Splitter => "group",
        _ => "generic",
    }
}

impl<'a> SceneSource for SkelterScene<'a> {
    type Node = NodeKey;

    fn roots(&self) -> Vec<SourceRoot<NodeKey>> {
        self.a11y.root_key().map(|top| SourceRoot { key: None, top, visible: true }).into_iter().collect()
    }
    fn children(&self, n: NodeKey) -> Vec<NodeKey> {
        self.children.get(&n).cloned().unwrap_or_default()
    }
    fn role(&self, n: NodeKey) -> &'static str {
        self.info(n).map_or("generic", |i| aria_role(i.role()))
    }
    fn name(&self, n: NodeKey) -> Cow<'_, str> {
        let Some(info) = self.info(n) else { return Cow::Borrowed("") };
        if let Some(l) = info.match_label() {
            return Cow::Borrowed(l);
        }
        let by: Vec<&str> = info.node.labelled_by().iter().filter_map(|l| self.a11y.node((n.0, *l))).filter_map(|l| {
            if l.role() == Role::Label { l.value().or_else(|| l.label()) } else { l.label() }
        }).collect();
        Cow::Owned(by.join(" "))
    }
    fn text(&self, n: NodeKey) -> Cow<'_, str> {
        Cow::Borrowed(self.own_text(n).unwrap_or(""))
    }
    fn value(&self, n: NodeKey) -> Option<String> {
        let info = self.info(n)?;
        let num = |v: f64| if v.fract() == 0.0 { format!("{v:.0}") } else { format!("{v}") };
        match aria_role(info.role()) {
            "textbox" | "combobox" => Some(info.value().unwrap_or("").to_string()),
            "slider" | "spinbutton" | "progressbar" => info.node.numeric_value().map(num),
            _ => None,
        }
    }
    fn states(&self, n: NodeKey) -> Vec<&'static str> {
        let Some(info) = self.info(n) else { return Vec::new() };
        let ak = info.node;
        let mut out = Vec::new();
        if info.toggled() == Some(Toggled::True) {
            out.push("checked");
        }
        if ak.is_selected() == Some(true) {
            out.push("selected");
        }
        if ak.is_expanded() == Some(true) {
            out.push("expanded");
        }
        if info.is_disabled() {
            out.push("disabled");
        }
        if ak.is_read_only() {
            out.push("read-only");
        }
        if self.a11y.focus() == Some(n) {
            out.push("focused");
        }
        out
    }
    fn is_visible(&self, n: NodeKey) -> bool {
        self.info(n).is_some_and(|i| !i.is_hidden() && i.bounds.is_some())
    }
    fn rect(&self, n: NodeKey) -> Option<egui::Rect> {
        let b = self.info(n)?.bounds?;
        Some(egui::Rect::from_min_max(egui::pos2(b.min.x, b.min.y), egui::pos2(b.max.x, b.max.y)))
    }
    fn anchor(&self, n: NodeKey) -> Option<Cow<'_, str>> {
        self.info(n)?.author_id().map(Cow::Borrowed)
    }
    fn hit(&self, pos: egui::Pos2) -> Option<NodeKey> {
        // The last node in tree order whose box holds `pos` is the topmost.
        self.walk
            .iter()
            .rev()
            .filter(|i| !i.is_hidden())
            .find(|i| i.bounds.is_some_and(|b| pos.x >= b.min.x && pos.x < b.max.x && pos.y >= b.min.y && pos.y < b.max.y))
            .map(NodeInfo::key)
    }
    fn select_css(&self, css: &str, among: &[NodeKey]) -> Result<Vec<NodeKey>, Error> {
        let doc = self.doc.ok_or_else(|| {
            Error::new("capability.absent", format!("css `{css}` needs the document (SkelterScene::with_document)"))
        })?;
        let mut out = Vec::new();
        for &n in among {
            // The document's own nodes (AccessKit ids are skelter's node ids); island subtrees have no CSS.
            if n.0 != TreeId::ROOT {
                continue;
            }
            let node = skelter_base::ids::NodeId(n.1.0);
            if doc.matches(node, css).map_err(|e| Error::new("selector.invalid", format!("css `{css}`: {e}")))? {
                out.push(n);
            }
        }
        Ok(out)
    }
}
