//! Stand-ins for wave-1 APIs the island needs and that don't exist yet. The consolidation replaces each
//! with the real API it names and deletes this module.

use presto_base::ids::NodeId;

/// An in-app drag as presto carries it: data by type (MIME-like strings, the svelters `drag-type`s),
/// as svelters-egui's `DragPayload` carries a `MimeData`.
///
/// Missing API: presto-core's drag-and-drop session (architecture §11.7: "inside the app, DnD carries a
/// JS-side id payload"). The island bridge needs only the payload, the source and the outcome.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PrestoDrag {
    /// `(type, data)` in the source's order of preference.
    pub entries: Vec<(String, Vec<u8>)>,
    /// The presto node that started the drag (`None` for a drag started in an island or by Rust).
    pub source: Option<NodeId>,
}

impl PrestoDrag {
    pub fn single(ty: impl Into<String>, data: impl Into<Vec<u8>>) -> PrestoDrag {
        PrestoDrag { entries: vec![(ty.into(), data.into())], source: None }
    }

    pub fn get(&self, ty: &str) -> Option<&[u8]> {
        self.entries.iter().find(|(t, _)| t == ty).map(|(_, d)| d.as_slice())
    }
}
