//! The font store: every font face the process can draw, shared by the UI thread (shaping) and the
//! compositor thread (rasterising) as an `Arc<FontStore>`.
//!
//! - **Collection.** One fontique `Collection` in shared mode, so each [`crate::TextSystem`] clones it
//!   and sees fonts registered later. `system_fonts: false` gives a hermetic set (tests).
//! - **Face ids.** Shaped runs name faces by [`FontId`], interned here from fontique's (blob id, face
//!   index); [`FontStore::face`] turns an id back into the font bytes for the rasteriser.
//! - **Variations.** A run's normalised variation coordinates are interned by hash
//!   ([`FontStore::intern_coords`]), so a `GlyphRun`/`GlyphKey` carries a `u64` and the rasteriser
//!   gets the coordinates back with [`FontStore::coords`].
//! - **Generation.** [`FontStore::generation`] increases whenever the font set changes; every cache
//!   keyed on shaping results includes it.
//! - **Fallback for bundled fonts.** With system fonts off, fontique has no fallback source, so each
//!   bundled family is registered as the fallback of every script whose sample text it covers.
//!   Bundled colour fonts (COLR, CBDT, sbix) join the `emoji` generic family, which parley queries
//!   for emoji clusters.

use parley::fontique::{
    self, Blob, Collection, CollectionOptions, FallbackKey, FamilyId, GenericFamily, Script, ScriptExt,
};
use parley::FontData;
use presto_base::ids::FontId;
use skrifa::{FontRef, MetadataProvider};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

/// What fonts a [`FontStore`] starts with.
#[derive(Clone, Debug)]
pub struct FontConfig {
    /// Enumerate the OS's fonts (DirectWrite, CoreText, fontconfig). Off in tests.
    pub system_fonts: bool,
    /// Font files (TTF/OTF/TTC bytes) registered at startup.
    pub bundled: Vec<Vec<u8>>,
    /// The family a style without a family list falls back to, and what the CSS generics
    /// `sans-serif`/`system-ui` resolve to when system fonts are off.
    pub default_family: String,
}

impl Default for FontConfig {
    fn default() -> Self {
        FontConfig { system_fonts: true, bundled: Vec::new(), default_family: "system-ui".into() }
    }
}

impl FontConfig {
    /// The hermetic test configuration: no system fonts, the bundled Noto subsets
    /// ([`crate::test_fonts`]), `Noto Sans` as the default family.
    pub fn testing() -> Self {
        FontConfig {
            system_fonts: false,
            bundled: crate::test_fonts::ALL.iter().map(|f| f.to_vec()).collect(),
            default_family: crate::test_fonts::SANS_FAMILY.into(),
        }
    }
}

#[derive(Default)]
struct Faces {
    by_key: HashMap<(u64, u32), FontId>,
    data: Vec<FontData>,
}

/// Every face the process draws with. `Send + Sync`; share it as `Arc<FontStore>`.
pub struct FontStore {
    collection: Mutex<Collection>,
    generation: AtomicU64,
    faces: RwLock<Faces>,
    coords: RwLock<HashMap<u64, Arc<[i16]>>>,
    default_family: String,
    system_fonts: bool,
}

impl std::fmt::Debug for FontStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FontStore")
            .field("generation", &self.generation())
            .field("faces", &self.faces.read().unwrap().data.len())
            .field("default_family", &self.default_family)
            .finish()
    }
}

impl FontStore {
    pub fn new(cfg: FontConfig) -> Arc<Self> {
        let collection = Collection::new(CollectionOptions { shared: true, system_fonts: cfg.system_fonts });
        let store = FontStore {
            collection: Mutex::new(collection),
            generation: AtomicU64::new(1),
            faces: RwLock::default(),
            coords: RwLock::default(),
            default_family: cfg.default_family,
            system_fonts: cfg.system_fonts,
        };
        for bytes in cfg.bundled {
            store.register_font_inner(bytes, !cfg.system_fonts);
        }
        if !cfg.system_fonts {
            let mut c = store.collection.lock().unwrap();
            if let Some(id) = c.family_id(&store.default_family) {
                for g in [GenericFamily::SansSerif, GenericFamily::SystemUi, GenericFamily::UiSansSerif] {
                    c.set_generic_families(g, std::iter::once(id));
                }
            }
        }
        Arc::new(store)
    }

    /// Registers the faces in a font file. Returns the families it added to. Bumps the generation.
    pub fn register_font(&self, bytes: Vec<u8>) -> Vec<String> {
        let names = self.register_font_inner(bytes, !self.system_fonts);
        self.generation.fetch_add(1, Ordering::Relaxed);
        names
    }

    fn register_font_inner(&self, bytes: Vec<u8>, as_fallback: bool) -> Vec<String> {
        let blob = Blob::new(Arc::new(bytes));
        let mut c = self.collection.lock().unwrap();
        let added = c.register_fonts(blob.clone(), None);
        let mut names = Vec::new();
        for (family, fonts) in &added {
            if let Some(name) = c.family_name(*family) {
                names.push(name.to_string());
            }
            if let Some(font) = fonts.first() {
                if as_fallback {
                    register_script_fallbacks(&mut c, *family, blob.data(), font.index());
                }
                // parley asks the `emoji` generic family for emoji clusters.
                if is_color_font(blob.data(), font.index()) {
                    c.append_generic_families(GenericFamily::Emoji, std::iter::once(*family));
                }
            }
        }
        names
    }

    /// The font-set generation: changes whenever a font is registered.
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }

    /// Tells the store the OS font set changed (a font installed, `WM_FONTCHANGE`).
    pub fn system_fonts_changed(&self) {
        self.collection.lock().unwrap().load_system_fonts();
        self.generation.fetch_add(1, Ordering::Relaxed);
    }

    pub fn default_family(&self) -> &str {
        &self.default_family
    }

    /// A clone of the shared collection, for a [`parley::FontContext`].
    pub(crate) fn collection(&self) -> Collection {
        self.collection.lock().unwrap().clone()
    }

    /// The id of a shaped run's face, interning it on first use.
    pub fn intern(&self, font: &FontData) -> FontId {
        let key = (font.data.id(), font.index);
        if let Some(id) = self.faces.read().unwrap().by_key.get(&key) {
            return *id;
        }
        let mut faces = self.faces.write().unwrap();
        if let Some(id) = faces.by_key.get(&key) {
            return *id;
        }
        let id = FontId(faces.data.len() as u32);
        faces.data.push(font.clone());
        faces.by_key.insert(key, id);
        id
    }

    /// The bytes and face index of `id`.
    pub fn face(&self, id: FontId) -> Option<FontData> {
        self.faces.read().unwrap().data.get(id.0 as usize).cloned()
    }

    /// Interns normalised variation coordinates (2.14 fixed point); 0 is the default instance.
    pub fn intern_coords(&self, coords: &[i16]) -> u64 {
        if coords.iter().all(|c| *c == 0) {
            return 0;
        }
        let mut h = std::collections::hash_map::DefaultHasher::new();
        coords.hash(&mut h);
        // 0 is reserved for the default instance.
        let key = h.finish().max(1);
        let known = self.coords.read().unwrap().contains_key(&key);
        if !known {
            self.coords.write().unwrap().insert(key, coords.into());
        }
        key
    }

    /// The coordinates interned as `key` (empty for 0 or an unknown key).
    pub fn coords(&self, key: u64) -> Arc<[i16]> {
        if key == 0 {
            return Arc::from(&[][..]);
        }
        self.coords.read().unwrap().get(&key).cloned().unwrap_or_else(|| Arc::from(&[][..]))
    }

    /// Whether some face of `family` maps `ch` (for the symbol fallback chain).
    pub(crate) fn family_has_char(&self, family: &str, ch: char) -> bool {
        let mut c = self.collection.lock().unwrap();
        let Some(info) = c.family_by_name(family) else { return false };
        let Some(font) = info.default_font() else { return false };
        let Some(blob) = font.load(None) else { return false };
        FontRef::from_index(blob.data(), font.index()).map(|f| f.charmap().map(ch).is_some()).unwrap_or(false)
    }
}

fn is_color_font(data: &[u8], index: u32) -> bool {
    use skrifa::raw::TableProvider;
    FontRef::from_index(data, index)
        .map(|f| f.colr().is_ok() || f.cbdt().is_ok() || f.sbix().is_ok())
        .unwrap_or(false)
}

/// Registers `family` as the fallback of each script whose sample text the face covers (at least
/// three quarters of the sample's characters).
fn register_script_fallbacks(c: &mut Collection, family: FamilyId, data: &[u8], index: u32) {
    let Ok(font) = FontRef::from_index(data, index) else { return };
    let charmap = font.charmap();
    for (script, sample) in Script::all_samples() {
        let (mut total, mut have) = (0usize, 0usize);
        for ch in sample.chars() {
            total += 1;
            if charmap.map(ch).is_some() {
                have += 1;
            }
        }
        if total > 0 && have * 4 >= total * 3 {
            c.append_fallbacks(FallbackKey::new(*script, None), std::iter::once(family));
        }
    }
}

// fontique re-exported for callers that build their own queries.
pub use fontique::Language;
