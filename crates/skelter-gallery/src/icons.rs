//! The gallery's demo icon set and story assets, so `icon name=…`, `button icon=…` and an imported
//! SVG draw in a cell (GAL2 requests 3 and 4).
//!
//! - **Icons:** [`DEMO`], simple 16-unit line icons drawn for the gallery (`currentColor`, so they
//!   tint like any registry icon), registered on the process's registry under host keys by
//!   [`register_demo`]. A host registers its own set in a setup hook or before `cli_main`; its keys
//!   win when they match.
//! - **Assets:** an `import x from './x.svg'` gives an `asset://app/<path>` ref, which skelter's
//!   `image` and `icon` `source` do not resolve by themselves. [`register_assets`] registers the
//!   image files of a story's directory as image sources under the refs the module loader gives
//!   them, so the import draws.

use skelter_widgets::paint::icon::registry::{Icon, IconRegistry, Owner};
use std::path::Path;
use svelters_host::modules::Assets;
use svelters_host::modules::assets::APP_ROOT;

const fn svg(body: &'static str) -> &'static str {
    body
}

/// `(key, SVG)`: a 16 × 16 viewBox, 1.25-unit strokes in `currentColor`.
pub const DEMO: &[(&str, &str)] = &[
    ("file.new", svg(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linejoin="round"><path d="M3.5 1.5h6l3 3v10h-9z"/><path d="M9.5 1.5v3h3"/><path d="M8 7.5v5M5.5 10h5"/></svg>"#)),
    ("file.open", svg(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linejoin="round"><path d="M1.5 3.5h4.5l1.5 1.5h6.5v8.5h-12.5z"/><path d="M1.5 7h12.5"/></svg>"#)),
    ("file.save", svg(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linejoin="round"><path d="M2.5 2.5h9l2 2v9h-11z"/><path d="M5 2.5v3.5h5.5v-3.5"/><path d="M5 13.5v-4h6v4"/></svg>"#)),
    ("edit.copy", svg(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linejoin="round"><path d="M5.5 5.5h8v8h-8z"/><path d="M10.5 3.5v-1h-8v8h1"/></svg>"#)),
    ("edit.cut", svg(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.25"><circle cx="4.5" cy="12" r="2"/><circle cx="11.5" cy="12" r="2"/><path d="M6 10.5l6-9M10 10.5l-6-9"/></svg>"#)),
    ("edit.paste", svg(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linejoin="round"><path d="M5.5 2.5h-2.5v12h10v-12h-2.5"/><path d="M5.5 1.5h5v2.5h-5z"/></svg>"#)),
    ("edit.undo", svg(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linecap="round" stroke-linejoin="round"><path d="M5 3.5l-3 3 3 3"/><path d="M2 6.5h7.5a4 4 0 0 1 0 8h-3"/></svg>"#)),
    ("edit.redo", svg(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linecap="round" stroke-linejoin="round"><path d="M11 3.5l3 3-3 3"/><path d="M14 6.5h-7.5a4 4 0 0 0 0 8h3"/></svg>"#)),
    ("search", svg(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linecap="round"><circle cx="6.5" cy="6.5" r="4.5"/><path d="M10 10l4.5 4.5"/></svg>"#)),
    ("settings", svg(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linecap="round"><circle cx="8" cy="8" r="2.25"/><path d="M8 1v2.5M8 12.5v2.5M1 8h2.5M12.5 8h2.5M3 3l1.8 1.8M11.2 11.2l1.8 1.8M3 13l1.8-1.8M11.2 4.8l1.8-1.8"/></svg>"#)),
    ("close", svg(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linecap="round"><path d="M3.5 3.5l9 9M12.5 3.5l-9 9"/></svg>"#)),
    ("add", svg(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linecap="round"><path d="M8 2.5v11M2.5 8h11"/></svg>"#)),
    ("remove", svg(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linecap="round"><path d="M2.5 8h11"/></svg>"#)),
    ("check", svg(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M2.5 8.5l3.5 3.5 7.5-8"/></svg>"#)),
    ("run.start", svg(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linejoin="round"><path d="M4.5 2.5v11l9-5.5z"/></svg>"#)),
    ("run.stop", svg(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linejoin="round"><path d="M3.5 3.5h9v9h-9z"/></svg>"#)),
    ("refresh", svg(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linecap="round" stroke-linejoin="round"><path d="M13 8a5 5 0 1 1-1.5-3.5"/><path d="M12 1.5v3.5h-3.5"/></svg>"#)),
    ("folder", svg(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linejoin="round"><path d="M1.5 3.5h4.5l1.5 1.5h7v8.5h-13z"/></svg>"#)),
    ("warning", svg(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linecap="round" stroke-linejoin="round"><path d="M8 1.5l6.5 12.5h-13z"/><path d="M8 6v4M8 12v.01"/></svg>"#)),
    ("info", svg(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linecap="round"><circle cx="8" cy="8" r="6.5"/><path d="M8 7v4.5M8 4.5v.01"/></svg>"#)),
];

/// Registers [`DEMO`] on the process's icon registry (host keys). A key already registered by a
/// host is kept. Idempotent.
pub fn register_demo() {
    let reg = IconRegistry::global();
    for (key, body) in DEMO {
        if !reg.contains(key) {
            let _ = reg.register(Owner::Host, key, Icon::svg(16, body.as_bytes().to_vec()));
        }
    }
}

/// The extensions of the files [`register_assets`] registers.
const IMAGE_EXTENSIONS: &[&str] = &["svg", "png"];

/// Registers the image files directly in `dir` (a story's directory) as skelter image sources,
/// each under the `asset://app/…` ref `assets` gives it (what an import of it evaluates to).
/// Returns the refs registered, and what failed.
pub fn register_assets(assets: &Assets, dir: &Path) -> (Vec<String>, Vec<String>) {
    let (mut ok, mut failed) = (Vec::new(), Vec::new());
    let Ok(rd) = std::fs::read_dir(dir) else { return (ok, failed) };
    let mut files: Vec<_> = rd.flatten().map(|e| e.path()).filter(|p| p.is_file() && p.extension().and_then(|e| e.to_str()).is_some_and(|e| IMAGE_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))).collect();
    files.sort();
    for f in files {
        let r = assets.register(APP_ROOT, &f).map_err(|e| e.to_string()).and_then(|r| {
            let bytes = std::fs::read(&f).map_err(|e| format!("{}: {e}", f.display()))?;
            skelter_widgets::controls::image::register_image(r.as_uri(), &bytes).map_err(|e| format!("{}: {e}", f.display()))?;
            Ok(r.as_uri().to_string())
        });
        match r {
            Ok(u) => ok.push(u),
            Err(e) => failed.push(e),
        }
    }
    (ok, failed)
}
