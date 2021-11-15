use crate::common::*;

pub type CssProvider = Gtk<gtk4::CssProvider>;

interface!(
    gtk4::CssProvider {
        fn constructor() {
            Gtk(gtk4::CssProvider::new())
        }

        fn setCss(this: Gtk<gtk4::CssProvider>, val: String) {
            this.load_from_data(val.as_bytes());
        }

        fn mount(this: Gtk<gtk4::CssProvider>, priority: u32) {
            let display = gdk4::Display::default().unwrap();
            gtk4::StyleContext::add_provider_for_display(&display, &*this, priority);
        }

        fn unmount(this: Gtk<gtk4::CssProvider>) {
            let display = gdk4::Display::default().unwrap();
            gtk4::StyleContext::remove_provider_for_display(&display, &*this);
        }
    }
);

pub type Button = Gtk<gtk4::Button>;

interface!(gtk4::Button {
    fn constructor() {
        Gtk(gtk4::Button::new())
    }

});
