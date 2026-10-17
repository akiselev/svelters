//! X8, G11: Gdk object values. A cursor name string becomes a GdkCursor, an image path or encoded image bytes
//! become a GdkTexture where a GdkPaintable is wanted (`picture`/`image` `paintable`).

use gtk::prelude::*;
use svelters_abi::Value;
use svelters_gtk::testing::{on_gtk, Harness};

fn png(w: i32, h: i32) -> Vec<u8> {
    let pixels: Vec<u8> = (0..w * h).flat_map(|_| [255u8, 0, 0, 255]).collect();
    let tex = gtk::gdk::MemoryTexture::new(w, h, gtk::gdk::MemoryFormat::R8g8b8a8, &gtk::glib::Bytes::from_owned(pixels), (w * 4) as usize);
    tex.save_to_png_bytes().to_vec()
}

#[test]
fn a_cursor_name_becomes_a_gdk_cursor() {
    on_gtk(|| {
        let h = Harness::new();
        let l = h.create("label");
        h.set_str(l, "cursor", "pointer").unwrap();
        assert_eq!(h.widget(l).cursor().unwrap().name().as_deref(), Some("pointer"));
        h.set_value(l, "cursor", &Value::Null).unwrap();
        assert!(h.widget(l).cursor().is_none(), "null clears it");
    });
}

#[test]
fn image_paths_and_bytes_become_textures() {
    on_gtk(|| {
        let h = Harness::new();
        let pic = h.create("picture");
        h.set_value(pic, "paintable", &Value::Bytes(png(12, 7))).unwrap();
        let p = h.widget(pic).downcast::<gtk::Picture>().unwrap().paintable().expect("a paintable");
        assert!(p.is::<gtk::gdk::Texture>());
        assert_eq!((p.intrinsic_width(), p.intrinsic_height()), (12, 7), "decoded from bytes");

        let path = std::env::temp_dir().join(format!("svelters-x8-{}.png", std::process::id()));
        std::fs::write(&path, png(5, 9)).unwrap();
        let img = h.create("image");
        h.set_str(img, "paintable", path.to_str().unwrap()).unwrap();
        let p = h.widget(img).downcast::<gtk::Image>().unwrap().paintable().expect("a paintable");
        assert_eq!((p.intrinsic_width(), p.intrinsic_height()), (5, 9), "loaded from a path");
        std::fs::remove_file(&path).ok();
        assert!(h.set_str(img, "paintable", "/no/such/file.png").is_err(), "a missing file is an error");
    });
}
