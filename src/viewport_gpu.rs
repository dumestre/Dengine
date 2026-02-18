use std::sync::{Arc, Mutex};

use eframe::egui;
use eframe::egui_wgpu;
use eframe::wgpu;
use eframe::wgpu::util::DeviceExt;
use eframe::wgpu::{TexelCopyBufferLayout, TexelCopyTextureInfo};
use glam::{Mat4, Vec3};

use engine_render::shader::{LIT_SHADER, LIT_UNIFORM_SIZE, LIT_VERTEX_STRIDE};

const MAX_GPU_TRIANGLES: usize = 120_000;

/// Normaliza um path removendo o prefixo verbatim do Windows (\\?\)
fn normalize_path(path: &str) -> String {
    if path.starts_with("\\\\?\\") {
        path[4..].to_string()
    } else {
        path.to_string()
    }
}
const MAX_GPU_VERTICES: usize = 160_000;
const GPU_UPLOAD_BUDGET_BYTES: usize = 8 * 1024 * 1024;

#[derive(Default)]
struct SceneState {
    mesh_id: u64,
    vertices: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    triangles: Vec<[u32; 3]>,
    mvp: [[f32; 4]; 4],
    model: [[f32; 4]; 4],
    camera_pos: [f32; 3],
    light_dir: [f32; 3],
    light_color: [f32; 3],
    light_intensity: f32,
    light_enabled: f32,
    texture_path: Option<String>,
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
    uniform_data: [u8; LIT_UNIFORM_SIZE],
    bind_group_layout: wgpu::BindGroupLayout,
    current_bind_group: Option<wgpu::BindGroup>,
    vertex_buffer: Option<wgpu::Buffer>,
    index_buffer: Option<wgpu::Buffer>,
    index_count: u32,
    uploaded_mesh_id: u64,
    pending_mesh_upload: Option<PendingMeshUpload>,
    staged_vertices: Vec<[f32; 3]>,
    staged_normals: Vec<[f32; 3]>,
    staged_uvs: Vec<[f32; 2]>,
    staged_triangles: Vec<[u32; 3]>,
    textures: std::collections::HashMap<String, (wgpu::Texture, wgpu::TextureView, wgpu::Sampler)>,
    current_texture_path: Option<String>,
    white_pixel_texture: (wgpu::Texture, wgpu::TextureView, wgpu::Sampler),
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

    pub fn update_scene(
        &self,
        mesh_id: u64,
        vertices: &[Vec3],
        normals: &[Vec3],
        uvs: &[[f32; 2]],
        triangles: &[[u32; 3]],
        mvp: Mat4,
        model: Mat4,
        camera_pos: Vec3,
        light_dir: Vec3,
        light_color: Vec3,
        light_intensity: f32,
        light_enabled: bool,
        texture_path: Option<String>,
    ) {
        let mut s = self.scene.lock().expect("scene lock");

        if s.mesh_id != mesh_id {
            s.mesh_id = mesh_id;
            s.vertices.clear();
            s.normals.clear();
            s.uvs.clear();
            s.triangles.clear();
            s.texture_path = texture_path.clone();
            s.vertices.reserve(vertices.len().min(MAX_GPU_VERTICES));
            s.normals.reserve(normals.len().min(MAX_GPU_VERTICES));
            s.uvs.reserve(uvs.len().min(MAX_GPU_VERTICES));
            s.triangles.reserve(triangles.len().min(MAX_GPU_TRIANGLES));

            for &v in vertices.iter().take(MAX_GPU_VERTICES) {
                s.vertices.push([v.x, v.y, v.z]);
            }
            for &n in normals.iter().take(MAX_GPU_VERTICES) {
                s.normals.push([n.x, n.y, n.z]);
            }
            // Pad normals if fewer than vertices
            while s.normals.len() < s.vertices.len() {
                s.normals.push([0.0, 1.0, 0.0]);
            }
            for &uv in uvs.iter().take(MAX_GPU_VERTICES) {
                s.uvs.push(uv);
            }
            // Pad UVs if fewer than vertices
            while s.uvs.len() < s.vertices.len() {
                s.uvs.push([0.0, 0.0]);
            }
            eprintln!("[GPU] Mesh upload: vertices={}, normals={}, uvs={}, triangles={}", 
                s.vertices.len(), s.normals.len(), s.uvs.len(), triangles.len().min(MAX_GPU_TRIANGLES));
            // Log primeiras UVs para debug
            if s.uvs.len() >= 3 {
                eprintln!("[GPU] Primeiras UVs: [{}, {}], [{}, {}], [{}, {}]", 
                    s.uvs[0][0], s.uvs[0][1], 
                    s.uvs[1][0], s.uvs[1][1],
                    s.uvs[2][0], s.uvs[2][1]);
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
        s.model = model.to_cols_array_2d();
        s.camera_pos = [camera_pos.x, camera_pos.y, camera_pos.z];
        s.light_dir = [light_dir.x, light_dir.y, light_dir.z];
        s.light_color = [light_color.x, light_color.y, light_color.z];
        s.light_intensity = light_intensity;
        s.light_enabled = if light_enabled { 1.0 } else { 0.0 };
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
    fn create_resources(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> GpuResources {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("viewport_gpu_shader"),
            source: wgpu::ShaderSource::Wgsl(LIT_SHADER.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });

        // Sampler padrão
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("viewport_gpu_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        // Textura branca 1x1 como fallback
        let white_pixel_data = [255, 255, 255, 255];
        let white_pixel_size = wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        };
        let white_pixel_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("viewport_gpu_white_pixel_texture"),
            size: white_pixel_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &white_pixel_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &white_pixel_data,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            white_pixel_size,
        );
        let white_pixel_view =
            white_pixel_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("viewport_gpu_ubo"),
            contents: &[0_u8; LIT_UNIFORM_SIZE],
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Vertex layout: pos(vec3) + normal(vec3) + uv(vec2) = 32 bytes
        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: LIT_VERTEX_STRIDE as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0, // pos
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 12,
                    shader_location: 1, // normal
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 24,
                    shader_location: 2, // uv
                },
            ],
        };

        let solid_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("viewport_gpu_solid_pipeline"),
            layout: Some(
                &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("viewport_gpu_pipeline_layout"),
                    bind_group_layouts: &[&bind_group_layout],
                    push_constant_ranges: &[],
                }),
            ),
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

        GpuResources {
            solid_pipeline,
            uniform_buffer,
            uniform_data: [0_u8; LIT_UNIFORM_SIZE],
            bind_group_layout,
            current_bind_group: None,
            vertex_buffer: None,
            index_buffer: None,
            index_count: 0,
            uploaded_mesh_id: 0,
            pending_mesh_upload: None,
            staged_vertices: Vec::new(),
            staged_normals: Vec::new(),
            staged_uvs: Vec::new(),
            staged_triangles: Vec::new(),
            textures: std::collections::HashMap::new(),
            current_texture_path: None,
            white_pixel_texture: (white_pixel_texture, white_pixel_view, sampler),
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
    normals: &[[f32; 3]],
    uvs: &[[f32; 2]],
    triangles: &[[u32; 3]],
    budget_left: &mut usize,
) {
    if *budget_left == 0 {
        return;
    }

    let vertex_available = vertices.len().min(normals.len()).min(uvs.len());
    if pending.vertex_cursor < pending.vertex_len && pending.vertex_cursor < vertex_available {
        let stride = LIT_VERTEX_STRIDE; // 32 bytes: pos(12) + normal(12) + uv(8)
        let max_count = (*budget_left / stride).max(1);
        let end = (pending.vertex_cursor + max_count)
            .min(pending.vertex_len)
            .min(vertex_available);
        let chunk_len = end - pending.vertex_cursor;
        let mut bytes = Vec::with_capacity(chunk_len * stride);
        for i in 0..chunk_len {
            let vi = pending.vertex_cursor + i;
            let v = &vertices[vi];
            let n = &normals[vi];
            let uv = &uvs[vi];
            // pos
            bytes.extend_from_slice(&v[0].to_le_bytes());
            bytes.extend_from_slice(&v[1].to_le_bytes());
            bytes.extend_from_slice(&v[2].to_le_bytes());
            // normal
            bytes.extend_from_slice(&n[0].to_le_bytes());
            bytes.extend_from_slice(&n[1].to_le_bytes());
            bytes.extend_from_slice(&n[2].to_le_bytes());
            // uv
            bytes.extend_from_slice(&uv[0].to_le_bytes());
            bytes.extend_from_slice(&uv[1].to_le_bytes());
        }
        let offset = (pending.vertex_cursor * stride) as u64;
        queue.write_buffer(&pending.vertex_buffer, offset, &bytes);
        pending.vertex_cursor = end;
        *budget_left = budget_left.saturating_sub(bytes.len());
        if *budget_left == 0 {
            return;
        }
    }

    if pending.tri_cursor < pending.tri_len && pending.tri_cursor < triangles.len() {
        let stride = std::mem::size_of::<[u32; 3]>();
        let max_count = (*budget_left / stride).max(1);
        let end = (pending.tri_cursor + max_count)
            .min(pending.tri_len)
            .min(triangles.len());
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
        use std::path::PathBuf;

        let resources = callback_resources
            .entry::<GpuResources>()
            .or_insert_with(|| self.create_resources(device, queue));

        let scene = self.scene.lock().expect("scene lock");
        let current_mesh_texture_path = scene.texture_path.clone().map(|p| normalize_path(&p));

        if scene.mesh_id == 0 || scene.vertices.is_empty() || scene.triangles.is_empty() {
            if resources.current_texture_path.as_ref() != current_mesh_texture_path.as_ref() {
                resources.current_texture_path = current_mesh_texture_path;
                resources.current_bind_group = None;
            }
            return Vec::new();
        }

        // Upload de mesh quando o ID muda
        if resources.uploaded_mesh_id != scene.mesh_id {
            let should_rebuild = resources
                .pending_mesh_upload
                .as_ref()
                .map_or(true, |p| p.mesh_id != scene.mesh_id);
            if should_rebuild {
                let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("viewport_gpu_vb"),
                    size: (scene.vertices.len() * LIT_VERTEX_STRIDE) as u64,
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
                resources.staged_normals.clear();
                resources.staged_normals.extend_from_slice(&scene.normals);
                resources.staged_uvs.clear();
                resources.staged_uvs.extend_from_slice(&scene.uvs);
                resources.staged_triangles.clear();
                resources
                    .staged_triangles
                    .extend_from_slice(&scene.triangles);

                let vertex_len = resources
                    .staged_vertices
                    .len()
                    .min(resources.staged_normals.len())
                    .min(resources.staged_uvs.len());
                resources.pending_mesh_upload = Some(PendingMeshUpload {
                    mesh_id: scene.mesh_id,
                    vertex_len,
                    tri_len: resources.staged_triangles.len(),
                    vertex_cursor: 0,
                    tri_cursor: 0,
                    vertex_buffer,
                    index_buffer,
                    index_count: (scene.triangles.len() * 3) as u32,
                });
            }
        }

        // Carrega textura se necessário
        let mut has_texture = 0.0_f32;

        eprintln!("[GPU] Texture path: {:?}", current_mesh_texture_path);

        if let Some(texture_path_str) = &current_mesh_texture_path {
            eprintln!("[GPU] Carregando textura: {}", texture_path_str);
            if resources.current_texture_path.as_ref() != Some(texture_path_str) {
                resources.current_texture_path = Some(texture_path_str.clone());
                resources.current_bind_group = None;
                eprintln!("[GPU] Bind group invalidado, novo path");
            }

            if resources.textures.get(texture_path_str).is_none() {
                eprintln!("[GPU] Textura não em cache, carregando do disco");
                // Normaliza o path para abrir o arquivo (remove \\?\ se existir)
                let disk_path = normalize_path(texture_path_str);
                let path = PathBuf::from(&disk_path);
                match image::open(&path) {
                    Ok(img) => {
                        eprintln!("[GPU] Imagem carregada: {}x{}", img.width(), img.height());
                        let rgba = img.to_rgba8();
                        let (width, height) = rgba.dimensions();
                        let size = wgpu::Extent3d {
                            width,
                            height,
                            depth_or_array_layers: 1,
                        };
                        let new_texture = device.create_texture(&wgpu::TextureDescriptor {
                            label: Some(&format!("viewport_gpu_texture_{}", texture_path_str)),
                            size,
                            mip_level_count: 1,
                            sample_count: 1,
                            dimension: wgpu::TextureDimension::D2,
                            format: wgpu::TextureFormat::Rgba8UnormSrgb,
                            usage: wgpu::TextureUsages::TEXTURE_BINDING
                                | wgpu::TextureUsages::COPY_DST,
                            view_formats: &[],
                        });
                        queue.write_texture(
                            TexelCopyTextureInfo {
                                texture: &new_texture,
                                mip_level: 0,
                                origin: wgpu::Origin3d::ZERO,
                                aspect: wgpu::TextureAspect::All,
                            },
                            &rgba,
                            TexelCopyBufferLayout {
                                offset: 0,
                                bytes_per_row: Some(width * 4),
                                rows_per_image: Some(height),
                            },
                            size,
                        );
                        let new_texture_view =
                            new_texture.create_view(&wgpu::TextureViewDescriptor::default());
                        resources.textures.insert(
                            texture_path_str.clone(),
                            (
                                new_texture,
                                new_texture_view,
                                resources.white_pixel_texture.2.clone(),
                            ),
                        );
                        eprintln!("[GPU] Textura carregada com sucesso!");
                        // Invalida bind group para recriar com a nova textura
                        resources.current_bind_group = None;
                    }
                    Err(e) => {
                        eprintln!("Falha ao carregar textura {}: {}", texture_path_str, e);
                        resources.textures.insert(
                            texture_path_str.clone(),
                            resources.white_pixel_texture.clone(),
                        );
                        // Invalida bind group mesmo em caso de erro (usa white pixel)
                        resources.current_bind_group = None;
                    }
                }
            }
            if resources.textures.get(texture_path_str).is_some() {
                has_texture = 1.0;
                eprintln!("[GPU] has_texture = 1.0");
            }
        } else {
            eprintln!("[GPU] Sem textura");
        }

        // Preenche uniform buffer (192 bytes)
        // Layout do shader:
        //   0..64   mvp (mat4)
        //  64..128  model (mat4)
        // 128..140  camera_pos (vec3)
        // 140..144  light_intensity (f32)
        // 144..156  light_dir (vec3)
        // 156..160  light_enabled (f32)
        // 160..172  light_color (vec3)
        // 172..176  has_texture (f32)
        // 176..192  tint (vec4)
        let mut offs = 0usize;
        for col in &scene.mvp {
            for f in col {
                push_f32(&mut resources.uniform_data, offs, *f);
                offs += 4;
            }
        }
        for col in &scene.model {
            for f in col {
                push_f32(&mut resources.uniform_data, offs, *f);
                offs += 4;
            }
        }
        // camera_pos (128..140)
        push_f32(&mut resources.uniform_data, offs, scene.camera_pos[0]);
        offs += 4;
        push_f32(&mut resources.uniform_data, offs, scene.camera_pos[1]);
        offs += 4;
        push_f32(&mut resources.uniform_data, offs, scene.camera_pos[2]);
        offs += 4;
        // light_intensity (140..144)
        push_f32(&mut resources.uniform_data, offs, scene.light_intensity);
        offs += 4;
        // light_dir (144..156)
        push_f32(&mut resources.uniform_data, offs, scene.light_dir[0]);
        offs += 4;
        push_f32(&mut resources.uniform_data, offs, scene.light_dir[1]);
        offs += 4;
        push_f32(&mut resources.uniform_data, offs, scene.light_dir[2]);
        offs += 4;
        // light_enabled (156..160)
        push_f32(&mut resources.uniform_data, offs, scene.light_enabled);
        offs += 4;
        // light_color (160..172)
        push_f32(&mut resources.uniform_data, offs, scene.light_color[0]);
        offs += 4;
        push_f32(&mut resources.uniform_data, offs, scene.light_color[1]);
        offs += 4;
        push_f32(&mut resources.uniform_data, offs, scene.light_color[2]);
        offs += 4;
        // has_texture (172..176)
        push_f32(&mut resources.uniform_data, offs, has_texture);
        offs += 4;
        // tint (176..192)
        push_f32(&mut resources.uniform_data, offs, 1.0); // R
        offs += 4;
        push_f32(&mut resources.uniform_data, offs, 1.0); // G
        offs += 4;
        push_f32(&mut resources.uniform_data, offs, 1.0); // B
        offs += 4;
        push_f32(&mut resources.uniform_data, offs, 1.0); // A
        offs += 4;
        let _ = offs;

        queue.write_buffer(&resources.uniform_buffer, 0, &resources.uniform_data);

        // Atualiza bind group se necessário
        let bind_group_needed = resources.current_bind_group.is_none()
            || resources.current_texture_path.as_ref().map(|s| s.as_str())
                != current_mesh_texture_path.as_ref().map(|s| s.as_str());
        if bind_group_needed {
            eprintln!("[GPU] Criando bind group...");
            let (_tex, tex_view, tex_sampler) = if let Some(path) = &resources.current_texture_path
            {
                if let Some(tex_data) = resources.textures.get(path) {
                    eprintln!("[GPU] Textura encontrada no cache: {}", path);
                    tex_data
                } else {
                    eprintln!("[GPU] Textura NAO encontrada no cache, usando white pixel: {}", path);
                    &resources.white_pixel_texture
                }
            } else {
                eprintln!("[GPU] Sem textura, usando white pixel");
                &resources.white_pixel_texture
            };

            resources.current_bind_group =
                Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("viewport_gpu_bind_group"),
                    layout: &resources.bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                buffer: &resources.uniform_buffer,
                                offset: 0,
                                size: None,
                            }),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(tex_sampler),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::TextureView(tex_view),
                        },
                    ],
                }));
            eprintln!("[GPU] Bind group criado!");
        }

        // Upload de mesh (chunked)
        let mut budget = GPU_UPLOAD_BUDGET_BYTES;
        while let Some(mut pending) = resources.pending_mesh_upload.take() {
            upload_pending_mesh_chunk(
                queue,
                &mut pending,
                &resources.staged_vertices,
                &resources.staged_normals,
                &resources.staged_uvs,
                &resources.staged_triangles,
                &mut budget,
            );
            let done = pending.vertex_cursor >= pending.vertex_len
                && pending.tri_cursor >= pending.tri_len;
            if done {
                resources.vertex_buffer = Some(pending.vertex_buffer);
                resources.index_buffer = Some(pending.index_buffer);
                resources.index_count = if pending.vertex_len > 0 {
                    pending.index_count
                } else {
                    0
                };
                resources.uploaded_mesh_id = pending.mesh_id;
                resources.staged_vertices.clear();
                resources.staged_normals.clear();
                resources.staged_uvs.clear();
                resources.staged_triangles.clear();
                break;
            }
            resources.pending_mesh_upload = Some(pending);
            if budget == 0 {
                break;
            }
        }

        Vec::new()
    }

    fn paint(
        &self,
        info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        callback_resources: &egui_wgpu::CallbackResources,
    ) {
        let Some(resources) = callback_resources.get::<GpuResources>() else {
            return;
        };
        let vb = &resources.vertex_buffer;
        let ib = &resources.index_buffer;
        let bind_group = &resources.current_bind_group;
        let (Some(vb), Some(ib), Some(bind_group)) = (vb, ib, bind_group) else {
            return;
        };
        if resources.index_count == 0 {
            return;
        }

        // Viewport e scissor na rect do callback
        let ppp = info.pixels_per_point;
        let v = &info.viewport;
        let x = v.min.x * ppp;
        let y = v.min.y * ppp;
        let w = (v.width() * ppp).max(1.0) as u32;
        let h = (v.height() * ppp).max(1.0) as u32;
        if w > 0 && h > 0 {
            render_pass.set_viewport(x, y, w as f32, h as f32, 0.0, 1.0);
            render_pass.set_scissor_rect(x as u32, y as u32, w.max(1), h.max(1));
        }

        render_pass.set_bind_group(0, bind_group, &[]);
        render_pass.set_pipeline(&resources.solid_pipeline);
        render_pass.set_vertex_buffer(0, vb.slice(..));
        render_pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..resources.index_count, 0, 0..1);
    }
}
