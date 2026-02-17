//! Asset Manager with Handle-based resource loading
//!
//! All assets are accessed through handles, never directly stored in components.

use std::collections::HashMap;
use std::path::Path;

use engine_core::components::{MaterialHandle, MeshHandle};

use crate::mesh::MeshData;

/// Asset Manager - handles loading and storing of engine assets
///
/// All assets are stored internally and accessed via handles.
/// This allows for efficient resource sharing and lifetime management.
pub struct AssetManager {
    pub meshes: HashMap<u64, MeshData>,
    pub materials: HashMap<u64, MaterialData>,
    pub next_mesh_id: u64,
    pub next_material_id: u64,
}

/// Material data
#[derive(Debug, Clone)]
pub struct MaterialData {
    pub name: String,
    pub albedo: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub albedo_texture: Option<String>,
}

impl Default for MaterialData {
    fn default() -> Self {
        Self {
            name: "Default".to_string(),
            albedo: [1.0, 1.0, 1.0, 1.0],
            metallic: 0.0,
            roughness: 0.5,
            albedo_texture: None,
        }
    }
}

impl MaterialData {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            ..Default::default()
        }
    }

    pub fn with_color(mut self, r: f32, g: f32, b: f32) -> Self {
        self.albedo = [r, g, b, 1.0];
        self
    }
}

impl Default for AssetManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetManager {
    pub fn new() -> Self {
        Self {
            meshes: HashMap::new(),
            materials: HashMap::new(),
            next_mesh_id: 1,
            next_material_id: 1,
        }
    }

    /// Load a mesh from file path
    pub fn load_mesh(&mut self, path: &Path) -> Result<MeshHandle, String> {
        // Check if already loaded
        for (id, mesh) in &self.meshes {
            if mesh.name == path.to_string_lossy() {
                return Ok(MeshHandle { id: *id });
            }
        }

        // Load mesh data
        let mesh_data = MeshData::load_from_file(path)?;

        // Create material with texture if available
        let _material_handle = if let Some(texture_path) = &mesh_data.albedo_texture_path {
            if texture_path.exists() {
                let material = MaterialData {
                    name: format!("{}_material", mesh_data.name),
                    albedo: [1.0, 1.0, 1.0, 1.0],
                    metallic: 0.5,
                    roughness: 0.5,
                    albedo_texture: Some(texture_path.to_string_lossy().to_string()),
                };
                let id = self.next_material_id;
                self.next_material_id += 1;
                self.materials.insert(id, material);
                Some(MaterialHandle { id })
            } else {
                None
            }
        } else {
            None
        };

        let id = self.next_mesh_id;
        self.next_mesh_id += 1;

        self.meshes.insert(id, mesh_data);

        Ok(MeshHandle { id })
    }

    /// Load a cube mesh
    pub fn create_cube(&mut self) -> MeshHandle {
        let mesh_data = MeshData::cube();
        let id = self.next_mesh_id;
        self.next_mesh_id += 1;
        self.meshes.insert(id, mesh_data);
        MeshHandle { id }
    }

    /// Load a sphere mesh
    pub fn create_sphere(&mut self, segments: u32) -> MeshHandle {
        let mesh_data = MeshData::sphere(segments);
        let id = self.next_mesh_id;
        self.next_mesh_id += 1;
        self.meshes.insert(id, mesh_data);
        MeshHandle { id }
    }

    /// Get mesh data by handle
    pub fn get_mesh(&self, handle: MeshHandle) -> Option<&MeshData> {
        self.meshes.get(&handle.id)
    }

    /// Get mesh data by handle (mutable)
    pub fn get_mesh_mut(&mut self, handle: MeshHandle) -> Option<&mut MeshData> {
        self.meshes.get_mut(&handle.id)
    }

    /// Check if mesh handle is valid
    pub fn is_mesh_valid(&self, handle: MeshHandle) -> bool {
        self.meshes.contains_key(&handle.id)
    }

    /// Unload mesh by handle
    pub fn unload_mesh(&mut self, handle: MeshHandle) -> bool {
        self.meshes.remove(&handle.id).is_some()
    }

    /// Create a material
    pub fn create_material(&mut self, name: &str) -> MaterialHandle {
        let material = MaterialData::new(name);
        let id = self.next_material_id;
        self.next_material_id += 1;
        self.materials.insert(id, material);
        MaterialHandle { id }
    }

    /// Get material data by handle
    pub fn get_material(&self, handle: MaterialHandle) -> Option<&MaterialData> {
        self.materials.get(&handle.id)
    }

    /// Get material data by handle (mutable)
    pub fn get_material_mut(&mut self, handle: MaterialHandle) -> Option<&mut MaterialData> {
        self.materials.get_mut(&handle.id)
    }

    /// Check if material handle is valid
    pub fn is_material_valid(&self, handle: MaterialHandle) -> bool {
        self.materials.contains_key(&handle.id)
    }

    /// Unload material by handle
    pub fn unload_material(&mut self, handle: MaterialHandle) -> bool {
        self.materials.remove(&handle.id).is_some()
    }

    /// Get all mesh handles (for iteration)
    pub fn mesh_handles(&self) -> Vec<MeshHandle> {
        self.meshes
            .keys()
            .map(|id| MeshHandle { id: *id })
            .collect()
    }

    /// Get mesh count
    pub fn mesh_count(&self) -> usize {
        self.meshes.len()
    }

    /// Get material count
    pub fn material_count(&self) -> usize {
        self.materials.len()
    }

    /// Clear all assets
    pub fn clear(&mut self) {
        self.meshes.clear();
        self.materials.clear();
        self.next_mesh_id = 1;
        self.next_material_id = 1;
    }
}

/// Extension trait for MeshHandle to allow loading
pub trait MeshHandleExt {
    fn load_from_file(manager: &mut AssetManager, path: &Path) -> Result<Self, String>
    where
        Self: Sized;
}

impl MeshHandleExt for MeshHandle {
    fn load_from_file(manager: &mut AssetManager, path: &Path) -> Result<Self, String> {
        manager.load_mesh(path)
    }
}
