//! The host ops: `impl Backend for EguiBackend`. Every scene edit goes through the scene's one write
//! path with `Origin::Js` (`scene/tree.rs`, `Scene::set_value`: kind and cycle checks, root cache,
//! dirty bits and per-root queues); this file adds the window list, egui memory cleanup and the
//! repaint.
//!
//! Also the TR1 visual channels of the `style` op (`visual_op`), until the STYLE lane's `style.rs`
//! takes the style ops. Writer: shared (changes small).

use crate::backend::EguiBackend;
use crate::kinds;
use crate::layout;
use crate::scene::{self, Handle, NodeFlags, Origin};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use svelters_abi::*;

impl EguiBackend {
    // ---- TR1 visual channels ----

    pub(crate) fn visual_op(&self, op: u32, h: svl_handle, text: Option<&str>) -> StrResult<()> {
        fn nums<const N: usize>(text: &str, what: &str) -> StrResult<[f32; N]> {
            let v: Vec<f32> = text.split_whitespace().map(str::parse::<f32>).collect::<Result<_, _>>().map_err(|_| format!("{what}: '{text}' is not {N} numbers"))?;
            v.try_into().map_err(|_| format!("{what}: '{text}' is not {N} numbers").into())
        }
        let mut scene = self.0.scene.borrow_mut();
        let id = scene.live(h)?;
        // an animated node's static visual is the base under its animations (TR2)
        let mut vis = self.0.anim.borrow().static_of(h).unwrap_or_else(|| scene.vis(scene.ix(id)));
        match op {
            SVL_STYLE_OPACITY => {
                vis.opacity = match text {
                    None => None,
                    Some(t) => {
                        let t = t.trim();
                        let a = match t.strip_suffix('%') {
                            Some(p) => p.parse::<f32>().map(|x| x / 100.0),
                            None => t.parse::<f32>(),
                        }
                        .map_err(|_| format!("opacity: '{t}' is not a number"))?;
                        (a < 1.0).then_some(a.max(0.0))
                    }
                }
            }
            SVL_STYLE_TRANSFORM => vis.transform = text.map(|t| nums::<6>(t, "transform")).transpose()?,
            SVL_STYLE_CLIP => vis.clip = text.map(|t| nums::<2>(t, "clip")).transpose()?,
            _ => vis.inert = text.is_some(),
        }
        if let Some(composed) = self.0.anim.borrow_mut().set_base(h, vis) {
            vis = composed;
        }
        if !scene.set_visual(id, vis) {
            return Ok(());
        }
        let r = scene.core[scene.ix(id)].root;
        drop(scene);
        self.touched(Some(r));
        Ok(())
    }

    /// TR2: `SVL_STYLE_ANIMATE`. A start samples the first keyframe at once and asks for frames;
    /// the logic pass steps it from then on (`animate_step`).
    pub(crate) fn animate_op(&self, h: svl_handle, text: Option<&str>) -> StrResult<()> {
        let cmd = crate::anim::parse(text.ok_or("animate: needs the animation text")?)?;
        let mut scene = self.0.scene.borrow_mut();
        let id = scene.live(h)?;
        let i = scene.ix(id);
        let composed = {
            let mut a = self.0.anim.borrow_mut();
            match cmd {
                crate::anim::Cmd::Start(spec) => {
                    let base = a.static_of(h).unwrap_or_else(|| scene.vis(i));
                    Some(a.start(h, base, spec))
                }
                crate::anim::Cmd::Cancel(n) => a.cancel(h, n),
            }
        };
        let r = scene.core[i].root;
        if let Some(v) = composed {
            scene.set_visual(id, v);
        }
        drop(scene);
        self.touched(Some(r));
        // the step that advances it runs in the logic pass of the root viewport
        if let Some(ctx) = self.ctx() {
            ctx.request_repaint_of(self.logic_vp());
        }
        Ok(())
    }

    // ---- ops helpers ----

    pub(crate) fn kind_of(&self, h: Handle) -> Result<usize, String> {
        let scene = self.0.scene.borrow();
        let id = scene.live(h)?;
        Ok(scene.core[scene.ix(id)].kind as usize)
    }
}

impl Backend for EguiBackend {
    fn resolve(&self, what: NameKind, kind: svl_id, name: &str) -> svl_id {
        if what == NameKind::Kind {
            return kinds::tag_index(name).map(kinds::kind_id).unwrap_or(0);
        }
        let Some(k) = kinds::kind_of_id(kind) else { return 0 };
        match what {
            NameKind::Prop => kinds::find_prop(k, name).map(|(id, _)| id).unwrap_or(0),
            NameKind::Signal => kinds::find_signal(k, name).map(|(id, _)| id).unwrap_or(0),
            NameKind::Method => kinds::find_method(k, name).map(|(id, _)| id).unwrap_or(0),
            NameKind::Slot => (kinds::slot_of(k) == Some(name)) as u32,
            _ => 0,
        }
    }

    fn create(&self, kind: svl_id) -> StrResult<svl_handle> {
        let k = kinds::kind_of_id(kind).filter(|&k| kinds::kind(k).tag.is_some()).ok_or_else(|| format!("unknown kind id {kind}"))?;
        let id = self.0.scene.borrow_mut().create(k, scene::Owner::Js).map_err(|e| e.to_string())?;
        Ok(id.to_raw())
    }

    fn insert(&self, parent: svl_handle, child: svl_handle, before: Option<svl_handle>, slot: svl_id) -> StrResult<()> {
        let pk = self.kind_of(parent)?;
        self.kind_of(child)?;
        if slot > 1 && kinds::slot_of(pk).is_some() {
            return Err(format!("unknown slot id {slot} on <{}>", kinds::kind(pk).tag.unwrap_or("?")));
        }
        let changed = {
            let mut scene = self.0.scene.borrow_mut();
            let (p, c) = (scene.live(parent)?, scene.live(child)?);
            let b = before.map(|b| scene::NodeId::from_raw(b).ok_or_else(|| format!("handle #{b} is not a child of #{parent}"))).transpose()?;
            scene.insert(p, c, b, Origin::Js).map_err(|e| e.to_string())?
        };
        if !changed {
            return Ok(()); // already in place
        }
        self.touched(self.root_of(parent));
        Ok(())
    }

    fn remove(&self, parent: svl_handle, child: svl_handle) -> StrResult<()> {
        {
            let mut scene = self.0.scene.borrow_mut();
            let (Some(p), Some(c)) = (scene::NodeId::from_raw(parent), scene::NodeId::from_raw(child)) else {
                return Err("stale or foreign handle".into());
            };
            scene.remove(p, c, Origin::Js).map_err(|e| e.to_string())?;
        }
        self.touched(self.root_of(parent));
        Ok(())
    }

    fn destroy(&self, h: svl_handle) {
        // Children still attached are orphaned: the host destroys them itself.
        let r = self.root_of(h);
        let d = {
            let mut scene = self.0.scene.borrow_mut();
            let Ok(id) = scene.live(h) else { return };
            match scene.destroy(id, Origin::Js) {
                Ok(d) => d,
                Err(_) => return,
            }
        };
        // Ephemeral widget state (cursor, scroll, animations) lives in egui memory under the node.
        if let Some(ctx) = self.ctx() {
            ctx.memory_mut(|m| m.data.remove::<egui::text_edit::TextEditState>(d.egui_id));
        }
        if d.was_window {
            self.0.windows.borrow_mut().retain(|&w| w != h);
        }
        self.0.anim.borrow_mut().forget(h);
        self.touched(r);
    }

    fn set_prop(&self, h: svl_handle, prop: svl_id, v: &Value) -> StrResult<()> {
        {
            let mut scene = self.0.scene.borrow_mut();
            let id = scene.live(h)?;
            // Host writes never announce a change: there is no echo to suppress (EGUI.md §1.4).
            scene.set_value(id, prop, v, Origin::Js).map_err(|e| e.to_string())?;
        }
        self.touched(self.root_of(h));
        Ok(())
    }

    fn get_prop(&self, h: svl_handle, prop: svl_id) -> StrResult<Value> {
        let scene = self.0.scene.borrow();
        let n = scene.get(h).ok_or_else(|| format!("stale or foreign handle {h}"))?;
        let def = kinds::prop_def(n.kind, prop).ok_or_else(|| format!("unknown prop id {prop} on <{}>", n.tag()))?;
        scene::get_prop(&n, def)
    }

    fn call(&self, h: svl_handle, method: svl_id, args: &[Value]) -> StrResult<Value> {
        let k = self.kind_of(h)?;
        let def = kinds::method_def(k, method).ok_or_else(|| format!("unknown method id {method}"))?;
        // RA3: a provider kind's own method (`#[svelters::widget] impl`)
        let reg = kinds::KindRegistry::global();
        if let (Some(m), Some(km)) = (reg.provider_method_index(k, method), reg.provider(k).and_then(|p| p.methods)) {
            let mut out = Vec::new();
            let r = {
                let mut scene = self.0.scene.borrow_mut();
                let i = scene.ix(scene.live(h)?);
                crate::provider::methods::call(&mut scene, i, km, m, args, &mut out)
            };
            self.0.pending_out.borrow_mut().extend(out);
            self.touched(self.root_of(h));
            return r;
        }
        if matches!(kinds::kind(k).tag, Some("virtual-list" | "tree-view")) {
            return Ok(self.vlist_call(h, def.name, args)?);
        }
        // WG4a: the views' methods take arguments (`crate::views::call`)
        if let Some(r) = crate::views::call(self, h, def.name, args) {
            return r;
        }
        if !args.is_empty() {
            return Err(format!("{} takes no arguments", def.name));
        }
        match def.name {
            "present" => {
                let mut ws = self.0.windows.borrow_mut();
                if !ws.contains(&h) {
                    ws.push(h);
                }
                let mut scene = self.0.scene.borrow_mut();
                let i = scene.ix(scene.live(h)?);
                scene.core[i].flags.insert(NodeFlags::PRESENTED);
                let r = scene.core[i].root;
                scene.mark_full(r);
            }
            "close" => self.0.closes.borrow_mut().push(h),
            "request-focus" => {
                let mut scene = self.0.scene.borrow_mut();
                let i = scene.ix(scene.live(h)?);
                scene.core[i].flags.insert(NodeFlags::WANT_FOCUS);
            }
            // WG2a: context-menu, menu-button, popup (applied and announced at the next pass)
            "popup" | "popdown" => {
                let mut scene = self.0.scene.borrow_mut();
                let i = scene.ix(scene.live(h)?);
                crate::menus::call(&mut scene, i, def.name == "popup");
            }
            other => return Err(format!("method {other} is not implemented")),
        }
        self.touched(self.root_of(h));
        Ok(Value::Null)
    }

    fn listen(&self, h: svl_handle, signal: svl_id, on: bool) -> StrResult<()> {
        let mut scene = self.0.scene.borrow_mut();
        let i = scene.ix(scene.live(h)?);
        let kind = scene.core[i].kind as usize;
        kinds::signal_def(kind, signal).ok_or_else(|| format!("unknown signal id {signal} on <{}>", scene.tag_at(i)))?;
        scene.set_listen(i, signal, on);
        let r = scene.core[i].root;
        drop(scene);
        self.touched(Some(r)); // a listener can change a widget's sense
        Ok(())
    }

    fn handle_info(&self, h: svl_handle) -> StrResult<svl_handle_info> {
        let k = self.kind_of(h)?;
        let host_owned = {
            let scene = self.0.scene.borrow();
            scene.live(h).is_ok_and(|id| scene.core[scene.ix(id)].owner == scene::Owner::Host)
        };
        Ok(svl_handle_info {
            kind: kinds::kind_id(k),
            // EM1: a slot (`Owner::Host`) belongs to the embedder; JS mounts into it but cannot destroy it
            lifetime: if host_owned {
                SVL_LIFE_BORROWED
            } else if kinds::kind(k).toplevel {
                SVL_LIFE_TOPLEVEL
            } else {
                SVL_LIFE_OWNED
            },
            flags: 0,
        })
    }

    fn run(&self, host: &HostRef<'_>) -> StrResult<()> {
        self.0.host.set(host.as_raw());
        self.0.quitting.set(false);
        crate::runner::run_eframe(self)
    }

    fn waker(&self) -> Waker {
        let shared = self.0.shared.clone();
        Arc::new(move || {
            shared.wake.store(true, Ordering::Release);
            // a wake does not know its viewport: the one that runs logic next (the root viewport,
            // whose logic routes repaints, §5.2; or the deferred viewport whose pass runs now)
            let vp = shared.logic_vp.lock().unwrap().unwrap_or(egui::ViewportId::ROOT);
            if let Some(ctx) = shared.ctx.lock().unwrap().as_ref() {
                ctx.request_repaint_of(vp);
            }
        })
    }

    fn quit(&self) {
        self.0.quitting.set(true);
        if let Some(ctx) = self.ctx() {
            ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Close);
            ctx.request_repaint();
        }
    }

    fn schedule_wake(&self, ms: i64) {
        if ms < 0 {
            self.0.deadline.set(None);
            return;
        }
        self.0.deadline.set(Some(Instant::now() + Duration::from_millis(ms as u64)));
        if !self.0.in_frame.get()
            && let Some(ctx) = self.ctx()
        {
            ctx.request_repaint_after_for(Duration::from_millis(ms as u64), egui::ViewportId::ROOT);
        }
    }

    fn request_frame(&self) {
        self.0.frame_pending.set(true);
        if let Some(ctx) = self.ctx() {
            ctx.request_repaint_of(self.logic_vp());
        }
    }

    fn style(&self, op: u32, h: svl_handle, text: Option<&str>) -> StrResult<i32> {
        // CS5e: the Rust CSS engine in process (`style.rs`); TR1's visual channels go through it.
        self.style_op(op, h, text)
    }

    fn supports_bounds(&self) -> bool {
        true
    }

    /// ABI v2.8: the node's rect in its window's content area. A dirty layout is brought up to date
    /// first (synchronously, as Qt's `QLayout::activate`), so FLIP sees the new place at once. A
    /// node outside a laid-out window reports 0, 0 and its measured size (leaves; 0 for boxes).
    fn bounds(&self, h: svl_handle) -> StrResult<[f64; 4]> {
        self.kind_of(h)?;
        self.layout_now(h);
        let scene = self.0.scene.borrow();
        let n = scene.n(h);
        if n.rect != egui::Rect::NOTHING {
            let r = n.rect;
            return Ok([r.min.x as f64, r.min.y as f64, r.width() as f64, r.height() as f64]);
        }
        let s = match (n.intrinsic, self.ctx()) {
            (Some(s), _) => s,
            (None, Some(ctx)) => layout::Metrics::new(&ctx).intrinsic(&n.w),
            (None, None) => egui::Vec2::ZERO,
        };
        Ok([0.0, 0.0, s.x as f64, s.y as f64])
    }

    fn supports_observe(&self) -> bool {
        true
    }

    fn observe(&self, h: svl_handle, prop: svl_id, on: bool) -> Result<(), ObserveError> {
        let mut scene = self.0.scene.borrow_mut();
        let i = scene.live(h).map(|id| scene.ix(id)).map_err(ObserveError::Failed)?;
        kinds::prop_def(scene.core[i].kind as usize, prop).ok_or_else(|| ObserveError::Failed(format!("unknown prop id {prop} on <{}>", scene.tag_at(i))))?;
        // Every prop is exact: the scene is the only place a value changes.
        scene.observe(i, prop, on);
        Ok(())
    }

    fn supports_start(&self) -> bool {
        true
    }

    fn start(&self, host: Option<&HostRef<'_>>) -> StrResult<()> {
        // egui needs no global init: attaching is all; the caller's frames drive everything.
        self.0.host.set(host.map(|h| h.as_raw()).unwrap_or(std::ptr::null()));
        Ok(())
    }

    fn describe(&self, what: NameKind, kind: svl_id, member: svl_id) -> Option<MemberDesc> {
        if what == NameKind::Method {
            // RA3: a provider kind's method is described by its Rust signature (`JsType`).
            let k = kinds::kind_of_id(kind)?;
            let reg = kinds::KindRegistry::global();
            return reg.provider_method_desc(k, reg.provider_method_index(k, member)?);
        }
        if what != NameKind::Prop {
            return None;
        }
        let k = kinds::kind_of_id(kind)?;
        let def = kinds::prop_def(k, member)?;
        if let kinds::P::Ext(i) = def.key {
            // RA1: a provider prop is described by its Rust type (`JsType`).
            return kinds::KindRegistry::global().provider_member_desc(k, i as usize);
        }
        let mut slot = SlotDesc::new(scene::tag_of(def.ty)).named(def.name);
        if def.nullable {
            slot = slot.flags(SVL_D_NULLABLE);
        }
        if def.ty == kinds::Ty::I64 {
            slot = slot.flags(SVL_D_INTEGRAL);
        }
        Some(MemberDesc { name: def.name.to_string(), params: vec![slot.clone()], ret: slot, flags: SVL_M_OBSERVABLE })
    }
}
