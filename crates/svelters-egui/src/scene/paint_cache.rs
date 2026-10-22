//! SG6 (ARCH-SCENE §8.3, ARCH-EMBED §8.2 D3): the opt-in paint cache behind
//! `WidgetCx::cached_shapes`. A node's shapes are kept as one `Arc<[Shape]>` in its root-free
//! local coordinates, keyed by the widget's own version, its size, `pixels_per_point` and the dark
//! or light theme; a hit replays them moved to the node's rect, a miss runs the paint closure.
//! Any prop write to the node drops its entry (PAINT dirt, `Scene::set_value`), and so does
//! freeing it.
//!
//! egui still tessellates every shape every frame (`Context::tessellate`), so the cache saves shape
//! construction and text layout, not tessellation; the benchmark (`tests` below) measures that.

use egui::{Painter, Pos2, Rect, Shape};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Key {
    version: u64,
    size: [u32; 2],
    ppp: u32,
    dark: bool,
}

/// The cached shapes, by slot.
#[derive(Default)]
pub(crate) struct PaintCache {
    entries: HashMap<u32, (Key, Arc<[Shape]>)>,
    pub hits: u64,
    pub misses: u64,
}

impl PaintCache {
    /// Paints node `slot`'s shapes at `rect` with `painter`: the cached ones if `version`, the
    /// size, scale and theme are unchanged, else `paint`'s, which draws into `shapes` for a rect of
    /// `rect`'s size at the origin. True on a hit.
    pub fn paint(&mut self, slot: u32, version: u64, painter: &Painter, rect: Rect, paint: impl FnOnce(&Painter, Rect, &mut Vec<Shape>)) -> bool {
        let key = Key {
            version,
            size: [rect.width().to_bits(), rect.height().to_bits()],
            ppp: painter.pixels_per_point().to_bits(),
            dark: painter.ctx().global_style().visuals.dark_mode,
        };
        let (hit, shapes) = match self.entries.get(&slot) {
            Some((k, s)) if *k == key => (true, s.clone()),
            _ => {
                let mut v = Vec::new();
                paint(painter, Rect::from_min_size(Pos2::ZERO, rect.size()), &mut v);
                let s: Arc<[Shape]> = v.into();
                self.entries.insert(slot, (key, s.clone()));
                (false, s)
            }
        };
        if hit {
            self.hits += 1;
        } else {
            self.misses += 1;
        }
        let d = rect.min.to_vec2();
        painter.extend(shapes.iter().map(|s| {
            let mut s = s.clone();
            s.translate(d);
            s
        }));
        hit
    }

    pub fn remove(&mut self, slot: u32) {
        self.entries.remove(&slot);
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Color32, FontId, Stroke, vec2};
    use std::time::Instant;

    /// A widget's paint: a frame, a grid of ticks and labels (text layout is the costly part).
    fn widget(p: &Painter, r: Rect, out: &mut Vec<Shape>) {
        out.push(Shape::rect_stroke(r, 2.0, Stroke::new(1.0, Color32::GRAY), egui::StrokeKind::Inside));
        for k in 0..8 {
            let x = r.min.x + 4.0 + k as f32 * 20.0;
            out.push(Shape::line_segment([egui::pos2(x, r.min.y + 2.0), egui::pos2(x, r.min.y + 6.0)], Stroke::new(1.0, Color32::WHITE)));
            let g = p.layout_no_wrap(format!("{:.2}", k as f32 * 0.25), FontId::monospace(10.0), Color32::WHITE);
            out.push(Shape::galley(egui::pos2(x, r.min.y + 8.0), g, Color32::WHITE));
        }
    }

    /// The shapes `n` widgets put in a frame, painted with or without the cache, at `offset`.
    fn frame(ctx: &egui::Context, cache: Option<&mut PaintCache>, n: usize, offset: f32) -> Vec<egui::epaint::ClippedShape> {
        let mut cache = cache;
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            let p = ui.painter();
            for i in 0..n {
                let r = Rect::from_min_size(egui::pos2(offset, i as f32 * 24.0), vec2(170.0, 22.0));
                match cache.as_deref_mut() {
                    Some(c) => {
                        c.paint(i as u32, 1, p, r, widget);
                    }
                    None => {
                        let mut v = Vec::new();
                        widget(p, Rect::from_min_size(Pos2::ZERO, r.size()), &mut v);
                        p.extend(v.into_iter().map(|mut s| {
                            s.translate(r.min.to_vec2());
                            s
                        }));
                    }
                }
            }
        });
        out.textures_delta.clear(); // no renderer here (epaint asserts deltas are handled)
        out.shapes
    }

    fn summary(shapes: &[egui::epaint::ClippedShape]) -> Vec<String> {
        shapes.iter().map(|s| format!("{:?}", s.shape.visual_bounding_rect())).collect()
    }

    /// The cache oracle (ARCH-SCENE §14 SG6, slint SL1): cached paint equals uncached paint, on a
    /// miss and on a hit at another position; a new version paints again.
    #[test]
    fn cached_paint_equals_uncached_paint() {
        let ctx = egui::Context::default();
        let mut c = PaintCache::default();
        for offset in [0.0, 13.0] {
            let plain = frame(&ctx, None, 5, offset);
            let cached = frame(&ctx, Some(&mut c), 5, offset);
            assert_eq!(summary(&plain), summary(&cached), "offset {offset}");
        }
        assert_eq!((c.misses, c.hits), (5, 5), "painted once, then replayed moved");
        let ctx2 = egui::Context::default();
        ctx2.set_pixels_per_point(2.0);
        let _ = frame(&ctx2, Some(&mut c), 5, 0.0);
        assert_eq!(c.misses, 10, "another scale paints again");
    }

    /// The paint-cache benchmark: 1000 widgets of 25 shapes (8 text layouts) a frame. Printed with
    /// `--nocapture` and recorded in the milestone.
    #[test]
    fn paint_cache_benchmark() {
        let ctx = egui::Context::default();
        let n = 1000;
        let mut c = PaintCache::default();
        let _ = (frame(&ctx, None, n, 0.0), frame(&ctx, Some(&mut c), n, 0.0));
        let time = |f: &mut dyn FnMut()| {
            let t = Instant::now();
            for _ in 0..20 {
                f();
            }
            t.elapsed().as_secs_f64() * 1e3 / 20.0
        };
        let plain = time(&mut || drop(frame(&ctx, None, n, 0.0)));
        let cached = time(&mut || drop(frame(&ctx, Some(&mut c), n, 0.0)));
        let tess = |shapes: Vec<egui::epaint::ClippedShape>| ctx.tessellate(shapes, 1.0).len();
        let t = Instant::now();
        let _ = tess(frame(&ctx, Some(&mut c), n, 0.0));
        let with_tess = t.elapsed().as_secs_f64() * 1e3;
        println!("paint_cache_benchmark: {n} widgets, uncached {plain:.2} ms/frame, cached {cached:.2} ms/frame, cached + tessellation {with_tess:.2} ms");
        assert!(c.hits > 0);
    }
}
