use std::sync::{Arc, Mutex};

use eframe::egui;
use eframe::egui_wgpu;
use eframe::wgpu;
use eframe::wgpu::util::DeviceExt;
use glam::{Mat4, Vec3};

const MAX_GPU_TRIANGLES: usize = 120_000;
const MAX_GPU_VERTICES: usize = 160_000;
const GPU_UPLOAD_BUDGET_BYTES: usize = 8 * 1024 * 1024;

const SHADER_SRC: &str = r#"
struct Uniforms {
    mvp: mat4x4<f32>,
    tint: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> ubo: Uniforms;

struct VsIn {
    @location(0) pos: vec3<f32>,
};

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
};

@vertex
fn vs_main(v: VsIn) -> VsOut {
    var out: VsOut;
    out.clip_pos = ubo.mvp * vec4<f32>(v.pos, 1.0);
    out.world_pos = v.pos;
    return out;
}

@fragment
fn fs_main(v: VsOut) -> @location(0) vec4<f32> {
    let dpdx_pos = dpdx(v.world_pos);
    let dpdy_pos = dpdy(v.world_pos);
    let c = cross(dpdx_pos, dpdy_pos);
    let len2 = max(dot(c, c), 1e-8);
    let n = c * inverseSqrt(len2);
    let l = normalize(vec3<f32>(0.42, 0.78, 0.46));
    let ndotl = max(dot(n, l), 0.0);
    let rim = pow(1.0 - abs(n.z), 1.4) * 0.12;
    let shade = clamp(0.30 + ndotl * 0.70 + rim, 0.0, 1.0);
    let color = ubo.tint.rgb * shade;
    return vec4<f32>(color, 1.0);
}
"#;

#[derive(Default)]
struct SceneState {
    mesh_id: u64,
    vertices: Vec<[f32; 3]>,
    triangles: Vec<[u32; 3]>,
    mvp: [[f32; 4]; 4],
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
    uniform_buffer: wgpu::Buffer,
    uniform_data: [u8; 80],
    bind_group: wgpu::BindGroup,
    vertex_buffer: Option<wgpu::Buffer>,
    index_buffer: Option<wgpu::Buffer>,
    index_count: u32,
    uploaded_mesh_id: u64,
    pending_mesh_upload: Option<PendingMeshUpload>,
    staged_vertices: Vec<[f32; 3]>,
    staged_triangles: Vec<[u32; 3]>,
}

struct PendingMeshUpload {
    mesh_id: u64,
    vertex_len: usize,
    tri_len: usize,
    vertex_cursor: usize,
    tri_cursor: usize,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
}

impl ViewportGpuRenderer {
    pub fn new(render_state: egui_wgpu::RenderState) -> Self {
        Self {
            target_format: render_state.target_format,
            scene: Arc::new(Mutex::new(SceneState::default())),
        }
    }

    pub fn update_scene(&self, mesh_id: u64, vertices: &[Vec3], triangles: &[[u32; 3]], mvp: Mat4) {
        let mut s = self.scene.lock().expect("scene lock");

        if s.mesh_id != mesh_id {
            s.mesh_id = mesh_id;
            s.vertices.clear();
            s.triangles.clear();
            s.vertices.reserve(vertices.len().min(MAX_GPU_VERTICES));
            s.triangles.reserve(triangles.len().min(MAX_GPU_TRIANGLES));

            for &v in vertices.iter().take(MAX_GPU_VERTICES) {
                s.vertices.push([v.x, v.y, v.z]);
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

        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("viewport_gpu_bind_layout"),
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

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("viewport_gpu_pipeline_layout"),
            bind_group_layouts: &[&bind_layout],
            push_constant_ranges: &[],
        });

        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<[f32; 3]>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 0,
                shader_location: 0,
            }],
        };

        let solid_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("viewport_gpu_solid_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[vertex_layout],
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
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth24Plus,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("viewport_gpu_ubo"),
            contents: &[0_u8; 80],
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("viewport_gpu_bind_group"),
            layout: &bind_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        GpuResources {
            solid_pipeline,
            uniform_buffer,
            uniform_data: [0_u8; 80],
            bind_group,
            vertex_buffer: None,
            index_buffer: None,
            index_count: 0,
            uploaded_mesh_id: 0,
            pending_mesh_upload: None,
            staged_vertices: Vec::new(),
            staged_triangles: Vec::new(),
        }
    }
}

fn push_f32(buf: &mut [u8], offset: usize, value: f32) {
    buf[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn upload_pending_mesh_chunk(
    queue: &wgpu::Queue,
    pending: &mut PendingMeshUpload,
    vertices: &[[f32; 3]],
    triangles: &[[u32; 3]],
    budget_left: &mut usize,
) {
    if *budget_left == 0 {
        return;
    }

    if pending.vertex_cursor < pending.vertex_len {
        let stride = std::mem::size_of::<[f32; 3]>();
        let max_count = (*budget_left / stride).max(1);
        let end = (pending.vertex_cursor + max_count).min(pending.vertex_len);
        let chunk_len = end - pending.vertex_cursor;
        let mut bytes = Vec::with_capacity(chunk_len * stride);
        for v in &vertices[pending.vertex_cursor..end] {
            bytes.extend_from_slice(&v[0].to_le_bytes());
            bytes.extend_from_slice(&v[1].to_le_bytes());
            bytes.extend_from_slice(&v[2].to_le_bytes());
        }
        let offset = (pending.vertex_cursor * stride) as u64;
        queue.write_buffer(&pending.vertex_buffer, offset, &bytes);
        pending.vertex_cursor = end;
        *budget_left = budget_left.saturating_sub(bytes.len());
        if *budget_left == 0 {
            return;
        }
    }

    if pending.tri_cursor < pending.tri_len {
        let stride = std::mem::size_of::<[u32; 3]>();
        let max_count = (*budget_left / stride).max(1);
        let end = (pending.tri_cursor + max_count).min(pending.tri_len);
        let chunk_len = end - pending.tri_cursor;
        let mut bytes = Vec::with_capacity(chunk_len * stride);
        for tri in &triangles[pending.tri_cursor..end] {
            bytes.extend_from_slice(&tri[0].to_le_bytes());
            bytes.extend_from_slice(&tri[1].to_le_bytes());
            bytes.extend_from_slice(&tri[2].to_le_bytes());
        }
        let offset = (pending.tri_cursor * stride) as u64;
        queue.write_buffer(&pending.index_buffer, offset, &bytes);
        pending.tri_cursor = end;
        *budget_left = budget_left.saturating_sub(bytes.len());
    }
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
                let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("viewport_gpu_vb"),
                    size: (scene.vertices.len() * std::mem::size_of::<[f32; 3]>()) as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("viewport_gpu_ib"),
                    size: (scene.triangles.len() * std::mem::size_of::<[u32; 3]>()) as u64,
                    usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });

                resources.staged_vertices.clear();
                resources.staged_vertices.extend_from_slice(&scene.vertices);
                resources.staged_triangles.clear();
                resources.staged_triangles.extend_from_slice(&scene.triangles);

                resources.pending_mesh_upload = Some(PendingMeshUpload {
                    mesh_id: scene.mesh_id,
                    vertex_len: resources.staged_vertices.len(),
                    tri_len: resources.staged_triangles.len(),
                    vertex_cursor: 0,
                    tri_cursor: 0,
                    vertex_buffer,
                    index_buffer,
                    index_count: (scene.triangles.len() * 3) as u32,
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
        push_f32(&mut resources.uniform_data, offs, 0.68);
        offs += 4;
        push_f32(&mut resources.uniform_data, offs, 0.68);
        offs += 4;
        push_f32(&mut resources.uniform_data, offs, 0.68);
        offs += 4;
        push_f32(&mut resources.uniform_data, offs, 1.0);
        queue.write_buffer(&resources.uniform_buffer, 0, &resources.uniform_data);

        let mut budget = GPU_UPLOAD_BUDGET_BYTES;
        if let Some(mut pending) = resources.pending_mesh_upload.take() {
            upload_pending_mesh_chunk(
                queue,
                &mut pending,
                &resources.staged_vertices,
                &resources.staged_triangles,
                &mut budget,
            );
            let done = pending.vertex_cursor >= pending.vertex_len
                && pending.tri_cursor >= pending.tri_len;
            if done {
                resources.vertex_buffer = Some(pending.vertex_buffer);
                resources.index_buffer = Some(pending.index_buffer);
                resources.index_count = pending.index_count;
                resources.uploaded_mesh_id = pending.mesh_id;
                resources.staged_vertices.clear();
                resources.staged_triangles.clear();
            } else {
                resources.pending_mesh_upload = Some(pending);
            }
        }

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
    }
}
