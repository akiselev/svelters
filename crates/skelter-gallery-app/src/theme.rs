//! Themes in one window: the chrome keeps the document's theme (`:root`), and every preview cell
//! takes a theme of its own through a class on its slot node that redeclares the whole token sheet.
//!
//! A theme's sheet ([`Theme::css`]) declares every `--vscode-*` and `--sv-*` token on `:root`, with
//! component tokens as `var()` of their role. The cell class declares the same map on the cell's
//! node, so the `var()`s resolve against the cell's roles and the cell's subtree inherits the whole
//! set: the stock kinds' UA tokens and `base.css` read the cascade first, so they follow it.
//!
//! What a theme changes outside its tokens (a kind that reads the scheme or density from the
//! `Theme` service) follows the chrome's theme, not the cell's; the CLI renders each theme in a
//! document of its own and is the reference for those.

use skelter_widgets::theme::{Contrast, Density, LAYERS, Theme, TokenMap};
use svelters_style::ColorScheme;

/// The four built-in themes, by their gallery names.
pub const BUILTIN: [&str; 4] = ["dark-modern", "light-modern", "hc-dark", "hc-light"];

/// The theme the chrome uses.
pub const CHROME: &str = "dark-modern";

/// A built-in theme by name, at `density`.
pub fn builtin(name: &str, density: Density) -> Option<Theme> {
    let t = match name {
        "dark-modern" => Theme::vscode(ColorScheme::Dark, Contrast::Normal),
        "light-modern" => Theme::vscode(ColorScheme::Light, Contrast::Normal),
        "hc-dark" => Theme::vscode(ColorScheme::Dark, Contrast::High),
        "hc-light" => Theme::vscode(ColorScheme::Light, Contrast::High),
        _ => return None,
    };
    Some(t.with_density(density))
}

/// A density by its name ([`Density::name`]).
pub fn density(name: &str) -> Option<Density> {
    Density::ALL.into_iter().find(|d| d.name() == name)
}

/// The class a cell of theme `name` carries.
pub fn class(name: &str) -> String {
    let safe: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect();
    format!("gal-theme-{safe}")
}

/// Every theme the gallery offers: the four built-ins, then a host's extra token sets (each on its
/// base, Dark Modern unless `skelter_gallery::Extensions::theme_with` named another).
#[derive(Clone, Debug, Default)]
pub struct Themes {
    pub extra: Vec<(String, TokenMap)>,
    /// G5: a host theme's scheme and contrast (`Extensions::theme_with`); Dark Modern's otherwise.
    pub bases: Vec<(String, ColorScheme, Contrast)>,
}

impl Themes {
    pub fn names(&self) -> Vec<String> {
        BUILTIN.iter().map(|s| s.to_string()).chain(self.extra.iter().map(|(n, _)| n.clone())).collect()
    }

    /// Theme `name` at `density`; `None` when unknown.
    pub fn get(&self, name: &str, density: Density) -> Option<Theme> {
        if let Some(t) = builtin(name, density) {
            return Some(t);
        }
        let (_, map) = self.extra.iter().find(|(n, _)| n == name)?;
        let (scheme, contrast) = self.bases.iter().find(|(n, _, _)| n == name).map_or((ColorScheme::Dark, Contrast::Normal), |(_, s, c)| (*s, *c));
        Some(Theme::vscode(scheme, contrast).with(map.iter()).with_density(density))
    }

    /// The sheet with one class rule per theme at `density` (the cells' themes).
    pub fn cell_sheet(&self, density: Density) -> String {
        let mut s = format!("{LAYERS}\n@layer sv.tokens {{\n");
        for name in self.names() {
            let Some(t) = self.get(&name, density) else { continue };
            s.push_str(&format!(".{} {{\n", class(&name)));
            for (k, v) in t.sheet_tokens().iter() {
                s.push_str(&format!("  {k}: {v};\n"));
            }
            s.push_str("}\n");
        }
        s.push_str("}\n");
        s
    }
}

/// The gallery's own classes (the chrome's look; cells take none of it).
pub const CHROME_CSS: &str = r#"
@layer components {
  .gal-root { display: flex; flex-direction: column; width: 100%; height: 100%; background-color: var(--vscode-editor-background); color: var(--vscode-foreground); }
  .gal-bar { display: flex; flex-direction: row; flex-wrap: wrap; align-items: center; gap: 8px; padding: 4px 8px; border-bottom: 1px solid var(--vscode-panel-border); background-color: var(--vscode-sideBar-background); }
  .gal-main { display: flex; flex-direction: row; flex-grow: 1; min-height: 0; }
  .gal-side { display: flex; flex-direction: column; width: 260px; flex-shrink: 0; border-right: 1px solid var(--vscode-panel-border); background-color: var(--vscode-sideBar-background); }
  .gal-canvas { display: flex; flex-direction: column; flex-grow: 1; min-width: 0; }
  .gal-right { display: flex; flex-direction: column; width: 480px; flex-shrink: 0; border-left: 1px solid var(--vscode-panel-border); }
  .gal-cells { display: flex; flex-direction: column; gap: 16px; padding: 12px; }
  .gal-row { display: flex; flex-direction: row; flex-wrap: wrap; gap: 12px; align-items: flex-start; }
  .gal-cell { display: flex; flex-direction: column; gap: 2px; }
  .gal-caption { font-size: 11px; color: var(--vscode-descriptionForeground); }
  .gal-frame { display: flex; flex-direction: column; overflow: hidden; padding: 8px; border: 1px solid var(--vscode-panel-border); background-color: var(--vscode-editor-background); color: var(--vscode-foreground); }
  .gal-frame-picked { border-color: var(--vscode-focusBorder); }
  .gal-error { color: var(--vscode-errorForeground); font-size: 12px; margin: 0 8px; }
  .gal-family { font-weight: 700; padding: 8px 8px 2px 8px; }
  .gal-gap { color: var(--vscode-errorForeground); }
  .gal-heading { font-weight: 700; padding: 4px 8px; }
  .gal-item { display: flex; flex-direction: row; padding: 2px 8px; }
  .gal-item:hover { background-color: var(--vscode-list-hoverBackground); }
  .gal-item-on { background-color: var(--vscode-list-activeSelectionBackground); color: var(--vscode-list-activeSelectionForeground); }
  .gal-pane { display: flex; flex-direction: column; flex-grow: 1; min-height: 0; }
}
"#;
