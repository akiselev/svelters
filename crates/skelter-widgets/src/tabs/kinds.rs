//! `tab-view` and `tab` as skelter-core kinds over the [`TabView`] model. They implement
//! `skelter_core::Kind` directly (not through `host::Hosted`): a tab-view reads its children's props
//! (the captions) and places its children (only the selected page is shown), which the hosted kind
//! interface does not carry.
//!
//! - The strip is chrome: [`Kind::chrome_padding`] reserves its height above the pages, and
//!   [`Kind::child_layout`] shows the selected page and hides the others (`display: none`).
//! - Captions come from the children ([`caption_of`]: a `tab`'s `title` and `closable`, another
//!   child's `aria-label`), re-read in `measure`, `prepare` and `record`.
//! - Input reaches the model in local points; a press on a caption selects it, takes focus and
//!   captures the pointer. A user pick raises `changed(selected)` and writes `selected` back as native
//!   state (`bind:selected`).
//! - **`close-requested`** is raised on the `tab` whose close button was clicked (core's
//!   `EventCx::emit_on`), and on the tab-view with the page index (any page, a `tab` or not).
//!
//! - **Overflow:** scroll buttons and an overflow button at the strip's end ([`TabView::overflows`]);
//!   the overflow menu is the node's overlay part (the window's overlay stack: drawn above the
//!   content, light-dismissed), `menu-open` mirrors it (`bind:menu-open`).
//!
//! Left out: AccessKit nodes for the captions (core builds one node per tree node; the captions are
//! not nodes), and a strip refresh when only a `tab`'s `title` changes (core tells the parent of a
//! child's prop change only for folding kinds; the strip catches up at the tab-view's next layout).

use super::{TabCaption, TabLook, TabView, caption_of};
use crate::table::text_painter::{record_core, with_text};
use crate::theme::Theme;
use skelter_base::accesskit::Role;
use skelter_base::change::ChangeClass;
use skelter_base::event::{PointerEvent, PointerPhase, PointerType, WheelEvent};
use skelter_base::geom::{Insets, Point, Vec2};
use skelter_core::overlay::{OverlayClass, OverlayPart};
use skelter_core::{
    A11yCx, ArgSpec, ChildLayoutCx, Event, EventCx, FocusReason, Handled, Kind, KindRegistry, KindSpec, MeasureCx, NodeView, PrepareCx, PropCx, PropDefault, PropError, PropId,
    PropSpec, RecordCx, RegistryError, SignalFlags, SignalSpec, Value, ValueType, common,
};

static TAB_VIEW_PROPS: &[PropSpec] = &[
    PropSpec::new("selected", ValueType::I64, ChangeClass::Layout).default(PropDefault::I64(0)).native().doc("Index of the shown page; a click on a caption changes it."),
    PropSpec::new("menu-open", ValueType::Bool, ChangeClass::Paint)
        .default(PropDefault::Bool(false))
        .native()
        .doc("The overflow menu (every tab, shown while the captions overflow the strip) is open; the user opening or closing it sets it."),
];

static TAB_VIEW_SIGNALS: &[SignalSpec] = &[
    SignalSpec::new("changed", Some(ValueType::I64), SignalFlags::PASSIVE)
        .args(&[ArgSpec::new("selected", ValueType::I64)])
        .doc("The user picked a tab; carries its index."),
    SignalSpec::new("close-requested", Some(ValueType::I64), SignalFlags::PASSIVE)
        .args(&[ArgSpec::new("index", ValueType::I64)])
        .doc("A tab's close button was clicked (or a middle click on it); carries the page index. Remove the page to close it."),
];

/// `tab-view`: a strip of captions over the selected page.
pub static TAB_VIEW: KindSpec = KindSpec {
    props: TAB_VIEW_PROPS,
    signals: TAB_VIEW_SIGNALS,
    role: Role::TabList,
    focusable: true,
    container: true,
    places_children: true,
    doc: "A strip of tab captions over the selected page (its children are the pages).",
    ..KindSpec::new("tab-view")
};

static TAB_PROPS: &[PropSpec] = &[
    PropSpec::new("title", ValueType::Str, ChangeClass::Layout).default(PropDefault::Str("")).doc("The tab's caption (also its accessible name)."),
    PropSpec::new("closable", ValueType::Bool, ChangeClass::Layout).default(PropDefault::Bool(false)).doc("The caption shows a close button (`close-requested`)."),
];

static TAB_SIGNALS: &[SignalSpec] =
    &[SignalSpec::new("close-requested", None, SignalFlags::PASSIVE).doc("The tab's close button was clicked (or a middle click on it); remove the tab to close it.")];

/// `tab`: one page of a `tab-view` (a flex box).
pub static TAB: KindSpec = KindSpec {
    props: TAB_PROPS,
    signals: TAB_SIGNALS,
    role: Role::TabPanel,
    container: true,
    doc: "A page of a tab-view; `title` names its caption.",
    ..KindSpec::new("tab")
};

/// Registers `tab-view` and `tab`.
pub fn register(reg: &mut KindRegistry) -> Result<(), RegistryError> {
    reg.register::<TabViewKind>()?;
    reg.register::<TabKind>()?;
    Ok(())
}

impl TabLook {
    /// The look from `theme`'s tokens (the defaults where a token is missing).
    pub fn from_theme(theme: &Theme) -> TabLook {
        let d = TabLook::default();
        let c = |name: &str, fallback| theme.color(name).unwrap_or(fallback);
        let mut text = d.text.clone();
        // VS Code's tab colours: inactive captions dimmer than the active one
        text.color = c("tab-fg", c("fg", text.color));
        text.size = theme.px("font-size").unwrap_or(text.size);
        if let Some(f) = theme.token("font-family") {
            text.family = f.to_owned();
        }
        TabLook {
            selected_fg: c("tab-active-fg", c("fg", d.selected_fg)),
            min_height: theme.px("tab-h").unwrap_or(d.min_height),
            hover_bg: c("list-hover", d.hover_bg),
            pressed_bg: c("toolbar-active", d.pressed_bg),
            // the theme's active-tab marker (`tab.activeBorderTop`; HC Dark's contrast border):
            // `accent` is the button fill, black in HC Dark
            indicator: c("tab-marker", c("accent", d.indicator)),
            separator: c("border", d.separator),
            close_hover_bg: c("toolbar-hover", d.close_hover_bg),
            focus: c("focus", d.focus),
            disabled_fg: c("disabled-fg", d.disabled_fg),
            radius: theme.px("radius").unwrap_or(d.radius),
            menu_bg: c("menu-bg", d.menu_bg),
            menu_border: c("menu-border-color", c("border", d.menu_border)),
            menu_sel: c("menu-sel", d.menu_sel),
            menu_sel_fg: c("menu-sel-fg", d.menu_sel_fg),
            menu_sel_border: (theme.contrast() == crate::theme::Contrast::High).then(|| c("focus", d.focus)),
            text,
            ..d
        }
    }
}

/// The page captions of `node`'s children.
fn captions(node: NodeView<'_>) -> Vec<TabCaption> {
    (0..node.children().len())
        .filter_map(|k| node.child(k))
        .enumerate()
        .map(|(k, c)| {
            let spec = c.kind().spec;
            let is_tab = spec.name == TAB.name;
            let title = is_tab.then(|| spec.prop_named("title").and_then(|p| c.prop_str(p)).unwrap_or(""));
            let closable = is_tab && spec.prop_named("closable").is_some_and(|p| c.prop_bool(p));
            caption_of(k, title, closable, c.prop_str(common::ARIA_LABEL).unwrap_or(""))
        })
        .collect()
}

/// The `tab-view` kind.
#[derive(Default)]
pub struct TabViewKind {
    pub view: TabView,
    captions: Vec<TabCaption>,
    look: TabLook,
    /// `menu-open` as written: the menu opens once the strip overflows.
    menu_wanted: bool,
}

impl TabViewKind {
    fn selected_id() -> PropId {
        TAB_VIEW.prop_id(0)
    }
    fn menu_id() -> PropId {
        TAB_VIEW.prop_id(1)
    }

    /// `menu-open` follows the model after an input.
    fn sync_menu(&mut self, cx: &mut EventCx) {
        let open = self.view.menu_open();
        self.menu_wanted = open;
        if cx.view(cx.node()).is_some_and(|v| v.prop_bool(Self::menu_id()) != open) {
            let _ = cx.set_native(Self::menu_id(), Value::Bool(open));
        }
    }

    /// Re-reads the captions and the look and lays the strip out at the node's width.
    fn refresh(&mut self, node: NodeView<'_>, theme: Option<&Theme>, ts: Option<&mut skelter_text::TextSystem>) {
        self.captions = captions(node);
        if let Some(t) = theme {
            self.look = TabLook::from_theme(t);
        }
        self.view.enabled = !node.disabled();
        self.view.rtl = crate::shim::core::node_rtl(&node);
        let (caps, look, width) = (&self.captions, &self.look, node.size().w);
        let view = &mut self.view;
        with_text(ts, |p| view.relayout(caps, look, p, width));
        if self.menu_wanted && !self.view.menu_open() {
            self.view.set_menu(true);
        }
    }

    fn pointer_event(phase: PointerPhase, p: &skelter_core::PointerInput) -> PointerEvent {
        PointerEvent { phase, pointer: p.pointer, kind: p.kind, button: p.button, buttons: p.buttons, pos: p.local, modifiers: p.modifiers, pressure: p.pressure, tilt: Vec2::ZERO, history: Default::default() }
    }

    /// A pointer event without a position (a cancel, the hover ending).
    fn bare(phase: PointerPhase) -> PointerEvent {
        PointerEvent { phase, pointer: 0, kind: PointerType::Mouse, button: None, buttons: 0, pos: Point::new(-1.0, -1.0), modifiers: Default::default(), pressure: 0.0, tilt: Vec2::ZERO, history: Default::default() }
    }

    fn apply(&mut self, cx: &mut EventCx, o: super::TabOutput) -> Handled {
        for e in &o.events {
            if e.signal == "changed" {
                let _ = cx.set_native(Self::selected_id(), Value::I64(self.view.selected));
                cx.emit(TAB_VIEW.signal_id(0), Some(Value::I64(self.view.selected)));
                cx.request_layout();
            }
        }
        let pages: Vec<_> = if o.close.is_empty() { Vec::new() } else { cx.view(cx.node()).map(|v| v.children().to_vec()).unwrap_or_default() };
        for k in o.close {
            let tab = pages.get(k).copied().filter(|&c| cx.view(c).is_some_and(|v| v.kind().spec.name == TAB.name));
            if let Some(t) = tab {
                cx.emit_on(t, TAB.signal_id(0), None);
            }
            cx.emit(TAB_VIEW.signal_id(1), Some(Value::I64(k as i64)));
        }
        if o.take_focus {
            cx.focus(FocusReason::Pointer);
        }
        self.sync_menu(cx);
        if o.repaint || o.handled {
            cx.request_paint();
        }
        if o.handled || o.take_focus { Handled::Yes } else { Handled::No }
    }
}

impl Kind for TabViewKind {
    fn spec() -> &'static KindSpec {
        &TAB_VIEW
    }

    fn prop_changed(&mut self, cx: &mut PropCx, prop: PropId, value: Option<&Value>) -> Result<(), PropError> {
        if prop == Self::selected_id() {
            let v = value.and_then(Value::as_i64).unwrap_or(0);
            self.view.set_selected(v);
            cx.request_layout();
        }
        if prop == Self::menu_id() {
            // applied at the next prepare, once the strip knows whether it overflows
            self.menu_wanted = value.and_then(Value::as_bool).unwrap_or(false);
            self.view.set_menu(self.menu_wanted);
        }
        Ok(())
    }

    fn chrome_padding(&self, cx: &mut MeasureCx) -> Option<Insets> {
        let node = cx.node;
        let caps = captions(node);
        let look = self.look.clone();
        let h = with_text(cx.service::<skelter_text::TextSystem>(), |p| super::TabStrip::layout(&caps, &look, p).height);
        Some(Insets { top: h, ..Insets::ZERO })
    }

    fn child_layout(&self, cx: &ChildLayoutCx, style: &mut taffy::Style) -> bool {
        let shown = self.view.shown(cx.count) == Some(cx.index);
        if !shown {
            style.display = taffy::Display::None;
        }
        shown
    }

    fn prepare(&mut self, cx: &mut PrepareCx) {
        let node = cx.node;
        let theme = cx.service::<Theme>().cloned();
        self.refresh(node, theme.as_ref(), cx.service::<skelter_text::TextSystem>());
    }

    fn record(&self, cx: &mut RecordCx) {
        record_core(cx, |p, list| self.view.record(&self.captions, &self.look, p, list));
        // the overflow menu, in a layer of the window's overlay stack (its button counts as inside)
        if let Some(m) = self.view.menu_rect() {
            let mut part = OverlayPart::new(OverlayClass::Menu, self.view.flip(m));
            part.inside = self.view.ctl_rect(super::Ctl::Menu).map(|r| self.view.flip(r)).into_iter().collect();
            cx.overlay(part, |cx| record_core(cx, |p, list| self.view.record_menu(&self.captions, &self.look, p, list)));
        }
    }

    /// The stack closed the menu (a press outside it, Escape).
    fn overlay_closed(&mut self, cx: &mut EventCx, _reason: skelter_core::overlay::DismissReason) -> Handled {
        self.view.set_menu(false);
        self.menu_wanted = false;
        self.sync_menu(cx);
        cx.request_paint();
        Handled::Yes
    }

    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        let o = match ev {
            Event::PointerDown(p) => {
                let o = self.view.pointer(&Self::pointer_event(PointerPhase::Down, p));
                if o.take_focus || !o.close.is_empty() || o.repaint {
                    cx.capture_pointer();
                }
                o
            }
            Event::PointerUp(p) => {
                cx.release_pointer();
                self.view.pointer(&Self::pointer_event(PointerPhase::Up, p))
            }
            Event::PointerMove(p) => self.view.pointer(&Self::pointer_event(PointerPhase::Move, p)),
            Event::PointerCancel => self.view.pointer(&Self::bare(PointerPhase::Cancel)),
            Event::HoverLeave => self.view.pointer(&Self::bare(PointerPhase::Leave)),
            Event::Wheel(w) => {
                let we = WheelEvent { pos: w.local, delta: w.delta, phase: None, momentum: None, modifiers: w.modifiers };
                if self.view.wheel(&we) {
                    cx.request_paint();
                    return Handled::Yes;
                }
                return Handled::No;
            }
            Event::Key(k) => self.view.key(k),
            Event::FocusIn(reason) => {
                self.view.focused = true;
                self.view.focus_visible = *reason == FocusReason::Keyboard;
                cx.request_paint();
                return Handled::No;
            }
            Event::FocusOut => {
                self.view.focused = false;
                self.view.focus_visible = false;
                cx.request_paint();
                return Handled::No;
            }
            _ => return Handled::No,
        };
        self.apply(cx, o)
    }

    fn a11y(&self, cx: &mut A11yCx) {
        cx.default_a11y();
        cx.out.set_role(Role::TabList);
        if let Some(k) = self.view.shown(self.captions.len()) {
            cx.out.set_value(self.captions[k].title.clone());
        }
    }
}

/// The `tab` kind: a page (its box draws as any box; its children lay out as a flex box).
#[derive(Default)]
pub struct TabKind;

impl Kind for TabKind {
    fn spec() -> &'static KindSpec {
        &TAB
    }

    fn record(&self, cx: &mut RecordCx) {
        cx.draw_box();
    }

    fn a11y(&self, cx: &mut A11yCx) {
        cx.default_a11y();
        cx.out.set_role(Role::TabPanel);
        if let Some(t) = TAB.prop_named("title").and_then(|p| cx.node.prop_str(p)).filter(|t| !t.is_empty()) {
            cx.out.set_label(t);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specs_have_distinct_ids_and_register() {
        assert_ne!(TAB_VIEW.id(), TAB.id());
        assert!(TAB_VIEW.places_children && TAB_VIEW.container && TAB_VIEW.focusable);
        assert!(TAB_VIEW.props[0].native_owned, "selected is native-owned (bind:selected)");
        assert_eq!(TAB_VIEW.signal_named("changed"), Some(TAB_VIEW.signal_id(0)));
        assert_eq!(TAB.prop_named("title"), Some(TAB.prop_id(0)));
        let mut reg = KindRegistry::new();
        register(&mut reg).expect("tab-view and tab register");
        assert!(reg.by_name("tab-view").is_some() && reg.by_name("tab").is_some());
    }

    #[test]
    fn only_the_selected_page_is_shown() {
        let mut k = TabViewKind::default();
        k.view.set_selected(1);
        assert_eq!(k.view.shown(3), Some(1));
        assert_eq!(k.view.shown(1), Some(0), "clamped to the pages");
    }

    #[test]
    fn the_look_follows_the_theme() {
        let t = Theme::dark_modern();
        let look = TabLook::from_theme(&t);
        // VS Code's tab colours: the inactive caption `tab-fg`, the active one `tab-active-fg`
        if let (Some(fg), Some(active)) = (t.color("tab-fg"), t.color("tab-active-fg")) {
            assert_eq!((look.text.color, look.selected_fg), (fg, active));
        }
        if let Some(h) = t.px("tab-h") {
            assert_eq!(look.min_height, h);
        }
    }

    #[test]
    fn close_requested_reaches_the_tab() {
        use crate::table::text_painter::with_text;
        use crate::testdoc::{child, click, doc, frame};
        let (mut d, root) = doc(skelter_base::geom::Size::new(400.0, 300.0), |r| register(r).unwrap());
        let tv = child(&mut d, root, TAB_VIEW.id(), "width: 300px; height: 200px");
        let tabs: Vec<_> = (0..2)
            .map(|k| {
                let t = child(&mut d, tv, TAB.id(), "");
                d.set_prop(t, TAB.prop_named("title").unwrap(), Value::from(if k == 0 { "One" } else { "Two" })).unwrap();
                d.set_prop(t, TAB.prop_named("closable").unwrap(), Value::Bool(true)).unwrap();
                t
            })
            .collect();
        let tab_close = TAB.signal_id(0);
        let view_close = TAB_VIEW.signal_id(1);
        for &t in &tabs {
            d.listen(t, tab_close, skelter_core::ListenFlags::EMPTY);
        }
        d.listen(tv, view_close, skelter_core::ListenFlags::EMPTY);
        frame(&mut d);
        // the second caption's close button, as the strip lays it out (no text system: no theme either)
        let caps: Vec<TabCaption> = ["One", "Two"].iter().enumerate().map(|(k, t)| caption_of(k, Some(t), true, "")).collect();
        let look = TabLook::default();
        let strip = with_text(None, |p| super::super::TabStrip::layout(&caps, &look, p));
        let r = strip.tabs[1].close.expect("closable");
        let out = click(&mut d, (r.min.x + r.max.x) / 2.0, (r.min.y + r.max.y) / 2.0);
        let mut ev: Vec<_> = out.into_iter().flat_map(|o| o.events).collect();
        ev.extend(d.take_events());
        let on_tab: Vec<_> = ev.iter().filter(|e| e.signal == tab_close).map(|e| e.target).collect();
        assert_eq!(on_tab, [tabs[1]], "on the tab whose button was clicked");
        let on_view: Vec<_> = ev.iter().filter(|e| e.signal == view_close).map(|e| e.arg.clone()).collect();
        assert_eq!(on_view, [Some(Value::I64(1))], "and on the tab-view with its index");
    }
}
