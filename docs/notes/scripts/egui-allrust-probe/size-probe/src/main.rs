use rquickjs::{Context, Runtime};

struct App {
    _rt: Runtime,
    ctx: Context,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut eframe::egui::Ui, _frame: &mut eframe::Frame) {
        let n: i32 = self.ctx.with(|c| c.eval("globalThis.n = (globalThis.n|0) + 1").unwrap());
        ui.label(format!("frames evaluated in QuickJS: {n}"));
    }
}

fn main() -> eframe::Result {
    let rt = Runtime::new().unwrap();
    let ctx = Context::full(&rt).unwrap();
    eframe::run_native("size", Default::default(), Box::new(|_| Ok(Box::new(App { _rt: rt, ctx }))))
}
