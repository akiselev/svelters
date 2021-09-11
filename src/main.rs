use svelters::Svelters;
use svelters_gtk4::Gtk;

fn main() {
    svelters::logging::init();
    tracing::info!("Starting...");
    // Run the local task set.
    let mut sv = Svelters::new(Gtk::module()).unwrap();
    sv.eval("./app/index.ts").unwrap();
}
