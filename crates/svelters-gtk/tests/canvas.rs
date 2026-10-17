//! X8 canvas on GTK: the recorded display list is replayed by the drawing area's draw func.
//! `render_image` tests check the replay itself; the widget test snapshots a real, mapped drawing area.

use gtk::prelude::*;
use svelters_gtk::canvas::render_image;
use svelters_gtk::testing::{on_gtk, pump, Harness, V};

/// ARGB32 pixel (premultiplied, native endian) of an image surface.
fn px(s: &mut gtk::cairo::ImageSurface, x: i32, y: i32) -> u32 {
    let stride = s.stride() as usize;
    let d = s.data().unwrap();
    let o = y as usize * stride + x as usize * 4;
    u32::from_ne_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
}

const RED: u32 = 0xffff0000;
const BLUE: u32 = 0xff0000ff;

#[test]
fn fills_strokes_paths_and_transforms() {
    on_gtk(|| {
        let ops = r##"[
          ["fillStyle","#ff0000"],["fillRect",0,0,10,10],
          ["save"],["translate",20,0],["fillStyle","#0000ff"],["fillRect",0,0,10,10],["restore"],
          ["beginPath"],["moveTo",0,20],["lineTo",30,20],["lineTo",30,30],["lineTo",0,30],["closePath"],
          ["fillStyle","rgba(0,255,0,1)"],["fill"],
          ["strokeStyle","#000000"],["lineWidth",2],["beginPath"],["moveTo",0,40],["lineTo",30,40],["stroke"],
          ["globalAlpha",0.5],["fillStyle","#ffffff"],["fillRect",0,0,5,5],
          ["globalAlpha",1],["clearRect",8,8,2,2]
        ]"##;
        let mut s = render_image(ops, 40, 50).unwrap();
        assert_eq!(px(&mut s, 7, 2), RED, "fillRect with fillStyle");
        assert_eq!(px(&mut s, 25, 5), BLUE, "translate inside save/restore");
        assert_eq!(px(&mut s, 12, 5), 0, "nothing where nothing was drawn");
        assert_eq!(px(&mut s, 15, 25), 0xff00ff00, "path fill with rgba()");
        assert_ne!(px(&mut s, 15, 40), 0, "stroked line");
        assert_eq!(px(&mut s, 15, 36), 0, "outside the 2px stroke");
        // half-transparent white over red: premultiplied (a=255) white*0.5 + red*0.5
        let p = px(&mut s, 2, 2);
        assert_eq!(p >> 24, 0xff);
        assert!((p >> 8 & 0xff) > 100 && (p >> 8 & 0xff) < 160, "globalAlpha blends: {p:08x}");
        assert_eq!(px(&mut s, 8, 8), 0, "clearRect makes pixels transparent");
        // restore without a save is ignored; unknown ops and bad arguments are skipped
        let mut s = render_image(r##"[["restore"],["bogus",1],["fillRect","x"],["fillStyle","#00ff00"],["fillRect",0,0,2,2]]"##, 4, 4).unwrap();
        assert_eq!(px(&mut s, 1, 1), 0xff00ff00);
    });
}

#[test]
fn arcs_clips_and_text_draw_pixels() {
    on_gtk(|| {
        let mut s = render_image(r##"[["fillStyle","#ff0000"],["beginPath"],["arc",20,20,10,0,6.2831853],["fill"]]"##, 40, 40).unwrap();
        assert_eq!(px(&mut s, 20, 20), RED, "arc centre");
        assert_eq!(px(&mut s, 2, 2), 0, "outside the circle");
        let mut s = render_image(r##"[["beginPath"],["rect",0,0,10,10],["clip"],["fillStyle","#0000ff"],["fillRect",0,0,40,40]]"##, 40, 40).unwrap();
        assert_eq!(px(&mut s, 5, 5), BLUE, "inside the clip");
        assert_eq!(px(&mut s, 20, 20), 0, "clipped away");
        let mut s = render_image(r##"[["fillStyle","#000000"],["font","bold 20px sans-serif"],["fillText","Hi",2,25]]"##, 60, 40).unwrap();
        let mut inked = 0;
        for y in 0..40 {
            for x in 0..60 {
                if px(&mut s, x, y) != 0 {
                    inked += 1;
                }
            }
        }
        assert!(inked > 30, "fillText inks pixels, got {inked}");
    });
}

/// Snapshot of a mapped widget as BGRA bytes.
fn snapshot(w: &gtk::Widget) -> (Vec<u8>, usize, usize) {
    let (width, height) = (w.width() as usize, w.height() as usize);
    let paintable = gtk::WidgetPaintable::new(Some(w));
    let snap = gtk::Snapshot::new();
    paintable.snapshot(&snap, width as f64, height as f64);
    let node = snap.to_node().expect("render node");
    let renderer = w.native().unwrap().renderer().unwrap();
    let tex = renderer.render_texture(&node, None);
    let mut data = vec![0u8; width * height * 4];
    tex.download(&mut data, width * 4);
    (data, width, height)
}

#[test]
fn the_draw_func_replays_ops_set_through_the_backend_and_follows_updates() {
    on_gtk(|| {
        let mut h = Harness::new();
        let win = h.create("window");
        let da = h.create("drawingarea");
        h.set_str(da, "ops", r##"[["fillStyle","#ff0000"],["fillRect",0,0,100,100]]"##).unwrap();
        h.set_value(da, "contentWidth", &svelters_abi::Value::I64(40)).unwrap();
        h.set_value(da, "contentHeight", &svelters_abi::Value::I64(30)).unwrap();
        h.insert(win, da, None, None).unwrap();
        h.call(win, "present").unwrap();
        pump(150);
        let w = h.widget(da);
        let (data, width, _) = snapshot(&w);
        let at = |x: usize, y: usize| (data[(y * width + x) * 4 + 2], data[(y * width + x) * 4 + 1], data[(y * width + x) * 4]);
        assert_eq!(at(3, 3), (255, 0, 0), "the draw func painted the display list");
        // A new list repaints (queue_draw) with the new colour.
        h.set_str(da, "ops", r##"[["fillStyle","#0000ff"],["fillRect",0,0,100,100]]"##).unwrap();
        pump(150);
        let (data, width, _) = snapshot(&w);
        assert_eq!((data[(3 * width + 3) * 4 + 2], data[(3 * width + 3) * 4]), (0, 255), "queue_draw after ops changed");
        assert_eq!(h.get(da, "ops"), V::Str(r##"[["fillStyle","#0000ff"],["fillRect",0,0,100,100]]"##.into()));
        assert!(h.set_str(da, "ops", "{not an array}").is_err(), "invalid lists are errors, not crashes");
        assert!(h.set_str(da, "ops", "{\"a\":1}").is_err());
    });
}
