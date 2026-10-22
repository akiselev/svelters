//! Shared test plumbing: the egui backend behind its real vtable, attached to a recording host
//! (no JS), with a kittest harness that runs the backend's frames. Headless: no display, no GPU.
#![allow(dead_code)]

use egui_kittest::Harness;
use std::cell::RefCell;
use std::rc::Rc;
use svelters_abi::*;
use svelters_egui::EguiBackend;

#[derive(Default)]
pub struct Log {
    pub events: Vec<(svl_handle, svl_id, Vec<Value>)>,
    pub changed: Vec<(svl_handle, svl_id)>,
    pub gone: Vec<(svl_handle, u32)>,
    pub checkpoints: usize,
    pub frames: usize,
    /// What `call_sync` answers.
    pub sync_answer: Option<Value>,
    pub syncs: Vec<(svl_handle, svl_id)>,
}

pub struct Rec(pub Rc<RefCell<Log>>);

impl HostCallbacks for Rec {
    fn post_event(&self, h: svl_handle, signal: svl_id, args: &[Value]) {
        self.0.borrow_mut().events.push((h, signal, args.to_vec()));
    }
    fn checkpoint(&self) {
        self.0.borrow_mut().checkpoints += 1;
    }
    fn call_sync(
        &self,
        h: svl_handle,
        signal: svl_id,
        _args: &[Value],
    ) -> Result<Value, SyncError> {
        let mut l = self.0.borrow_mut();
        l.syncs.push((h, signal));
        l.sync_answer
            .clone()
            .ok_or(SyncError::Failed("no answer".into()))
    }
    fn on_destroyed(&self, h: svl_handle, reason: u32) {
        self.0.borrow_mut().gone.push((h, reason));
    }
    fn frame(&self) {
        self.0.borrow_mut().frames += 1;
    }
    fn prop_changed(&self, h: svl_handle, prop: svl_id) {
        self.0.borrow_mut().changed.push((h, prop));
    }
}

pub struct Rig {
    pub backend: EguiBackend,
    pub vt: &'static BackendVtable,
    pub hv: &'static HostVtable,
    pub log: Rc<RefCell<Log>>,
    pub harness: Harness<'static, EguiBackend>,
}

impl Rig {
    pub fn new() -> Rig {
        let backend = EguiBackend::new();
        let vt: &'static BackendVtable = Box::leak(Box::new(backend.vtable()));
        let log: Rc<RefCell<Log>> = Rc::default();
        let hv: &'static HostVtable = Box::leak(Box::new(HostVtable::new(Rec(log.clone()))));
        vt.backend_ref().start(hv).expect("start");
        let harness = Harness::builder()
            .with_size(egui::vec2(480.0, 360.0))
            .build_ui_state(|ui, b: &mut EguiBackend| b.ui(ui), backend.clone());
        Rig {
            backend,
            vt,
            hv,
            log,
            harness,
        }
    }

    pub fn br(&self) -> BackendRef<'static> {
        self.vt.backend_ref()
    }

    pub fn kind(&self, h: svl_handle) -> svl_id {
        self.br().handle_info(h).unwrap().kind
    }

    pub fn create(&self, tag: &str) -> svl_handle {
        let k = self.br().resolve(NameKind::Kind, 0, tag);
        assert_ne!(k, 0, "unknown tag {tag}");
        self.br().create(k).unwrap()
    }

    pub fn prop_id(&self, h: svl_handle, name: &str) -> svl_id {
        let id = self.br().resolve(NameKind::Prop, self.kind(h), name);
        assert_ne!(id, 0, "no prop {name}");
        id
    }

    pub fn signal_id(&self, h: svl_handle, name: &str) -> svl_id {
        let id = self.br().resolve(NameKind::Signal, self.kind(h), name);
        assert_ne!(id, 0, "no signal {name}");
        id
    }

    pub fn set(&self, h: svl_handle, name: &str, v: Value) {
        self.br().set_prop(h, self.prop_id(h, name), &v).unwrap()
    }

    pub fn get(&self, h: svl_handle, name: &str) -> Value {
        self.br().get_prop(h, self.prop_id(h, name)).unwrap()
    }

    pub fn listen(&self, h: svl_handle, name: &str) {
        self.br().listen(h, self.signal_id(h, name), true).unwrap()
    }

    pub fn observe(&self, h: svl_handle, name: &str, on: bool) {
        self.br().observe(h, self.prop_id(h, name), on).unwrap()
    }

    pub fn insert(&self, parent: svl_handle, child: svl_handle) {
        self.br().insert(parent, child, None, 0).unwrap()
    }

    pub fn call(&self, h: svl_handle, method: &str) {
        let m = self.br().resolve(NameKind::Method, self.kind(h), method);
        self.br().call(h, m, &[]).unwrap();
    }

    /// A presented window holding a column box; returns (window, box).
    pub fn window(&self) -> (svl_handle, svl_handle) {
        let w = self.create("window");
        let b = self.create("box");
        self.set(b, "gap", Value::F64(8.0));
        self.insert(w, b);
        self.call(w, "present");
        (w, b)
    }

    pub fn leaf(&self, parent: svl_handle, tag: &str, props: &[(&str, Value)]) -> svl_handle {
        let h = self.create(tag);
        for (k, v) in props {
            self.set(h, k, v.clone());
        }
        self.insert(parent, h);
        h
    }

    pub fn run(&mut self) -> u64 {
        self.harness.run()
    }

    /// Where `h` was drawn last frame.
    pub fn drawn(&self, h: svl_handle) -> egui::Rect {
        self.backend.with_scene(|s| s.n(h).drawn)
    }

    /// A primary click at the centre of `h` (by position, so same-labelled widgets work).
    pub fn click(&mut self, h: svl_handle) {
        let pos = self.drawn(h).center();
        self.harness.hover_at(pos);
        for pressed in [true, false] {
            self.harness.event(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
        }
        self.harness.run();
    }

    /// Focuses text edit `h`, selects everything and types `text` (one frame of input).
    pub fn type_into(&mut self, h: svl_handle, text: &str) {
        self.click(h);
        self.harness
            .key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        self.harness.event(egui::Event::Text(text.to_string()));
        self.harness.run();
    }

    pub fn take(&self) -> Log {
        let mut l = self.log.borrow_mut();
        let answer = l.sync_answer.clone();
        let mut out = std::mem::take(&mut *l);
        l.sync_answer = answer;
        out.sync_answer = None;
        out
    }
}
