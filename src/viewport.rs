use std::collections::HashSet;
use std::path::Path;
use std::sync::mpsc::{self, Receiver};

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
    last_viewport_rect: Option<Rect>,
    dropped_asset_label: Option<String>,
    active_mesh: Option<MeshData>,
    mesh_status: Option<String>,
    mesh_loading: bool,
    mesh_rx: Option<Receiver<Result<MeshData, String>>>,
}

struct MeshData {
    name: String,
    vertices: Vec<Vec3>,
    triangles: Vec<[u32; 3]>,
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
            last_viewport_rect: None,
            dropped_asset_label: None,
            active_mesh: None,
            mesh_status: None,
            mesh_loading: false,
            mesh_rx: None,
        }
    }

    pub fn contains_point(&self, p: Pos2) -> bool {
        self.last_viewport_rect.is_some_and(|r| r.contains(p))
    }

    pub fn panel_rect(&self) -> Option<Rect> {
        self.last_viewport_rect
    }

    pub fn on_asset_dropped(&mut self, asset_name: &str) {
        if asset_name.ends_with(".fbx") || asset_name.ends_with(".obj") || asset_name.ends_with(".glb") {
            self.object_selected = true;
            self.dropped_asset_label = Some(asset_name.to_string());
        }
    }

    pub fn on_asset_file_dropped(&mut self, path: &Path) {
        let asset_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("asset")
            .to_string();
        self.on_asset_dropped(&asset_name);

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .unwrap_or_default();

        match ext.as_str() {
            "fbx" | "obj" | "glb" | "gltf" => {
                let path_buf = path.to_path_buf();
                let (tx, rx) = mpsc::channel();
                self.mesh_loading = true;
                self.mesh_status = Some("Carregando malha...".to_string());
                self.mesh_rx = Some(rx);
                std::thread::spawn(move || {
                    let _ = tx.send(load_mesh_from_path(&path_buf));
                });
            }
            _ => {}
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
                if self.mesh_loading {
                    ui.ctx().request_repaint();
                }
                if let Some(rx) = &self.mesh_rx {
                    match rx.try_recv() {
                        Ok(Ok(mesh)) => {
                            self.active_mesh = Some(mesh);
                            self.mesh_status = Some("Mesh carregada".to_string());
                            self.mesh_loading = false;
                            self.mesh_rx = None;
                        }
                        Ok(Err(err)) => {
                            self.active_mesh = None;
                            self.mesh_status = Some(format!("Falha ao carregar malha: {err}"));
                            self.mesh_loading = false;
                            self.mesh_rx = None;
                        }
                        Err(mpsc::TryRecvError::Disconnected) => {
                            self.mesh_loading = false;
                            self.mesh_rx = None;
                        }
                        Err(mpsc::TryRecvError::Empty) => {}
                    }
                }

                let content = ui.max_rect();
                let viewport_rect = Rect::from_min_max(
                    egui::pos2(content.left() + left_reserved, content.top()),
                    egui::pos2(content.right() - right_reserved, content.bottom() - bottom_reserved),
                );
                if viewport_rect.width() < 80.0 || viewport_rect.height() < 80.0 {
                    self.last_viewport_rect = None;
                    return;
                }
                self.last_viewport_rect = Some(viewport_rect);

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
                if let Some(asset) = &self.dropped_asset_label {
                    ui.painter().text(
                        egui::pos2(viewport_rect.left() + 12.0, viewport_rect.top() + 28.0),
                        Align2::LEFT_TOP,
                        format!("Asset: {asset}"),
                        FontId::proportional(11.0),
                        Color32::from_rgb(144, 206, 168),
                    );
                }
                if let Some(status) = &self.mesh_status {
                    ui.painter().text(
                        egui::pos2(viewport_rect.left() + 12.0, viewport_rect.top() + 44.0),
                        Align2::LEFT_TOP,
                        status,
                        FontId::proportional(10.0),
                        Color32::from_gray(190),
                    );
                }
                if self.mesh_loading {
                    let loading_rect = Rect::from_center_size(viewport_rect.center(), egui::vec2(160.0, 30.0));
                    ui.painter().rect_filled(
                        loading_rect,
                        6.0,
                        Color32::from_rgba_unmultiplied(25, 30, 33, 220),
                    );
                    ui.painter().rect_stroke(
                        loading_rect,
                        6.0,
                        Stroke::new(1.0, Color32::from_rgb(72, 92, 96)),
                        egui::StrokeKind::Outside,
                    );
                    ui.scope_builder(
                        egui::UiBuilder::new()
                            .max_rect(loading_rect)
                            .layout(
                                egui::Layout::left_to_right(egui::Align::Center)
                                    .with_main_align(egui::Align::Center),
                            ),
                        |ui| {
                            ui.add(egui::Spinner::new().size(14.0));
                            ui.add_space(8.0);
                            ui.label("Carregando malha...");
                        },
                    );
                }

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
                        egui::pos2(viewport_rect.right() - 66.0, viewport_rect.bottom() - 66.0),
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

                    if let Some(mesh) = &self.active_mesh {
                        draw_wire_mesh(ui, viewport_rect, mvp, mesh, self.object_selected);
                    } else {
                        draw_wire_cube(ui, viewport_rect, mvp, self.object_selected);
                    }

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

fn load_mesh_from_path(path: &Path) -> Result<MeshData, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .ok_or_else(|| "extensão inválida".to_string())?;

    let mut mesh = match ext.as_str() {
        "fbx" => load_fbx_ascii_mesh(path)?,
        "obj" => load_obj_mesh(path)?,
        "glb" | "gltf" => load_gltf_mesh(path)?,
        _ => return Err("formato não suportado".to_string()),
    };
    normalize_mesh(&mut mesh);
    Ok(mesh)
}

fn load_fbx_ascii_mesh(path: &Path) -> Result<MeshData, String> {
    let text = std::fs::read_to_string(path).map_err(|_| {
        "FBX binário não suportado neste parser inicial (exporte como ASCII ou use OBJ/GLB)"
            .to_string()
    })?;

    let verts_blob = extract_fbx_array_blob(&text, "Vertices")
        .ok_or_else(|| "FBX sem bloco Vertices".to_string())?;
    let poly_blob = extract_fbx_array_blob(&text, "PolygonVertexIndex")
        .ok_or_else(|| "FBX sem bloco PolygonVertexIndex".to_string())?;

    let verts_raw = parse_fbx_f32_array(verts_blob);
    if verts_raw.len() < 3 {
        return Err("FBX sem vértices válidos".to_string());
    }
    let mut vertices = Vec::with_capacity(verts_raw.len() / 3);
    for chunk in verts_raw.chunks_exact(3) {
        vertices.push(Vec3::new(chunk[0], chunk[1], chunk[2]));
    }

    let poly_idx = parse_fbx_i32_array(poly_blob);
    if poly_idx.is_empty() {
        return Err("FBX sem índices válidos".to_string());
    }

    let mut triangles: Vec<[u32; 3]> = Vec::new();
    let mut poly: Vec<u32> = Vec::new();
    for raw in poly_idx {
        if raw >= 0 {
            poly.push(raw as u32);
            continue;
        }
        let end_idx = (-raw - 1) as u32;
        poly.push(end_idx);
        if poly.len() >= 3 {
            for i in 1..(poly.len() - 1) {
                triangles.push([poly[0], poly[i], poly[i + 1]]);
            }
        }
        poly.clear();
    }

    triangles.retain(|tri| {
        (tri[0] as usize) < vertices.len()
            && (tri[1] as usize) < vertices.len()
            && (tri[2] as usize) < vertices.len()
    });
    if triangles.is_empty() {
        return Err("FBX sem triângulos válidos".to_string());
    }

    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("FBX")
        .to_string();
    Ok(MeshData {
        name,
        vertices,
        triangles,
    })
}

fn extract_fbx_array_blob<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let key_pos = text.find(key)?;
    let after_key = &text[key_pos..];
    let a_pos_rel = after_key.find("a:")?;
    let arr_start = key_pos + a_pos_rel + 2;
    let tail = &text[arr_start..];
    let end_rel = tail.find('}')?;
    Some(tail[..end_rel].trim())
}

fn parse_fbx_f32_array(data: &str) -> Vec<f32> {
    data.split(|c: char| c == ',' || c.is_ascii_whitespace())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<f32>().ok())
        .collect()
}

fn parse_fbx_i32_array(data: &str) -> Vec<i32> {
    data.split(|c: char| c == ',' || c.is_ascii_whitespace())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<i32>().ok())
        .collect()
}

fn load_obj_mesh(path: &Path) -> Result<MeshData, String> {
    let opt = tobj::LoadOptions {
        triangulate: true,
        single_index: true,
        ..Default::default()
    };
    let (models, _mats) = tobj::load_obj(path, &opt).map_err(|e| e.to_string())?;

    let mut vertices = Vec::new();
    let mut triangles = Vec::new();
    for m in models {
        let base = vertices.len() as u32;
        let mesh = m.mesh;
        for p in mesh.positions.chunks_exact(3) {
            vertices.push(Vec3::new(p[0], p[1], p[2]));
        }
        for idx in mesh.indices.chunks_exact(3) {
            triangles.push([base + idx[0], base + idx[1], base + idx[2]]);
        }
    }
    if vertices.is_empty() || triangles.is_empty() {
        return Err("OBJ sem vértices/triângulos".to_string());
    }
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("OBJ")
        .to_string();
    Ok(MeshData {
        name,
        vertices,
        triangles,
    })
}

fn load_gltf_mesh(path: &Path) -> Result<MeshData, String> {
    let (doc, buffers, _images) = gltf::import(path).map_err(|e| e.to_string())?;
    let mut vertices = Vec::new();
    let mut triangles = Vec::new();

    for mesh in doc.meshes() {
        for primitive in mesh.primitives() {
            if primitive.mode() != gltf::mesh::Mode::Triangles {
                continue;
            }
            let reader = primitive.reader(|buf| Some(&buffers[buf.index()].0));
            let Some(positions) = reader.read_positions() else {
                continue;
            };

            let base = vertices.len() as u32;
            let local_verts: Vec<Vec3> = positions.map(|p| Vec3::new(p[0], p[1], p[2])).collect();
            let vcount = local_verts.len() as u32;
            vertices.extend(local_verts);

            if let Some(indices) = reader.read_indices() {
                let idx_u32: Vec<u32> = indices.into_u32().collect();
                for tri in idx_u32.chunks_exact(3) {
                    triangles.push([base + tri[0], base + tri[1], base + tri[2]]);
                }
            } else {
                let mut i = 0;
                while i + 2 < vcount {
                    triangles.push([base + i, base + i + 1, base + i + 2]);
                    i += 3;
                }
            }
        }
    }

    if vertices.is_empty() || triangles.is_empty() {
        return Err("GLTF/GLB sem triângulos suportados".to_string());
    }
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("GLB")
        .to_string();
    Ok(MeshData {
        name,
        vertices,
        triangles,
    })
}

fn normalize_mesh(mesh: &mut MeshData) {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for v in &mesh.vertices {
        min = min.min(*v);
        max = max.max(*v);
    }
    let center = (min + max) * 0.5;
    let extents = (max - min).max(Vec3::splat(1e-5));
    let longest = extents.x.max(extents.y).max(extents.z);
    let scale = if longest > 0.0 { 1.1 / longest } else { 1.0 };
    for v in &mut mesh.vertices {
        *v = (*v - center) * scale;
    }
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
        (Vec3::X, Color32::from_rgb(228, 78, 88), 0.0_f32, 0.0_f32, Some("X")),
        (
            Vec3::NEG_X,
            Color32::from_rgb(124, 50, 57),
            std::f32::consts::PI,
            0.0_f32,
            None,
        ),
        (Vec3::Y, Color32::from_rgb(98, 206, 110), 0.0_f32, 1.45_f32, Some("Y")),
        (Vec3::NEG_Y, Color32::from_rgb(54, 110, 62), 0.0_f32, -1.45_f32, None),
        (
            Vec3::Z,
            Color32::from_rgb(84, 153, 236),
            std::f32::consts::FRAC_PI_2,
            0.0_f32,
            Some("Z"),
        ),
        (
            Vec3::NEG_Z,
            Color32::from_rgb(52, 92, 138),
            -std::f32::consts::FRAC_PI_2,
            0.0_f32,
            None,
        ),
    ];

    let mut projected: Vec<(f32, Pos2, Color32, f32, f32, Option<&'static str>)> = axes
        .iter()
        .map(|(axis, color, yaw, pitch, label)| {
            let cam = view.transform_vector3(*axis);
            let pos = center + Vec2::new(cam.x, -cam.y) * (radius * 0.68);
            (cam.z, pos, *color, *yaw, *pitch, *label)
        })
        .collect();
    projected.sort_by(|a, b| a.0.total_cmp(&b.0));

    for (depth, pos, color, _yaw, _pitch, label) in projected {
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
        if let Some(label) = label {
            painter.circle_filled(
                pos,
                if depth > 0.0 { 7.0 } else { 6.4 },
                Color32::from_rgba_unmultiplied(22, 24, 28, if depth > 0.0 { 220 } else { 185 }),
            );
            painter.circle_stroke(pos, 7.0, Stroke::new(1.0, draw_color));
            painter.text(
                pos,
                Align2::CENTER_CENTER,
                label,
                FontId::proportional(9.0),
                Color32::from_rgb(245, 245, 245),
            );
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

fn draw_wire_mesh(ui: &mut egui::Ui, viewport: Rect, mvp: Mat4, mesh: &MeshData, selected: bool) {
    let projected: Vec<Option<Pos2>> = mesh
        .vertices
        .iter()
        .map(|p| project_point(viewport, mvp, *p))
        .collect();
    let stroke = if selected {
        Stroke::new(1.5, Color32::from_rgb(15, 232, 121))
    } else {
        Stroke::new(1.1, Color32::from_rgb(150, 150, 165))
    };

    let mut drawn = HashSet::<(u32, u32)>::new();
    for tri in &mesh.triangles {
        let edges = [(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])];
        for (a, b) in edges {
            let key = if a < b { (a, b) } else { (b, a) };
            if !drawn.insert(key) {
                continue;
            }
            let ai = a as usize;
            let bi = b as usize;
            if let (Some(pa), Some(pb)) = (projected.get(ai).and_then(|p| *p), projected.get(bi).and_then(|p| *p))
            {
                ui.painter().line_segment([pa, pb], stroke);
            }
        }
    }
    ui.painter().text(
        egui::pos2(viewport.left() + 12.0, viewport.top() + 60.0),
        Align2::LEFT_TOP,
        format!("Mesh: {}", mesh.name),
        FontId::proportional(10.0),
        Color32::from_gray(180),
    );
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
