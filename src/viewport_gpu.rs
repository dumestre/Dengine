use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use eframe::egui;
use eframe::egui_wgpu;
use eframe::wgpu;
use eframe::wgpu::util::DeviceExt;
use glam::{Mat4, Vec3, Vec4};

const MAX_GPU_TRIANGLES: usize = 120_000;
const MAX_GPU_VERTICES: usize = 160_000;
const SORT_TRIANGLES_THRESHOLD: usize = 80_000;
const GPU_UPLOAD_BUDGET_BYTES: usize = 8 * 1024 * 1024;
const MAX_WIREFRAME_TRIANGLES_GPU: usize = 45_000;

const SHADER_SRC: &str = r#"
struct Uniforms {
    mvp: mat4x4<f32>,
    tint: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> ubo: Uniforms;
@group(0) @binding(1)
var albedo_tex: texture_2d<f32>;
@group(0) @binding(2)
var albedo_sampler: sampler;

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
};

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) shade: f32,
};

@vertex
fn vs_main(v: VsIn) -> VsOut {
    var out: VsOut;
    let p = vec4<f32>(v.pos, 1.0);
    out.clip_pos = ubo.mvp * p;
    out.uv = v.uv;
    out.shade = 0.75 + 0.25 * clamp(v.pos.y * 0.7 + 0.3, 0.0, 1.0);
    return out;
}

@fragment
fn fs_main(v: VsOut) -> @location(0) vec4<f32> {
    let tex = textureSample(albedo_tex, albedo_sampler, v.uv).rgb;
    let c = tex * ubo.tint.rgb * v.shade;
    return vec4<f32>(c, 1.0);
}
"#;

const WIRE_SHADER_SRC: &str = r#"
struct Uniforms {
    mvp: mat4x4<f32>,
    tint: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> ubo: Uniforms;

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
};

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
};

@vertex
fn vs_main(v: VsIn) -> VsOut {
    var out: VsOut;
    out.clip_pos = ubo.mvp * vec4<f32>(v.pos, 1.0);
    return out;
}

@fragment
fn fs_main(_v: VsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(0.08, 0.92, 0.48, 0.95);
}
"#;

#[allow(dead_code)]
#[derive(Clone, Copy)]
pub struct SceneTextureUpdate<'a> {
    pub id: u64,
    pub size: [u32; 2],
    pub rgba: &'a [u8],
}

#[derive(Default)]
struct SceneState {
    mesh_id: u64,
    vertices: Vec<[f32; 5]>,
    triangles: Vec<[u32; 3]>,
    mvp: [[f32; 4]; 4],
    texture_id: u64,
    texture_size: [u32; 2],
    texture_rgba: Vec<u8>,
    show_wireframe: bool,
}

pub struct ViewportGpuRenderer {
    target_format: wgpu::TextureFormat,
    scene: Arc<Mutex<SceneState>>,
}

struct Draw3dCallback {
    target_format: wgpu::TextureFormat,
    scene: Arc<Mutex<SceneState>>,
}

struct GpuResources {
    solid_pipeline: wgpu::RenderPipeline,
    wire_pipeline: wgpu::RenderPipeline,
    bind_layout: wgpu::BindGroupLayout,
    uniform_buffer: wgpu::Buffer,
    uniform_data: [u8; 80],
    bind_group: wgpu::BindGroup,
    _white_texture: wgpu::Texture,
    white_view: wgpu::TextureView,
    active_texture: Option<wgpu::Texture>,
    active_texture_view: Option<wgpu::TextureView>,
    sampler: wgpu::Sampler,
    vertex_buffer: Option<wgpu::Buffer>,
    index_buffer: Option<wgpu::Buffer>,
    line_index_buffer: Option<wgpu::Buffer>,
    index_count: u32,
    line_index_count: u32,
    uploaded_mesh_id: u64,
    uploaded_texture_id: u64,
    depth_sorted_mesh_id: u64,
    draw_wireframe: bool,
    pending_mesh_upload: Option<PendingMeshUpload>,
    pending_texture_upload: Option<PendingTextureUpload>,
}

struct PendingMeshUpload {
    mesh_id: u64,
    vertices: Vec<[f32; 5]>,
    triangles: Vec<[u32; 3]>,
    line_indices: Vec<u32>,
    vertex_cursor: usize,
    tri_cursor: usize,
    line_cursor: usize,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    line_index_buffer: Option<wgpu::Buffer>,
    index_count: u32,
    line_index_count: u32,
}

struct PendingTextureUpload {
    texture_id: u64,
    size: [u32; 2],
    rgba: Vec<u8>,
    uploaded_rows: u32,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
}

impl ViewportGpuRenderer {
    pub fn new(render_state: egui_wgpu::RenderState) -> Self {
        Self {
            target_format: render_state.target_format,
            scene: Arc::new(Mutex::new(SceneState::default())),
        }
    }

    pub fn update_scene(
        &self,
        mesh_id: u64,
        vertices: &[Vec3],
        triangles: &[[u32; 3]],
        mvp: Mat4,
        _texture: Option<SceneTextureUpdate<'_>>,
        show_wireframe: bool,
    ) {
        let mut s = self.scene.lock().expect("scene lock");

        if s.mesh_id != mesh_id {
            s.mesh_id = mesh_id;
            s.vertices.clear();
            s.triangles.clear();
            s.vertices.reserve(vertices.len().min(MAX_GPU_VERTICES));
            s.triangles.reserve(triangles.len().min(MAX_GPU_TRIANGLES));

            let mut min = Vec3::splat(f32::INFINITY);
            let mut max = Vec3::splat(f32::NEG_INFINITY);
            for &v in vertices {
                min = min.min(v);
                max = max.max(v);
            }
            let ext = (max - min).max(Vec3::splat(1e-5));

            for &v in vertices.iter().take(MAX_GPU_VERTICES) {
                let u = (v.x - min.x) / ext.x;
                let vv = (v.z - min.z) / ext.z;
                s.vertices.push([v.x, v.y, v.z, u, vv]);
            }
            let tri_target = triangles.len().min(MAX_GPU_TRIANGLES).max(1);
            let tri_step = ((triangles.len() as f32 / tri_target as f32).ceil() as usize).max(1);
            for (i, tri) in triangles.iter().enumerate() {
                if i % tri_step != 0 || s.triangles.len() >= tri_target {
                    continue;
                }
                if tri[0] as usize >= s.vertices.len()
                    || tri[1] as usize >= s.vertices.len()
                    || tri[2] as usize >= s.vertices.len()
                {
                    continue;
                }
                s.triangles.push(*tri);
            }
        }

        s.mvp = mvp.to_cols_array_2d();
        s.show_wireframe = show_wireframe;
        s.texture_id = 0;
        s.texture_size = [0, 0];
        s.texture_rgba.clear();
    }

    pub fn paint_callback(&self, rect: egui::Rect) -> egui::PaintCallback {
        egui_wgpu::Callback::new_paint_callback(
            rect,
            Draw3dCallback {
                target_format: self.target_format,
                scene: self.scene.clone(),
            },
        )
    }
}

impl Draw3dCallback {
    fn create_resources(&self, device: &wgpu::Device) -> GpuResources {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("viewport_gpu_shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER_SRC.into()),
        });
        let wire_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("viewport_gpu_wire_shader"),
            source: wgpu::ShaderSource::Wgsl(WIRE_SHADER_SRC.into()),
        });

        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("viewport_gpu_bind_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("viewport_gpu_pipeline_layout"),
            bind_group_layouts: &[&bind_layout],
            push_constant_ranges: &[],
        });

        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<[f32; 5]>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: (std::mem::size_of::<f32>() * 3) as u64,
                    shader_location: 1,
                },
            ],
        };

        let solid_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("viewport_gpu_solid_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[vertex_layout.clone()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: self.target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Back),
                front_face: wgpu::FrontFace::Ccw,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let wire_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("viewport_gpu_wire_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &wire_shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[vertex_layout],
            },
            fragment: Some(wgpu::FragmentState {
                module: &wire_shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: self.target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("viewport_gpu_ubo"),
            contents: &[0_u8; 80],
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("viewport_gpu_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            ..Default::default()
        });
        let white_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("viewport_gpu_white_tex"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let white_view = white_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("viewport_gpu_bind_group"),
            layout: &bind_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&white_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        GpuResources {
            solid_pipeline,
            wire_pipeline,
            bind_layout,
            uniform_buffer,
            uniform_data: [0_u8; 80],
            bind_group,
            _white_texture: white_texture,
            white_view,
            active_texture: None,
            active_texture_view: None,
            sampler,
            vertex_buffer: None,
            index_buffer: None,
            line_index_buffer: None,
            index_count: 0,
            line_index_count: 0,
            uploaded_mesh_id: 0,
            uploaded_texture_id: 0,
            depth_sorted_mesh_id: 0,
            draw_wireframe: false,
            pending_mesh_upload: None,
            pending_texture_upload: None,
        }
    }
}

fn push_f32(buf: &mut [u8], offset: usize, value: f32) {
    buf[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn build_line_indices(tris: &[[u32; 3]]) -> Vec<u32> {
    let mut edges = HashSet::<(u32, u32)>::with_capacity(tris.len() * 2);
    let mut out = Vec::with_capacity(tris.len() * 6);
    for t in tris {
        let pairs = [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])];
        for (a, b) in pairs {
            let key = if a < b { (a, b) } else { (b, a) };
            if edges.insert(key) {
                out.push(a);
                out.push(b);
            }
        }
    }
    out
}

fn clip_depth(mvp: &Mat4, p: [f32; 5]) -> f32 {
    let cp = *mvp * Vec4::new(p[0], p[1], p[2], 1.0);
    if cp.w.abs() < 1e-6 {
        return 0.0;
    }
    cp.z / cp.w
}

fn upload_pending_mesh_chunk(queue: &wgpu::Queue, pending: &mut PendingMeshUpload, budget_left: &mut usize) {
    if *budget_left == 0 {
        return;
    }

    if pending.vertex_cursor < pending.vertices.len() {
        let stride = std::mem::size_of::<[f32; 5]>();
        let max_count = (*budget_left / stride).max(1);
        let end = (pending.vertex_cursor + max_count).min(pending.vertices.len());
        let chunk_len = end - pending.vertex_cursor;
        let mut bytes = Vec::with_capacity(chunk_len * stride);
        for v in &pending.vertices[pending.vertex_cursor..end] {
            bytes.extend_from_slice(&v[0].to_le_bytes());
            bytes.extend_from_slice(&v[1].to_le_bytes());
            bytes.extend_from_slice(&v[2].to_le_bytes());
            bytes.extend_from_slice(&v[3].to_le_bytes());
            bytes.extend_from_slice(&v[4].to_le_bytes());
        }
        let offset = (pending.vertex_cursor * stride) as u64;
        queue.write_buffer(&pending.vertex_buffer, offset, &bytes);
        pending.vertex_cursor = end;
        *budget_left = budget_left.saturating_sub(bytes.len());
        if *budget_left == 0 {
            return;
        }
    }

    if pending.tri_cursor < pending.triangles.len() {
        let stride = std::mem::size_of::<[u32; 3]>();
        let max_count = (*budget_left / stride).max(1);
        let end = (pending.tri_cursor + max_count).min(pending.triangles.len());
        let chunk_len = end - pending.tri_cursor;
        let mut bytes = Vec::with_capacity(chunk_len * stride);
        for tri in &pending.triangles[pending.tri_cursor..end] {
            bytes.extend_from_slice(&tri[0].to_le_bytes());
            bytes.extend_from_slice(&tri[1].to_le_bytes());
            bytes.extend_from_slice(&tri[2].to_le_bytes());
        }
        let offset = (pending.tri_cursor * stride) as u64;
        queue.write_buffer(&pending.index_buffer, offset, &bytes);
        pending.tri_cursor = end;
        *budget_left = budget_left.saturating_sub(bytes.len());
        if *budget_left == 0 {
            return;
        }
    }

    if pending.line_cursor < pending.line_indices.len() {
        let Some(line_buffer) = pending.line_index_buffer.as_ref() else {
            pending.line_cursor = pending.line_indices.len();
            return;
        };
        let stride = std::mem::size_of::<u32>();
        let max_count = (*budget_left / stride).max(1);
        let end = (pending.line_cursor + max_count).min(pending.line_indices.len());
        let chunk_len = end - pending.line_cursor;
        let mut bytes = Vec::with_capacity(chunk_len * stride);
        for i in &pending.line_indices[pending.line_cursor..end] {
            bytes.extend_from_slice(&i.to_le_bytes());
        }
        let offset = (pending.line_cursor * stride) as u64;
        queue.write_buffer(line_buffer, offset, &bytes);
        pending.line_cursor = end;
        *budget_left = budget_left.saturating_sub(bytes.len());
    }
}

fn upload_pending_texture_chunk(
    queue: &wgpu::Queue,
    pending: &mut PendingTextureUpload,
    budget_left: &mut usize,
) {
    if *budget_left == 0 {
        return;
    }
    if pending.uploaded_rows >= pending.size[1] || pending.size[0] == 0 {
        return;
    }

    let bytes_per_row = (pending.size[0] * 4) as usize;
    if bytes_per_row == 0 {
        pending.uploaded_rows = pending.size[1];
        return;
    }

    let remaining_rows = pending.size[1] - pending.uploaded_rows;
    let rows_by_budget = ((*budget_left / bytes_per_row).max(1)) as u32;
    let rows = remaining_rows.min(rows_by_budget);

    let start = (pending.uploaded_rows as usize) * bytes_per_row;
    let end = start + (rows as usize) * bytes_per_row;
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &pending.texture,
            mip_level: 0,
            origin: wgpu::Origin3d {
                x: 0,
                y: pending.uploaded_rows,
                z: 0,
            },
            aspect: wgpu::TextureAspect::All,
        },
        &pending.rgba[start..end],
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(pending.size[0] * 4),
            rows_per_image: Some(rows),
        },
        wgpu::Extent3d {
            width: pending.size[0],
            height: rows,
            depth_or_array_layers: 1,
        },
    );

    pending.uploaded_rows += rows;
    *budget_left = budget_left.saturating_sub((rows as usize) * bytes_per_row);
}

impl egui_wgpu::CallbackTrait for Draw3dCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let resources = callback_resources
            .entry::<GpuResources>()
            .or_insert_with(|| self.create_resources(device));

        let scene = self.scene.lock().expect("scene lock");
        if scene.mesh_id == 0 || scene.vertices.is_empty() || scene.triangles.is_empty() {
            return Vec::new();
        }

        if resources.uploaded_mesh_id != scene.mesh_id {
            let should_rebuild = resources
                .pending_mesh_upload
                .as_ref()
                .map_or(true, |p| p.mesh_id != scene.mesh_id);
            if should_rebuild {
                let draw_wire_for_mesh =
                    scene.show_wireframe && scene.triangles.len() <= MAX_WIREFRAME_TRIANGLES_GPU;
                let line_indices = if draw_wire_for_mesh {
                    build_line_indices(&scene.triangles)
                } else {
                    Vec::new()
                };
                let line_index_count = line_indices.len() as u32;
                let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("viewport_gpu_vb"),
                    size: (scene.vertices.len() * std::mem::size_of::<[f32; 5]>()) as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("viewport_gpu_ib"),
                    size: (scene.triangles.len() * std::mem::size_of::<[u32; 3]>()) as u64,
                    usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                let line_index_buffer = if line_indices.is_empty() {
                    None
                } else {
                    Some(device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("viewport_gpu_wire_ib"),
                        size: (line_indices.len() * std::mem::size_of::<u32>()) as u64,
                        usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    }))
                };

                resources.pending_mesh_upload = Some(PendingMeshUpload {
                    mesh_id: scene.mesh_id,
                    vertices: scene.vertices.clone(),
                    triangles: scene.triangles.clone(),
                    line_indices,
                    vertex_cursor: 0,
                    tri_cursor: 0,
                    line_cursor: 0,
                    vertex_buffer,
                    index_buffer,
                    line_index_buffer,
                    index_count: (scene.triangles.len() * 3) as u32,
                    line_index_count,
                });
            }
        }

        if scene.texture_id == 0 {
            if resources.uploaded_texture_id != 0 || resources.pending_texture_upload.is_some() {
                resources.bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("viewport_gpu_bind_group_white"),
                    layout: &resources.bind_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: resources.uniform_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(&resources.white_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::Sampler(&resources.sampler),
                        },
                    ],
                });
                resources.active_texture = None;
                resources.active_texture_view = None;
                resources.uploaded_texture_id = 0;
                resources.pending_texture_upload = None;
            }
        } else if resources.uploaded_texture_id != scene.texture_id
            && !scene.texture_rgba.is_empty()
            && scene.texture_size[0] > 0
            && scene.texture_size[1] > 0
        {
            let should_rebuild = resources
                .pending_texture_upload
                .as_ref()
                .map_or(true, |p| p.texture_id != scene.texture_id);
            if should_rebuild {
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("viewport_gpu_active_tex"),
                    size: wgpu::Extent3d {
                        width: scene.texture_size[0],
                        height: scene.texture_size[1],
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
                resources.bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("viewport_gpu_bind_group_textured"),
                    layout: &resources.bind_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: resources.uniform_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(&view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::Sampler(&resources.sampler),
                        },
                    ],
                });
                resources.pending_texture_upload = Some(PendingTextureUpload {
                    texture_id: scene.texture_id,
                    size: scene.texture_size,
                    rgba: scene.texture_rgba.clone(),
                    uploaded_rows: 0,
                    texture,
                    view,
                });
            }
        }

        let mut offs = 0usize;
        for col in &scene.mvp {
            for f in col {
                push_f32(&mut resources.uniform_data, offs, *f);
                offs += 4;
            }
        }
        let (base_r, base_g, base_b, base_a) = if resources.uploaded_texture_id == 0 {
            // Default viewport material similar to Blender's neutral gray.
            (0.68, 0.68, 0.68, 1.0)
        } else {
            (1.0, 1.0, 1.0, 1.0)
        };
        push_f32(&mut resources.uniform_data, offs, base_r);
        offs += 4;
        push_f32(&mut resources.uniform_data, offs, base_g);
        offs += 4;
        push_f32(&mut resources.uniform_data, offs, base_b);
        offs += 4;
        push_f32(&mut resources.uniform_data, offs, base_a);
        queue.write_buffer(&resources.uniform_buffer, 0, &resources.uniform_data);

        let mut budget = GPU_UPLOAD_BUDGET_BYTES;
        if let Some(mut pending) = resources.pending_mesh_upload.take() {
            upload_pending_mesh_chunk(queue, &mut pending, &mut budget);
            let done = pending.vertex_cursor >= pending.vertices.len()
                && pending.tri_cursor >= pending.triangles.len()
                && pending.line_cursor >= pending.line_indices.len();
            if done {
                resources.vertex_buffer = Some(pending.vertex_buffer);
                resources.index_buffer = Some(pending.index_buffer);
                resources.line_index_buffer = pending.line_index_buffer;
                resources.index_count = pending.index_count;
                resources.line_index_count = pending.line_index_count;
                resources.uploaded_mesh_id = pending.mesh_id;
                resources.depth_sorted_mesh_id = 0;
            } else {
                resources.pending_mesh_upload = Some(pending);
            }
        }

        if let Some(mut pending) = resources.pending_texture_upload.take() {
            upload_pending_texture_chunk(queue, &mut pending, &mut budget);
            if pending.uploaded_rows >= pending.size[1] {
                resources.uploaded_texture_id = pending.texture_id;
                resources.active_texture = Some(pending.texture);
                resources.active_texture_view = Some(pending.view);
            } else {
                resources.pending_texture_upload = Some(pending);
            }
        }

        if resources.uploaded_mesh_id == scene.mesh_id
            && resources.depth_sorted_mesh_id != scene.mesh_id
            && scene.triangles.len() <= SORT_TRIANGLES_THRESHOLD
        {
            if let Some(ib) = &resources.index_buffer {
                let mvp = Mat4::from_cols_array_2d(&scene.mvp);
                let mut tri_order: Vec<(f32, [u32; 3])> = scene
                    .triangles
                    .iter()
                    .copied()
                    .map(|tri| {
                        let a = scene.vertices.get(tri[0] as usize).copied().unwrap_or([0.0; 5]);
                        let b = scene.vertices.get(tri[1] as usize).copied().unwrap_or([0.0; 5]);
                        let c = scene.vertices.get(tri[2] as usize).copied().unwrap_or([0.0; 5]);
                        let d = (clip_depth(&mvp, a) + clip_depth(&mvp, b) + clip_depth(&mvp, c))
                            / 3.0;
                        (d, tri)
                    })
                    .collect();
                tri_order.sort_by(|a, b| b.0.total_cmp(&a.0));

                let mut sorted = Vec::<u8>::with_capacity(scene.triangles.len() * 3 * 4);
                for (_, tri) in tri_order {
                    sorted.extend_from_slice(&tri[0].to_le_bytes());
                    sorted.extend_from_slice(&tri[1].to_le_bytes());
                    sorted.extend_from_slice(&tri[2].to_le_bytes());
                }
                queue.write_buffer(ib, 0, &sorted);
                resources.depth_sorted_mesh_id = scene.mesh_id;
            }
        }

        resources.draw_wireframe = scene.show_wireframe
            && resources.uploaded_mesh_id == scene.mesh_id
            && resources.line_index_count > 0;

        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        callback_resources: &egui_wgpu::CallbackResources,
    ) {
        let Some(resources) = callback_resources.get::<GpuResources>() else {
            return;
        };
        let (Some(vb), Some(ib)) = (&resources.vertex_buffer, &resources.index_buffer) else {
            return;
        };
        if resources.index_count == 0 {
            return;
        }

        render_pass.set_bind_group(0, &resources.bind_group, &[]);

        render_pass.set_pipeline(&resources.solid_pipeline);
        render_pass.set_vertex_buffer(0, vb.slice(..));
        render_pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..resources.index_count, 0, 0..1);

        if resources.draw_wireframe {
            if let Some(wib) = &resources.line_index_buffer {
                if resources.line_index_count > 0 {
                    render_pass.set_pipeline(&resources.wire_pipeline);
                    render_pass.set_vertex_buffer(0, vb.slice(..));
                    render_pass.set_index_buffer(wib.slice(..), wgpu::IndexFormat::Uint32);
                    render_pass.draw_indexed(0..resources.line_index_count, 0, 0..1);
                }
            }
        }
    }
}
