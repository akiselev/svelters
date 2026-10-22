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
//!
//! EV2b adds a native behaviour, [`Pick`]: `<label use:pick={{ accepts: ['mcad.face'], onpicked }}>`,
//! imported from `svelters:behaviors/cad` (the dumped `behaviors/cad.js`, typed by `behaviors.d.ts`).

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

svelters_egui::state_bits! { pub mod cad { INVALID = "invalid", PICK_TARGET = "pick-target", PICK_HOVER = "pick-hover" } }

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
    /// RA3 (CX3): the committed value, for `bind:value|commit` (same shape as `value`).
    #[event]
    commit: Event<f64>,
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
    /// Where `moveCaret` put the caret (UTF-16 column).
    #[state]
    caret: Option<u32>,
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
                        cx.emit(&self.commit, v);
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

    /// RA3: a single-line text input (its `TextEdit` has the node's id), so the Edit menu's
    /// commands reach it.
    fn text_input(&self) -> Option<bool> {
        Some(false)
    }
}

impl QuantityField {
    /// Whether the unit is shown (the `units-shown` prop).
    pub fn units_shown(&self) -> bool {
        self.units_shown
    }
}

/// RA3: the field's methods, called from Svelte on the element (`bind:this={field}`, then
/// `field.commit()`), the way cadatomic's `<code-editor>` exposes `revealLine(n)`.
#[svelters::widget]
impl QuantityField {
    /// Gives the field keyboard focus.
    pub fn focus(&mut self, cx: &mut MethodCx<'_>) {
        cx.request_focus();
    }

    /// Replaces the text being edited without committing it (as typing does: `onedited` fires);
    /// returns whether it parses.
    pub fn set_text(&mut self, cx: &mut MethodCx<'_>, text: String) -> bool {
        self.text = text;
        cx.emit(&self.edited, self.text.clone());
        self.parse_error = self.dim.parse(&self.text).is_err();
        cx.set_state(&cad::INVALID, self.parse_error);
        !self.parse_error
    }

    /// Commits the text being edited, as Enter does (`bind:value` and `onaccepted` hear it); the SI
    /// value, or null when the text does not parse.
    pub fn commit(&mut self, cx: &mut MethodCx<'_>) -> Option<f64> {
        let v = self.dim.parse(&self.text).ok()?;
        cx.set(&mut self.value, Some(v));
        self.shown = None;
        cx.emit(&self.accepted, v);
        cx.emit(&self.commit, v);
        Some(v)
    }

    /// The committed value in `unit` (one of the dimension's units, e.g. `cm`), or null.
    pub fn value_in(&self, unit: String, places: Option<u32>) -> Option<f64> {
        let (_, scale) = self.dim.units().iter().find(|(u, _)| *u == unit)?;
        let v = self.value? / scale;
        let p = 10f64.powi(places.unwrap_or(6) as i32);
        Some((v * p).round() / p)
    }

    /// Puts the caret at `at` (K1's shape: `{ line, column }`, 0-based; a field has line 0 only);
    /// throws `FieldError` (`out-of-range`) outside the text. A `Json<T>` argument and a `Result`
    /// with a `svelters::Error`, as cadatomic's `<code-editor>` methods take and return.
    #[method(rename = "moveCaret")]
    fn move_caret(&mut self, cx: &mut MethodCx<'_>, at: svelters::Json<Position>) -> Result<bool, FieldError> {
        let len = self.text.encode_utf16().count() as u32;
        if at.line != 0 || at.column > len {
            return Err(FieldError::OutOfRange { line: at.line, column: at.column });
        }
        let changed = self.caret != Some(at.column);
        self.caret = Some(at.column);
        cx.request_repaint();
        Ok(changed)
    }

    /// Not a method: an associated helper stays Rust-only.
    #[method(skip)]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The caret `moveCaret` placed (UTF-16 column), Rust-only.
    #[js(skip)]
    pub fn caret(&self) -> Option<u32> {
        self.caret
    }
}

/// A text position as cadatomic's editor kinds take it: 0-based, column in UTF-16 code units.
#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema, Clone, Copy, Debug, PartialEq)]
pub struct Position {
    pub line: u32,
    pub column: u32,
}

/// What `<quantity-field>`'s methods throw.
#[derive(svelters::Error, Debug)]
pub enum FieldError {
    /// The position is outside the text.
    OutOfRange { line: u32, column: u32 },
}

impl std::fmt::Display for FieldError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FieldError::OutOfRange { line, column } => write!(f, "{line}:{column} is outside the text"),
        }
    }
}

/// RA3, the fn form: a level meter, `<level-meter level={0.4} label="Load" />`. Stateless: the fn
/// draws it from its props every frame; its size is what it allocates (measured in a sizing pass).
#[svelters::widget(namespace = "cad")]
pub fn level_meter(ui: &mut egui::Ui, #[prop] level: f64, #[prop(default = 120.0)] width: f64, label: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width as f32, 14.0), egui::Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 2.0, ui.visuals().extreme_bg_color);
    let fill = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width() * level.clamp(0.0, 1.0) as f32, rect.height()));
    p.rect_filled(fill, 2.0, ui.visuals().selection.bg_fill);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::ProgressIndicator, true, format!("{label} {:.0}%", level * 100.0)));
    resp
}

/// The fn form with a bound prop: `<step-count bind:count={n} />`; the `&mut` parameter is what
/// the fn writes, announced to `bind:count`.
#[svelters::widget(namespace = "cad")]
pub fn step_count(ui: &mut egui::Ui, #[prop(bind)] count: &mut i64) -> egui::Response {
    ui.horizontal(|ui| {
        if ui.button("-").clicked() {
            *count -= 1;
        }
        ui.label(format!("count {count}"));
        if ui.button("+").clicked() {
            *count += 1;
        }
    })
    .response
}

svelters::wrap_widget! {
    /// RA3, `wrap_widget!`: egui's `DragValue`, `<wrapped-drag-value bind:value={x} speed={0.1} suffix=" mm" />`.
    /// egui writes `value` in place; the macro announces the change (`bind:value` is exact).
    #[widget(namespace = "cad", tag = "wrapped-drag-value")]
    pub struct DragValueKind {
        /// The value.
        #[prop(bind)]
        value: f64,
        /// How much a pixel of drag changes it.
        #[prop(default = 1.0)]
        speed: f64,
        /// Text after the number.
        #[prop]
        suffix: String,
    }
    |p, ui| ui.add(egui::DragValue::new(&mut p.value).speed(p.speed).suffix(p.suffix.clone()))
}

/// RA3, a container kind: `<collapsing-section title="Advanced" bind:open>…children…</collapsing-section>`.
/// Its children (any elements) are laid out under the header while open (`Widget::layout`) and
/// drawn with `cx.children(ui)`.
#[svelters::widget(namespace = "cad", caps = CONTAINER | CUSTOM_LAYOUT)]
pub struct CollapsingSection {
    /// The header text, also its accessible name.
    #[prop(measure)]
    title: String,
    /// Whether the children show.
    #[prop(bind, default = true)]
    open: bool,
}

const SECTION_HEADER: f32 = 20.0;

impl Widget for CollapsingSection {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, _avail: AvailableSpace) -> egui::Vec2 {
        let t = cx.text_size(&self.title, None);
        egui::vec2(t.x + 20.0, SECTION_HEADER)
    }

    fn layout(&mut self, cx: &mut LayoutCx<'_>, inputs: svelters_egui::taffy::LayoutInput) -> svelters_egui::taffy::LayoutOutput {
        use svelters_egui::taffy::{LayoutOutput, RunMode, Size};
        let width = inputs.known_dimensions.width.or(inputs.available_space.width.into_option()).unwrap_or(200.0);
        let mut y = SECTION_HEADER;
        let mut w: f32 = 0.0;
        if self.open {
            for c in cx.children().to_vec() {
                let s = cx.measure(c, egui::vec2(width - 12.0, 10000.0));
                if inputs.run_mode == RunMode::PerformLayout {
                    cx.place(c, egui::pos2(12.0, y), s);
                }
                y += s.y + 4.0;
                w = w.max(s.x + 12.0);
            }
        }
        LayoutOutput::from_outer_size(Size { width: inputs.known_dimensions.width.unwrap_or(w.max(width)), height: inputs.known_dimensions.height.unwrap_or(y) })
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        let header = egui::Rect::from_min_size(cx.rect().min, egui::vec2(cx.rect().width(), SECTION_HEADER));
        let resp = ui.interact(header, cx.id(), egui::Sense::click());
        let p = ui.painter();
        let mark = if self.open { "-" } else { "+" };
        let color = ui.visuals().text_color();
        p.text(header.left_center(), egui::Align2::LEFT_CENTER, format!("{mark} {}", self.title), egui::TextStyle::Button.resolve(ui.style()), color);
        if resp.clicked() {
            let open = !self.open;
            cx.set(&mut self.open, open);
            cx.request_measure();
        }
        if self.open {
            cx.children(ui);
        }
        resp
    }

    fn a11y(&self, _cx: &A11yCx<'_>, node: &mut egui::accesskit::Node) {
        node.set_role(egui::accesskit::Role::Button);
        node.set_label(self.title.clone());
        node.set_expanded(self.open);
    }
}

/// RA3 (on SG6's paint cache): `<spark-line values={[…]} />` builds its shapes through
/// `cx.cached_shapes`, so they are rebuilt only when `values` (its version) or its size change.
#[svelters::widget(namespace = "cad", tag = "spark-line")]
pub struct Sparkline {
    /// The samples, drawn left to right, scaled to their range.
    #[prop]
    values: Vec<f64>,
    /// Bumped when `values` changes (the cache's version).
    #[state]
    version: u64,
    #[state]
    seen: Vec<f64>,
}

impl Widget for Sparkline {
    fn measure(&mut self, _cx: &mut MeasureCx<'_>, avail: AvailableSpace) -> egui::Vec2 {
        egui::vec2(avail.width_or(120.0).min(120.0), 32.0)
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        if self.seen != self.values {
            self.seen = self.values.clone();
            self.version += 1;
        }
        let stroke = egui::Stroke::new(1.5, ui.visuals().text_color());
        let values = &self.values;
        cx.cached_shapes(ui.painter(), self.version, |_painter, rect, shapes| {
            let size = rect.size();
            let (lo, hi) = values.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &v| (a.min(v), b.max(v)));
            let span = if hi > lo { hi - lo } else { 1.0 };
            let n = values.len().max(2) - 1;
            let points = values
                .iter()
                .enumerate()
                .map(|(i, &v)| egui::pos2(size.x * i as f32 / n as f32, size.y - size.y * ((v - lo) / span) as f32))
                .collect();
            shapes.push(egui::Shape::line(points, stroke));
        });
        ui.interact(cx.rect(), cx.id(), egui::Sense::hover())
    }
}

/// RA3: a document an app hands to a kind as an object-handle prop (`Obj<T>`), as K1's
/// `<code-editor doc={…}>`: `new DocHandle('plan.md')` from `svelters:app/cad`.
#[svelters::object(module = "cad")]
pub struct DocHandle {
    name: String,
    edits: u32,
}

#[svelters::export]
impl DocHandle {
    #[js(constructor)]
    pub fn new(name: String) -> DocHandle {
        DocHandle { name, edits: 0 }
    }

    /// The document's name.
    #[js(getter)]
    pub fn name(&self) -> String {
        self.name.clone()
    }

    /// How many times a `<doc-title>` was clicked (written by the kind, in Rust).
    #[js(getter)]
    pub fn edits(&self) -> u32 {
        self.edits
    }
}

/// RA3, an object-handle prop: `<doc-title doc={handle} />` shows the document's name; a click
/// counts an edit on the shared Rust object (no JS involved).
#[svelters::widget(namespace = "cad", tag = "doc-title")]
pub struct DocTitle {
    /// The document (an exported object; null shows "no document").
    #[prop(measure)]
    doc: Option<svelters::Obj<DocHandle>>,
}

impl Widget for DocTitle {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, _avail: AvailableSpace) -> egui::Vec2 {
        cx.text_size(&self.title(), None) + egui::vec2(8.0, 4.0)
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        let resp = ui.interact(cx.rect(), cx.id(), egui::Sense::click());
        ui.painter().text(cx.rect().left_center(), egui::Align2::LEFT_CENTER, self.title(), egui::TextStyle::Body.resolve(ui.style()), ui.visuals().text_color());
        if resp.clicked()
            && let Some(d) = &self.doc
        {
            d.borrow_mut().edits += 1;
        }
        let title = self.title();
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, title.clone()));
        resp
    }
}

impl DocTitle {
    fn title(&self) -> String {
        match &self.doc {
            Some(d) => format!("doc {}", d.borrow().name),
            None => "no document".into(),
        }
    }
}

#[svelters::widget]
impl DocTitle {
    /// The document's name, read in Rust through the handle (null without one).
    fn doc_name(&self) -> Option<String> {
        self.doc.as_ref().map(|d| d.borrow().name.clone())
    }
}

/// What a pick produced (EV2b's `picked` event).
#[derive(svelters::Record, Clone, Debug, Default, PartialEq)]
pub struct PickRef {
    /// The value kind the host presents.
    pub kind: String,
    /// The host's text.
    pub label: String,
}

/// EV2b (ARCH-EMBED §8.5): a native behaviour, `use:pick` from `svelters:behaviors/cad`. While a pick
/// runs (`active`), its host is a pick target (`cad.pick-target`), hovering it sets `cad.pick-hover`,
/// and a click reports what the host presents. All of it runs in Rust every frame.
#[svelters::behavior(namespace = "cad", name = "pick")]
pub struct Pick {
    /// The value kinds the host presents (the first one is reported).
    #[prop]
    accepts: Vec<String>,
    /// Whether a pick is running.
    #[prop(default = true)]
    active: bool,
    /// The host was clicked during a pick.
    #[event]
    picked: Event<PickRef>,
    /// How many frames the hook ran (Rust-only state).
    #[state]
    frames: u64,
}

impl Behavior for Pick {
    fn sense(&self) -> egui::Sense {
        if self.active {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        }
    }

    fn after_ui(&mut self, host: NodeRef<'_>, resp: &egui::Response, cx: &mut BehaviorCx<'_>) {
        self.frames += 1;
        cx.set_host_state(&cad::PICK_TARGET, self.active);
        cx.set_host_state(&cad::PICK_HOVER, self.active && resp.hovered());
        if self.active && resp.clicked() {
            cx.emit(
                &self.picked,
                PickRef {
                    kind: self.accepts.first().cloned().unwrap_or_default(),
                    label: host.text().to_string(),
                },
            );
        }
    }

    fn detached(&mut self, host: &mut NodeMut<'_>) {
        host.set_state(&cad::PICK_TARGET, false);
        host.set_state(&cad::PICK_HOVER, false);
    }
}

impl Pick {
    /// Frames the hook ran.
    pub fn frames(&self) -> u64 {
        self.frames
    }
    /// The `accepts` param.
    pub fn accepts(&self) -> &[String] {
        &self.accepts
    }
    /// The `active` param.
    pub fn active(&self) -> bool {
        self.active
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
    std::hint::black_box(<Pick as KindDecl>::KIND);
    std::hint::black_box(<LevelMeter as KindDecl>::KIND);
    std::hint::black_box(<StepCount as KindDecl>::KIND);
    std::hint::black_box(<DragValueKind as KindDecl>::KIND);
    std::hint::black_box(<CollapsingSection as KindDecl>::KIND);
    std::hint::black_box(<Sparkline as KindDecl>::KIND);
    std::hint::black_box(<DocTitle as KindDecl>::KIND);
}
