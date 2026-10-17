//! X8: `parent:header` puts a visible section header label on a ListBox row (the header_func replacement).

use gtk::prelude::*;

use svelters_gtk::testing::{on_gtk, pump, Harness, V};

fn dark_pixels(w: &gtk::Widget) -> usize {
    let (width, height) = (w.width() as usize, w.height() as usize);
    let paintable = gtk::WidgetPaintable::new(Some(w));
    let snap = gtk::Snapshot::new();
    paintable.snapshot(&snap, width as f64, height as f64);
    let node = snap.to_node().expect("render node");
    let tex = w.native().unwrap().renderer().unwrap().render_texture(&node, None);
    let mut data = vec![0u8; width * height * 4];
    tex.download(&mut data, width * 4);
    data.chunks(4).filter(|p| p[0] < 100 && p[1] < 100 && p[2] < 100).count()
}

#[test]
fn a_header_attribute_shows_a_label_above_the_row_and_null_removes_it() {
    on_gtk(|| {
        let mut h = Harness::new();
        let win = h.create("window");
        let lb = h.create("list-box");
        let a = h.create("label");
        let b = h.create("label");
        h.set_str(a, "label", "apple").unwrap();
        h.set_str(b, "label", "beet").unwrap();
        h.insert(win, lb, None, None).unwrap();
        h.insert(lb, a, None, Some("children")).unwrap();
        h.insert(lb, b, None, Some("children")).unwrap();
        h.call(win, "present").unwrap();
        pump(150);
        let list = h.widget(lb);
        let plain = dark_pixels(&list);
        h.set_child_attr(lb, a, "header", &V::Str("Fruit".into())).unwrap();
        pump(150);
        let row = h.widget(a).ancestor(gtk::ListBoxRow::static_type()).and_downcast::<gtk::ListBoxRow>().unwrap();
        let header = row.header().and_then(|w| w.downcast::<gtk::Label>().ok()).expect("a header label");
        assert_eq!(header.label(), "Fruit");
        assert!(header.is_mapped() && header.width() > 10, "the header is shown: mapped={} width={}", header.is_mapped(), header.width());
        assert!(dark_pixels(&list) > plain + 20, "the header text is painted: {} vs {}", dark_pixels(&list), plain);
        h.set_child_attr(lb, a, "header", &V::Null).unwrap();
        pump(100);
        assert!(row.header().is_none(), "null removes the header");
    });
}
