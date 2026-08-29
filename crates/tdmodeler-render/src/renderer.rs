//! `wgpu` renderer for triangulated solids (behind the `gui` feature).
//!
//! Uploads a [`TriangleMesh`] and draws it with simple Lambert shading. The
//! caller is responsible for the `wgpu` instance/device/surface (see
//! `tdmodeler-app` for the `winit` + `egui` integration).

use glam::Mat4;
use std::num::NonZeroU32;
use wgpu::util::DeviceExt;

use tdmodeler_mesh::TriangleMesh;
use crate::camera::OrbitCamera;

const SHADER: &str = r#"
struct Uniforms { mvp: mat4x4<f32>, normal_mat: mat4x4<f32>, light: vec3<f32> };
@group(0) @binding(0) var<uniform> u: Uniforms;

struct VSOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) normal: vec3<f32>,
};

@vertex
fn vs(@location(0) position: vec3<f32>, @location(1) normal: vec3<f32>) -> VSOut {
    var out: VSOut;
    out.pos = u.mvp * vec4<f32>(position, 1.0);
    out.normal = (u.normal_mat * vec4<f32>(normal, 0.0)).xyz;
    return out;
}

@fragment
fn fs(in: VSOut) -> @location(0) vec4<f32> {
    let n = normalize(in.normal);
    let l = normalize(u.light);
    let diff = max(dot(n, l), 0.0) * 0.8 + 0.2;
    return vec4<f32>(vec3<f32>(0.3, 0.6, 1.0) * diff, 1.0);
}
"#;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    mvp: [[f32; 4]; 4],
    normal_mat: [[f32; 4]; 4],
    light: [f32; 3],
    _pad: f32,
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    uniform_buf: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    depth_view: wgpu::TextureView,
    depth_size: (u32, u32),
    vertex_buf: Option<wgpu::Buffer>,
    index_buf: Option<wgpu::Buffer>,
    index_count: u32,
}

impl Renderer {
    pub fn new(
        device: wgpu::Device,
        queue: wgpu::Queue,
        format: wgpu::TextureFormat,
        depth_size: (u32, u32),
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tdmodeler-shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });

        let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("uniform-bgl"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uniforms-bg"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buf.as_entire_binding(),
            }],
        });

        let pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("pipeline-layout"),
                bind_group_layouts: Some(&bind_group_layout),
                immediate_size: None,
            });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tdmodeler-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 6 * std::mem::size_of::<f32>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        wgpu::VertexAttribute {
                            offset: 3 * std::mem::size_of::<f32>() as u64,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                    ],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                }),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Back),
                front_face: wgpu::FrontFace::Ccw,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth24Plus,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            cache: None,
            multiview_mask: None,
        });

        let depth_view = Self::make_depth(&device, depth_size);

        Self {
            device,
            queue,
            pipeline,
            uniform_buf,
            bind_group,
            depth_view,
            depth_size,
            vertex_buf: None,
            index_buf: None,
            index_count: 0,
        }
    }

    fn make_depth(device: &wgpu::Device, size: (u32, u32)) -> wgpu::TextureView {
        let w = NonZeroU32::new(size.0.max(1)).unwrap();
        let h = NonZeroU32::new(size.1.max(1)).unwrap();
        device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("depth"),
                size: wgpu::Extent3d {
                    width: w.get(),
                    height: h.get(),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth24Plus,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default())
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.depth_size = (width, height);
        self.depth_view = Self::make_depth(&self.device, (width, height));
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// Upload (or replace) the mesh to draw.
    pub fn set_mesh(&mut self, mesh: &TriangleMesh) {
        let mut verts: Vec<f32> = Vec::with_capacity(mesh.num_vert() * 6);
        for (i, p) in mesh.positions.iter().enumerate() {
            verts.extend_from_slice(p);
            verts.extend_from_slice(&mesh.normals[i]);
        }
        let vertex_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vertices"),
            contents: bytemuck::cast_slice(&verts),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let idx: Vec<u32> = mesh.indices.clone();
        let index_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("indices"),
            contents: bytemuck::cast_slice(&idx),
            usage: wgpu::BufferUsages::INDEX,
        });

        self.vertex_buf = Some(vertex_buf);
        self.index_buf = Some(index_buf);
        self.index_count = idx.len() as u32;
    }

    pub fn render(&self, view: &wgpu::TextureView, camera: &OrbitCamera, aspect: f32) {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("encoder"),
            });
        self.record(&mut encoder, view, camera, aspect);
        self.queue.submit(Some(encoder.finish()));
    }

    /// Record the 3D draw into an existing encoder (does not submit). Used by the
    /// GUI to composite an egui overlay in a subsequent pass.
    pub fn record(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        camera: &OrbitCamera,
        aspect: f32,
    ) {
        let vp = camera.view_proj(aspect);
        let mvp: [[f32; 4]; 4] = vp.to_cols_array_2d();
        // normal matrix: identity rotation here (no non-uniform scale)
        let normal_mat = Mat4::IDENTITY.to_cols_array_2d();
        let uni = Uniforms {
            mvp,
            normal_mat,
            light: [0.4, 0.8, 0.6],
            _pad: 0.0,
        };
        self.queue
            .write_buffer(&self.uniform_buf, 0, bytemuck::bytes_of(&uni));

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.12,
                            g: 0.12,
                            b: 0.14,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            if let (Some(vb), Some(ib)) = (self.vertex_buf.as_ref(), self.index_buf.as_ref()) {
                pass.set_vertex_buffer(0, vb.slice(..));
                pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..self.index_count, 0, 0..1);
            }
        }
    }
}
