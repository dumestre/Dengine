//! Core components for the ECS-based engine

use glam::{Mat4, Quat, Vec3};

/// Transform component - position, rotation, and scale of an entity
#[derive(Debug, Clone, Copy)]
pub struct Transform {
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

impl Transform {
    pub fn new(position: Vec3, rotation: Quat, scale: Vec3) -> Self {
        Self {
            position,
            rotation,
            scale,
        }
    }

    pub fn from_position(position: Vec3) -> Self {
        Self {
            position,
            ..Default::default()
        }
    }

    pub fn from_translation(x: f32, y: f32, z: f32) -> Self {
        Self::from_position(Vec3::new(x, y, z))
    }

    /// Get the model matrix (local transform)
    pub fn to_mat4(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.position)
    }

    /// Get forward direction (negative Z in local space)
    pub fn forward(&self) -> Vec3 {
        self.rotation * -Vec3::Z
    }

    /// Get right direction (positive X in local space)
    pub fn right(&self) -> Vec3 {
        self.rotation * Vec3::X
    }

    /// Get up direction (positive Y in local space)
    pub fn up(&self) -> Vec3 {
        self.rotation * Vec3::Y
    }

    /// Translate by delta
    pub fn translate(&mut self, delta: Vec3) {
        self.position += delta;
    }

    /// Rotate by quat
    pub fn rotate(&mut self, rotation: Quat) {
        self.rotation = rotation * self.rotation;
    }

    /// Scale by delta
    pub fn scale_by(&mut self, scale: Vec3) {
        self.scale *= scale;
    }
}

/// Handle to a mesh asset - used instead of direct mesh storage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MeshHandle {
    pub id: u64,
}

impl MeshHandle {
    pub fn invalid() -> Self {
        Self { id: 0 }
    }

    pub fn is_valid(&self) -> bool {
        self.id != 0
    }
}

impl Default for MeshHandle {
    fn default() -> Self {
        Self::invalid()
    }
}

/// Handle to a material asset
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MaterialHandle {
    pub id: u64,
}

impl MaterialHandle {
    pub fn invalid() -> Self {
        Self { id: 0 }
    }

    pub fn is_valid(&self) -> bool {
        self.id != 0
    }
}

impl Default for MaterialHandle {
    fn default() -> Self {
        Self::invalid()
    }
}

/// Mesh renderer component - references mesh and material assets
#[derive(Debug, Clone, Copy)]
pub struct MeshRenderer {
    pub mesh: MeshHandle,
    pub material: MaterialHandle,
}

impl Default for MeshRenderer {
    fn default() -> Self {
        Self {
            mesh: MeshHandle::invalid(),
            material: MaterialHandle::invalid(),
        }
    }
}

impl MeshRenderer {
    pub fn new(mesh: MeshHandle) -> Self {
        Self {
            mesh,
            material: MaterialHandle::invalid(),
        }
    }

    pub fn with_material(mesh: MeshHandle, material: MaterialHandle) -> Self {
        Self { mesh, material }
    }

    pub fn is_valid(&self) -> bool {
        self.mesh.is_valid()
    }
}

/// Tag component for camera entities
#[derive(Debug, Clone, Copy, Default)]
pub struct Camera;

/// Tag component for light entities
#[derive(Debug, Clone, Copy)]
pub struct Light {
    pub color: Vec3,
    pub intensity: f32,
}

impl Default for Light {
    fn default() -> Self {
        Self {
            color: Vec3::ONE,
            intensity: 1.0,
        }
    }
}

impl Light {
    pub fn new(color: Vec3, intensity: f32) -> Self {
        Self { color, intensity }
    }

    pub fn directional() -> Self {
        Self {
            color: Vec3::ONE,
            intensity: 1.0,
        }
    }
}

/// Tag component for player-controlled entities
#[derive(Debug, Clone, Copy, Default)]
pub struct Player;
