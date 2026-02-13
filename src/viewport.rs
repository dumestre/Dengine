use eframe::egui::{self, Align2, Color32, FontId, Rect, Stroke};
use egui_gizmo::{Gizmo, GizmoMode, GizmoOrientation};
use glam::{Mat4, Vec3};

pub struct ViewportPanel {
    is_3d: bool,
    is_ortho: bool,
    gizmo_mode: GizmoMode,
    gizmo_orientation: GizmoOrientation,
    model_matrix: Mat4,
}

impl ViewportPanel {
    pub fn new() -> Self {
        Self {
            is_3d: true,
            is_ortho: false,
            gizmo_mode: GizmoMode::Translate,
            gizmo_orientation: GizmoOrientation::Global,
            model_matrix: Mat4::IDENTITY,
        }
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        mode_label: &str,
        left_reserved: f32,
        right_reserved: f32,
        bottom_reserved: f32,
    ) {
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(28, 28, 30))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(48, 48, 52))),
            )
            .show(ctx, |ui| {
                let content = ui.max_rect();
                let viewport_rect = Rect::from_min_max(
                    egui::pos2(content.left() + left_reserved, content.top()),
                    egui::pos2(content.right() - right_reserved, content.bottom() - bottom_reserved),
                );
                if viewport_rect.width() < 80.0 || viewport_rect.height() < 80.0 {
                    return;
                }

                ui.painter()
                    .rect_filled(viewport_rect, 0.0, Color32::from_rgb(22, 22, 24));
                ui.painter().rect_stroke(
                    viewport_rect,
                    0.0,
                    Stroke::new(1.0, Color32::from_rgb(58, 58, 62)),
                    egui::StrokeKind::Outside,
                );

                let grid_step = 24.0;
                let mut x = viewport_rect.left();
                while x <= viewport_rect.right() {
                    ui.painter().line_segment(
                        [egui::pos2(x, viewport_rect.top()), egui::pos2(x, viewport_rect.bottom())],
                        Stroke::new(1.0, Color32::from_rgba_unmultiplied(86, 86, 92, 24)),
                    );
                    x += grid_step;
                }
                let mut y = viewport_rect.top();
                while y <= viewport_rect.bottom() {
                    ui.painter().line_segment(
                        [egui::pos2(viewport_rect.left(), y), egui::pos2(viewport_rect.right(), y)],
                        Stroke::new(1.0, Color32::from_rgba_unmultiplied(86, 86, 92, 24)),
                    );
                    y += grid_step;
                }

                ui.painter().text(
                    egui::pos2(viewport_rect.left() + 12.0, viewport_rect.top() + 10.0),
                    Align2::LEFT_TOP,
                    format!("Viewport - {}", mode_label),
                    FontId::proportional(13.0),
                    Color32::from_gray(210),
                );

                let controls_rect = Rect::from_min_max(
                    egui::pos2(viewport_rect.right() - 334.0, viewport_rect.top() + 6.0),
                    egui::pos2(viewport_rect.right() - 8.0, viewport_rect.top() + 32.0),
                );
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(controls_rect)
                        .layout(egui::Layout::right_to_left(egui::Align::Center)),
                    |ui| {
                        let proj_label = if self.is_ortho { "Ortho" } else { "Persp" };
                        if ui
                            .add_sized([74.0, 22.0], egui::Button::new(proj_label).corner_radius(6))
                            .clicked()
                            && self.is_3d
                        {
                            self.is_ortho = !self.is_ortho;
                        }
                        ui.add_space(6.0);

                        let dim_label = if self.is_3d { "3D" } else { "2D" };
                        if ui
                            .add_sized(
                                [52.0, 22.0],
                                egui::Button::new(dim_label)
                                    .corner_radius(6)
                                    .stroke(Stroke::new(1.0, Color32::from_rgb(15, 232, 121))),
                            )
                            .clicked()
                        {
                            self.is_3d = !self.is_3d;
                        }
                        ui.add_space(10.0);

                        ui.radio_value(&mut self.gizmo_mode, GizmoMode::Translate, "Move");
                        ui.radio_value(&mut self.gizmo_mode, GizmoMode::Rotate, "Rotate");
                        ui.radio_value(&mut self.gizmo_mode, GizmoMode::Scale, "Scale");
                        ui.add_space(8.0);
                        ui.radio_value(&mut self.gizmo_orientation, GizmoOrientation::Global, "Global");
                        ui.radio_value(&mut self.gizmo_orientation, GizmoOrientation::Local, "Local");
                    },
                );

                if self.is_3d {
                    let aspect = (viewport_rect.width() / viewport_rect.height()).max(0.1);
                    let eye = Vec3::new(2.8, 2.2, 2.8);
                    let target = Vec3::ZERO;
                    let view = Mat4::look_at_rh(eye, target, Vec3::Y);
                    let proj = if self.is_ortho {
                        Mat4::orthographic_rh_gl(-2.0 * aspect, 2.0 * aspect, -2.0, 2.0, 0.1, 50.0)
                    } else {
                        Mat4::perspective_rh_gl(45.0_f32.to_radians(), aspect, 0.1, 50.0)
                    };

                    let gizmo = Gizmo::new("scene_transform_gizmo")
                        .view_matrix(view.to_cols_array_2d().into())
                        .projection_matrix(proj.to_cols_array_2d().into())
                        .model_matrix(self.model_matrix.to_cols_array_2d().into())
                        .mode(self.gizmo_mode)
                        .orientation(self.gizmo_orientation)
                        .viewport(viewport_rect);

                    if let Some(result) = gizmo.interact(ui) {
                        self.model_matrix = Mat4::from(result.transform());
                    }
                }
            });
    }
}
