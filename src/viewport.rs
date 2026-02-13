use eframe::egui::{self, Align2, Color32, FontId, PointerButton, Pos2, Rect, Sense, Stroke, TextureHandle, TextureOptions, Vec2};
use egui_gizmo::{Gizmo, GizmoMode, GizmoOrientation};
use epaint::ColorImage;
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
    rotation_icon: Option<TextureHandle>,
    scale_icon: Option<TextureHandle>,
    transform_icon: Option<TextureHandle>,
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
            rotation_icon: None,
            scale_icon: None,
            transform_icon: None,
        }
    }

    fn ensure_icons_loaded(&mut self, ctx: &egui::Context) {
        if self.rotation_icon.is_none() {
            self.rotation_icon = load_png_as_texture(ctx, "src/assets/icons/rotation.png");
        }
        if self.scale_icon.is_none() {
            self.scale_icon = load_png_as_texture(ctx, "src/assets/icons/scale.png");
        }
        if self.transform_icon.is_none() {
            self.transform_icon = load_png_as_texture(ctx, "src/assets/icons/transform.png");
        }
    }

    fn gizmo_icon_button(
        ui: &mut egui::Ui,
        texture: Option<&TextureHandle>,
        fallback: &str,
        selected: bool,
        tooltip: &str,
    ) -> bool {
        let button = if let Some(texture) = texture {
            egui::Button::image(egui::Image::new(texture).fit_to_exact_size(egui::vec2(14.0, 14.0)))
        } else {
            egui::Button::new(fallback)
        }
        .corner_radius(6)
        .fill(if selected {
            Color32::from_rgb(64, 64, 68)
        } else {
            Color32::from_rgb(42, 42, 46)
        })
        .stroke(if selected {
            Stroke::new(1.0, Color32::from_rgb(15, 232, 121))
        } else {
            Stroke::new(1.0, Color32::from_rgb(72, 72, 78))
        });

        ui.add_sized([28.0, 24.0], button)
            .on_hover_text(tooltip)
            .clicked()
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        mode_label: &str,
        left_reserved: f32,
        right_reserved: f32,
        bottom_reserved: f32,
    ) {
        self.ensure_icons_loaded(ctx);

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
                    egui::pos2(viewport_rect.right() - 395.0, viewport_rect.top() + 6.0),
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

                        if Self::gizmo_icon_button(
                            ui,
                            self.transform_icon.as_ref(),
                            "T",
                            self.gizmo_mode == GizmoMode::Translate,
                            "Transform",
                        ) {
                            self.gizmo_mode = GizmoMode::Translate;
                            self.object_selected = true;
                        }
                        if Self::gizmo_icon_button(
                            ui,
                            self.scale_icon.as_ref(),
                            "S",
                            self.gizmo_mode == GizmoMode::Scale,
                            "Scale",
                        ) {
                            self.gizmo_mode = GizmoMode::Scale;
                            self.object_selected = true;
                        }
                        if Self::gizmo_icon_button(
                            ui,
                            self.rotation_icon.as_ref(),
                            "R",
                            self.gizmo_mode == GizmoMode::Rotate,
                            "Rotation",
                        ) {
                            self.gizmo_mode = GizmoMode::Rotate;
                            self.object_selected = true;
                        }
                    },
                );

                ui.painter().text(
                    egui::pos2(viewport_rect.left() + 12.0, viewport_rect.bottom() - 10.0),
                    Align2::LEFT_BOTTOM,
                    "Mouse (Unity): Alt+LMB orbitar | RMB arrastar olhar (camera fixa) | MMB pan | Alt+RMB zoom | Scroll zoom | LMB selecionar | Touchpad: clique selecionar | 2 dedos pan | Pinch zoom | Shift+2 dedos orbitar",
                    FontId::proportional(11.0),
                    Color32::from_gray(170),
                );

                if self.is_3d {
                    let pointer_delta = ctx.input(|i| i.pointer.delta());
                    let scroll_delta = ctx.input(|i| i.smooth_scroll_delta);
                    let pinch_zoom = ctx.input(|i| i.zoom_delta());
                    let alt_down = ctx.input(|i| i.modifiers.alt);
                    let shift_down = ctx.input(|i| i.modifiers.shift);
                    let primary_down = ctx.input(|i| i.pointer.primary_down());
                    let middle_down = ctx.input(|i| i.pointer.middle_down());
                    let secondary_down = ctx.input(|i| i.pointer.secondary_down());
                    let pointer_over_controls = ctx.input(|i| {
                        i.pointer
                            .hover_pos()
                            .is_some_and(|p| controls_rect.contains(p))
                    });
                    let view_gizmo_rect = Rect::from_min_size(
                        egui::pos2(viewport_rect.right() - 66.0, controls_rect.bottom() + 8.0),
                        egui::vec2(56.0, 56.0),
                    );

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

                    if let Some((next_yaw, next_pitch)) = draw_view_orientation_gizmo(ui, view_gizmo_rect, view) {
                        self.camera_yaw = next_yaw;
                        self.camera_pitch = next_pitch;
                        ui.ctx().request_repaint();
                    }

                    let pointer_over_view_gizmo = ctx.input(|i| {
                        i.pointer
                            .hover_pos()
                            .is_some_and(|p| view_gizmo_rect.contains(p))
                    });
                    let can_navigate_camera =
                        viewport_resp.hovered() && !pointer_over_controls && !pointer_over_view_gizmo;

                    if can_navigate_camera {
                        // Unity-like orbit: Alt + LMB.
                        if alt_down && primary_down {
                            self.camera_yaw -= pointer_delta.x * 0.012;
                            self.camera_pitch =
                                (self.camera_pitch - pointer_delta.y * 0.009).clamp(-1.45, 1.45);
                            ui.ctx().request_repaint();
                        }

                        // Free-look: RMB drag rotates view, keeping camera position fixed.
                        if secondary_down && !alt_down {
                            let old_orbit = Vec3::new(
                                self.camera_yaw.cos() * self.camera_pitch.cos(),
                                self.camera_pitch.sin(),
                                self.camera_yaw.sin() * self.camera_pitch.cos(),
                            );
                            let eye = self.camera_target + old_orbit * self.camera_distance;
                            self.camera_yaw -= pointer_delta.x * 0.012;
                            self.camera_pitch =
                                (self.camera_pitch - pointer_delta.y * 0.009).clamp(-1.45, 1.45);
                            let new_orbit = Vec3::new(
                                self.camera_yaw.cos() * self.camera_pitch.cos(),
                                self.camera_pitch.sin(),
                                self.camera_yaw.sin() * self.camera_pitch.cos(),
                            );
                            self.camera_target = eye - new_orbit * self.camera_distance;
                            ui.ctx().request_repaint();
                        }

                        // Unity-like pan: MMB drag.
                        if middle_down {
                            let right = Vec3::new(self.camera_yaw.sin(), 0.0, -self.camera_yaw.cos());
                            let up = Vec3::Y;
                            let pan_scale = self.camera_distance * 0.002;
                            self.camera_target += (-pointer_delta.x * pan_scale) * right;
                            self.camera_target += (pointer_delta.y * pan_scale) * up;
                            ui.ctx().request_repaint();
                        }

                        // Unity-like dolly: Alt + RMB drag.
                        if alt_down && secondary_down && pointer_delta.y.abs() > 0.0 {
                            self.camera_distance =
                                (self.camera_distance + pointer_delta.y * 0.02).clamp(0.8, 80.0);
                            ui.ctx().request_repaint();
                        }

                        // Scroll zoom (mouse wheel / touchpad scroll).
                        if scroll_delta.y.abs() > 0.0 {
                            self.camera_distance =
                                (self.camera_distance - scroll_delta.y * 0.01).clamp(0.8, 80.0);
                            ui.ctx().request_repaint();
                        }

                        // Touchpad: dois dedos = pan; Shift + dois dedos = orbita.
                        if scroll_delta.length_sq() > 0.0 {
                            if shift_down {
                                self.camera_yaw -= scroll_delta.x * 0.008;
                                self.camera_pitch =
                                    (self.camera_pitch - scroll_delta.y * 0.006).clamp(-1.45, 1.45);
                            } else {
                                let right = Vec3::new(self.camera_yaw.sin(), 0.0, -self.camera_yaw.cos());
                                let up = Vec3::Y;
                                let pan_scale = self.camera_distance * 0.0016;
                                self.camera_target += (-scroll_delta.x * pan_scale) * right;
                                self.camera_target += (scroll_delta.y * pan_scale) * up;
                            }
                            ui.ctx().request_repaint();
                        }

                        // Touchpad pinch: aproxima/afasta camera.
                        if (pinch_zoom - 1.0).abs() > 1e-4 {
                            self.camera_distance = (self.camera_distance / pinch_zoom).clamp(0.8, 80.0);
                            ui.ctx().request_repaint();
                        }
                    }

                    if viewport_resp.clicked_by(PointerButton::Primary)
                        && !pointer_over_controls
                        && !pointer_over_view_gizmo
                        && !alt_down
                    {
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

fn load_png_as_texture(ctx: &egui::Context, png_path: &str) -> Option<TextureHandle> {
    let bytes = std::fs::read(png_path).ok()?;
    let rgba = image::load_from_memory(&bytes).ok()?.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let color_image = ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
    Some(ctx.load_texture(png_path.to_owned(), color_image, TextureOptions::LINEAR))
}

fn draw_view_orientation_gizmo(ui: &mut egui::Ui, rect: Rect, view: Mat4) -> Option<(f32, f32)> {
    let id = ui.id().with("viewport_view_orientation_gizmo");
    let resp = ui.interact(rect, id, Sense::click());
    let painter = ui.painter();
    let center = rect.center();
    let radius = rect.width().min(rect.height()) * 0.5;

    painter.circle_filled(
        center,
        radius,
        Color32::from_rgba_unmultiplied(28, 31, 36, if resp.hovered() { 230 } else { 205 }),
    );
    painter.circle_stroke(center, radius, Stroke::new(1.0, Color32::from_rgb(74, 82, 95)));

    let axes = [
        (Vec3::X, Color32::from_rgb(228, 78, 88), 0.0_f32, 0.0_f32),
        (Vec3::NEG_X, Color32::from_rgb(124, 50, 57), std::f32::consts::PI, 0.0_f32),
        (Vec3::Y, Color32::from_rgb(98, 206, 110), 0.0_f32, 1.45_f32),
        (Vec3::NEG_Y, Color32::from_rgb(54, 110, 62), 0.0_f32, -1.45_f32),
        (
            Vec3::Z,
            Color32::from_rgb(84, 153, 236),
            std::f32::consts::FRAC_PI_2,
            0.0_f32,
        ),
        (
            Vec3::NEG_Z,
            Color32::from_rgb(52, 92, 138),
            -std::f32::consts::FRAC_PI_2,
            0.0_f32,
        ),
    ];

    let mut projected: Vec<(f32, Pos2, Color32, f32, f32)> = axes
        .iter()
        .map(|(axis, color, yaw, pitch)| {
            let cam = view.transform_vector3(*axis);
            let pos = center + Vec2::new(cam.x, -cam.y) * (radius * 0.68);
            (cam.z, pos, *color, *yaw, *pitch)
        })
        .collect();
    projected.sort_by(|a, b| a.0.total_cmp(&b.0));

    for (depth, pos, color, _yaw, _pitch) in projected {
        let thickness = if depth > 0.0 { 2.1 } else { 1.4 };
        let alpha = if depth > 0.0 { 255 } else { 155 };
        let draw_color = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha);

        painter.line_segment([center, pos], Stroke::new(thickness, draw_color));
        painter.circle_filled(pos, if depth > 0.0 { 4.2 } else { 3.2 }, draw_color);

        let hit_rect = Rect::from_center_size(pos, egui::vec2(14.0, 14.0));
        let hit_resp = ui.interact(hit_rect, id.with((pos.x as i32, pos.y as i32)), Sense::click());
        if hit_resp.hovered() {
            painter.circle_stroke(pos, 6.0, Stroke::new(1.0, Color32::WHITE));
        }
        if hit_resp.clicked() {
            return Some((_yaw, _pitch));
        }
    }

    None
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
