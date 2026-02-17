//! Engine Editor - Egui-based editor interface
//! 
//! This module provides the editor UI using egui.
//! It does NOT contain rendering logic - it only displays the rendered texture.

pub mod viewport;
pub mod inspector;
pub mod hierarchy;

pub use viewport::*;
pub use inspector::*;
pub use hierarchy::*;
