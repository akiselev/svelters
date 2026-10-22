//! Read access to the scene from outside the frame: [`EguiBackend::with_scene`],
//! [`EguiBackend::prop`] and [`EguiBackend::handles_of`] (tests, devtools).
//!
//! SG1 replaces them with `Embed::scene() -> SceneHandle` and the `SceneRef`/`NodeRef`/`SceneMut`/
//! `NodeMut` access API (ARCH-SCENE §6); tests then use `scene.of_kind`. Writer lane: SCENE.

use super::{Arena, Handle};
use crate::backend::EguiBackend;
use crate::kinds;
use svelters_abi::Value;

impl EguiBackend {
    /// Reads the scene (tests, devtools).
    pub fn with_scene<R>(&self, f: impl FnOnce(&Arena) -> R) -> R {
        f(&self.0.scene.borrow())
    }

    /// A prop of a live node by name, as the host would read it.
    pub fn prop(&self, h: Handle, name: &str) -> Result<Value, String> {
        let scene = self.0.scene.borrow();
        let n = scene.get(h).ok_or_else(|| format!("stale or foreign handle {h}"))?;
        let (_, def) = kinds::find_prop(n.kind, name).ok_or_else(|| format!("<{}> has no prop {name}", n.tag()))?;
        super::get_prop(n, def)
    }

    /// The live handles of kind `tag` in creation order of their slots (tests).
    pub fn handles_of(&self, tag: &str) -> Vec<Handle> {
        let scene = self.0.scene.borrow();
        let mut v: Vec<Handle> = scene.handles().into_iter().filter(|&h| scene.n(h).tag() == tag).collect();
        v.sort();
        v
    }
}
