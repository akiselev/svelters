//! An island rendered on a real device: its texture is registered (again after a resize), holds egui's
//! pixels, and egui-wgpu paint callbacks run on the island's renderer. Runs where `PRESTO_GPU_TESTS` is
//! set and an adapter exists (CI: lavapipe, WARP, Metal).

use presto_base::geom::{Rect, Scale};
use presto_base::ids::TextureId;
use presto_base::time::Time;
use presto_egui_island::{EguiIsland, IslandDesc, TextureRegistry, island_tree_id};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
    std::env::var_os("PRESTO_GPU_TESTS")?;
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).ok()?;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()
}

#[derive(Default)]
struct Registry {
    registered: Mutex<Vec<(TextureId, wgpu::Extent3d)>>,
    unregistered: Mutex<Vec<TextureId>>,
}

impl TextureRegistry for Registry {
    fn register_texture(&self, id: TextureId, texture: wgpu::Texture) {
        self.registered.lock().unwrap().push((id, texture.size()));
    }
    fn unregister_texture(&self, id: TextureId) {
        self.unregistered.lock().unwrap().push(id);
    }
}

/// Counts its three egui-wgpu stages.
struct Probe(Arc<[AtomicUsize; 3]>);

impl egui_wgpu::CallbackTrait for Probe {
    fn prepare(
        &self,
        _: &wgpu::Device,
        _: &wgpu::Queue,
        _: &egui_wgpu::ScreenDescriptor,
        _: &mut wgpu::CommandEncoder,
        _: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        self.0[0].fetch_add(1, Ordering::Relaxed);
        Vec::new()
    }
    fn finish_prepare(
        &self,
        _: &wgpu::Device,
        _: &wgpu::Queue,
        _: &mut wgpu::CommandEncoder,
        _: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        self.0[1].fetch_add(1, Ordering::Relaxed);
        Vec::new()
    }
    fn paint(&self, _: egui::PaintCallbackInfo, _: &mut wgpu::RenderPass<'static>, _: &egui_wgpu::CallbackResources) {
        self.0[2].fetch_add(1, Ordering::Relaxed);
    }
}

/// The texture's RGBA8 pixels (width 64: rows are already 256-byte aligned).
fn read_back(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture) -> Vec<u8> {
    let size = texture.size();
    assert_eq!(size.width * 4 % wgpu::COPY_BYTES_PER_ROW_ALIGNMENT, 0);
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(size.width * size.height * 4),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(size.width * 4), rows_per_image: None },
        },
        size,
    );
    queue.submit([encoder.finish()]);
    buffer.slice(..).map_async(wgpu::MapMode::Read, |r| r.expect("map"));
    device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
    let pixels = buffer.slice(..).get_mapped_range().expect("mapped").to_vec();
    pixels
}

#[test]
fn renders_into_a_registered_texture_with_paint_callbacks() {
    let Some((device, queue)) = device() else {
        eprintln!("no GPU (set PRESTO_GPU_TESTS on a machine with an adapter)");
        return;
    };
    let registry = Registry::default();
    let mut island = EguiIsland::with_device(
        &device,
        IslandDesc {
            texture: TextureId(3),
            tree: island_tree_id(3),
            rect: Rect::new(10.0, 10.0, 74.0, 58.0),
            scale: Scale(1.0),
            renderer: egui_wgpu::RendererOptions::PREDICTABLE,
        },
    );
    let counts = Arc::new([AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0)]);
    let mut ui = |ui: &mut egui::Ui| {
        let painter = ui.painter();
        painter.rect_filled(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(32.0, 48.0)), 0.0, egui::Color32::RED);
        painter.add(egui_wgpu::Callback::new_paint_callback(
            egui::Rect::from_min_size(egui::pos2(32.0, 0.0), egui::vec2(32.0, 48.0)),
            Probe(counts.clone()),
        ));
    };

    let mut frame = |island: &mut EguiIsland, now: u64| {
        island.run(Time(now), &mut ui);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        let user = island.render(&device, &queue, &mut encoder, &registry);
        queue.submit(user.into_iter().chain([encoder.finish()]));
    };
    frame(&mut island, 1);

    let registered = registry.registered.lock().unwrap().clone();
    assert_eq!(registered.len(), 1);
    assert_eq!(registered[0].0, TextureId(3));
    assert_eq!((registered[0].1.width, registered[0].1.height), (64, 48));
    assert_eq!(counts.each_ref().map(|c| c.load(Ordering::Relaxed)), [1, 1, 1], "prepare, finish_prepare, paint");

    let texture = island.texture().expect("rendered").clone();
    assert_eq!(texture.format(), wgpu::TextureFormat::Rgba8Unorm);
    let px = read_back(&device, &queue, &texture);
    let at = |x: u32, y: u32| &px[((y * 64 + x) * 4) as usize..][..4];
    assert_eq!(at(8, 24), &[255, 0, 0, 255], "egui's red rect");
    assert_eq!(at(48, 24)[3], 0, "the callback drew nothing: cleared to transparent");

    // Rendering without a new pass does nothing.
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    assert!(island.render(&device, &queue, &mut encoder, &registry).is_empty());
    assert_eq!(counts[2].load(Ordering::Relaxed), 1);

    // A resize reallocates and re-registers the texture.
    island.set_rect(Rect::new(10.0, 10.0, 74.0, 74.0), Scale(2.0));
    frame(&mut island, 2);
    let registered = registry.registered.lock().unwrap().clone();
    assert_eq!(registered.len(), 2);
    assert_eq!((registered[1].1.width, registered[1].1.height), (128, 128));

    island.release(&registry);
    assert_eq!(*registry.unregistered.lock().unwrap(), vec![TextureId(3)]);
}
