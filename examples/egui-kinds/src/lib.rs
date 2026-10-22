//! RA1 (ARCH-EMBED §8.2): a native egui widget kind defined by an app crate, the way cadatomic defines
//! its own. `<quantity-field>` is a unit-aware numeric field: Svelte binds its SI value, sets its
//! dimension and label, and hears `onaccepted` when Enter commits a parsable entry.
//!
//! ```svelte
//! <quantity-field label="Width" dim="length" bind:value={width} onaccepted={(e, v) => …} />
//! ```
//!
//! The binary dumps the compiler's spec fragment and the typings (`app/.svelters/app`); the app's
//! Vite config compiles against the fragment, and svelte-check checks `App.svelte` against the typings.

use svelters_egui::prelude::*;

/// What a quantity measures.
#[derive(svelters::Enum, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Dim {
    /// Metres; shown in millimetres.
    #[default]
    Length,
    /// Radians; shown in degrees.
    Angle,
}

impl Dim {
    fn units(self) -> &'static [(&'static str, f64)] {
        match self {
            Dim::Length => &[("mm", 0.001), ("cm", 0.01), ("m", 1.0), ("in", 0.0254)],
            Dim::Angle => &[("deg", std::f64::consts::PI / 180.0), ("rad", 1.0)],
        }
    }

    /// `"25 mm"` or `"25"` (the display unit) in SI units.
    pub fn parse(self, text: &str) -> Result<f64, String> {
        let t = text.trim();
        let split = t.find(|c: char| c.is_alphabetic()).unwrap_or(t.len());
        let (num, unit) = (t[..split].trim(), t[split..].trim());
        let n: f64 = num.parse().map_err(|_| format!("`{num}` is not a number"))?;
        let units = self.units();
        let scale = if unit.is_empty() { units[0].1 } else { units.iter().find(|(u, _)| *u == unit).ok_or_else(|| format!("unknown unit `{unit}`"))?.1 };
        Ok(n * scale)
    }

    /// An SI value in the display unit.
    pub fn format(self, si: f64, units_shown: bool) -> String {
        let (u, s) = self.units()[0];
        let v = si / s;
        let v = (v * 1e6).round() / 1e6;
        if units_shown { format!("{v} {u}") } else { format!("{v}") }
    }
}

svelters_egui::state_bits! { pub mod cad { INVALID = "invalid" } }

/// A unit-aware numeric field: `<quantity-field dim="length" bind:value={x} onaccepted={…} />`.
#[svelters::widget(namespace = "cad", tag = "quantity-field", caps = FOCUSABLE | EDITABLE)]
pub struct QuantityField {
    /// The committed value in SI units; the field shows it in the display unit.
    #[prop(bind)]
    value: Option<f64>,
    /// What the value measures.
    #[prop]
    dim: Dim,
    /// Shows the unit after the number.
    #[prop(default = true)]
    units_shown: bool,
    /// The field's label, also its accessible name.
    #[prop]
    label: String,
    /// An inline problem shown under the field (the host's validation).
    #[prop(measure)]
    problem: Option<String>,
    /// Enter committed a parsable entry; carries the SI value.
    #[event]
    accepted: Event<f64>,
    /// The text changed (every keystroke); carries the raw text.
    #[event]
    edited: Event<String>,
    /// The text being edited (Rust-only node state).
    #[state]
    text: String,
    /// The value `text` was last formatted from.
    #[state]
    shown: Option<Option<f64>>,
    #[state]
    parse_error: bool,
}

impl Widget for QuantityField {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, avail: AvailableSpace) -> egui::Vec2 {
        let field = cx.text_field_size(&self.text, avail.width_or(160.0));
        let label = cx.text_size(&self.label, None);
        let problem = if self.problem.is_some() { cx.row_height() } else { 0.0 };
        egui::vec2(field.x + label.x + 8.0, field.y + problem)
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        let focused = ui.ctx().memory(|m| m.has_focus(cx.id()));
        if !focused && self.shown != Some(self.value) {
            self.text = self.value.map(|v| self.dim.format(v, self.units_shown)).unwrap_or_default();
            self.shown = Some(self.value);
        }
        let mut resp = None;
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(self.label.as_str());
                let w = ui.available_width();
                let r = ui.add(egui::TextEdit::singleline(&mut self.text).id(cx.id()).desired_width(w));
                if r.changed() {
                    cx.emit(&self.edited, self.text.clone());
                    self.parse_error = self.dim.parse(&self.text).is_err();
                    cx.set_state(&cad::INVALID, self.parse_error);
                }
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !self.parse_error {
                    if let Ok(v) = self.dim.parse(&self.text) {
                        cx.set(&mut self.value, Some(v));
                        self.shown = None;
                        cx.emit(&self.accepted, v);
                    }
                }
                resp = Some(r);
            });
            if let Some(p) = &self.problem {
                ui.colored_label(ui.visuals().error_fg_color, p.as_str());
            }
        });
        resp.expect("the field was added")
    }

    fn a11y(&self, _cx: &A11yCx<'_>, node: &mut egui::accesskit::Node) {
        node.set_label(self.label.clone());
        if self.parse_error || self.problem.is_some() {
            node.set_invalid(egui::accesskit::Invalid::True);
        }
    }
}

impl QuantityField {
    /// Whether the unit is shown (the `units-shown` prop).
    pub fn units_shown(&self) -> bool {
        self.units_shown
    }
}

/// Where the binary writes the typings, stubs and `kinds.json`: the app's `.svelters/app`.
pub fn dump_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("app/.svelters/app")
}

/// The app bundle (`pnpm build` in `app/`).
pub fn bundle() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("app/dist/app.js")
}

/// Keeps this crate's registrations linked into binaries that only reach them through the registry.
pub fn link() {
    std::hint::black_box(<QuantityField as KindDecl>::KIND);
}
