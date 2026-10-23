//! DS1: installing a [`Theme`] on the backend, at run time.

use super::{BASE_CSS, Density, Theme};
use crate::backend::EguiBackend;
use crate::style::{THEME_BASE, THEME_TOKENS};

impl EguiBackend {
    /// Installs `theme`, now and at any later time (no restart; DESIGN.md §4.2):
    /// - its token sheet and the base sheet at theme origin (`@layer sv.tokens`, `sv.base`; the
    ///   runtime's sheet clear keeps them), so Svelte panels restyle at the next logic;
    /// - `prefers-color-scheme` and `prefers-contrast` from its scheme and contrast;
    /// - its egui `Style` on the context ([`Theme::apply_to`]): now if the backend has one, and at
    ///   the next logic in any case, so stock widgets and Rust panels follow;
    /// - a new [`theme_generation`](EguiBackend::theme_generation), for caches of shaped or
    ///   tessellated chrome.
    ///
    /// Sheets the host added with [`set_tokens`](EguiBackend::set_tokens) (unlayered) still win
    /// over the theme's tokens; author rules win over both.
    pub fn set_theme(&self, theme: &Theme) {
        {
            let mut s = self.0.scene.borrow_mut();
            s.style.scheme = Some(theme.scheme());
            s.style.contrast = Some(theme.contrast().media());
            for (name, css) in [(THEME_TOKENS, theme.css()), (THEME_BASE, BASE_CSS.to_string())] {
                for d in s.apply_sheet(name, Some(&css)) {
                    eprintln!("[svelters-egui/theme] {name}:{}:{}: {}", d.line, d.column, d.message);
                }
            }
            // DS3 slots in here: `@theme:affordance` (`@layer sv.affordance`) when
            // `theme.affordances()`, removed otherwise.
            // U3: utility values resolve against this theme's token names
            s.set_theme_tokens(theme.utility_tokens());
        }
        *self.0.theme.borrow_mut() = Some(theme.clone());
        self.0.theme_generation.set(self.0.theme_generation.get() + 1);
        self.0.theme_pending.set(true);
        if let Some(ctx) = self.ctx() {
            theme.apply_to(&ctx);
            ctx.request_repaint();
        }
        self.touch_all();
    }

    /// The installed theme (`None` before [`set_theme`](EguiBackend::set_theme): stock egui
    /// visuals, and the default theme's tokens for the cascade).
    pub fn theme(&self) -> Option<Theme> {
        self.0.theme.borrow().clone()
    }

    /// Bumped by every [`set_theme`](EguiBackend::set_theme) (0 before the first).
    pub fn theme_generation(&self) -> u64 {
        self.0.theme_generation.get()
    }

    /// Switches the installed theme's density (installs the default theme first if none is).
    pub fn set_density(&self, density: Density) {
        let t = self.theme().unwrap_or_default();
        self.set_theme(&t.with_density(density));
    }

    /// At logic: the context gets the installed theme's Style if `set_theme` ran since (the
    /// backend may have had no context then, or an embedder's context replaced it).
    pub(crate) fn apply_pending_theme(&self, ctx: &egui::Context) {
        if self.0.theme_pending.replace(false)
            && let Some(t) = self.0.theme.borrow().as_ref()
        {
            t.apply_to(ctx);
        }
    }
}
