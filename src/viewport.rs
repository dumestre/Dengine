use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke};
use egui_gizmo::{Gizmo, GizmoMode, GizmoOrientation};
use glam::{Mat4, Vec3};

pub struct ViewportPanel {
    is_3d: bool,
    is_ortho: bool,
    gizmo_mode: GizmoMode,
    gizmo_orientation: GizmoOrientation,
    model_matrix: Mat4,
    camera_yaw: f32,
    camera_pitch: f32,
    camera_distance: f32,
    camera_target: Vec3,
    object_selected: bool,
}

impl ViewportPanel {
    pub fn new() -> Self {
        Self {
            is_3d: true,
            is_ortho: false,
            gizmo_mode: GizmoMode::Translate,
            gizmo_orientation: GizmoOrientation::Global,
            model_matrix: Mat4::IDENTITY,
            camera_yaw: 0.78,
            camera_pitch: 0.42,
            camera_distance: 4.8,
            camera_target: Vec3::ZERO,
            object_selected: false,
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

                let viewport_resp =
                    ui.interact(viewport_rect, ui.id().with("scene_viewport_input"), Sense::click_and_drag());

                let controls_rect = Rect::from_min_max(
                    egui::pos2(viewport_rect.right() - 370.0, viewport_rect.top() + 6.0),
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

                        if ui
                            .add_sized(
                                [64.0, 22.0],
                                egui::Button::new("Move")
                                    .corner_radius(6)
                                    .fill(if self.gizmo_mode == GizmoMode::Translate {
                                        Color32::from_rgb(64, 64, 68)
                                    } else {
                                        Color32::from_rgb(42, 42, 46)
                                    })
                                    .stroke(if self.gizmo_mode == GizmoMode::Translate {
                                        Stroke::new(1.0, Color32::from_rgb(15, 232, 121))
                                    } else {
                                        Stroke::new(1.0, Color32::from_rgb(72, 72, 78))
                                    }),
                            )
                            .clicked()
                        {
                            self.gizmo_mode = GizmoMode::Translate;
                        }
                        if ui
                            .add_sized(
                                [66.0, 22.0],
                                egui::Button::new("Rotate")
                                    .corner_radius(6)
                                    .fill(if self.gizmo_mode == GizmoMode::Rotate {
                                        Color32::from_rgb(64, 64, 68)
                                    } else {
                                        Color32::from_rgb(42, 42, 46)
                                    })
                                    .stroke(if self.gizmo_mode == GizmoMode::Rotate {
                                        Stroke::new(1.0, Color32::from_rgb(15, 232, 121))
                                    } else {
                                        Stroke::new(1.0, Color32::from_rgb(72, 72, 78))
                                    }),
                            )
                            .clicked()
                        {
                            self.gizmo_mode = GizmoMode::Rotate;
                        }
                        if ui
                            .add_sized(
                                [60.0, 22.0],
                                egui::Button::new("Scale")
                                    .corner_radius(6)
                                    .fill(if self.gizmo_mode == GizmoMode::Scale {
                                        Color32::from_rgb(64, 64, 68)
                                    } else {
                                        Color32::from_rgb(42, 42, 46)
                                    })
                                    .stroke(if self.gizmo_mode == GizmoMode::Scale {
                                        Stroke::new(1.0, Color32::from_rgb(15, 232, 121))
                                    } else {
                                        Stroke::new(1.0, Color32::from_rgb(72, 72, 78))
                                    }),
                            )
                            .clicked()
                        {
                            self.gizmo_mode = GizmoMode::Scale;
                        }
                    },
                );

                ui.painter().text(
                    egui::pos2(viewport_rect.left() + 12.0, viewport_rect.bottom() - 10.0),
                    Align2::LEFT_BOTTOM,
                    "LMB: orbitar | MMB: pan | RMB: selecionar | Ctrl+Scroll: zoom",
                    FontId::proportional(11.0),
                    Color32::from_gray(170),
                );

                if self.is_3d {
                    let pointer_delta = ctx.input(|i| i.pointer.delta());
                    let scroll_delta = ctx.input(|i| i.smooth_scroll_delta.y);
                    let ctrl_down = ctx.input(|i| i.modifiers.ctrl);
                    let primary_down = ctx.input(|i| i.pointer.primary_down());
                    let middle_down = ctx.input(|i| i.pointer.middle_down());

                    if viewport_resp.hovered() {
                        if primary_down {
                            self.camera_yaw -= pointer_delta.x * 0.012;
                            self.camera_pitch =
                                (self.camera_pitch - pointer_delta.y * 0.009).clamp(-1.45, 1.45);
                            ui.ctx().request_repaint();
                        }

                        if middle_down {
                            if ctrl_down {
                                self.camera_distance =
                                    (self.camera_distance + pointer_delta.y * 0.02).clamp(0.8, 80.0);
                            } else {
                                let right = Vec3::new(self.camera_yaw.sin(), 0.0, -self.camera_yaw.cos());
                                let up = Vec3::Y;
                                let pan_scale = self.camera_distance * 0.002;
                                self.camera_target += (-pointer_delta.x * pan_scale) * right;
                                self.camera_target += (pointer_delta.y * pan_scale) * up;
                            }
                            ui.ctx().request_repaint();
                        }

                        if ctrl_down && scroll_delta.abs() > 0.0 {
                            self.camera_distance =
                                (self.camera_distance - scroll_delta * 0.01).clamp(0.8, 80.0);
                            ui.ctx().request_repaint();
                        }
                    }

                    let aspect = (viewport_rect.width() / viewport_rect.height()).max(0.1);
                    let orbit = Vec3::new(
                        self.camera_yaw.cos() * self.camera_pitch.cos(),
                        self.camera_pitch.sin(),
                        self.camera_yaw.sin() * self.camera_pitch.cos(),
                    );
                    let eye = self.camera_target + orbit * self.camera_distance;
                    let view = Mat4::look_at_rh(eye, self.camera_target, Vec3::Y);
                    let proj = if self.is_ortho {
                        Mat4::orthographic_rh_gl(-2.0 * aspect, 2.0 * aspect, -2.0, 2.0, 0.1, 50.0)
                    } else {
                        Mat4::perspective_rh_gl(45.0_f32.to_radians(), aspect, 0.1, 50.0)
                    };
                    let mvp = proj * view * self.model_matrix;

                    if viewport_resp.secondary_clicked() {
                        let hover_pos = ctx.input(|i| i.pointer.hover_pos());
                        let object_screen = project_point(viewport_rect, mvp, Vec3::ZERO);
                        self.object_selected = if let (Some(cursor), Some(center)) = (hover_pos, object_screen) {
                            cursor.distance(center) <= 18.0
                        } else {
                            false
                        };
                    }

                    draw_wire_cube(ui, viewport_rect, mvp, self.object_selected);

                    if self.object_selected {
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
                }
            });
    }
}

fn project_point(viewport: Rect, mvp: Mat4, point: Vec3) -> Option<Pos2> {
    let clip = mvp * point.extend(1.0);
    if clip.w.abs() <= 1e-6 {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    if ndc.z < -1.1 || ndc.z > 1.1 {
        return None;
    }
    let x = viewport.left() + (ndc.x * 0.5 + 0.5) * viewport.width();
    let y = viewport.top() + (1.0 - (ndc.y * 0.5 + 0.5)) * viewport.height();
    Some(egui::pos2(x, y))
}

fn draw_wire_cube(ui: &mut egui::Ui, viewport: Rect, mvp: Mat4, selected: bool) {
    let s = 0.55;
    let points = [
        Vec3::new(-s, -s, -s),
        Vec3::new(s, -s, -s),
        Vec3::new(s, s, -s),
        Vec3::new(-s, s, -s),
        Vec3::new(-s, -s, s),
        Vec3::new(s, -s, s),
        Vec3::new(s, s, s),
        Vec3::new(-s, s, s),
    ];
    let edges = [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 0),
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 4),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ];
    let projected: Vec<Option<Pos2>> = points
        .iter()
        .map(|p| project_point(viewport, mvp, *p))
        .collect();
    let stroke = if selected {
        Stroke::new(1.8, Color32::from_rgb(15, 232, 121))
    } else {
        Stroke::new(1.4, Color32::from_rgb(148, 148, 162))
    };
    for (a, b) in edges {
        if let (Some(pa), Some(pb)) = (projected[a], projected[b]) {
            ui.painter().line_segment([pa, pb], stroke);
        }
    }
}
