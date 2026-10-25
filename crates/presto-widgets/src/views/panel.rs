//! `panel` (ported from svelters-egui `widgets/panel.rs`): a side or top/bottom panel with the chrome
//! background, sized by `size` along its side's axis (width for `left`/`right`, height for
//! `top`/`bottom`; it does not shrink). `resizable` makes the inner edge a [`Sash`]: drag it within
//! the panel's `min-*`/`max-*`, or focus it and use the arrow keys; `changed(size)` reports each new
//! size. The separator is a 1 pt line inside the panel along its inner edge.

use super::draw;
use super::legacy::{Event, EventCx, Handled, Kind, KindSpec, PropError, PropType, RecordCx, Value, prop, signal};
use super::split::{Sash, SashIntent, SashParams, record_sash};
use presto_base::accesskit;
use presto_base::change::ChangeClass;
use presto_base::cursor::CursorIcon;
use presto_base::event::{KeyState, MouseButton, PointerPhase};
use presto_base::geom::{Rect, Size};
use presto_base::time::Time;

/// Where a panel sits; its sash is on the opposite (inner) edge.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PanelSide {
    #[default]
    Left,
    Right,
    Top,
    Bottom,
}

impl PanelSide {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "left" => PanelSide::Left,
            "right" => PanelSide::Right,
            "top" => PanelSide::Top,
            "bottom" => PanelSide::Bottom,
            _ => return None,
        })
    }
    /// The axis its size is measured on.
    pub fn axis(self) -> usize {
        if matches!(self, PanelSide::Top | PanelSide::Bottom) { 1 } else { 0 }
    }
    /// +1 when dragging the inner edge towards larger coordinates grows the panel.
    fn sign(self) -> f32 {
        if matches!(self, PanelSide::Right | PanelSide::Bottom) { -1.0 } else { 1.0 }
    }
}

/// A side or top/bottom panel.
#[derive(Clone, Debug)]
pub struct Panel {
    pub side: PanelSide,
    /// Its width or height in points; `None` for its content's.
    pub size: Option<f32>,
    pub resizable: bool,
    /// Draws the line on the inner edge.
    pub separator: bool,
    /// The sash's limits (the panel's `min-*`/`max-*` along its axis).
    pub min: f32,
    pub max: f32,
    /// The sash's hit area thickness.
    pub hit: f32,
    sash: Sash,
    /// The border box as laid out.
    rect: Size,
    now: Time,
}

impl Default for Panel {
    fn default() -> Self {
        Panel { side: PanelSide::Left, size: None, resizable: false, separator: true, min: 0.0, max: f32::INFINITY, hit: 4.0, sash: Sash::default(), rect: Size::ZERO, now: Time::ZERO }
    }
}

static PANEL_SPEC: KindSpec = KindSpec {
    name: "panel",
    props: &[
        prop("side", PropType::Enum(&["left", "right", "top", "bottom"]), ChangeClass::Layout),
        prop("size", PropType::F64, ChangeClass::Layout),
        prop("resizable", PropType::Bool, ChangeClass::Paint),
        prop("separator", PropType::Bool, ChangeClass::Paint),
    ],
    signals: &[signal("changed", true, false)],
    slots: &[""],
    role: accesskit::Role::Pane,
};

impl Panel {
    /// The layout's border box size.
    pub fn set_layout(&mut self, size: Size) {
        self.rect = size;
        let ax = self.side.axis();
        self.sash.set_size(self.size.unwrap_or(if ax == 0 { size.w } else { size.h }));
    }

    /// The inner edge coordinate along the axis (local).
    fn edge(&self) -> f32 {
        match self.side {
            PanelSide::Left => self.rect.w,
            PanelSide::Top => self.rect.h,
            PanelSide::Right | PanelSide::Bottom => 0.0,
        }
    }

    /// The separator line inside the panel along its inner edge, and the sash's hit area.
    pub fn edge_rects(&self) -> (Rect, Rect) {
        let at = self.edge();
        let inward = self.side.sign();
        let (l0, l1) = if inward > 0.0 { (at - 1.0, at) } else { (at, at + 1.0) };
        let (h0, h1) = (at - self.hit / 2.0, at + self.hit / 2.0);
        if self.side.axis() == 0 {
            (Rect::new(l0, 0.0, l1, self.rect.h), Rect::new(h0, 0.0, h1, self.rect.h))
        } else {
            (Rect::new(0.0, l0, self.rect.w, l1), Rect::new(0.0, h0, self.rect.w, h1))
        }
    }

    fn params(&self) -> SashParams {
        SashParams { axis: self.side.axis(), sign: self.side.sign(), min: self.min.max(0.0), max: self.max, step: 10.0, collapsible: false, double_click: false }
    }

    fn apply(&mut self, cx: &mut EventCx<'_>, intents: Vec<SashIntent>) {
        for i in intents {
            match i {
                SashIntent::Start => cx.capture = Some(true),
                SashIntent::Resize { size } | SashIntent::Commit { size } | SashIntent::Cancel { size } => {
                    if self.size != Some(size) {
                        self.size = Some(size);
                        cx.mark(ChangeClass::Layout);
                        cx.emit("changed", Value::F64(size as f64));
                    }
                }
            }
        }
    }

    /// The sash's AccessKit node (a `Splitter` named after the panel, else what it does).
    pub fn sash_a11y(&self, name: &str, node: &mut accesskit::Node) {
        self.sash.a11y(&self.params(), node);
        node.set_label(if name.is_empty() { "Resize panel".to_string() } else { name.to_string() });
    }

    /// Focus moved to the sash (or off it).
    pub fn focus_sash(&mut self, f: bool) {
        self.sash.set_focused(f);
    }
}

impl Kind for Panel {
    fn spec() -> &'static KindSpec {
        &PANEL_SPEC
    }

    fn set_prop(&mut self, name: &str, value: &Value) -> Result<ChangeClass, PropError> {
        match name {
            "side" => {
                self.side = PanelSide::parse(value.as_str().unwrap_or("left")).ok_or(PropError::Invalid("side: left, right, top or bottom"))?;
                Ok(ChangeClass::Layout)
            }
            "size" => {
                // a drag owns the size until it ends
                if self.sash.dragging() {
                    return Err(PropError::Stale);
                }
                self.size = match value {
                    Value::Null => None,
                    v => Some(v.as_f32().ok_or(PropError::Invalid("a number of points or null"))?),
                };
                if let Some(s) = self.size {
                    self.sash.set_size(s);
                }
                Ok(ChangeClass::Layout)
            }
            "resizable" => {
                self.resizable = value.as_bool().ok_or(PropError::Invalid("a boolean"))?;
                Ok(ChangeClass::Paint)
            }
            "separator" => {
                self.separator = value.as_bool().ok_or(PropError::Invalid("a boolean"))?;
                Ok(ChangeClass::Paint)
            }
            _ => Err(PropError::Unknown),
        }
    }

    fn record(&self, cx: &mut RecordCx<'_>) {
        draw::fill(cx.list, Rect::from_origin_size(Default::default(), cx.size), 0.0, cx.palette.surface_raised);
        let (line, _) = self.edge_rects();
        let shown = self.resizable && self.sash.hover_shown(self.now);
        if self.separator || shown {
            record_sash(cx, line, self.side.axis(), shown);
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, ev: &Event) -> Handled {
        self.now = cx.now;
        if !self.resizable {
            return Handled::No;
        }
        match ev {
            Event::Pointer(p) => {
                let (_, hit) = self.edge_rects();
                let inside = hit.contains(p.pos);
                let primary = p.button == Some(MouseButton::Primary) || (p.phase != PointerPhase::Down && p.buttons & 1 != 0);
                let was_shown = self.sash.hover_shown(cx.now);
                let params = self.params();
                let dragging = self.sash.dragging();
                let intents = self.sash.pointer(cx.now, cx.policy, &params, p.phase, primary, p.pos, inside);
                if was_shown != self.sash.hover_shown(cx.now) {
                    cx.mark(ChangeClass::Paint);
                }
                if let Some(t) = self.sash.wake() {
                    cx.wake_at(t);
                }
                if dragging && !self.sash.dragging() {
                    cx.capture = Some(false);
                }
                self.apply(cx, intents);
                if inside || self.sash.dragging() {
                    cx.cursor = Some(if self.side.axis() == 0 { CursorIcon::ColResize } else { CursorIcon::RowResize });
                    Handled::Yes
                } else {
                    Handled::No
                }
            }
            Event::Key(k) if k.state == KeyState::Pressed => {
                let Some(key) = k.key else { return Handled::No };
                let params = self.params();
                if !self.sash.claims(&params, key) {
                    return Handled::No;
                }
                let intents = self.sash.key(&params, key, k.modifiers);
                self.apply(cx, intents);
                Handled::Yes
            }
            Event::Timer => {
                cx.mark(ChangeClass::Paint);
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn a11y(&self, node: &mut accesskit::Node) {
        node.set_role(accesskit::Role::Pane);
    }
}

#[cfg(test)]
mod tests {
    use super::super::legacy::{Os, Policy};
    use super::*;
    use presto_base::event::{Modifiers, PointerEvent, PointerType};
    use presto_base::geom::{Point, Vec2};

    fn ptr(phase: PointerPhase, x: f32) -> Event {
        Event::Pointer(PointerEvent {
            phase,
            pointer: 0,
            kind: PointerType::Mouse,
            button: matches!(phase, PointerPhase::Down | PointerPhase::Up).then_some(MouseButton::Primary),
            buttons: if matches!(phase, PointerPhase::Down | PointerPhase::Move) { 1 } else { 0 },
            pos: Point::new(x, 50.0),
            modifiers: Modifiers::default(),
            pressure: 0.5,
            tilt: Vec2::ZERO,
            history: Default::default(),
        })
    }

    #[test]
    fn a_right_panel_grows_when_its_left_edge_moves_left() {
        let pol = Policy::for_os(Os::Windows);
        let mut p = Panel { side: PanelSide::Right, resizable: true, min: 100.0, max: 400.0, ..Default::default() };
        p.set_layout(Size::new(200.0, 300.0));
        let (line, hit) = p.edge_rects();
        assert_eq!(line, Rect::new(0.0, 0.0, 1.0, 300.0));
        assert!(hit.contains(Point::new(-1.0, 10.0)));
        let mut cx = EventCx::new(Time(0), Size::new(200.0, 300.0), &pol);
        assert_eq!(p.event(&mut cx, &ptr(PointerPhase::Down, 0.0)), Handled::Yes);
        p.event(&mut cx, &ptr(PointerPhase::Move, -50.0));
        assert_eq!(p.size, Some(250.0));
        p.event(&mut cx, &ptr(PointerPhase::Move, -500.0));
        assert_eq!(p.size, Some(400.0), "clamped to max");
        p.event(&mut cx, &ptr(PointerPhase::Up, -500.0));
        assert_eq!(cx.capture, Some(false));
        assert_eq!(cx.emitted("changed").count(), 2);
    }

    #[test]
    fn a_js_write_during_a_drag_is_refused() {
        let pol = Policy::for_os(Os::Linux);
        let mut p = Panel { resizable: true, ..Default::default() };
        p.set_layout(Size::new(200.0, 300.0));
        let mut cx = EventCx::new(Time(0), Size::new(200.0, 300.0), &pol);
        p.event(&mut cx, &ptr(PointerPhase::Down, 200.0));
        p.event(&mut cx, &ptr(PointerPhase::Move, 230.0));
        assert_eq!(p.size, Some(230.0));
        assert_eq!(p.set_prop("size", &Value::F64(100.0)), Err(PropError::Stale));
        p.event(&mut cx, &ptr(PointerPhase::Up, 230.0));
        assert_eq!(p.set_prop("size", &Value::F64(100.0)), Ok(ChangeClass::Layout));
    }
}
