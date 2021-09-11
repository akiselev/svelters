use std::time::Duration;

use svelters::Svelters;
use svelters_gtk4::Gtk;

fn main() {
    svelters::logging::init();
    tracing::info!("Starting...");
    // Run the local task set.
    let sv = Svelters::new(Gtk::module()).unwrap();
    if let Err(err) = sv.eval("./app/index.ts") {
        tracing::error!("Eval error: {}", err);
        std::thread::sleep(Duration::from_secs(2));
    }
}
