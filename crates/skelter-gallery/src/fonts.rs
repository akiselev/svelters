//! The fonts every cell draws with: skelter-text's test fonts plus the gallery's bundled set
//! (`fonts/README.md`): the theme's own families (Inter, JetBrains Mono) and Arabic, CJK and
//! symbol fallbacks, all OFL subsets. System fonts are off by default, so a shot is the same on
//! every machine; [`FontSet::System`] adds the OS's fonts behind the bundled ones (for looking).

use skelter_text::{FontConfig, FontStore};
use std::sync::Arc;

macro_rules! font {
    ($f:literal) => {
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/fonts/", $f)).as_slice()
    };
}

/// The bundled files, in the order they are registered (an earlier font wins a script's fallback).
pub const BUNDLED: &[&[u8]] = &[
    font!("Inter-Light.subset.ttf"),
    font!("Inter-Regular.subset.ttf"),
    font!("Inter-SemiBold.subset.ttf"),
    font!("Inter-Bold.subset.ttf"),
    font!("JetBrainsMono-Regular.subset.ttf"),
    font!("NotoSansArabic-Regular.subset.ttf"),
    font!("NotoSansSC-Regular.subset.ttf"),
    font!("NotoSansKR-Regular.subset.ttf"),
    font!("NotoSansSymbols2-Regular.subset.ttf"),
    font!("NotoSansSymbols-Regular.subset.ttf"),
    font!("NotoSansMath-Regular.subset.ttf"),
];

/// The family a style without one draws with, and what `sans-serif`/`system-ui` resolve to: the
/// theme's body font (`--sv-font-family: Inter, system-ui, sans-serif`).
pub const DEFAULT_FAMILY: &str = "Inter";

/// The families tried, in order, for a Common-script symbol (★ ⚙ ◀ ✂ ⌘ ⋯) the text's own fonts lack.
pub const SYMBOL_CHAIN: &[&str] = &["Noto Sans Symbols 2", "Noto Sans Symbols", "Noto Color Emoji", "Noto Sans Math"];

/// Which fonts a renderer uses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum FontSet {
    /// skelter-text's test fonts and the gallery's bundled set, nothing else: the same pixels on
    /// every machine.
    #[default]
    Bundled,
    /// The bundled set first, then the OS's fonts (`--system-fonts`): for a script the bundled
    /// set lacks. Not deterministic across machines.
    System,
}

/// The configuration for `set`.
pub fn config(set: FontSet) -> FontConfig {
    let mut bundled: Vec<Vec<u8>> = BUNDLED.iter().map(|f| f.to_vec()).collect();
    bundled.extend(skelter_text::test_fonts::ALL.iter().map(|f| f.to_vec()));
    FontConfig { system_fonts: set == FontSet::System, bundled, default_family: DEFAULT_FAMILY.into() }
}

/// A font store for `set`, with the gallery's symbol chain (then the OS's, for [`FontSet::System`]).
pub fn store(set: FontSet) -> Arc<FontStore> {
    let fonts = FontStore::new(config(set));
    let mut chain: Vec<String> = SYMBOL_CHAIN.iter().map(|s| s.to_string()).collect();
    if set == FontSet::System {
        for f in fonts.symbol_chain() {
            if !chain.iter().any(|c| **c == *f) {
                chain.push(f.to_string());
            }
        }
    }
    fonts.set_symbol_chain(chain);
    fonts
}
