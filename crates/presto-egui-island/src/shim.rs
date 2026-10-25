//! Stand-ins for wave-1 APIs the island needs and that don't exist yet. The consolidation replaces each
//! with the real API it names and deletes this module.

use presto_base::ids::{NodeId, TextureId};

/// Where an island's texture is registered for compositing.
///
/// Missing API: `presto_compositor::CompositorHandle::register_texture(id, wgpu::Texture)` (batch 1
/// brief) and an `unregister_texture(id)` on the handle (the brief gives the renderer one, not the
/// handle). The consolidation implements this trait for `CompositorHandle`, or replaces it.
pub trait TextureRegistry {
    /// Register (or replace) the texture composited for `id`. The island calls it whenever it
    /// (re)allocates its target, so after a resize the compositor samples the new texture.
    fn register_texture(&self, id: TextureId, texture: wgpu::Texture);
    fn unregister_texture(&self, id: TextureId);
}

/// The system clipboard's text, as the island needs it for egui's copy, cut and paste.
///
/// Missing API: `presto_platform::Clipboard` (reached through `PlatformCx::clipboard`, batch 1 brief);
/// its methods aren't published yet.
pub trait ClipboardText {
    fn get_text(&mut self) -> Option<String>;
    fn set_text(&mut self, text: String);
}

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
