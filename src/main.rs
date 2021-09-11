use std::time::Duration;

use rquickjs::{ClassDef, Function, Object, This, Value};
use svelters::Svelters;
use svelters_core::prelude::*;
use svelters_gtk4::{Gtk, GtkExecutor};

#[tracing::instrument]
fn main() {
    svelters::logging::init();
    tracing::info!("Starting GTK...");
    Gtk::start("org.svelters.gtk4.example", |app| {
        let sv = Svelters::new(Gtk::module()).unwrap();
        sv.spawn_executor(GtkExecutor);
        sv.with_js(|ctx| {
            ctx.eval_global(r#"import * as Gtk from "Gtk";"#)?;
            let doc_obj: Object = ctx.globals().get("document")?;
            let doc: &Document = Document::from_js_ref(ctx, doc_obj.clone().into_value())?;
            let app = doc.new_instance(ctx, Gtk(app.clone()))?;
            ctx.globals().set("application", app)?;
            Ok(())
        })
        .expect("Failed to create Application in JS");
        sv.execute_pending_jobs().unwrap();
        sv.with_js(|ctx| {
            let doc_obj: Object = ctx.globals().get("document")?;
            let func: Function = doc_obj.get("createElement")?;
            let window: Value = func.call((This(doc_obj), "applicationWindow"))?;
            ctx.globals().set("window", window)?;
            Ok(())
        })
        .expect("Failed to create ApplicationWindow in JS");
        if let Err(err) = sv.eval("./app/index.ts") {
            tracing::error!("Eval error: {}", err);
            std::thread::sleep(Duration::from_secs(2));
        }
    })
}
