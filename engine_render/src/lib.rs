//! Engine Render - Subsistema de renderização
//!
//! Este módulo gerencia assets, materiais, shaders e dados de mesh.

pub mod asset_manager;
pub mod mesh;
pub mod renderer;
pub mod shader;

pub use asset_manager::*;
pub use mesh::*;
pub use renderer::*;
pub use shader::*;
