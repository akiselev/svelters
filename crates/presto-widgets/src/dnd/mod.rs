//! Drag and drop inside the app (architecture §11.7; svelters-egui `kinds/dnd.rs` and `interact/dnd.rs`,
//! WG4b, WG6, OW5a1b).
//!
//! Three props make any node a drag source, a drop target, or both ([`DndProps`]):
//! - `drag-type` (non-empty: a source): the payload's type, a MIME-like string (`cad/part`);
//! - `drag-data`: the payload's data, a string, **or** a JS-side id: Svelte keeps the dragged object
//!   and the payload carries its handle ([`PayloadData::Js`]), so no JSON round trip;
//! - `drop-types` (non-empty: a target): accepted types, separated by spaces or commas; `*` accepts
//!   any type and `cad/*` any type starting with `cad/` ([`accepts`]).
//!
//! Signals: the source gets `drag-begin` and `drag-end(dropped, action)`, the target
//! `drag-enter(type)`, `drag-leave` and `drop(type, data, x, y, action)` (`x`, `y` in the target's
//! own points). Every `drag-enter` is matched by a `drag-leave`, a drop included (`drag-leave`, then
//! `drop`). The action comes from the modifiers held at the release ([`DropAction::from_modifiers`]);
//! a file drop from the OS is always `copy`.
//!
//! The pieces:
//! - [`DragSource`]: per source node, turns a press that moves past the drag threshold into a
//!   drag start ([`source`]);
//! - [`DragSession`]: the one drag in flight per app: the payload, the target under the pointer (the
//!   deepest accepting one), the cursor, and the events of every step ([`session`]);
//! - [`ghost`]: the label that follows the pointer and the active target's outline, recorded into
//!   the drag layer's and the target's display lists.
//!
//! The `egui::DragAndDrop` bridge for egui islands is lane EGUI-ISLAND's; it starts and takes drags
//! through [`DragSession::begin`] and [`DragSession::payload`].

pub mod ghost;
pub mod session;
pub mod source;

pub use session::{DndEvent, DragSession, TargetCandidate};
pub use source::{DragSource, SourceIntent};

use presto_base::event::Modifiers;
use presto_base::ids::NodeId;

/// The drag-and-drop props of a node.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DndProps {
    pub drag_type: String,
    pub drag_data: PayloadData,
    pub drop_types: String,
}

impl DndProps {
    pub fn is_source(&self) -> bool {
        !self.drag_type.is_empty()
    }
    pub fn is_target(&self) -> bool {
        !self.drop_types.is_empty()
    }
    /// The payload a drag from this node carries.
    pub fn payload(&self, source: NodeId) -> DragPayload {
        DragPayload { entries: vec![(self.drag_type.clone(), self.drag_data.clone())], source: Some(source) }
    }
}

/// One entry's data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PayloadData {
    Text(String),
    /// A handle to a value JS keeps (svelters-presto resolves it when it delivers `drop`).
    Js(u64),
}

impl Default for PayloadData {
    fn default() -> Self {
        PayloadData::Text(String::new())
    }
}

/// What a drag carries: entries by type, in the source's order of preference (a Svelte drag has
/// one: `drag-type`, `drag-data`; a Rust drag or the clipboard's contents may have several).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DragPayload {
    pub entries: Vec<(String, PayloadData)>,
    /// The source node (`None` for drags from Rust, an egui island or the OS).
    pub source: Option<NodeId>,
}

impl DragPayload {
    /// One text entry, from no node.
    pub fn text(ty: impl Into<String>, data: impl Into<String>) -> DragPayload {
        DragPayload { entries: vec![(ty.into(), PayloadData::Text(data.into()))], source: None }
    }

    /// The first type (a Svelte drag's `drag-type`); `""` when empty.
    pub fn ty(&self) -> &str {
        self.entries.first().map(|e| e.0.as_str()).unwrap_or("")
    }

    /// The entry a target with `drop-types` `types` takes: for the first of its patterns the payload
    /// has a type for, that entry.
    pub fn best(&self, types: &str) -> Option<(&str, &PayloadData)> {
        patterns(types).find_map(|p| self.entries.iter().find(|(t, _)| type_matches(p, t)).map(|(t, d)| (t.as_str(), d)))
    }
}

/// The type a file drop from the OS has; its data is the `file://` URIs, one per line.
pub const URI_LIST: &str = "text/uri-list";

/// Whether type pattern `pattern` matches `ty`: equal, `*`, or a `prefix/*`.
pub fn type_matches(pattern: &str, ty: &str) -> bool {
    pattern == "*" || pattern == ty || pattern.strip_suffix('*').is_some_and(|prefix| prefix.ends_with('/') && ty.starts_with(prefix))
}

fn patterns(types: &str) -> impl Iterator<Item = &str> {
    types.split(|c: char| c.is_whitespace() || c == ',').filter(|t| !t.is_empty())
}

/// Whether `types` (a `drop-types` value) accepts a drag of type `ty`.
pub fn accepts(types: &str, ty: &str) -> bool {
    patterns(types).any(|t| type_matches(t, ty))
}

/// What a drop does with the data (Qt's `Qt::DropAction`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DropAction {
    /// Nothing took it (`drag-end` only).
    #[default]
    None,
    Move,
    Copy,
    Link,
}

impl DropAction {
    pub fn name(self) -> &'static str {
        match self {
            DropAction::None => "none",
            DropAction::Move => "move",
            DropAction::Copy => "copy",
            DropAction::Link => "link",
        }
    }

    pub fn of(name: &str) -> Option<DropAction> {
        Some(match name {
            "none" => DropAction::None,
            "move" => DropAction::Move,
            "copy" => DropAction::Copy,
            "link" => DropAction::Link,
            _ => return None,
        })
    }

    /// The action for the modifiers held (Qt's defaults): Ctrl copies, Ctrl+Shift links; on macOS
    /// Option copies and Cmd+Option links; otherwise move.
    pub fn from_modifiers(m: Modifiers, macos: bool) -> DropAction {
        if macos {
            match (m.contains(Modifiers::SUPER), m.contains(Modifiers::ALT)) {
                (true, true) => DropAction::Link,
                (false, true) => DropAction::Copy,
                _ => DropAction::Move,
            }
        } else {
            match (m.contains(Modifiers::CTRL), m.contains(Modifiers::SHIFT)) {
                (true, true) => DropAction::Link,
                (true, false) => DropAction::Copy,
                _ => DropAction::Move,
            }
        }
    }
}

/// The `file://` URI of an absolute path (RFC 8089; bytes outside the unreserved set, `/` and `:`
/// are percent-encoded; on Windows, `\\` separators become `/` and a drive path gets a leading `/`).
pub fn file_uri(path: &std::path::Path) -> String {
    let s = path.to_string_lossy();
    let s = if cfg!(windows) { s.replace('\\', "/") } else { s.into_owned() };
    let mut out = String::from("file://");
    if !s.starts_with('/') {
        out.push('/');
    }
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' | b':' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// The `text/uri-list` data of dropped files: their URIs, one per line (`\r\n`, RFC 2483).
pub fn uri_list<'a>(paths: impl IntoIterator<Item = &'a std::path::Path>) -> String {
    paths.into_iter().map(file_uri).collect::<Vec<_>>().join("\r\n")
}

/// How close to a scroll container's edge a drag scrolls it, in points.
pub const EDGE: f32 = 24.0;

/// The auto-scroll step for a drag held at `p` in a scroll container's viewport `r`: up to 12
/// points a frame towards the nearer edge, faster nearer it (positive moves the content towards
/// its end, as a scroll offset increase). Zero outside `r` or away from the edges. The views lane's
/// scroll-area applies it while a [`DragSession`] is active.
pub fn edge_scroll(r: presto_base::geom::Rect, p: presto_base::geom::Point) -> presto_base::geom::Vec2 {
    use presto_base::geom::Vec2;
    if !r.contains(p) {
        return Vec2::ZERO;
    }
    let f = |d: f32| if d < EDGE { (EDGE - d) / EDGE * 12.0 } else { 0.0 };
    Vec2::new(f(r.max.x - p.x) - f(p.x - r.min.x), f(r.max.y - p.y) - f(p.y - r.min.y))
}

#[cfg(test)]
mod tests {
    use super::*;
    use presto_base::geom::{Point, Rect};

    #[test]
    fn drop_types_match_exactly_by_prefix_or_any() {
        assert!(accepts("cad/part", "cad/part"));
        assert!(accepts("text/plain, cad/part", "cad/part"));
        assert!(accepts("cad/*", "cad/net"));
        assert!(!accepts("cad/*", "cadx/net"));
        assert!(accepts("*", "anything"));
        assert!(!accepts("cad/part", "cad/net"));
        assert!(!accepts("", "cad/part"));
    }

    #[test]
    fn targets_take_their_first_pattern_the_payload_has() {
        let p = DragPayload { entries: vec![("text/plain".into(), PayloadData::Text("R1 10k".into())), ("cad/part".into(), PayloadData::Js(7))], source: None };
        assert_eq!(p.ty(), "text/plain");
        assert_eq!(p.best("cad/* text/plain"), Some(("cad/part", &PayloadData::Js(7))));
        assert_eq!(p.best("text/plain"), Some(("text/plain", &PayloadData::Text("R1 10k".into()))));
        assert_eq!(p.best("image/png"), None);
        assert_eq!(p.best(""), None);
    }

    #[test]
    fn actions_follow_the_platform_modifiers() {
        let m = |a: Modifiers, b: Modifiers| a.union(b);
        assert_eq!(DropAction::from_modifiers(Modifiers::default(), false), DropAction::Move);
        assert_eq!(DropAction::from_modifiers(Modifiers::CTRL, false), DropAction::Copy);
        assert_eq!(DropAction::from_modifiers(m(Modifiers::CTRL, Modifiers::SHIFT), false), DropAction::Link);
        assert_eq!(DropAction::from_modifiers(Modifiers::ALT, true), DropAction::Copy);
        assert_eq!(DropAction::from_modifiers(m(Modifiers::SUPER, Modifiers::ALT), true), DropAction::Link);
        assert_eq!(DropAction::from_modifiers(Modifiers::CTRL, true), DropAction::Move);
    }

    #[test]
    fn uri_lists_and_edge_scroll() {
        let paths = [std::path::Path::new("/a b/x.step"), std::path::Path::new("/y.kicad_pcb")];
        assert_eq!(uri_list(paths), "file:///a%20b/x.step\r\nfile:///y.kicad_pcb");
        let r = Rect::new(0.0, 0.0, 100.0, 100.0);
        assert_eq!(edge_scroll(r, Point::new(50.0, 50.0)), presto_base::geom::Vec2::ZERO);
        assert!(edge_scroll(r, Point::new(50.0, 2.0)).y < 0.0, "near the top: towards the start");
        assert!(edge_scroll(r, Point::new(50.0, 98.0)).y > 0.0);
        assert_eq!(edge_scroll(r, Point::new(150.0, 98.0)), presto_base::geom::Vec2::ZERO);
    }
}
