//! Engine Core - ECS-based game engine core
//! 
//! This module provides the core ECS functionality without any GUI dependencies.

pub mod components;
pub mod ecs;
pub mod systems;

pub use components::*;
pub use ecs::*;
pub use systems::*;
