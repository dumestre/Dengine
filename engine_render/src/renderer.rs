//! Renderer - renders the ECS world to a texture
//!
//! The renderer takes the ECS world and produces a texture that can be displayed by the editor.

use engine_core::ecs::EngineWorld;
use engine_core::systems::{CameraSystem, RenderSystem, Renderable};

use crate::asset_manager::AssetManager;
use crate::mesh::MeshData;

/// Render pass configuration
pub struct RenderConfig {
    pub clear_color: [f32; 4],
    pub width: u32,
    pub height: u32,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            clear_color: [0.1, 0.1, 0.1, 1.0],
            width: 1920,
            height: 1080,
        }
    }
}

/// Renderer - produces rendered output
pub struct Renderer {
    camera: CameraSystem,
    asset_manager: AssetManager,
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new(RenderConfig::default())
    }
}

impl Renderer {
    pub fn new(_config: RenderConfig) -> Self {
        let mut asset_manager = AssetManager::new();

        // Pre-create default meshes
        let _cube = asset_manager.create_cube();
        let _sphere = asset_manager.create_sphere(32);
        let _plane = asset_manager.create_plane();

        Self {
            camera: CameraSystem::default(),
            asset_manager,
        }
    }

    /// Get the asset manager
    pub fn asset_manager(&mut self) -> &mut AssetManager {
        &mut self.asset_manager
    }

    /// Get the camera system
    pub fn camera(&mut self) -> &mut CameraSystem {
        &mut self.camera
    }

    /// Set camera position
    pub fn set_camera_position(&mut self, x: f32, y: f32, z: f32) {
        self.camera.set_position(glam::Vec3::new(x, y, z));
    }

    /// Set camera target
    pub fn set_camera_target(&mut self, x: f32, y: f32, z: f32) {
        self.camera.look_at(glam::Vec3::new(x, y, z));
    }

    /// Update camera aspect ratio (call on resize)
    pub fn set_viewport_size(&mut self, width: u32, height: u32) {
        if height > 0 {
            self.camera.set_aspect_ratio(width as f32 / height as f32);
        }
    }

    /// Load a mesh and return handle
    pub fn load_mesh(
        &mut self,
        path: &std::path::Path,
    ) -> Result<engine_core::components::MeshHandle, String> {
        self.asset_manager.load_mesh(path)
    }

    /// Create a cube mesh and return handle
    pub fn create_cube(&mut self) -> engine_core::components::MeshHandle {
        self.asset_manager.create_cube()
    }

    /// Get mesh data
    pub fn get_mesh(&self, handle: engine_core::components::MeshHandle) -> Option<&MeshData> {
        self.asset_manager.get_mesh(handle)
    }

    /// Render the world and return renderables for external rendering
    pub fn render(&mut self, world: &EngineWorld) -> RenderOutput {
        let mut render_system = RenderSystem;
        let renderables = render_system.update(world);

        RenderOutput {
            renderables,
            view_projection: self.camera.view_projection(),
            camera_position: self.camera.position,
        }
    }
}

/// Output from render call - contains all data needed for actual GPU rendering
#[derive(Debug)]
pub struct RenderOutput {
    pub renderables: Vec<Renderable>,
    pub view_projection: glam::Mat4,
    pub camera_position: glam::Vec3,
}

/// Extension for AssetManager to add plane creation
trait AssetManagerExt {
    fn create_plane(&mut self) -> engine_core::components::MeshHandle;
}

impl AssetManagerExt for AssetManager {
    fn create_plane(&mut self) -> engine_core::components::MeshHandle {
        let mesh_data = MeshData::plane();
        let id = self.next_mesh_id;
        self.next_mesh_id += 1;
        self.meshes.insert(id, mesh_data);
        engine_core::components::MeshHandle { id }
    }
}
