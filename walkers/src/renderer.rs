//! Walkers' own renderer, which draws a tile from buffers the GPU already holds.
//!
//! Nothing in here knows about egui. It is handed a device, a queue and a render pass, and it
//! draws; whoever is hosting it decides where those come from. Inside an egui app that is
//! [`crate::egui_backend`], through a paint callback.
//!
//! The point of it is that a tile's geometry does not change as the map moves. Handing egui
//! shapes means copying and transforming every vertex on every frame; here the vertices are
//! uploaded once and the map's movement arrives as a uniform.

use ecolor::Color32;
use egui_wgpu::wgpu;
use emath::{Pos2, Rect, TSTransform};

use crate::drawable::Mesh;

/// What the shader needs to put a tile's vertices on the screen.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniform {
    /// Turns a tile coordinate into a point on the screen.
    scale: [f32; 2],
    offset: [f32; 2],

    /// The part of the screen being drawn to, in points, because clip space is relative to it.
    viewport_origin: [f32; 2],
    viewport_size: [f32; 2],
}

/// A vertex as the shader reads it. Smaller than what egui uses, which carries texture
/// coordinates a filled polygon has no use for.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 2],
    color: [u8; 4],
}

impl Vertex {
    fn of(position: Pos2, color: Color32) -> Self {
        Self {
            position: [position.x, position.y],
            color: color.to_array(),
        }
    }
}

/// Frames a mesh may go undrawn before its buffers are let go. Tiles come and go as the map
/// moves, and their buffers should not outlive them by much.
const FORGET_AFTER: u64 = 120;

/// One mesh's buffers, kept between frames.
struct Uploaded {
    /// Holds the tile's geometry, so that nothing else can be allocated at the address being
    /// used as its key while it is still in here.
    _keepalive: std::sync::Arc<Vec<crate::drawable::Drawable>>,

    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    indices_count: u32,
    last_drawn: u64,
}

/// Identifies a mesh by where it lives, which is unique for as long as [`Uploaded`] holds onto
/// the tile it belongs to.
pub(crate) fn key_of(mesh: &Mesh) -> usize {
    std::ptr::from_ref(mesh) as usize
}

pub(crate) struct Renderer {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    uploaded: std::collections::HashMap<usize, Uploaded>,
}

impl Renderer {
    pub(crate) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("walkers"),
            source: wgpu::ShaderSource::Wgsl(include_str!("renderer.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("walkers"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("walkers"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("walkers"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Unorm8x4,
                            offset: 8,
                            shader_location: 1,
                        },
                    ],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some(if format.is_srgb() {
                    // The same choice egui makes, for the same reason.
                    "fs_main_linear_framebuffer"
                } else {
                    "fs_main_gamma_framebuffer"
                }),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    // egui works in premultiplied alpha, and so must anything drawn beside it.
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::OneMinusDstAlpha,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });

        Self {
            pipeline,
            bind_group_layout,
            uploaded: Default::default(),
        }
    }

    /// Put a mesh on the GPU, unless it is already there, and say it is still wanted.
    pub(crate) fn upload(
        &mut self,
        device: &wgpu::Device,
        mesh: &Mesh,
        keepalive: &std::sync::Arc<Vec<crate::drawable::Drawable>>,
        frame: u64,
    ) {
        use wgpu::util::DeviceExt as _;

        let uploaded = self.uploaded.entry(key_of(mesh)).or_insert_with(|| {
            let vertices: Vec<Vertex> = mesh
                .vertices
                .iter()
                .map(|vertex| Vertex::of(vertex.position, vertex.color))
                .collect();

            Uploaded {
                _keepalive: keepalive.to_owned(),
                vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("walkers"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                }),
                indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("walkers"),
                    contents: bytemuck::cast_slice(&mesh.indices),
                    usage: wgpu::BufferUsages::INDEX,
                }),
                indices_count: mesh.indices.len() as u32,
                last_drawn: frame,
            }
        });

        uploaded.last_drawn = frame;
    }

    /// Let go of what has not been drawn for a while.
    pub(crate) fn forget_stale(&mut self, frame: u64) {
        self.uploaded
            .retain(|_, mesh| frame.saturating_sub(mesh.last_drawn) < FORGET_AFTER);
    }

    /// Where a tile's vertices should end up, ready to be handed to the shader.
    pub(crate) fn placement(
        &self,
        device: &wgpu::Device,
        transform: TSTransform,
        viewport: Rect,
    ) -> wgpu::BindGroup {
        use wgpu::util::DeviceExt as _;

        let uniform = Uniform {
            scale: [transform.scaling, transform.scaling],
            offset: [transform.translation.x, transform.translation.y],
            viewport_origin: [viewport.min.x, viewport.min.y],
            viewport_size: [viewport.width(), viewport.height()],
        };

        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("walkers"),
            contents: bytemuck::bytes_of(&uniform),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("walkers"),
            layout: &self.bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        })
    }

    /// Draw one uploaded mesh, wherever `placement` says.
    pub(crate) fn draw(
        &self,
        render_pass: &mut wgpu::RenderPass<'static>,
        key: usize,
        placement: &wgpu::BindGroup,
    ) {
        let Some(mesh) = self.uploaded.get(&key) else {
            return;
        };

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, placement, &[]);
        render_pass.set_vertex_buffer(0, mesh.vertices.slice(..));
        render_pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..mesh.indices_count, 0, 0..1);
    }
}
