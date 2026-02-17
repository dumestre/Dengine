//! Viewport - displays the rendered scene
//!
//! The viewport shows the rendered texture from the engine.
//! It handles camera controls and object selection.

use egui::{Align2, Color32, FontId, Pos2, Rect, Stroke, TextureHandle, Ui, Vec2};

use engine_core::ecs::EngineWorld;
use engine_render::Renderer;

/// Viewport state for the editor
pub struct ViewportEditor {
    pub texture: Option<TextureHandle>,
    pub selected_entity: Option<u64>,
    pub hovered_entity: Option<u64>,
}

impl Default for ViewportEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl ViewportEditor {
    pub fn new() -> Self {
        Self {
            texture: None,
            selected_entity: None,
            hovered_entity: None,
        }
    }

    /// Render the viewport UI
    pub fn show(
        &mut self,
        ui: &mut Ui,
        _renderer: &Renderer,
        _world: &EngineWorld,
        left_reserved: f32,
        right_reserved: f32,
        bottom_reserved: f32,
    ) {
        // Get available space
        let available = ui.available_rect_before_wrap();

        let viewport_rect = Rect::from_min_size(
            Pos2::new(available.left() + left_reserved, available.top()),
            Vec2::new(
                available.width() - left_reserved - right_reserved,
                available.height() - bottom_reserved,
            ),
        );

        if viewport_rect.width() < 50.0 || viewport_rect.height() < 50.0 {
            return;
        }

        // Draw background
        ui.painter()
            .rect_filled(viewport_rect, 0.0, Color32::from_rgb(22, 22, 24));

        // Draw border
        ui.painter().rect_stroke(
            viewport_rect,
            0.0,
            Stroke::new(1.0, Color32::from_rgb(58, 58, 62)),
            egui::StrokeKind::Middle,
        );

        // Draw viewport label
        ui.painter().text(
            Pos2::new(viewport_rect.left() + 12.0, viewport_rect.top() + 10.0),
            Align2::LEFT_TOP,
            "Viewport - ECS",
            FontId::proportional(13.0),
            Color32::from_gray(210),
        );

        // Draw sample content (placeholder until full integration)
        self.draw_sample_content(ui, &viewport_rect);

        // Draw grid
        self.draw_grid(ui, &viewport_rect);
    }

    fn draw_sample_content(&self, ui: &Ui, rect: &Rect) {
        // This would display the actual rendered texture
        // For now, show a placeholder with scene info
        let center = rect.center();

        ui.painter().text(
            Pos2::new(center.x, center.y - 20.0),
            Align2::CENTER_CENTER,
            "ECS Viewport",
            FontId::proportional(18.0),
            Color32::from_gray(180),
        );

        ui.painter().text(
            Pos2::new(center.x, center.y + 10.0),
            Align2::CENTER_CENTER,
            "(Render texture will appear here)",
            FontId::proportional(12.0),
            Color32::from_gray(120),
        );
    }

    fn draw_grid(&self, ui: &Ui, rect: &Rect) {
        let grid_step = 24.0;
        let mut x = rect.left();

        while x <= rect.right() {
            ui.painter().line_segment(
                [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                Stroke::new(1.0, Color32::from_rgba_unmultiplied(86, 86, 92, 24)),
            );
            x += grid_step;
        }

        let mut y = rect.top();
        while y <= rect.bottom() {
            ui.painter().line_segment(
                [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                Stroke::new(1.0, Color32::from_rgba_unmultiplied(86, 86, 92, 24)),
            );
            y += grid_step;
        }
    }

    /// Handle viewport input (camera controls, selection, etc.)
    pub fn handle_input(&mut self, _ui: &mut Ui, _renderer: &Renderer, _world: &mut EngineWorld) {
        // TODO: Implement camera orbit, pan, zoom
        // TODO: Implement object picking and selection
    }

    /// Set the render texture
    pub fn set_texture(&mut self, texture: TextureHandle) {
        self.texture = Some(texture);
    }

    /// Clear the render texture
    pub fn clear_texture(&mut self) {
        self.texture = None;
    }

    /// Get selected entity
    pub fn selected_entity(&self) -> Option<u64> {
        self.selected_entity
    }

    /// Set selected entity
    pub fn set_selected_entity(&mut self, entity: Option<u64>) {
        self.selected_entity = entity;
    }
}
