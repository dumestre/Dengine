//! ECS World wrapper for the game engine

use hecs::World as HecsWorld;

use crate::components::*;

/// Spawned entity handle
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntityHandle {
    pub id: u64,
}

impl EntityHandle {
    pub fn invalid() -> Self {
        Self { id: u64::MAX }
    }

    pub fn is_valid(&self) -> bool {
        self.id != u64::MAX
    }
}

/// ECS World wrapper with convenience methods
#[derive(Default)]
pub struct EngineWorld {
    pub world: HecsWorld,
    next_entity_id: u64,
}

impl EngineWorld {
    pub fn new() -> Self {
        Self::default()
    }

    /// Spawn an entity with components (tuple)
    pub fn spawn(&mut self, components: impl hecs::DynamicBundle) -> EntityHandle {
        let _entity = self.world.spawn(components);
        self.next_entity_id += 1;
        EntityHandle {
            id: self.next_entity_id,
        }
    }

    /// Spawn an entity with transform
    pub fn spawn_with_transform(&mut self, transform: Transform) -> EntityHandle {
        self.spawn((transform,))
    }

    /// Spawn a mesh entity with transform and mesh
    pub fn spawn_mesh(&mut self, transform: Transform, mesh: MeshHandle) -> EntityHandle {
        let renderer = MeshRenderer::new(mesh);
        self.spawn((transform, renderer))
    }

    /// Despawn an entity by ID
    pub fn despawn(&mut self, handle: EntityHandle) -> bool {
        // Note: In hecs, you can't easily despawn by custom ID
        // This would need a mapping from handle to entity
        let _ = handle;
        false
    }

    /// Get entity count
    pub fn entity_count(&self) -> usize {
        self.world.len() as usize
    }

    /// Get mutable access to a component
    pub fn get<T: Send + Sync + 'static>(&mut self, handle: EntityHandle) -> Option<&mut T> {
        // Would need entity mapping
        let _ = handle;
        None
    }

    /// Get immutable access to a component  
    pub fn get_ref<T: Send + Sync + 'static>(&self, handle: EntityHandle) -> Option<&T> {
        // Would need entity mapping
        let _ = handle;
        None
    }

    /// Add a component to an entity
    pub fn add<T: Send + Sync + 'static>(&mut self, handle: EntityHandle, component: T) -> bool {
        let _ = handle;
        let _ = component;
        false
    }

    /// Get underlying hecs world reference
    pub fn world(&self) -> &HecsWorld {
        &self.world
    }

    /// Get mutable underlying hecs world
    pub fn world_mut(&mut self) -> &mut HecsWorld {
        &mut self.world
    }
}
