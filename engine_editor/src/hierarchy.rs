//! Hierarchy - shows the scene hierarchy
//!
//! Displays all entities in the scene as a tree.

use egui::{Align2, Color32, FontId, Pos2, Rect, Ui, Vec2};

use engine_core::components::Transform;
use engine_core::ecs::EngineWorld;

/// Hierarchy state for the editor
pub struct HierarchyEditor {
    pub selected_entity: Option<String>,
}

impl Default for HierarchyEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl HierarchyEditor {
    pub fn new() -> Self {
        Self {
            selected_entity: None,
        }
    }

    /// Render the hierarchy UI
    pub fn show(&mut self, ui: &mut Ui, world: &EngineWorld, width: f32) {
        // Draw hierarchy panel header
        let available = ui.available_rect_before_wrap();
        let header_height = 32.0;

        let header_rect = Rect::from_min_size(
            Pos2::new(available.left(), available.top()),
            Vec2::new(width, header_height),
        );

        ui.painter()
            .rect_filled(header_rect, 0.0, Color32::from_rgb(35, 35, 38));

        ui.painter().text(
            header_rect.center(),
            Align2::CENTER_CENTER,
            "Hierarchy",
            FontId::proportional(14.0),
            Color32::from_gray(200),
        );

        ui.add_space(header_height + 8.0);

        // List all entities with Transform component
        let mut entity_names: Vec<String> = Vec::new();

        for (transform,) in &mut world.world().query::<(&Transform,)>() {
            // Generate a name based on entity ID
            let _ = transform;
            let name = format!("Entity_{}", entity_names.len());
            entity_names.push(name);
        }

        if entity_names.is_empty() {
            ui.label("No entities in scene");
            ui.label("Add entities via ECS");
        } else {
            ui.label(format!("{} entities", entity_names.len()));
            ui.separator();

            for name in &entity_names {
                let is_selected = self.selected_entity.as_deref() == Some(name);

                if ui.selectable_label(is_selected, name).clicked() {
                    self.selected_entity = Some(name.clone());
                }
            }
        }
    }

    /// Get selected entity name
    pub fn selected_name(&self) -> Option<&str> {
        self.selected_entity.as_deref()
    }
}
