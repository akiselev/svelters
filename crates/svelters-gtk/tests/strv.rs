//! GStrv properties (GtkWidget:css-classes) travel as space-separated strings. Needs a display
//! (xvfb-run).

use gtk::prelude::*;
use svelters_gtk::testing::{on_gtk, Harness, V};

#[test]
fn css_classes_round_trip_as_tokens() {
    on_gtk(|| {
        let h = Harness::new();
        let b = h.create("button");
        h.set_str(b, "cssClasses", "a b").unwrap();
        assert_eq!(h.get(b, "cssClasses"), V::Str("a b".into()));
        let w = h.widget(b);
        assert!(w.has_css_class("a") && w.has_css_class("b"));

        // Runs of ASCII whitespace separate tokens; the native name resolves too.
        h.set_str(b, "css-classes", " suggested-action\t x ").unwrap();
        assert_eq!(h.get(b, "cssClasses"), V::Str("suggested-action x".into()));
        assert_eq!(w.css_classes().len(), 2);

        h.set_str(b, "cssClasses", "").unwrap();
        assert_eq!(h.get(b, "cssClasses"), V::Str(String::new()));
        assert!(w.css_classes().is_empty());
    });
}
