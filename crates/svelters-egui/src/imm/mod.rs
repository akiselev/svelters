//! OW-RUST (ARCH-WIDGETS Q5, plans/milestones/OW-RUST-immediate.md): svelters' own widgets in
//! immediate mode, for Rust code that draws in egui without a scene node: cadatomic's shell
//! chrome (toolbars, status bar) and berth's `ChromePainter` (tab strips, tab buttons, title
//! bands).
//!
//! An `imm` widget is the same widget as the Svelte one, minus the node:
//! - **look**: the same paint function the kind calls (`widgets::*::paint`), the same UA table,
//!   and the same CSS: a *virtual element* ([`El`]: tag, classes, state) matched by the scene's
//!   own `Stylist` (theme tokens, `sv.base`, `sv.affordance` and author sheets), cached per chain
//!   and state until the cascade restyles ([`Sv`]);
//! - **behaviour**: the same units (`Hoverable`, `Focusable`, `Pressable`, `Toggleable`) through
//!   [`interact::run`](crate::interact::run), their state kept in egui memory by the widget's id;
//! - **focus**: EV3's record without a node (`focus::record_foreign`): in the Tab order with the
//!   scene's widgets (paint order), R11 key claims, `shortcut_override`;
//! - **AccessKit**: the same role, states and actions (`widget_info`, then the units' `a11y`).
//!
//! ```ignore
//! let sv = backend.imm();                                  // once; clones share the cache
//! let bar = sv.within(El::new("toolbar").class("main"));   // `.main button { … }` reaches these
//! if imm::Button::new("Save").icon("save").show(ui, &bar).clicked() { … }
//! imm::Checkbox::new(&mut wrap, "Wrap").show(ui, &bar);
//! imm::Button::new("").icon("codicons:close").class("tab-close").show_at(ui, rect, &bar); // berth lays out
//! // paint-only chrome on the caller's own response (a draggable tab):
//! let st = sv.chrome(ui, &tab_resp, &El::new("tab").class("pinned"), Ua::NONE, StateBits::SELECTED);
//! let geom = imm::chrome::surface(ui.painter(), tab_resp.id, &st, rect);
//! ```
//!
//! Writer lane: OW-RUST.

pub mod chrome;
mod el;
mod widgets;

pub use el::El;
pub use widgets::{Button, Checkbox, Icon, Label, Separator, Switch, ToggleButton};

use crate::backend::EguiBackend;
use crate::interact::{FocusParams, Focusable, Hoverable, Intents, PartResponses, Press, PressParams, Pressable, Toggle, ToggleParams, Toggleable, Unit};
use crate::paint::{PaintCx, StyleView, Ua};
use crate::provider::StateBits;
use el::VNode;
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use svelters_style::ComputedStyle;

// ---- the style source ----

#[derive(Default)]
struct Cache {
    generation: u64,
    map: HashMap<u64, Arc<ComputedStyle>>,
}

/// Where `imm` widgets get their CSS: a backend's cascade (its theme and every sheet the app
/// installed) and the chain of virtual ancestors they are drawn in. Cheap to clone; clones share
/// one cache of computed styles, dropped whenever the cascade restyles everything (a sheet, the
/// theme, the scheme or the media changed).
///
/// Without a backend ([`Sv::ua_only`]), or while the backend's cascade is off (no sheet and no
/// theme yet), widgets paint from their UA tables, exactly as scene nodes do then.
#[derive(Clone)]
pub struct Sv {
    backend: Option<EguiBackend>,
    path: Rc<Vec<El>>,
    cache: Rc<RefCell<Cache>>,
}

impl std::fmt::Debug for Sv {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sv").field("backend", &self.backend.is_some()).field("path", &self.path).finish()
    }
}

/// The root of every chain unless [`Sv::with_root`] says otherwise: `:root` (the theme's tokens)
/// matches it, so `var(--sv-*)` resolves as in the scene.
pub const ROOT_TAG: &str = "chrome";

impl Sv {
    /// CSS from `backend`'s cascade.
    pub fn new(backend: &EguiBackend) -> Sv {
        Sv { backend: Some(backend.clone()), path: Rc::new(vec![El::new(ROOT_TAG)]), cache: Rc::default() }
    }

    /// No cascade: UA tables only (the built-in VS Code Modern look of the context's scheme).
    pub fn ua_only() -> Sv {
        Sv { backend: None, path: Rc::new(vec![El::new(ROOT_TAG)]), cache: Rc::default() }
    }

    /// The same source with `root` as the chain's root element (`:root` matches it).
    pub fn with_root(&self, root: El) -> Sv {
        let mut path = (*self.path).clone();
        path[0] = root;
        Sv { backend: self.backend.clone(), path: Rc::new(path), cache: self.cache.clone() }
    }

    /// The same source with `parent` as the innermost ancestor of the widgets drawn with it, so
    /// descendant selectors (`.statusbar button`) and inheritance reach them.
    pub fn within(&self, parent: El) -> Sv {
        let mut path = (*self.path).clone();
        path.push(parent);
        Sv { backend: self.backend.clone(), path: Rc::new(path), cache: self.cache.clone() }
    }

    /// The computed style of `el` in `state` under this source's ancestors; `None` when no
    /// cascade runs (widgets then paint from their UA tables, as scene nodes do).
    ///
    /// Inside a scene pass (a provider kind's `ui` drawing `imm` widgets) the scene is busy: the
    /// last style computed for the same chain and state is used, else none.
    pub fn computed(&self, el: &El, state: StateBits) -> Option<Arc<ComputedStyle>> {
        let backend = self.backend.as_ref()?;
        let chain: Vec<&El> = self.path.iter().chain(std::iter::once(el)).collect();
        let last = chain.len() - 1;
        let key = |k: usize, st: StateBits| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            chain[..=k].hash(&mut h);
            st.0.hash(&mut h);
            h.finish()
        };
        let mut cache = self.cache.borrow_mut();
        let Ok(scene) = backend.0.scene.try_borrow() else {
            return cache.map.get(&key(last, state)).cloned();
        };
        let style = &scene.style;
        if !style.is_active() {
            return None;
        }
        if cache.generation != style.generation() {
            cache.map.clear();
            cache.generation = style.generation();
        }
        let mut parent: Option<Arc<ComputedStyle>> = None;
        for k in 0..=last {
            let st = if k == last { state } else { StateBits::NONE };
            let c = cache
                .map
                .entry(key(k, st))
                .or_insert_with(|| Arc::new(style.compute_virtual(VNode::last(&chain[..=k], st), parent.as_deref())))
                .clone();
            parent = Some(c);
        }
        parent
    }

    /// `el`'s part `name` (`::part(name)`) over `host` (its computed style): `None` until the
    /// engine computes part styles (OW-CSS stage 2), as for scene nodes.
    pub fn part_computed(&self, el: &El, state: StateBits, host: &ComputedStyle, name: &str) -> Option<Arc<ComputedStyle>> {
        let backend = self.backend.as_ref()?;
        let chain: Vec<&El> = self.path.iter().chain(std::iter::once(el)).collect();
        let scene = backend.0.scene.try_borrow().ok()?;
        scene.style.part_virtual(VNode::last(&chain, state), host, name).map(Arc::new)
    }

    /// `el`'s style over `ua` in `state`, for `ctx`'s scheme: what a foreign painter hands the
    /// paint primitives (`styled.view()`).
    pub fn styled(&self, ctx: &egui::Context, el: &El, ua: Ua, state: StateBits) -> Styled {
        Styled { css: self.computed(el, state), ua, state, dark: ctx.theme() == egui::Theme::Dark }
    }
}

impl EguiBackend {
    /// OW-RUST: the style source of `imm` widgets drawn by Rust code next to this backend's
    /// scene (its theme and sheets). Keep one (or clones of it): the clones share a cache.
    pub fn imm(&self) -> Sv {
        Sv::new(self)
    }
}

/// An owned style: the computed style (if a cascade runs) and the UA table under it, in a state.
/// [`Styled::view`] is the [`StyleView`] the paint primitives read.
#[derive(Clone, Debug)]
pub struct Styled {
    pub css: Option<Arc<ComputedStyle>>,
    pub ua: Ua,
    pub state: StateBits,
    pub dark: bool,
}

impl Styled {
    pub fn view(&self) -> StyleView<'_> {
        StyleView::new(self.css.as_deref(), self.ua, self.state, self.dark)
    }
}

// ---- running units without a node ----

/// The units an `imm` widget runs, and their state between frames (egui memory, by id).
#[derive(Debug, Default)]
struct Units {
    hover: Hoverable,
    focus: Focusable,
    press: Pressable,
    toggle: Toggleable,
    state: StateBits,
}

/// Which units a widget composes (in declaration order: hover, focus, press, toggle), as its kind
/// does.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Compose {
    /// Hoverable (every widget with states; not a separator).
    pub hover: bool,
    pub focus: Option<FocusParams>,
    pub press: Option<PressParams>,
    pub toggle: Option<ToggleParams>,
}

/// What the units did this pass.
pub(crate) struct Ran {
    /// The child `Ui` at the widget's rect (disabled when the widget is; its opacity is the
    /// widget's).
    pub ui: egui::Ui,
    pub resp: egui::Response,
    pub state: StateBits,
    pub press: Intents<Press>,
    pub toggle: Intents<Toggle>,
    units: Arc<Mutex<Units>>,
    compose: Compose,
}

fn lock(u: &Mutex<Units>) -> std::sync::MutexGuard<'_, Units> {
    u.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Runs the units of `c` for widget `id` at `rect`, as the walk and `WidgetCx::run_units` do for
/// a node: a child `Ui` at the rect (disabled unless `enabled`), `ui.interact` with the units'
/// merged sense, the units in order with the bits kept from last frame plus `props` (the prop
/// mirror: `selected`, `default`), focus requests applied, a repaint when a unit asks.
pub(crate) fn run(ui: &mut egui::Ui, id: egui::Id, rect: egui::Rect, enabled: bool, c: Compose, props: StateBits) -> Ran {
    let mut b = egui::UiBuilder::new().max_rect(rect).id_salt(id.with("imm-scope")).layout(egui::Layout::left_to_right(egui::Align::Center));
    if !enabled {
        b = b.disabled();
    }
    let child = ui.new_child(b);
    let units = ui.ctx().data_mut(|d| d.get_temp_mut_or_insert_with(id.with("svelters-imm"), || Arc::new(Mutex::new(Units::default()))).clone());
    let (resp, state, press, toggle) = {
        let mut u = lock(&units);
        let Units { hover, focus, press, toggle, state } = &mut *u;
        let fp = c.focus.unwrap_or_default();
        let pp = c.press.unwrap_or_default();
        let tp = c.toggle.unwrap_or_default();
        let mut sense = egui::Sense::hover();
        if c.focus.is_some() {
            sense |= focus.sense(&fp);
        }
        if c.press.is_some() {
            sense |= press.sense(&pp);
        }
        if c.toggle.is_some() {
            sense |= toggle.sense(&tp);
        }
        let resp = child.interact(rect, id, sense);
        // the prop mirror: set before the units run (§2.3)
        let keep = StateBits(Hoverable::OWNS.0 | Focusable::OWNS.0 | Pressable::OWNS.0 | Toggleable::OWNS.0);
        *state = StateBits(state.0 & keep.0) | props;
        state.set(StateBits::DISABLED, !child.is_enabled());
        let (mut fh, mut ff, mut fpr, mut ft) = (Intents::default(), Intents::default(), Intents::default(), Intents::default());
        crate::interact::run(&child, &resp, state, &PartResponses::default(), |cx| {
            if c.hover {
                hover.input(&(), cx, &mut fh);
            }
            if c.focus.is_some() {
                focus.input(&fp, cx, &mut ff);
            }
            if c.press.is_some() {
                press.input(&pp, cx, &mut fpr);
            }
            if c.toggle.is_some() {
                toggle.input(&tp, cx, &mut ft);
            }
        });
        if let Some(d) = [press.next_frame(), focus.next_frame()].into_iter().flatten().min() {
            child.ctx().request_repaint_after(d);
        }
        (resp, *state, fpr, ft)
    };
    Ran { ui: child, resp, state, press, toggle, units, compose: c }
}

impl Ran {
    /// A paint context on the widget's `Ui`, traced as target `id.value()` (debug builds, when
    /// [`paint::trace::enable`](crate::paint::trace::enable) is on) with host tag `tag`.
    pub fn paint_cx<'a>(&'a self, tag: &'a str) -> PaintCx<'a> {
        let mut p = PaintCx::for_node(self.ui.painter(), self.resp.id.value());
        p.tag = tag;
        p
    }

    /// For a kind without `Caps::PAINTS_BOX` (checkbox, switch, label, separator): the box the
    /// walk paints behind it from its CSS alone (`style::before_leaf`: background, border,
    /// shadows; no UA table), and its CSS `opacity` on the widget's `Ui`. Nothing without a
    /// cascade.
    pub fn host_box(&mut self, tag: &str, st: &Styled, rect: egui::Rect) {
        let Some(css) = st.css.as_deref() else { return };
        let s = StyleView::new(Some(css), Ua::NONE, self.state, st.dark);
        if s.opacity() < 1.0 {
            self.ui.multiply_opacity(s.opacity());
        }
        crate::paint::paint_box(&mut self.paint_cx(tag), &s, rect);
    }

    /// After painting: AccessKit (`info`, then `a11y` and, with `units_a11y`, the units'
    /// properties and actions, as the kind's `Widget::a11y`), EV3's focus record with
    /// `focus_role`'s key claims, the accessible `name` (EM1), the R10 cursor and the tooltip (R9: also on a disabled widget,
    /// the reason). Returns the response.
    #[allow(clippy::too_many_arguments)] // one call per widget kind; each argument is a kind's choice
    pub fn finish(
        self,
        cursor: Option<egui::CursorIcon>,
        info: Option<&dyn Fn() -> egui::WidgetInfo>,
        a11y: impl FnOnce(&mut egui::accesskit::Node),
        units_a11y: bool,
        focus_role: Option<crate::focus::Role>,
        name: Option<&str>,
        tooltip: Option<&str>,
    ) -> egui::Response {
        let Ran { ui, resp, state, units, compose: c, .. } = self;
        if state.contains(StateBits::HOVERED)
            && let Some(cur) = cursor
        {
            ui.ctx().set_cursor_icon(cur);
        }
        if let Some(info) = info {
            resp.widget_info(info);
        }
        // EM1: an accessible name over the widget's own (as `a11y::decorate`)
        let resp = match name {
            Some(n) if !n.is_empty() => resp.accessible_name(n),
            _ => resp,
        };
        {
            let u = lock(&units);
            let (fp, pp, tp) = (c.focus.unwrap_or_default(), c.press.unwrap_or_default(), c.toggle.unwrap_or_default());
            ui.ctx().accesskit_node_builder(resp.id, |n| {
                a11y(n);
                if !units_a11y {
                    return;
                }
                if c.hover {
                    u.hover.a11y(&(), n);
                }
                if c.focus.is_some() {
                    u.focus.a11y(&fp, n);
                }
                if c.press.is_some() {
                    u.press.a11y(&pp, n);
                }
                if c.toggle.is_some() {
                    u.toggle.a11y(&tp, n);
                }
            });
        }
        if let Some(r) = focus_role {
            crate::focus::record_foreign(&ui, &resp, r);
        }
        match tooltip {
            Some(t) if !t.is_empty() => {
                if resp.enabled() {
                    resp.on_hover_text(t)
                } else {
                    resp.on_disabled_hover_text(t)
                }
            }
            _ => resp,
        }
    }
}

/// What an `imm` widget returns: egui's response (`clicked()`, `hovered()`, `rect`, …, through
/// `Deref`) and what the widget's units reported, as the kind's events.
#[derive(Clone, Debug)]
pub struct Response {
    pub inner: egui::Response,
    /// `clicked` (pointer, Space, Enter, AccessKit `Click`, `repeat` steps).
    pub activated: bool,
    pub double_clicked: bool,
    pub secondary_clicked: bool,
    /// A toggle's value changed (`changed` with the new value written to the caller's `bool`).
    pub changed: bool,
    /// The widget's state bits this pass (`:hover`, `:active`, `:focus-visible`, `:checked`, …).
    pub state: StateBits,
}

impl Response {
    /// Activated this pass (a button's `clicked` event).
    pub fn clicked(&self) -> bool {
        self.activated
    }
    pub fn changed(&self) -> bool {
        self.changed
    }
}

impl std::ops::Deref for Response {
    type Target = egui::Response;
    fn deref(&self) -> &egui::Response {
        &self.inner
    }
}

/// `name` in the `icons` family, or `family:name`, as the button's `icon` prop.
pub(crate) fn icon_key(key: &str) -> crate::paint::IconKey {
    match key.split_once(':') {
        Some((f, n)) => crate::paint::IconKey::new(f, n),
        None => crate::paint::IconKey::new("icons", key),
    }
}

/// The one place `imm` paints an icon by key (`imm::Icon`, `imm::chrome::icon`): today's icon
/// path (`paint::paint_icon`, the font glyph). ICR1 re-points this function at its registry
/// (`icons::paint`, falling back to the glyph on an unknown key).
pub(crate) fn paint_icon_key(pc: &mut PaintCx<'_>, v: &StyleView, key: &str, rect: egui::Rect) {
    crate::paint::paint_icon(pc, v, &icon_key(key), rect);
}

/// The id of an `imm` widget: `id` as given, else `ui`'s next auto id (stable while the same
/// widgets are drawn in the same order, as egui's own widgets).
pub(crate) fn widget_id(ui: &mut egui::Ui, id: Option<egui::Id>) -> egui::Id {
    id.unwrap_or_else(|| {
        let id = ui.next_auto_id();
        ui.skip_ahead_auto_ids(1);
        id
    })
}
