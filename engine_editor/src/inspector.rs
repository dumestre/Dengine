//! Inspector - shows and edits entity properties
//!
//! Displays and allows editing of components on the selected entity.

use egui::{Align2, Color32, DragValue, FontId, Pos2, Rect, Ui, Vec2};

use engine_core::components::Transform;
use engine_core::ecs::EntityHandle;

/// Inspector state for the editor
pub struct InspectorEditor {
    pub is_open: bool,
}

impl Default for InspectorEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl InspectorEditor {
    pub fn new() -> Self {
        Self { is_open: true }
    }

    /// Render the inspector UI
    pub fn show(
        &mut self,
        ui: &mut Ui,
        selected_entity: Option<EntityHandle>,
        world: &mut engine_core::ecs::EngineWorld,
    ) {
        // Draw inspector panel header
        let available = ui.available_rect_before_wrap();
        let header_height = 32.0;

        let header_rect = Rect::from_min_size(
            Pos2::new(available.left(), available.top()),
            Vec2::new(available.width(), header_height),
        );

        ui.painter()
            .rect_filled(header_rect, 0.0, Color32::from_rgb(35, 35, 38));

        ui.painter().text(
            header_rect.center(),
            Align2::CENTER_CENTER,
            "Inspector",
            FontId::proportional(14.0),
            Color32::from_gray(200),
        );

        ui.add_space(header_height + 8.0);

        // Show entity properties if selected
        if let Some(entity) = selected_entity {
            self.show_entity_properties(ui, entity, world);
        } else {
            ui.label("No entity selected");
        }
    }

    fn show_entity_properties(
        &mut self,
        ui: &mut Ui,
        entity: EntityHandle,
        world: &mut engine_core::ecs::EngineWorld,
    ) {
        // Get transform component
        if let Some(transform) = world.get::<Transform>(entity) {
            egui::CollapsingHeader::new("Transform")
                .default_open(true)
                .show(ui, |ui| {
                    // Position
                    ui.label("Position");
                    let mut pos = [
                        transform.position.x,
                        transform.position.y,
                        transform.position.z,
                    ];
                    ui.columns(3, |cols| {
                        cols[0].add(DragValue::new(&mut pos[0]).prefix("X"));
                        cols[1].add(DragValue::new(&mut pos[1]).prefix("Y"));
                        cols[2].add(DragValue::new(&mut pos[2]).prefix("Z"));
                    });

                    // Scale
                    ui.label("Scale");
                    let mut scale = [transform.scale.x, transform.scale.y, transform.scale.z];
                    ui.columns(3, |cols| {
                        cols[0].add(DragValue::new(&mut scale[0]).prefix("X"));
                        cols[1].add(DragValue::new(&mut scale[1]).prefix("Y"));
                        cols[2].add(DragValue::new(&mut scale[2]).prefix("Z"));
                    });
                });
        }

        // Show other components...
        ui.label("");
        ui.label("Components:");
        ui.label("(MeshRenderer, Light, etc. - coming soon)");
    }
}
