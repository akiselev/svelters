//! OW-CSS measurement helper (ignored): writes the sheets the egui backend installs at startup
//! (the theme's token sheets for both schemes and `base.css`) into `$OWCSS_SHEETS`, for
//! `svelters-style`'s `parse_cost` test. See `plans/milestones/OW-CSS.md`.

use svelters_egui::theme::{BASE_CSS, Contrast, Theme};
use svelters_style::ColorScheme;

#[test]
#[ignore = "measurement helper: OWCSS_SHEETS=<dir> cargo test -p svelters-egui --test css_parse_cost -- --ignored"]
fn dump_startup_sheets() {
    let dir = std::path::PathBuf::from(std::env::var("OWCSS_SHEETS").expect("OWCSS_SHEETS"));
    std::fs::write(dir.join("theme-tokens-dark.css"), Theme::vscode(ColorScheme::Dark, Contrast::Normal).css()).unwrap();
    std::fs::write(dir.join("theme-tokens-light.css"), Theme::vscode(ColorScheme::Light, Contrast::Normal).css()).unwrap();
    std::fs::write(dir.join("theme-base.css"), BASE_CSS).unwrap();
}
