//! Core systems for the ECS-based engine

use glam::{Mat4, Vec3};

use crate::components::*;
use crate::ecs::*;

/// Movement system - applies velocity-based movement to entities
pub struct MovementSystem {
    pub speed: f32,
}

impl Default for MovementSystem {
    fn default() -> Self {
        Self { speed: 5.0 }
    }
}

impl MovementSystem {
    pub fn new(speed: f32) -> Self {
        Self { speed }
    }

    pub fn update(&mut self, world: &mut EngineWorld, dt: f32) {
        // Iterate over entities with Transform and Player components
        for (transform, _player) in &mut world.world_mut().query::<(&mut Transform, &Player)>() {
            // Player movement logic - can be extended with input
            let _ = transform;
            let _ = dt;
        }
    }

    /// Move entity by direction vector
    pub fn move_entity(world: &mut EngineWorld, direction: Vec3, speed: f32, dt: f32) {
        for (transform, _player) in &mut world.world_mut().query::<(&mut Transform, &Player)>() {
            transform.position += direction * speed * dt;
        }
    }
}

/// Transform system - updates transform matrices
pub struct TransformSystem;

impl TransformSystem {
    pub fn update(&mut self, _world: &mut EngineWorld, _dt: f32) {
        // Transform updates are typically handled during rendering
    }
}

/// Render system - collects renderable entities
pub struct RenderSystem;

impl RenderSystem {
    pub fn update(&mut self, world: &EngineWorld) -> Vec<Renderable> {
        let mut renderables = Vec::new();

        for (transform, mesh_renderer) in &mut world.world().query::<(&Transform, &MeshRenderer)>()
        {
            let mesh_renderer: &MeshRenderer = mesh_renderer;
            if mesh_renderer.is_valid() {
                renderables.push(Renderable {
                    transform: *transform,
                    mesh: mesh_renderer.mesh,
                    material: mesh_renderer.material,
                });
            }
        }

        renderables
    }
}

/// Renderable data for the renderer
#[derive(Debug, Clone, Copy)]
pub struct Renderable {
    pub transform: Transform,
    pub mesh: MeshHandle,
    pub material: MaterialHandle,
}

/// Camera system - manages camera view and projection
pub struct CameraSystem {
    pub position: Vec3,
    pub target: Vec3,
    pub up: Vec3,
    pub fov: f32,
    pub aspect_ratio: f32,
    pub near: f32,
    pub far: f32,
}

impl Default for CameraSystem {
    fn default() -> Self {
        Self {
            position: Vec3::new(0.0, 5.0, 10.0),
            target: Vec3::ZERO,
            up: Vec3::Y,
            fov: 45.0,
            aspect_ratio: 16.0 / 9.0,
            near: 0.1,
            far: 100.0,
        }
    }
}

impl CameraSystem {
    pub fn new(fov: f32, aspect_ratio: f32) -> Self {
        let mut camera = Self::default();
        camera.fov = fov;
        camera.aspect_ratio = aspect_ratio;
        camera
    }

    /// Get view matrix
    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(self.position, self.target, self.up)
    }

    /// Get projection matrix (perspective)
    pub fn projection_matrix(&self) -> Mat4 {
        Mat4::perspective_rh_gl(
            self.fov.to_radians(),
            self.aspect_ratio,
            self.near,
            self.far,
        )
    }

    /// Get view-projection matrix
    pub fn view_projection(&self) -> Mat4 {
        self.projection_matrix() * self.view_matrix()
    }

    /// Move camera target
    pub fn look_at(&mut self, target: Vec3) {
        self.target = target;
    }

    /// Move camera position
    pub fn set_position(&mut self, position: Vec3) {
        self.position = position;
    }

    /// Orbit camera around target
    pub fn orbit(&mut self, yaw: f32, pitch: f32, distance: f32) {
        let x = yaw.cos() * pitch.cos() * distance;
        let y = pitch.sin() * distance;
        let z = yaw.sin() * pitch.cos() * distance;
        self.position = self.target + Vec3::new(x, y, z);
    }

    /// Update aspect ratio (e.g., on window resize)
    pub fn set_aspect_ratio(&mut self, aspect_ratio: f32) {
        self.aspect_ratio = aspect_ratio;
    }
}

/// Tag for entities that should be culled
#[derive(Debug, Clone, Copy, Default)]
pub struct Cullable {
    pub bounding_radius: f32,
}

impl Cullable {
    pub fn new(radius: f32) -> Self {
        Self {
            bounding_radius: radius,
        }
    }
}
