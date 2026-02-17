//! Estruturas de dados de mesh e carregamento de arquivos
//!
//! MeshData contém geometria pronta para upload à GPU:
//! posições, normais, UVs e índices.

use std::path::{Path, PathBuf};

use glam::{Vec2, Vec3};

/// Dados de vértice para renderização
#[derive(Debug, Clone, Default)]
pub struct Vertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub texcoord: Vec2,
}

impl Vertex {
    pub fn new(position: Vec3, normal: Vec3, texcoord: Vec2) -> Self {
        Self {
            position,
            normal,
            texcoord,
        }
    }

    /// Empacota o vértice em [f32; 8] para upload à GPU
    /// Layout: [px, py, pz, nx, ny, nz, u, v]
    pub fn to_packed(&self) -> [f32; 8] {
        [
            self.position.x,
            self.position.y,
            self.position.z,
            self.normal.x,
            self.normal.y,
            self.normal.z,
            self.texcoord.x,
            self.texcoord.y,
        ]
    }
}

/// Dados de mesh — geometria crua pronta para upload à GPU
#[derive(Debug, Clone)]
pub struct MeshData {
    pub name: String,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub albedo_texture_path: Option<PathBuf>,
}

impl Default for MeshData {
    fn default() -> Self {
        Self {
            name: String::new(),
            vertices: Vec::new(),
            indices: Vec::new(),
            albedo_texture_path: None,
        }
    }
}

impl MeshData {
    /// Carrega mesh de arquivo (suporta .obj, .gltf, .glb)
    pub fn load_from_file(path: &Path) -> Result<Self, String> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .ok_or("Sem extensão de arquivo")?;

        match ext.as_str() {
            "obj" => Self::load_obj(path),
            "gltf" | "glb" => Self::load_gltf(path),
            _ => Err(format!("Formato não suportado: {}", ext)),
        }
    }

    /// Carrega arquivo OBJ
    fn load_obj(path: &Path) -> Result<Self, String> {
        let load_options = tobj::LoadOptions {
            triangulate: true,
            single_index: false,
            ignore_points: true,
            ignore_lines: true,
        };
        let (models, _) = tobj::load_obj(path, &load_options)
            .map_err(|e| format!("Falha ao carregar OBJ: {}", e))?;

        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        for model in models {
            let mesh = &model.mesh;
            let positions = &mesh.positions;
            let normals = &mesh.normals;
            let texcoords = &mesh.texcoords;
            let indices_raw = &mesh.indices;

            for i in 0..positions.len() / 3 {
                let position =
                    Vec3::new(positions[i * 3], positions[i * 3 + 1], positions[i * 3 + 2]);

                let normal = if normals.len() >= (i + 1) * 3 {
                    Vec3::new(normals[i * 3], normals[i * 3 + 1], normals[i * 3 + 2])
                } else {
                    Vec3::ZERO // Será calculada depois
                };

                let texcoord = if texcoords.len() >= (i + 1) * 2 {
                    Vec2::new(texcoords[i * 2], texcoords[i * 2 + 1])
                } else {
                    Vec2::ZERO
                };

                vertices.push(Vertex::new(position, normal, texcoord));
            }

            for &idx in indices_raw {
                indices.push(idx);
            }
        }

        let mut mesh = Self {
            name: path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            vertices,
            indices,
            albedo_texture_path: None,
        };

        // Se não tinha normais, calcula a partir dos triângulos
        mesh.ensure_normals();
        Ok(mesh)
    }

    /// Carrega arquivo GLTF/GLB
    fn load_gltf(path: &Path) -> Result<Self, String> {
        let file = std::fs::read(path).map_err(|e| format!("Falha ao ler arquivo: {}", e))?;
        let document =
            gltf::Gltf::from_slice(&file).map_err(|e| format!("Falha ao parsear GLTF: {}", e))?;

        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let albedo_texture_path: Option<PathBuf> = None;

        // Carrega dados dos buffers
        let mut buffer_data = Vec::new();
        for buffer in document.buffers() {
            let start = buffer.index() * buffer.length();
            let end = start + buffer.length();
            if end <= file.len() {
                buffer_data.push(file[start..end].to_vec());
            } else {
                buffer_data.push(Vec::new());
            }
        }

        for mesh in document.meshes() {
            for primitive in mesh.primitives() {
                let reader =
                    primitive.reader(|buffer| Some(buffer_data[buffer.index()].as_slice()));

                let base_vertex = vertices.len();

                if let Some(positions) = reader.read_positions() {
                    for position in positions {
                        vertices.push(Vertex {
                            position: Vec3::new(position[0], position[1], position[2]),
                            normal: Vec3::ZERO,
                            texcoord: Vec2::ZERO,
                        });
                    }
                }

                if let Some(normals) = reader.read_normals() {
                    for (i, normal) in normals.enumerate() {
                        let idx = base_vertex + i;
                        if idx < vertices.len() {
                            vertices[idx].normal = Vec3::new(normal[0], normal[1], normal[2]);
                        }
                    }
                }

                if let Some(texcoords) = reader.read_tex_coords(0) {
                    for (i, texcoord) in texcoords.into_f32().enumerate() {
                        let idx = base_vertex + i;
                        if idx < vertices.len() {
                            vertices[idx].texcoord = Vec2::new(texcoord[0], texcoord[1]);
                        }
                    }
                }

                if let Some(indices_data) = reader.read_indices() {
                    for idx in indices_data.into_u32() {
                        indices.push(idx);
                    }
                }
            }
        }

        let mut mesh = Self {
            name: path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            vertices,
            indices,
            albedo_texture_path,
        };

        mesh.ensure_normals();
        Ok(mesh)
    }

    /// Garante que todos os vértices tenham normais válidas.
    /// Se alguma normal for zero, calcula por face (flat) e acumula (smooth).
    pub fn ensure_normals(&mut self) {
        let has_zero_normals = self
            .vertices
            .iter()
            .any(|v| v.normal.length_squared() < 1e-6);

        if !has_zero_normals {
            return;
        }

        // Zera todas as normais e recalcula
        for v in &mut self.vertices {
            v.normal = Vec3::ZERO;
        }

        // Acumula normais por face em cada vértice (smooth shading)
        for tri in self.indices.chunks(3) {
            if tri.len() < 3 {
                continue;
            }
            let i0 = tri[0] as usize;
            let i1 = tri[1] as usize;
            let i2 = tri[2] as usize;
            if i0 >= self.vertices.len() || i1 >= self.vertices.len() || i2 >= self.vertices.len() {
                continue;
            }
            let p0 = self.vertices[i0].position;
            let p1 = self.vertices[i1].position;
            let p2 = self.vertices[i2].position;
            let edge1 = p1 - p0;
            let edge2 = p2 - p0;
            let face_normal = edge1.cross(edge2);
            // Não normaliza aqui — a magnitude pondera pela área do triângulo
            self.vertices[i0].normal += face_normal;
            self.vertices[i1].normal += face_normal;
            self.vertices[i2].normal += face_normal;
        }

        // Normaliza todas
        for v in &mut self.vertices {
            let len = v.normal.length();
            if len > 1e-6 {
                v.normal /= len;
            } else {
                v.normal = Vec3::Y; // fallback
            }
        }
    }

    /// Cria mesh de cubo
    pub fn cube() -> Self {
        let positions = [
            // Front face
            [-0.5, -0.5, 0.5],
            [0.5, -0.5, 0.5],
            [0.5, 0.5, 0.5],
            [-0.5, 0.5, 0.5],
            // Back face
            [-0.5, -0.5, -0.5],
            [-0.5, 0.5, -0.5],
            [0.5, 0.5, -0.5],
            [0.5, -0.5, -0.5],
            // Top face
            [-0.5, 0.5, -0.5],
            [-0.5, 0.5, 0.5],
            [0.5, 0.5, 0.5],
            [0.5, 0.5, -0.5],
            // Bottom face
            [-0.5, -0.5, -0.5],
            [0.5, -0.5, -0.5],
            [0.5, -0.5, 0.5],
            [-0.5, -0.5, 0.5],
            // Right face
            [0.5, -0.5, -0.5],
            [0.5, 0.5, -0.5],
            [0.5, 0.5, 0.5],
            [0.5, -0.5, 0.5],
            // Left face
            [-0.5, -0.5, -0.5],
            [-0.5, -0.5, 0.5],
            [-0.5, 0.5, 0.5],
            [-0.5, 0.5, -0.5],
        ];

        let normals = [
            [0.0, 0.0, 1.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, -1.0],
            [0.0, 0.0, -1.0],
            [0.0, 0.0, -1.0],
            [0.0, 0.0, -1.0],
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.0, -1.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0],
        ];

        let texcoords = [
            [0.0, 0.0],
            [1.0, 0.0],
            [1.0, 1.0],
            [0.0, 1.0],
            [0.0, 0.0],
            [1.0, 0.0],
            [1.0, 1.0],
            [0.0, 1.0],
            [0.0, 0.0],
            [1.0, 0.0],
            [1.0, 1.0],
            [0.0, 1.0],
            [0.0, 0.0],
            [1.0, 0.0],
            [1.0, 1.0],
            [0.0, 1.0],
            [0.0, 0.0],
            [1.0, 0.0],
            [1.0, 1.0],
            [0.0, 1.0],
            [0.0, 0.0],
            [1.0, 0.0],
            [1.0, 1.0],
            [0.0, 1.0],
        ];

        let indices = [
            0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7, 8, 9, 10, 8, 10, 11, 12, 13, 14, 12, 14, 15, 16,
            17, 18, 16, 18, 19, 20, 21, 22, 20, 22, 23,
        ];

        let mut vertices = Vec::new();
        for i in 0..24 {
            vertices.push(Vertex {
                position: Vec3::new(positions[i][0], positions[i][1], positions[i][2]),
                normal: Vec3::new(normals[i][0], normals[i][1], normals[i][2]),
                texcoord: Vec2::new(texcoords[i][0], texcoords[i][1]),
            });
        }

        Self {
            name: "Cube".to_string(),
            vertices,
            indices: indices.to_vec(),
            albedo_texture_path: None,
        }
    }

    /// Cria mesh de esfera
    pub fn sphere(segments: u32) -> Self {
        let rings = segments;
        let sectors = segments;

        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        let r_step = std::f32::consts::PI / rings as f32;
        let t_step = 2.0 * std::f32::consts::PI / sectors as f32;

        for i in 0..=rings {
            let phi = i as f32 * r_step;
            let sin_phi = phi.sin();
            let cos_phi = phi.cos();

            for j in 0..=sectors {
                let theta = j as f32 * t_step;
                let sin_theta = theta.sin();
                let cos_theta = theta.cos();

                let x = cos_theta * sin_phi;
                let y = cos_phi;
                let z = sin_theta * sin_phi;

                let u = 1.0 - j as f32 / sectors as f32;
                let v = 1.0 - i as f32 / rings as f32;

                vertices.push(Vertex {
                    position: Vec3::new(x * 0.5, y * 0.5, z * 0.5),
                    normal: Vec3::new(x, y, z).normalize(),
                    texcoord: Vec2::new(u, v),
                });
            }
        }

        for i in 0..rings {
            for j in 0..sectors {
                let first = i * (sectors + 1) + j;
                let second = first + sectors + 1;

                indices.push(first);
                indices.push(second);
                indices.push(first + 1);

                indices.push(second);
                indices.push(second + 1);
                indices.push(first + 1);
            }
        }

        Self {
            name: "Sphere".to_string(),
            vertices,
            indices,
            albedo_texture_path: None,
        }
    }

    /// Cria mesh de plano
    pub fn plane() -> Self {
        let positions = [
            [-0.5, 0.0, -0.5],
            [0.5, 0.0, -0.5],
            [0.5, 0.0, 0.5],
            [-0.5, 0.0, 0.5],
        ];

        let normals = [
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ];

        let texcoords = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];

        let indices = [0, 2, 1, 0, 3, 2];

        let mut vertices = Vec::new();
        for i in 0..4 {
            vertices.push(Vertex {
                position: Vec3::new(positions[i][0], positions[i][1], positions[i][2]),
                normal: Vec3::new(normals[i][0], normals[i][1], normals[i][2]),
                texcoord: Vec2::new(texcoords[i][0], texcoords[i][1]),
            });
        }

        Self {
            name: "Plane".to_string(),
            vertices,
            indices: indices.to_vec(),
            albedo_texture_path: None,
        }
    }

    /// Quantidade de vértices
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// Quantidade de índices
    pub fn index_count(&self) -> usize {
        self.indices.len()
    }

    /// Verifica se o mesh é válido
    pub fn is_valid(&self) -> bool {
        !self.vertices.is_empty() && !self.indices.is_empty()
    }
}

/// Calcula normais por face a partir de posições e triângulos (flat shading).
/// Retorna um Vec de normais, uma por posição.
pub fn compute_flat_normals(positions: &[Vec3], triangles: &[[u32; 3]]) -> Vec<Vec3> {
    let mut normals = vec![Vec3::ZERO; positions.len()];

    for tri in triangles {
        let i0 = tri[0] as usize;
        let i1 = tri[1] as usize;
        let i2 = tri[2] as usize;
        if i0 >= positions.len() || i1 >= positions.len() || i2 >= positions.len() {
            continue;
        }
        let edge1 = positions[i1] - positions[i0];
        let edge2 = positions[i2] - positions[i0];
        let face_normal = edge1.cross(edge2);
        normals[i0] += face_normal;
        normals[i1] += face_normal;
        normals[i2] += face_normal;
    }

    for n in &mut normals {
        let len = n.length();
        if len > 1e-6 {
            *n /= len;
        } else {
            *n = Vec3::Y;
        }
    }

    normals
}
