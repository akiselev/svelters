//! The host ops: `impl Backend for EguiBackend`. Tree edits go through `scene/tree.rs`, prop values
//! through `scene/props.rs`; this file adds the kind checks, the layout flags and the repaint.
//!
//! Also the TR1 visual channels of the `style` op (`visual_op`), until the STYLE lane's `style.rs`
//! takes the style ops. Writer: shared (changes small).

use crate::backend::EguiBackend;
use crate::kinds::{self, KINDS};
use crate::layout;
use crate::scene::{self, Handle, tree};
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
        let n = scene.get_mut(h).ok_or_else(|| format!("stale or foreign handle {h}"))?;
        let before = n.vis;
        match op {
            SVL_STYLE_OPACITY => {
                n.vis.opacity = match text {
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
            SVL_STYLE_TRANSFORM => n.vis.transform = text.map(|t| nums::<6>(t, "transform")).transpose()?,
            SVL_STYLE_CLIP => n.vis.clip = text.map(|t| nums::<2>(t, "clip")).transpose()?,
            _ => n.vis.inert = text.is_some(),
        }
        if n.vis == before {
            return Ok(());
        }
        if n.vis.clip != before.clip {
            layout::restyle(n);
            layout::invalidate(&mut scene, h);
            self.0.dirty.layout_dirty.set(true);
        }
        drop(scene);
        self.touched();
        Ok(())
    }

    // ---- ops helpers ----

    pub(crate) fn kind_of(&self, h: Handle) -> Result<usize, String> {
        self.0.scene.borrow().get(h).map(|n| n.kind).ok_or_else(|| format!("stale or foreign handle {h}"))
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
        let k = kinds::kind_of_id(kind).filter(|&k| KINDS[k].tag.is_some()).ok_or_else(|| format!("unknown kind id {kind}"))?;
        let h = self.0.scene.borrow_mut().alloc(|h| scene::new_node(k, h));
        Ok(h)
    }

    fn insert(&self, parent: svl_handle, child: svl_handle, before: Option<svl_handle>, slot: svl_id) -> StrResult<()> {
        let pk = self.kind_of(parent)?;
        let ck = self.kind_of(child)?;
        if kinds::slot_of(pk).is_none() {
            return Err(format!("<{}> takes no children", KINDS[pk].tag.unwrap_or("?")));
        }
        if slot > 1 {
            return Err(format!("unknown slot id {slot} on <{}>", KINDS[pk].tag.unwrap_or("?")));
        }
        if KINDS[ck].toplevel {
            return Err("a window cannot be inserted under another element".into());
        }
        if parent == child {
            return Err("cannot insert an element into itself".into());
        }
        {
            let mut scene = self.0.scene.borrow_mut();
            if !tree::insert(&mut scene, parent, child, before)? {
                return Ok(()); // already in place
            }
            // EM1: a subtree joining an embedded root takes the root's ids (ARCH-EMBED §4.4)
            crate::ids::adopt(&mut scene, &self.0.root_bases.borrow(), parent, child);
        }
        self.0.dirty.layout_dirty.set(true);
        self.touched();
        Ok(())
    }

    fn remove(&self, parent: svl_handle, child: svl_handle) -> StrResult<()> {
        tree::remove(&mut self.0.scene.borrow_mut(), parent, child)?;
        self.0.dirty.layout_dirty.set(true);
        self.touched();
        Ok(())
    }

    fn destroy(&self, h: svl_handle) {
        // Children still attached are orphaned: the host destroys them itself.
        let Some(n) = tree::destroy(&mut self.0.scene.borrow_mut(), h) else { return };
        // Ephemeral widget state (cursor, scroll, animations) lives in egui memory under the node.
        if let Some(ctx) = self.ctx() {
            ctx.memory_mut(|m| m.data.remove::<egui::text_edit::TextEditState>(n.id));
        }
        if n.is_window() {
            self.0.windows.borrow_mut().retain(|&w| w != h);
        }
        self.0.dirty.layout_dirty.set(true);
        self.touched();
    }

    fn set_prop(&self, h: svl_handle, prop: svl_id, v: &Value) -> StrResult<()> {
        let mut scene = self.0.scene.borrow_mut();
        let n = scene.get_mut(h).ok_or_else(|| format!("stale or foreign handle {h}"))?;
        let def = kinds::prop_def(n.kind, prop).ok_or_else(|| format!("unknown prop id {prop} on <{}>", n.tag()))?;
        // Host writes never announce a change: there is no echo to suppress (EGUI.md §1.4).
        let d = scene::set_prop(n, def, v).map_err(|e| format!("<{}> {}: {e}", n.tag(), def.name))?;
        if d.style {
            layout::invalidate(&mut scene, h);
            self.0.dirty.layout_dirty.set(true);
        }
        if def.key == kinds::P::Anchor {
            crate::ids::reanchor(&mut scene, &self.0.root_bases.borrow(), h);
        }
        drop(scene);
        if d.measure {
            self.0.dirty.remeasure.borrow_mut().push(h);
        }
        self.touched();
        Ok(())
    }

    fn get_prop(&self, h: svl_handle, prop: svl_id) -> StrResult<Value> {
        let scene = self.0.scene.borrow();
        let n = scene.get(h).ok_or_else(|| format!("stale or foreign handle {h}"))?;
        let def = kinds::prop_def(n.kind, prop).ok_or_else(|| format!("unknown prop id {prop} on <{}>", n.tag()))?;
        scene::get_prop(n, def)
    }

    fn call(&self, h: svl_handle, method: svl_id, args: &[Value]) -> StrResult<Value> {
        let k = self.kind_of(h)?;
        let def = kinds::method_def(k, method).ok_or_else(|| format!("unknown method id {method}"))?;
        if !args.is_empty() {
            return Err(format!("{} takes no arguments", def.name));
        }
        match def.name {
            "present" => {
                let mut ws = self.0.windows.borrow_mut();
                if !ws.contains(&h) {
                    ws.push(h);
                }
                self.0.scene.borrow_mut().n_mut(h).presented = true;
                self.0.dirty.layout_dirty.set(true);
            }
            "close" => self.0.closes.borrow_mut().push(h),
            "request-focus" => self.0.scene.borrow_mut().n_mut(h).want_focus = true,
            other => return Err(format!("method {other} is not implemented")),
        }
        self.touched();
        Ok(Value::Null)
    }

    fn listen(&self, h: svl_handle, signal: svl_id, on: bool) -> StrResult<()> {
        let mut scene = self.0.scene.borrow_mut();
        let n = scene.get_mut(h).ok_or_else(|| format!("stale or foreign handle {h}"))?;
        kinds::signal_def(n.kind, signal).ok_or_else(|| format!("unknown signal id {signal} on <{}>", n.tag()))?;
        if on {
            if !n.listen.contains(&signal) {
                n.listen.push(signal);
            }
        } else {
            n.listen.retain(|&s| s != signal);
        }
        drop(scene);
        self.touched(); // a listener can change a widget's sense
        Ok(())
    }

    fn handle_info(&self, h: svl_handle) -> StrResult<svl_handle_info> {
        let k = self.kind_of(h)?;
        let host_owned = self.0.scene.borrow().get(h).is_some_and(|n| n.host_owned);
        Ok(svl_handle_info {
            kind: kinds::kind_id(k),
            // EM1: a slot belongs to the embedder; JS mounts into it but cannot destroy it
            lifetime: if host_owned {
                SVL_LIFE_BORROWED
            } else if KINDS[k].toplevel {
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
            if let Some(ctx) = shared.ctx.lock().unwrap().as_ref() {
                ctx.request_repaint();
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
            ctx.request_repaint_after(Duration::from_millis(ms as u64));
        }
    }

    fn request_frame(&self) {
        self.0.frame_pending.set(true);
        if let Some(ctx) = self.ctx() {
            ctx.request_repaint();
        }
    }

    fn style(&self, op: u32, h: svl_handle, text: Option<&str>) -> StrResult<i32> {
        // G1: setter-only dialect (layout props are setters, D3); the cascade is G2.
        match op {
            SVL_STYLE_TARGET => Ok(SVL_STYLE_TARGET_NONE),
            SVL_STYLE_SHEET | SVL_STYLE_SCHEME => Ok(0),
            // TR1 visual channels (opacity since v2.3, the rest v2.8): paint state on the node
            SVL_STYLE_OPACITY | SVL_STYLE_TRANSFORM | SVL_STYLE_CLIP | SVL_STYLE_INERT => {
                self.visual_op(op, h, text)?;
                Ok(0)
            }
            _ => {
                self.kind_of(h)?;
                Ok(0)
            }
        }
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
        let n = scene.get_mut(h).ok_or_else(|| ObserveError::Failed(format!("stale or foreign handle {h}")))?;
        kinds::prop_def(n.kind, prop).ok_or_else(|| ObserveError::Failed(format!("unknown prop id {prop} on <{}>", n.tag())))?;
        // Every prop is exact: the scene is the only place a value changes.
        let c = n.observe.entry(prop).or_insert(0);
        if on {
            *c += 1;
        } else {
            *c = c.saturating_sub(1);
            if *c == 0 {
                n.observe.remove(&prop);
            }
        }
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
        if what != NameKind::Prop {
            return None;
        }
        let def = kinds::prop_def(kinds::kind_of_id(kind)?, member)?;
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
