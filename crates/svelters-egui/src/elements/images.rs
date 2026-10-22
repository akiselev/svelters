//! The `image` kind's loader: an egui [`ImageLoader`] that reads `file://` uris and decodes PNG
//! (the `image` crate, PNG only) from files and from any bytes loader (`bytes://` uris that
//! `Context::include_bytes` registered). Installed on a context by [`install`] the first time an
//! `image` node is measured. An embedder that installs more loaders (egui_extras' `http`, `svg`,
//! other formats) gets them too: egui asks every loader in turn.
//!
//! Decoding is synchronous and cached per uri, so an `image` is measured at its size in the first
//! layout.

use egui::load::{BytesPoll, ImageLoadResult, ImageLoader, ImagePoll, LoadError, SizeHint};
use egui::{ColorImage, Context};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// The loader's id (`Context::is_loader_installed`).
pub const LOADER_ID: &str = concat!(module_path!(), "::PngLoader");

/// Installs the loader on `ctx` once.
pub fn install(ctx: &Context) {
    if !ctx.is_loader_installed(LOADER_ID) {
        ctx.add_image_loader(Arc::new(PngLoader::default()));
    }
}

/// The uri an `image`'s `source` names: a bare path becomes a `file://` uri.
pub fn uri_of(source: &str) -> String {
    if source.contains("://") { source.to_string() } else { format!("file://{source}") }
}

type Entry = Result<Arc<ColorImage>, String>;

#[derive(Default)]
struct PngLoader {
    cache: Mutex<HashMap<String, Entry>>,
}

/// Decodes a PNG. An empty image is an error (a zero-sized texture is not drawable); one with a
/// side over `max_side` (the renderer's `max_texture_side`, which egui asserts and a GPU backend
/// refuses) is scaled down to fit, keeping its aspect ratio.
fn decode(bytes: &[u8], max_side: usize) -> Entry {
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png).map_err(|e| e.to_string())?;
    let (w, h) = (img.width(), img.height());
    if w == 0 || h == 0 {
        return Err(format!("empty image ({w}x{h})"));
    }
    let max = u32::try_from(max_side.max(1)).unwrap_or(u32::MAX);
    let img = if w > max || h > max { img.resize(max, max, image::imageops::FilterType::Triangle) } else { img };
    let img = img.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    Ok(Arc::new(ColorImage::from_rgba_unmultiplied(size, img.as_raw())))
}

impl ImageLoader for PngLoader {
    fn id(&self) -> &str {
        LOADER_ID
    }

    // ZU3: debt: file:// PNGs are read on the UI thread in egui's ImageLoader::load (first load per uri; AS1's loaders own the fix).
    #[allow(clippy::disallowed_methods)]
    fn load(&self, ctx: &Context, uri: &str, _size_hint: SizeHint) -> ImageLoadResult {
        if let Some(e) = self.cache.lock().unwrap_or_else(|p| p.into_inner()).get(uri) {
            return e.clone().map(|image| ImagePoll::Ready { image }).map_err(LoadError::Loading);
        }
        let max_side = ctx.input(|i| i.max_texture_side);
        let entry = if let Some(path) = uri.strip_prefix("file://") {
            let lower = path.to_ascii_lowercase();
            if !lower.ends_with(".png") {
                return Err(LoadError::NotSupported);
            }
            std::fs::read(path).map_err(|e| format!("{path}: {e}")).and_then(|b| decode(&b, max_side))
        } else {
            match ctx.try_load_bytes(uri) {
                Ok(BytesPoll::Ready { bytes, .. }) => {
                    if !bytes.starts_with(b"\x89PNG") {
                        return Err(LoadError::NotSupported);
                    }
                    decode(&bytes, max_side)
                }
                Ok(BytesPoll::Pending { size }) => return Ok(ImagePoll::Pending { size }),
                Err(e) => return Err(e),
            }
        };
        self.cache.lock().unwrap_or_else(|p| p.into_inner()).insert(uri.to_string(), entry.clone());
        entry.map(|image| ImagePoll::Ready { image }).map_err(LoadError::Loading)
    }

    fn forget(&self, uri: &str) {
        self.cache.lock().unwrap_or_else(|p| p.into_inner()).remove(uri);
    }

    fn forget_all(&self) {
        self.cache.lock().unwrap_or_else(|p| p.into_inner()).clear();
    }

    fn byte_size(&self) -> usize {
        self.cache.lock().unwrap_or_else(|p| p.into_inner()).values().map(|e| e.as_ref().map_or(0, |i| i.pixels.len() * 4)).sum()
    }
}
