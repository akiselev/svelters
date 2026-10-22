//! Styling for the GTK backend (ABI v2.3 `style` op; plans/architecture/backend-abi.md).
//!
//! * Component sheets are `GtkCssProvider`s installed for the display at APPLICATION priority, one
//!   per sheet id, so a hot reload of one component re-parses only its provider. The token layer
//!   (`@tokens`) sits just below them.
//! * Classes are applied by diffing against what this module set earlier (`add_css_class` /
//!   `remove_css_class`), never `set_css_classes`, so the widget's own classes (`text-button`,
//!   `toggle`) survive.
//! * Inline declarations are deduplicated by content into shared classes `sv-s<n>` in one
//!   append-only provider above the components, flushed once per main-loop turn. A new widget with
//!   an already seen style costs a class swap; only a new distinct style reloads the provider.
//! * Every provider this module owns gets the scheme through its `prefers-color-scheme` property,
//!   because GTK evaluates `@media (prefers-color-scheme: dark)` per provider.
use super::{Inner, Res};
use gtk::glib;
use gtk::prelude::*;
use std::collections::HashMap;
use svelters_abi::*;

const TOKENS_ID: &str = "@tokens";
const PRIO_TOKENS: u32 = gtk::STYLE_PROVIDER_PRIORITY_APPLICATION - 10;
const PRIO_COMPONENT: u32 = gtk::STYLE_PROVIDER_PRIORITY_APPLICATION;
const PRIO_INLINE: u32 = gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1;

#[derive(Default)]
pub(crate) struct StyleState {
    /// Component providers in registration order (token layer excluded).
    sheets: Vec<(String, gtk::CssProvider)>,
    tokens: Option<gtk::CssProvider>,
    dark: bool,
    /// Classes this module added per widget handle.
    classes: HashMap<u64, Vec<String>>,
    /// The inline class currently on a widget.
    inline_of: HashMap<u64, String>,
    /// Inline declaration text -> class.
    inline_seen: HashMap<String, String>,
    inline_css: String,
    inline_provider: Option<gtk::CssProvider>,
    flush_pending: bool,
    /// Parse errors reported by providers, newest last (for tests and `last_error`-style debugging).
    errors: Vec<String>,
}

fn scheme_value(dark: bool) -> Option<glib::Value> {
    let ty = glib::Type::from_name("GtkInterfaceColorScheme")?;
    let class = glib::EnumClass::with_type(ty)?;
    class.to_value_by_nick(if dark { "dark" } else { "light" })
}

impl Inner {
    fn new_provider(&self, dark: bool) -> gtk::CssProvider {
        let p = gtk::CssProvider::new();
        if let Some(v) = scheme_value(dark) {
            p.set_property_from_value("prefers-color-scheme", &v);
        }
        let me = self.me.clone();
        p.connect_parsing_error(move |_, section, err| {
            let msg = format!("{}: {}", section.to_str(), err.message());
            eprintln!("[svelters-gtk] css parse error: {msg}");
            if let Some(i) = me.upgrade() {
                i.style.borrow_mut().errors.push(msg);
            }
        });
        p
    }

    fn install(&self, p: &gtk::CssProvider, prio: u32) {
        if let Some(d) = gtk::gdk::Display::default() {
            gtk::style_context_add_provider_for_display(&d, p, prio);
        }
    }

    fn uninstall(&self, p: &gtk::CssProvider) {
        if let Some(d) = gtk::gdk::Display::default() {
            gtk::style_context_remove_provider_for_display(&d, p);
        }
    }

    /// CSS parse errors seen so far (drained).
    pub(crate) fn take_style_errors(&self) -> Vec<String> {
        std::mem::take(&mut self.style.borrow_mut().errors)
    }

    pub(crate) fn style_op(&self, op: u32, h: u64, text: Option<&str>) -> Res<i32> {
        // Menu items, menus and context menus are not widgets (menus.rs): nothing to style.
        if h != 0 && self.menu_kind(h).is_some_and(|k| !matches!(k, super::menus::KIND_MENUBAR | super::menus::KIND_TOOLBAR)) {
            return Ok(0);
        }
        match op {
            SVL_STYLE_TARGET => return Ok(SVL_STYLE_TARGET_CSS),
            SVL_STYLE_CLASSES => self.style_classes(h, text.unwrap_or(""))?,
            SVL_STYLE_NAME => {
                let (w, _) = self.widget_of(h)?;
                w.set_widget_name(text.unwrap_or(""));
            }
            SVL_STYLE_INLINE => self.style_inline(h, text)?,
            SVL_STYLE_OPACITY => {
                let (w, _) = self.widget_of(h)?;
                let o = match text {
                    None => 1.0,
                    Some(t) => t.parse::<f64>().map_err(|_| format!("opacity: '{t}' is not a number"))?,
                };
                w.set_opacity(o.clamp(0.0, 1.0));
            }
            // ABI v2.8 (TR1): the visual channels of transitions (visual.rs)
            SVL_STYLE_TRANSFORM | SVL_STYLE_CLIP | SVL_STYLE_INERT => self.visual_op(op, h, text)?,
            SVL_STYLE_SHEET => self.style_sheet(text),
            SVL_STYLE_SCHEME => self.style_scheme(text.unwrap_or("light")),
            other => return Err(format!("unknown style op {other}")),
        }
        Ok(0)
    }

    fn style_classes(&self, h: u64, text: &str) -> Res<()> {
        let (w, _) = self.widget_of(h)?;
        let want: Vec<String> = text.split_whitespace().map(str::to_string).collect();
        let mut st = self.style.borrow_mut();
        let keep_inline = st.inline_of.get(&h).cloned();
        let old = st.classes.remove(&h).unwrap_or_default();
        for c in &old {
            if !want.contains(c) && keep_inline.as_deref() != Some(c.as_str()) {
                w.remove_css_class(c);
            }
        }
        for c in &want {
            w.add_css_class(c);
        }
        if !want.is_empty() {
            st.classes.insert(h, want);
        }
        Ok(())
    }

    fn style_inline(&self, h: u64, text: Option<&str>) -> Res<()> {
        let (w, _) = self.widget_of(h)?;
        let mut st = self.style.borrow_mut();
        let new = text.filter(|t| !t.trim().is_empty()).map(|t| {
            if let Some(c) = st.inline_seen.get(t) {
                return c.clone();
            }
            let class = format!("sv-s{}", st.inline_seen.len() + 1);
            let body: String = t.lines().map(|l| format!("  {l}\n")).collect();
            st.inline_css.push_str(&format!(".{class} {{\n{body}}}\n"));
            st.inline_seen.insert(t.to_string(), class.clone());
            st.flush_pending = true;
            class
        });
        if let Some(old) = st.inline_of.get(&h).cloned() {
            if Some(&old) != new.as_ref() {
                w.remove_css_class(&old);
                st.inline_of.remove(&h);
            }
        }
        if let Some(c) = new {
            w.add_css_class(&c);
            st.inline_of.insert(h, c);
        }
        let schedule = st.flush_pending;
        drop(st);
        if schedule {
            self.schedule_inline_flush();
        }
        Ok(())
    }

    fn schedule_inline_flush(&self) {
        // `flush_pending` stays set until the idle source runs; a second call must not queue another.
        if self.style_flush_queued.replace(true) {
            return;
        }
        let me = self.me.clone();
        glib::idle_add_local_full(glib::Priority::HIGH, move || {
            if let Some(i) = me.upgrade() {
                i.flush_inline();
            }
            glib::ControlFlow::Break
        });
    }

    /// Loads the accumulated inline rules (also callable directly, e.g. by tests, to settle early).
    pub(crate) fn flush_inline(&self) {
        self.style_flush_queued.set(false);
        let (css, dark, existing) = {
            let mut st = self.style.borrow_mut();
            if !st.flush_pending {
                return;
            }
            st.flush_pending = false;
            (st.inline_css.clone(), st.dark, st.inline_provider.clone())
        };
        let p = match existing {
            Some(p) => p,
            None => {
                let p = self.new_provider(dark);
                self.install(&p, PRIO_INLINE);
                self.style.borrow_mut().inline_provider = Some(p.clone());
                p
            }
        };
        #[allow(deprecated)]
        p.load_from_data(&css);
    }

    fn style_sheet(&self, text: Option<&str>) {
        let dark = self.style.borrow().dark;
        let Some(text) = text else {
            // Clear everything a previous bundle registered (hot reload).
            let (sheets, tokens) = {
                let mut st = self.style.borrow_mut();
                (std::mem::take(&mut st.sheets), st.tokens.take())
            };
            for (_, p) in sheets.iter() {
                self.uninstall(p);
            }
            if let Some(t) = tokens {
                self.uninstall(&t);
            }
            return;
        };
        let (id, body) = match text.split_once(SVL_STYLE_SEP) {
            Some((id, body)) => (id, Some(body)),
            None => (text, None),
        };
        if id == TOKENS_ID {
            let Some(body) = body else { return };
            let existing = self.style.borrow().tokens.clone();
            let p = existing.unwrap_or_else(|| {
                let p = self.new_provider(dark);
                self.install(&p, PRIO_TOKENS);
                self.style.borrow_mut().tokens = Some(p.clone());
                p
            });
            #[allow(deprecated)]
            p.load_from_data(body);
            return;
        }
        let existing = self.style.borrow().sheets.iter().find(|(i, _)| i == id).map(|(_, p)| p.clone());
        match (existing, body) {
            (Some(p), Some(body)) => {
                #[allow(deprecated)]
                p.load_from_data(body);
            }
            (Some(p), None) => {
                self.uninstall(&p);
                self.style.borrow_mut().sheets.retain(|(i, _)| i != id);
            }
            (None, Some(body)) => {
                let p = self.new_provider(dark);
                #[allow(deprecated)]
                p.load_from_data(body);
                self.install(&p, PRIO_COMPONENT);
                self.style.borrow_mut().sheets.push((id.to_string(), p));
            }
            (None, None) => {}
        }
    }

    fn style_scheme(&self, text: &str) {
        let scheme = text.split(SVL_STYLE_SEP).next().unwrap_or("light");
        let dark = scheme == "dark";
        let providers: Vec<gtk::CssProvider> = {
            let mut st = self.style.borrow_mut();
            st.dark = dark;
            st.sheets
                .iter()
                .map(|(_, p)| p.clone())
                .chain(st.tokens.clone())
                .chain(st.inline_provider.clone())
                .collect()
        };
        if let Some(v) = scheme_value(dark) {
            for p in &providers {
                p.set_property_from_value("prefers-color-scheme", &v);
            }
        }
        if let Some(s) = gtk::Settings::default() {
            #[allow(deprecated)]
            s.set_gtk_application_prefer_dark_theme(dark);
        }
    }

    /// Forgets the style bookkeeping of a destroyed widget.
    pub(crate) fn style_forget(&self, h: u64) {
        if let Ok(mut st) = self.style.try_borrow_mut() {
            st.classes.remove(&h);
            st.inline_of.remove(&h);
        }
    }
}
