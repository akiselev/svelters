//! AS1 (plans/research/ASSETS.md §2.3, §3): assets in egui.
//!
//! [`install`] gives a context the loaders that serve `asset://<root>/<path>` refs (and `data:`
//! uris) to every egui image: the bytes loader, an SVG rasteriser, and, for PNG, WG2b's loader
//! (`crate::elements::images`), which reaches the bytes through `Context::try_load_bytes`. JS only
//! ever hands egui the ref string; Rust code uses [`image`], [`source`], [`texture`] and
//! [`color_image`].
//!
//! A change reported to [`Assets::changed`] forgets the image in egui (every loader and the
//! texture), marks the `image` and `icon` nodes that show it for measure (the next logic pass
//! remeasures them, so a resized SVG relayouts) and repaints. No JS runs.

use crate::elements::images::decode;
use egui::load::{Bytes, BytesLoadResult, BytesLoader, BytesPoll, ImageLoadResult, ImageLoader, ImagePoll, LoadError, SizeHint, SizedTexture, TextureLoadResult, TextureLoader, TexturePoll};
use egui::{ColorImage, Context, Id, ImageSource, TextureHandle, TextureOptions, Vec2};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use svelters_modules::{AssetBytes, AssetRef, Assets, Subscription};

const SVG_MIME: &str = "image/svg+xml";

/// `currentColor` icons render white, then tint to the node's text colour like a glyph.
pub(crate) const SVG_STYLE: &str = "svg { color: #fff }";

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// What `install` keeps per context (in the context's data).
struct State {
    roots: Mutex<Vec<(Arc<Assets>, Subscription)>>,
    /// Uris whose image was forgotten since the frame last drained them.
    changed: Mutex<Vec<String>>,
    /// `texture()`'s cache, by (uri, version, pixel size), with the `pixels_per_point` it was made
    /// at (ZM1: a zoom drops the uri's textures of other scales).
    textures: Mutex<HashMap<TextureKey, (TextureHandle, u32)>>,
}

/// (uri, asset version, pixel size)
type TextureKey = (String, u64, [u32; 2]);

fn state_id() -> Id {
    Id::unique("svelters-egui::assets")
}

fn state(ctx: &Context) -> Option<Arc<State>> {
    ctx.data(|d| d.get_temp::<Arc<State>>(state_id()))
}

/// Serves `asset://` refs of `assets` (and `data:` uris, and the SVG and PNG images over them) to
/// every egui image of `ctx`; idempotent per context. Another registry installed later is served
/// too (the first one that has the ref answers). Reacts to [`Assets::on_change`] for the life of
/// the context.
pub fn install(ctx: &Context, assets: Arc<Assets>) {
    let st = match state(ctx) {
        Some(st) => st,
        None => {
            let st = Arc::new(State { roots: Mutex::default(), changed: Mutex::default(), textures: Mutex::default() });
            ctx.add_bytes_loader(Arc::new(AssetBytesLoader { state: st.clone(), cache: Mutex::default() }));
            ctx.add_image_loader(Arc::new(SvgLoader::default()));
            // ZM1: `data:` SVGs re-render per size like `.svg` uris (egui keys only those on size)
            ctx.add_texture_loader(Arc::new(SvgTextureLoader::default()));
            crate::elements::images::install(ctx);
            ctx.data_mut(|d| d.insert_temp(state_id(), st.clone()));
            st
        }
    };
    let mut roots = lock(&st.roots);
    if roots.iter().any(|(a, _)| Arc::ptr_eq(a, &assets)) {
        return;
    }
    let sink_state = st.clone();
    let sink_ctx = ctx.clone();
    let sub = assets.on_change(move |refs| {
        {
            let mut changed = lock(&sink_state.changed);
            for r in refs {
                sink_ctx.forget_image(r.as_uri());
                changed.push(r.as_uri().to_string());
            }
        }
        sink_ctx.request_repaint();
    });
    roots.push((assets, sub));
}

/// The uris changed since the last call (the frame marks their nodes for measure).
pub(crate) fn take_changed(ctx: &Context) -> Vec<String> {
    state(ctx).map(|st| std::mem::take(&mut *lock(&st.changed))).unwrap_or_default()
}

/// An `egui::Image` of `r`: `ui.add(assets::image(&r).fit_to_exact_size(vec2(16., 16.)).tint(fg))`.
pub fn image(r: &AssetRef) -> egui::Image<'static> {
    egui::Image::new(source(r))
}

/// `r` as an egui image source. (`impl From<&AssetRef> for ImageSource` would break the orphan
/// rules: both types are foreign here.)
pub fn source(r: &AssetRef) -> ImageSource<'static> {
    ImageSource::Uri(Cow::Owned(r.as_uri().to_string()))
}

/// Rasterises (SVG, to fit inside `px`, keeping its aspect ratio) or decodes (PNG, at its own size)
/// the asset.
pub fn color_image(assets: &Assets, r: &AssetRef, px: [u32; 2]) -> Result<ColorImage, String> {
    let bytes = assets.bytes(r).map_err(|e| e.to_string())?;
    if r.mime() == SVG_MIME {
        rasterize(&bytes, SizeHint::Size { width: px[0].max(1), height: px[1].max(1), maintain_aspect_ratio: true })
    } else {
        decode(&bytes, usize::MAX).map(|i| (*i).clone())
    }
}

/// The asset as a texture of `size` points on `ctx`, cached by (ref, version, pixel size). `install`
/// must have run for the registry that serves `r`.
pub fn texture(ctx: &Context, r: &AssetRef, size: Vec2) -> Result<TextureHandle, String> {
    let st = state(ctx).ok_or_else(|| "assets::install has not run on this context".to_string())?;
    let ppp = ctx.pixels_per_point();
    let px = [(size.x * ppp).round().max(1.0) as u32, (size.y * ppp).round().max(1.0) as u32];
    let assets = lock(&st.roots).iter().map(|(a, _)| a.clone()).find(|a| a.is_mounted(r.root())).ok_or_else(|| format!("{r}: no installed registry serves this root"))?;
    let key = (r.as_uri().to_string(), assets.version(r), px);
    if let Some((t, _)) = lock(&st.textures).get(&key) {
        return Ok(t.clone());
    }
    let img = color_image(&assets, r, px)?;
    let t = ctx.load_texture(r.as_uri(), img, TextureOptions::default());
    let mut cache = lock(&st.textures);
    // an older version of the asset, or the same one at another scale (ZM1), is dead
    let scale = ppp.to_bits();
    cache.retain(|k, (_, p)| k.0 != key.0 || (k.1 == key.1 && *p == scale));
    cache.insert(key, (t.clone(), scale));
    Ok(t)
}

// ---- bytes ----

struct AssetBytesLoader {
    state: Arc<State>,
    cache: Mutex<HashMap<String, (Bytes, String)>>,
}

impl AssetBytesLoader {
    const ID: &'static str = concat!(module_path!(), "::AssetBytesLoader");
}

impl BytesLoader for AssetBytesLoader {
    fn id(&self) -> &str {
        Self::ID
    }

    fn load(&self, _ctx: &Context, uri: &str) -> BytesLoadResult {
        let ready = |(bytes, mime): &(Bytes, String)| BytesPoll::Ready { size: None, bytes: bytes.clone(), mime: Some(mime.clone()) };
        if let Some(e) = lock(&self.cache).get(uri) {
            return Ok(ready(e));
        }
        let entry = if uri.starts_with("asset://") {
            let r = AssetRef::parse(uri).map_err(|e| LoadError::Loading(e.to_string()))?;
            let roots: Vec<Arc<Assets>> = lock(&self.state.roots).iter().map(|(a, _)| a.clone()).collect();
            let mut err = format!("{uri}: no installed registry serves this root");
            let mut found = None;
            for a in roots.iter().filter(|a| a.is_mounted(r.root())) {
                match a.bytes(&r) {
                    Ok(b) => {
                        found = Some(b);
                        break;
                    }
                    Err(e) => err = e.to_string(),
                }
            }
            let b = found.ok_or(LoadError::Loading(err))?;
            let bytes = match b {
                AssetBytes::Static(s) => Bytes::Static(s),
                AssetBytes::Shared(s) => Bytes::Shared(s),
            };
            (bytes, r.mime().to_string())
        } else if let Some(rest) = uri.strip_prefix("data:") {
            let (mime, data) = decode_data_uri(rest).map_err(|e| LoadError::Loading(format!("data uri: {e}")))?;
            (Bytes::Shared(data.into()), mime)
        } else {
            return Err(LoadError::NotSupported);
        };
        let poll = ready(&entry);
        lock(&self.cache).insert(uri.to_string(), entry);
        Ok(poll)
    }

    fn forget(&self, uri: &str) {
        lock(&self.cache).remove(uri);
    }

    fn forget_all(&self) {
        lock(&self.cache).clear();
    }

    fn byte_size(&self) -> usize {
        lock(&self.cache).values().map(|(b, m)| b.len() + m.len()).sum()
    }
}

/// `<mime>[;param]*[;base64],<payload>` (the part after `data:`): the mime type (default
/// `text/plain`) and the decoded payload (base64, or percent-encoded text).
fn decode_data_uri(rest: &str) -> Result<(String, Vec<u8>), String> {
    let (head, payload) = rest.split_once(',').ok_or("missing ','")?;
    let mut parts = head.split(';');
    let mime = parts.next().filter(|m| !m.is_empty()).unwrap_or("text/plain").to_ascii_lowercase();
    let base64 = parts.any(|p| p.eq_ignore_ascii_case("base64"));
    let data = if base64 { base64_decode(payload)? } else { percent_decode(payload)? };
    Ok((mime, data))
}

fn base64_decode(s: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' => break,
            b' ' | b'\t' | b'\r' | b'\n' => continue,
            _ => return Err(format!("invalid base64 byte {c:#04x}")),
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Ok(out)
}

fn percent_decode(s: &str) -> Result<Vec<u8>, String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hex = b.get(i + 1..i + 3).and_then(|h| std::str::from_utf8(h).ok()).and_then(|h| u8::from_str_radix(h, 16).ok());
            out.push(hex.ok_or("bad percent escape")?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    Ok(out)
}

// ---- svg ----

/// usvg options: the injected `style_sheet` (`SVG_STYLE`: `currentColor` white; ICR1's colour
/// slots), and no reads outside the document (only `data:` hrefs).
pub(crate) fn svg_options(style_sheet: &str) -> resvg::usvg::Options<'static> {
    let mut o = resvg::usvg::Options::default();
    o.style_sheet = Some(style_sheet.to_string());
    o.image_href_resolver.resolve_string = Box::new(|_, _| None);
    o
}

/// Rasterises an SVG at `hint` (the logic of egui_extras' loader: the hint is in pixels, the SVG's
/// own size in points).
fn rasterize(svg: &[u8], hint: SizeHint) -> Result<ColorImage, String> {
    rasterize_styled(svg, hint, SVG_STYLE)
}

/// [`rasterize`] with another injected style sheet (ICR1's colour slots, `crate::icons`).
pub(crate) fn rasterize_styled(svg: &[u8], hint: SizeHint, style_sheet: &str) -> Result<ColorImage, String> {
    use resvg::tiny_skia::Pixmap;
    use resvg::usvg::{Transform, Tree};
    let tree = Tree::from_data(svg, &svg_options(style_sheet)).map_err(|e| e.to_string())?;
    let src = Vec2::new(tree.size().width(), tree.size().height());
    if src.x <= 0.0 || src.y <= 0.0 {
        return Err("empty svg".into());
    }
    let scaled = match hint {
        SizeHint::Size { width, height, maintain_aspect_ratio } => {
            if maintain_aspect_ratio {
                let mut size = src * (width as f32 / src.x);
                if size.y > height as f32 {
                    size *= height as f32 / size.y;
                }
                size
            } else {
                Vec2::new(width as f32, height as f32)
            }
        }
        SizeHint::Height(h) => src * (h as f32 / src.y),
        SizeHint::Width(w) => src * (w as f32 / src.x),
        SizeHint::Scale(s) => s.into_inner() * src,
    }
    .round();
    let (w, h) = (scaled.x as u32, scaled.y as u32);
    let mut pixmap = Pixmap::new(w, h).ok_or_else(|| format!("svg of {w}x{h} pixels cannot be rasterised"))?;
    resvg::render(&tree, Transform::from_scale(w as f32 / src.x, h as f32 / src.y), &mut pixmap.as_mut());
    Ok(ColorImage::from_rgba_premultiplied([w as usize, h as usize], pixmap.data()).with_source_size(src))
}

type SvgEntry = Result<Arc<ColorImage>, String>;

/// Rasterises SVG at the size egui asks for; cached per (uri, size hint) with the pass it was last
/// asked for in, cleared by `forget`. ZM1: of a uri asked for at several sizes (a zoom, a resize),
/// the sizes not asked for in the last pass are dropped at the end of a pass.
#[derive(Default)]
struct SvgLoader {
    cache: Mutex<SvgCache>,
}

#[derive(Default)]
struct SvgCache {
    pass: u64,
    entries: HashMap<String, HashMap<SizeHint, (u64, SvgEntry)>>,
}

/// Drops the sizes of a multi-size uri not used in the last pass (egui's texture loader's rule).
fn evict_sizes<K, V>(entries: &mut HashMap<K, HashMap<SizeHint, (u64, V)>>, pass: u64) {
    entries.retain(|_, sizes| {
        if sizes.len() >= 2 {
            sizes.retain(|_, (last, _)| pass <= *last + 1);
        }
        !sizes.is_empty()
    });
}

impl SvgLoader {
    const ID: &'static str = concat!(module_path!(), "::SvgLoader");
}

impl ImageLoader for SvgLoader {
    fn id(&self) -> &str {
        Self::ID
    }

    fn load(&self, ctx: &Context, uri: &str, hint: SizeHint) -> ImageLoadResult {
        {
            let mut c = lock(&self.cache);
            let pass = c.pass;
            if let Some((last, e)) = c.entries.get_mut(uri).and_then(|b| b.get_mut(&hint)) {
                *last = pass;
                return e.clone().map(|image| ImagePoll::Ready { image }).map_err(LoadError::Loading);
            }
        }
        // only what can be an SVG goes to the bytes loaders
        if !(uri.starts_with("asset://") || uri.starts_with("data:") || egui::load::has_extension(uri, "svg")) {
            return Err(LoadError::NotSupported);
        }
        match ctx.try_load_bytes(uri)? {
            BytesPoll::Pending { size } => Ok(ImagePoll::Pending { size }),
            BytesPoll::Ready { bytes, mime, .. } => {
                if mime.as_deref() != Some(SVG_MIME) && !egui::load::has_extension(uri, "svg") {
                    return Err(LoadError::NotSupported);
                }
                let e = rasterize(&bytes, hint).map(Arc::new);
                let mut c = lock(&self.cache);
                let pass = c.pass;
                c.entries.entry(uri.to_string()).or_default().insert(hint, (pass, e.clone()));
                e.map(|image| ImagePoll::Ready { image }).map_err(LoadError::Loading)
            }
        }
    }

    fn forget(&self, uri: &str) {
        lock(&self.cache).entries.remove(uri);
    }

    fn forget_all(&self) {
        lock(&self.cache).entries.clear();
    }

    fn end_pass(&self, pass_index: u64) {
        let mut c = lock(&self.cache);
        c.pass = pass_index;
        evict_sizes(&mut c.entries, pass_index);
    }

    fn byte_size(&self) -> usize {
        lock(&self.cache).entries.values().flat_map(|b| b.values()).map(|(_, e)| e.as_ref().map_or(0, |i| i.pixels.len() * 4)).sum()
    }
}

/// `data:image/svg+xml` uris (no `.svg` extension).
fn is_data_svg(uri: &str) -> bool {
    uri.get(..18).is_some_and(|p| p.eq_ignore_ascii_case("data:image/svg+xml"))
}

/// ZM1: textures of `data:` SVGs, one per size hint (physical pixels), so they render at device
/// resolution at any zoom. egui's default texture loader keys only `.svg` uris on the size: it would
/// keep the first raster of a `data:` SVG and scale it. Same eviction rule as egui's.
#[derive(Default)]
struct SvgTextureLoader {
    cache: Mutex<SvgTextures>,
}

type SizedEntry = (u64, (Vec2, TextureHandle));

#[derive(Default)]
struct SvgTextures {
    pass: u64,
    /// per (uri, options) and size hint: (pass last used, (source size, texture))
    entries: HashMap<(String, TextureOptions), HashMap<SizeHint, SizedEntry>>,
}

impl SvgTextureLoader {
    const ID: &'static str = concat!(module_path!(), "::SvgTextureLoader");
}

impl TextureLoader for SvgTextureLoader {
    fn id(&self) -> &str {
        Self::ID
    }

    fn load(&self, ctx: &Context, uri: &str, options: TextureOptions, hint: SizeHint) -> TextureLoadResult {
        if !is_data_svg(uri) {
            return Err(LoadError::NotSupported);
        }
        let mut c = lock(&self.cache);
        let pass = c.pass;
        let sizes = c.entries.entry((uri.to_string(), options)).or_default();
        if let Some((last, (size, t))) = sizes.get_mut(&hint) {
            *last = pass;
            return Ok(TexturePoll::Ready { texture: SizedTexture::new(t.id(), *size) });
        }
        match ctx.try_load_image(uri, hint)? {
            ImagePoll::Pending { size } => Ok(TexturePoll::Pending { size }),
            ImagePoll::Ready { image } => {
                let size = image.source_size;
                let t = ctx.load_texture(uri, image, options);
                let texture = SizedTexture::new(t.id(), size);
                sizes.insert(hint, (pass, (size, t)));
                Ok(TexturePoll::Ready { texture })
            }
        }
    }

    fn forget(&self, uri: &str) {
        lock(&self.cache).entries.retain(|k, _| k.0 != uri);
    }

    fn forget_all(&self) {
        lock(&self.cache).entries.clear();
    }

    fn end_pass(&self, pass_index: u64) {
        let mut c = lock(&self.cache);
        c.pass = pass_index;
        evict_sizes(&mut c.entries, pass_index);
    }

    fn byte_size(&self) -> usize {
        lock(&self.cache).entries.values().flat_map(|b| b.values()).map(|(_, (_, t))| t.byte_size()).sum()
    }
}

/// The nodes that show a changed uri, marked for measure (their size may have changed).
pub(crate) fn mark_nodes(scene: &mut crate::scene::Scene, uris: &[String]) {
    use crate::scene::W;
    let hits: Vec<_> = scene
        .ids()
        .filter(|&id| {
            let i = scene.ix(id);
            match scene.w(i) {
                W::Image { source, .. } | W::Icon { source, .. } => !source.is_empty() && uris.contains(&crate::elements::images::uri_of(source)),
                _ => false,
            }
        })
        .collect();
    for id in hits {
        scene.mark_measure(id);
    }
}
