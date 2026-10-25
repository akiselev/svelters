//! The renderer's pipelines and bind-group layouts, created once per (device, target format).
//!
//! Bind groups: `@group(0)` the frame globals (uniform), `@group(1)` the frame's instance records
//! (one read-only storage buffer, shared by every pipeline at `@binding(0)`), `@group(2)` a texture and
//! sampler (atlas page, registered surface or path intermediate).

use std::borrow::Cow;

use crate::gpu::PathVertex;

/// MSAA sample count of the path intermediates. WebGPU guarantees 4× for every renderable format.
pub const PATH_SAMPLES: u32 = 4;

pub struct Pipelines {
    pub globals_layout: wgpu::BindGroupLayout,
    pub instances_layout: wgpu::BindGroupLayout,
    pub texture_layout: wgpu::BindGroupLayout,
    pub quad: wgpu::RenderPipeline,
    pub shadow: wgpu::RenderPipeline,
    pub line: wgpu::RenderPipeline,
    pub mask: wgpu::RenderPipeline,
    pub image: wgpu::RenderPipeline,
    pub path_raster: wgpu::RenderPipeline,
    pub path_sprite: wgpu::RenderPipeline,
    pub clear: wgpu::RenderPipeline,
}

impl Pipelines {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("presto-render shaders"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shaders.wgsl"))),
        });
        let vf = wgpu::ShaderStages::VERTEX_FRAGMENT;
        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("presto globals"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: vf,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let instances_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("presto instances"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: vf,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("presto texture"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: vf,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: vf,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let plain = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("presto plain"),
            bind_group_layouts: &[Some(&globals_layout), Some(&instances_layout)],
            immediate_size: 0,
        });
        let textured = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("presto textured"),
            bind_group_layouts: &[Some(&globals_layout), Some(&instances_layout), Some(&texture_layout)],
            immediate_size: 0,
        });

        let blend = Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING);
        let strip = wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, ..Default::default() };
        let make = |name: &str,
                    layout: &wgpu::PipelineLayout,
                    vs: &str,
                    fs: &str,
                    primitive: wgpu::PrimitiveState,
                    buffers: &[Option<wgpu::VertexBufferLayout<'_>>],
                    blend: Option<wgpu::BlendState>,
                    samples: u32| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(name),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some(vs),
                    compilation_options: Default::default(),
                    buffers,
                },
                primitive,
                depth_stencil: None,
                multisample: wgpu::MultisampleState { count: samples, mask: !0, alpha_to_coverage_enabled: false },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format, blend, write_mask: wgpu::ColorWrites::ALL })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };

        let path_vertex = wgpu::VertexBufferLayout {
            array_stride: size_of::<PathVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x2, offset: 0, shader_location: 0 }],
        };
        let list = wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, ..Default::default() };

        Pipelines {
            quad: make("presto quad", &plain, "vs_quad", "fs_quad", strip, &[], blend, 1),
            shadow: make("presto shadow", &plain, "vs_shadow", "fs_shadow", strip, &[], blend, 1),
            line: make("presto line", &plain, "vs_line", "fs_line", strip, &[], blend, 1),
            mask: make("presto mask", &textured, "vs_mask", "fs_mask", strip, &[], blend, 1),
            image: make("presto image", &textured, "vs_image", "fs_image", strip, &[], blend, 1),
            path_raster: make(
                "presto path raster",
                &plain,
                "vs_path_raster",
                "fs_path_raster",
                list,
                &[Some(path_vertex)],
                blend,
                PATH_SAMPLES,
            ),
            path_sprite: make("presto path sprite", &textured, "vs_path_sprite", "fs_path_sprite", strip, &[], blend, 1),
            clear: make("presto clear", &plain, "vs_clear", "fs_clear", Default::default(), &[], None, 1),
            globals_layout,
            instances_layout,
            texture_layout,
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn shaders_parse_and_validate() {
        let src = include_str!("shaders.wgsl");
        let module = naga::front::wgsl::parse_str(src).unwrap_or_else(|e| panic!("{}", e.emit_to_string(src)));
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::empty())
            .validate(&module)
            .unwrap_or_else(|e| panic!("{}", e.emit_to_string(src)));
    }
}
