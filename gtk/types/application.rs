use gdk4::prelude::ApplicationExt;

use crate::common::*;

pub type Application = Gtk<gtk4::Application>;

interface!(gtk4::Application {
    fn quit(this: Application) {
        this.quit()
    }
});
