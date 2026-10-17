//! [`Toolkit`] over GTK4: widgets are reached through the backend's handle table and driven with
//! GTK's own APIs.

use crate::driver::Toolkit;
use gtk::glib;
use gtk::prelude::*;
use serde_json::{Value as Json, json};
use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};
use svelters_gtk::GtkBackend;

pub struct GtkToolkit {
    pub backend: &'static GtkBackend,
}

impl GtkToolkit {
    fn widget(&self, h: u64) -> Result<gtk::Widget, String> {
        self.backend
            .object(h)
            .and_then(|o| o.downcast::<gtk::Widget>().ok())
            .ok_or_else(|| format!("handle {h} is not a live widget"))
    }

    fn texture(&self, h: u64) -> Result<gtk::gdk::Texture, String> {
        let w = self.widget(h)?;
        let paintable = gtk::WidgetPaintable::new(Some(&w));
        let (pw, ph) = (w.width().max(1) as f64, w.height().max(1) as f64);
        let snapshot = gtk::Snapshot::new();
        paintable.snapshot(&snapshot, pw, ph);
        let node = snapshot.to_node().ok_or("empty snapshot (widget not drawn yet?)")?;
        let renderer = w.native().and_then(|n| n.renderer()).ok_or("widget is not realized")?;
        Ok(renderer.render_texture(&node, None))
    }

    /// Calls `done` with the time from `start` to the first painted frame of window `h`.
    pub fn on_first_frame(&self, h: u64, start: Instant, done: Rc<Cell<Option<Duration>>>) {
        let Ok(w) = self.widget(h) else { return };
        w.add_tick_callback(move |_, clock| {
            let done = done.clone();
            clock.connect_after_paint(move |_| {
                if done.get().is_none() {
                    done.set(Some(start.elapsed()));
                }
            });
            glib::ControlFlow::Break
        });
    }
}

fn dump_widget(w: &gtk::Widget, depth: u32, out: &mut Vec<Json>) {
    // Buttons, entries and header bars are leaves: their internal labels and text widgets are
    // toolkit details, not app content.
    let mut leaf = true;
    let text = if let Some(l) = w.downcast_ref::<gtk::Label>() {
        l.text().to_string()
    } else if let Some(b) = w.downcast_ref::<gtk::Button>() {
        b.label().map(|s| s.to_string()).unwrap_or_default()
    } else if let Some(e) = w.downcast_ref::<gtk::Entry>() {
        e.text().to_string()
    } else if let Some(hb) = w.downcast_ref::<gtk::HeaderBar>() {
        hb.title_widget()
            .and_then(|t| t.downcast::<gtk::Label>().ok())
            .map(|l| l.text().to_string())
            .unwrap_or_default()
    } else if let Some(win) = w.downcast_ref::<gtk::Window>() {
        leaf = false;
        win.title().map(|s| s.to_string()).unwrap_or_default()
    } else {
        leaf = false;
        String::new()
    };
    out.push(json!({"d": depth, "class": w.type_().name(), "text": text}));
    if leaf {
        return;
    }
    let mut child = w.first_child();
    while let Some(c) = child {
        dump_widget(&c, depth + 1, out);
        child = c.next_sibling();
    }
}

impl Toolkit for GtkToolkit {
    fn click(&self, h: u64) -> Result<(), String> {
        let b = self.widget(h)?.downcast::<gtk::Button>().map_err(|_| format!("handle {h} is not a button"))?;
        b.emit_clicked();
        Ok(())
    }

    fn type_text(&self, h: u64, text: &str) -> Result<(), String> {
        let e = self.widget(h)?.downcast::<gtk::Editable>().map_err(|_| format!("handle {h} is not editable"))?;
        e.select_region(0, -1);
        e.delete_selection();
        let mut pos = e.position();
        for ch in text.chars() {
            e.insert_text(&ch.to_string(), &mut pos);
        }
        Ok(())
    }

    fn dump(&self, h: u64) -> Json {
        let mut out = Vec::new();
        if let Ok(w) = self.widget(h) {
            dump_widget(&w, 0, &mut out);
        }
        Json::Array(out)
    }

    fn screenshot(&self, h: u64, path: &str) -> Result<(), String> {
        self.texture(h)?.save_to_png(path).map_err(|e| e.to_string())
    }

    fn render(&self, h: u64) -> Result<(), String> {
        self.texture(h).map(|_| ())
    }

    fn close(&self, h: u64) -> Result<(), String> {
        self.widget(h)?.downcast::<gtk::Window>().map_err(|_| format!("handle {h} is not a window"))?.close();
        Ok(())
    }

    fn native_toplevels(&self) -> usize {
        gtk::Window::list_toplevels().len()
    }

    fn schedule(&self, ms: u32, f: Box<dyn FnOnce()>) {
        glib::timeout_add_local_once(Duration::from_millis(ms as u64), f);
    }

    fn take_toolkit_timing(&self) -> (u64, u64) {
        svelters_gtk::timing::take()
    }
}
