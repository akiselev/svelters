//! 10⁶ primitives: the CPU side of a frame (culling and LOD over every chunk) timed over a camera
//! sweep, and the counts it draws at each zoom. The display-rate gate itself (a PCB pan at the
//! display's rate on real GPUs) is a local measurement (plan W-V2 **[local]**), not this test.

use skelter_base::color::Color;
use skelter_base::geom::Rect;
use skelter_base::ids::CanvasId;
use skelter_canvas::chunk::Cap;
use skelter_canvas::{plan, Camera, Canvas, CanvasFonts, ChunkBuilder, DVec2, LodParams, Plane, Projection, WorldRect};
use std::time::Instant;

/// Chunks per side and primitives per chunk: 32 × 32 × 977 ≈ 10⁶.
const SIDE: usize = 32;
const PER_CHUNK: usize = 977;
/// One chunk is a 1 mm square of a board (nanometres), the board starts 0.25 m from the origin.
const CHUNK_NM: f64 = 1.0e6;
const BOARD_ORIGIN: f64 = 2.5e8;

/// A deterministic board: tracks, vias, pads and arcs in each 1 mm cell, sizes from 1 µm to 1 mm.
fn board() -> std::sync::Arc<Canvas> {
    let canvas = Canvas::new(CanvasId(1), CanvasFonts::new());
    let mut seed = 0x9E37_79B9_7F4A_7C15u64;
    let mut rnd = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    canvas.edit(|e| {
        for cy in 0..SIDE {
            for cx in 0..SIDE {
                let o = DVec2::new(BOARD_ORIGIN + cx as f64 * CHUNK_NM, BOARD_ORIGIN + cy as f64 * CHUNK_NM);
                let mut b = ChunkBuilder::new(o);
                for i in 0..PER_CHUNK {
                    let p = o + DVec2::new(rnd() * CHUNK_NM, rnd() * CHUNK_NM);
                    let size = 1.0e3 * (1000.0f64).powf(rnd());
                    let c = Color::rgba(0.8, 0.3, 0.1, 1.0);
                    match i % 4 {
                        0 => b.segment(p, p + DVec2::new(size, size * 0.5), size * 0.1, Cap::Round, c),
                        1 => b.circle(p, size * 0.5, 0.0, c),
                        2 => b.rect(WorldRect::new(p.x, p.y, p.x + size, p.y + size * 0.6), size * 0.1, 0.0, c),
                        _ => b.arc(p, size * 0.5, size * 0.05, 0.0, 2.0, c),
                    };
                }
                e.insert(Plane::Static, b.build());
            }
        }
    });
    canvas
}

#[test]
fn a_million_primitives_cull_and_lod_within_budget() {
    let canvas = board();
    let snap = canvas.snapshot();
    assert_eq!(snap.statics.chunks.len(), SIDE * SIDE);
    assert!(snap.statics.primitive_count() >= 1_000_000);

    let size = (1600.0, 1000.0);
    let device = Rect::new(0.0, 0.0, 3200.0, 2000.0);
    let lod = LodParams::default();
    let center = DVec2::new(BOARD_ORIGIN + 16.0 * CHUNK_NM, BOARD_ORIGIN + 16.0 * CHUNK_NM);
    // Zooms from the whole board (32 mm in 1000 pt) to 20 µm across the canvas.
    let zooms = [1000.0 / (36.0 * CHUNK_NM), 1600.0 / (4.0 * CHUNK_NM), 1600.0 / 2.0e4];
    let mut out = Vec::new();
    let mut counts = Vec::new();
    for &zoom in &zooms {
        let frames = 60;
        let started = Instant::now();
        let mut first = None;
        for f in 0..frames {
            // A pan of two points a frame at this zoom.
            let step = 2.0 / zoom;
            let cam = Camera::new(center + DVec2::new(f as f64 * step, f as f64 * -step * 0.5), zoom);
            let proj = Projection::new(cam, size, [2.0, 0.0, 0.0, 2.0, 0.0, 0.0]);
            out.clear();
            let stats = plan(&snap.statics, &proj, &device, &lod, &mut out);
            first.get_or_insert(stats);
        }
        let stats = first.expect("frames ran");
        let per_frame = started.elapsed() / frames;
        eprintln!("zoom {zoom:e}: {per_frame:?} per frame, {stats:?}");
        counts.push(stats);
        // The planning budget: a small part of a 120 Hz frame, even in a debug build.
        assert!(per_frame.as_secs_f64() < 4.0e-3, "culling took {per_frame:?} per frame at zoom {zoom:e}");
    }
    let [whole, mid, deep] = [counts[0], counts[1], counts[2]];
    assert_eq!(whole.visible as usize, SIDE * SIDE, "the whole board is in view");
    assert!(whole.shapes < 1_000_000, "LOD drops sub-pixel primitives when zoomed out ({})", whole.shapes);
    // A chunk's bounds reach up to 1 mm past its cell (primitives near its edge).
    assert!(mid.visible < 64, "four millimetres show a few chunks ({})", mid.visible);
    assert!(deep.visible <= 9, "deep zoom sees the chunks under the view ({})", deep.visible);
    assert!(deep.shapes > 0);
}

#[test]
fn panning_keeps_chunk_transforms_exact_far_from_the_origin() {
    // 1 device pixel per 10 nm at 0.25 m: the chunk translation stays a small, exact number.
    let canvas = board();
    let snap = canvas.snapshot();
    let o = snap.statics.chunks[0].1.origin;
    let cam = Camera::new(o + DVec2::new(5000.0, 5000.0), 0.05);
    let proj = Projection::new(cam, (1000.0, 1000.0), [2.0, 0.0, 0.0, 2.0, 0.0, 0.0]);
    let x = proj.chunk(o);
    assert_eq!(x.t, [500.0, 500.0]);
    let moved = Projection::new(cam.pan_by((1.0, 0.0)), (1000.0, 1000.0), [2.0, 0.0, 0.0, 2.0, 0.0, 0.0]).chunk(o);
    assert_eq!(moved.t, [502.0, 500.0], "a one-point drag moves the content two device pixels");
}
