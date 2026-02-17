use crate::EngineLanguage;
use eframe::egui::{
    self, Align2, Color32, FontFamily, FontId, Id, Order, Pos2, Rect, Stroke, TextureHandle,
};
use epaint::ColorImage;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

const INSPECTOR_MIN_WIDTH: f32 = 260.0;
const INSPECTOR_MAX_WIDTH: f32 = 520.0;

#[derive(Clone, Copy)]
struct TransformDraft {
    position: [f32; 3],
    rotation: [f32; 3],
    scale: [f32; 3],
}

#[derive(Clone)]
pub struct FiosControllerDraft {
    pub enabled: bool,
    pub move_speed: f32,
    pub rotate_speed: f32,
    pub action_speed: f32,
    pub module_ref: String,
    pub primary_clip: String,
}

#[derive(Clone, Copy)]
pub struct RigidbodyDraft {
    pub enabled: bool,
    pub mass: f32,
    pub use_gravity: bool,
    pub jump_impulse: f32,
    pub gravity: [f32; 3],
}

#[derive(Clone, Copy)]
pub struct LightDraft {
    pub enabled: bool,
    pub color: [f32; 3],
    pub intensity: f32,
    pub range: f32,
}

impl Default for LightDraft {
    fn default() -> Self {
        Self {
            enabled: true,
            color: [1.0, 1.0, 1.0],
            intensity: 1.0,
            range: 10.0,
        }
    }
}

#[derive(Clone)]
pub struct AnimatorDraft {
    pub enabled: bool,
    pub controller_ref: String,
    pub clip_ref: String,
}

impl Default for AnimatorDraft {
    fn default() -> Self {
        Self {
            enabled: true,
            controller_ref: "None".to_string(),
            clip_ref: "None".to_string(),
        }
    }
}

impl Default for TransformDraft {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
        }
    }
}

impl Default for FiosControllerDraft {
    fn default() -> Self {
        Self {
            enabled: true,
            move_speed: 3.5,
            rotate_speed: 90.0,
            action_speed: 2.0,
            module_ref: "None".to_string(),
            primary_clip: "None".to_string(),
        }
    }
}

impl Default for RigidbodyDraft {
    fn default() -> Self {
        Self {
            enabled: true,
            mass: 1.0,
            use_gravity: true,
            jump_impulse: 5.0,
            gravity: [0.0, -9.81, 0.0],
        }
    }
}

pub struct InspectorWindow {
    pub open: bool,
    menu_icon_texture: Option<TextureHandle>,
    lock_icon_texture: Option<TextureHandle>,
    unlock_icon_texture: Option<TextureHandle>,
    add_icon_texture: Option<TextureHandle>,
    is_locked: bool,
    dock_side: Option<InspectorDockSide>,
    window_pos: Option<Pos2>,
    window_width: f32,
    dragging_from_header: bool,
    resizing_width: bool,
    fonts_initialized: bool,
    object_transforms: HashMap<String, TransformDraft>,
    object_transform_enabled: HashMap<String, bool>,
    last_selected_object: String,
    pending_live_request: Option<(String, TransformDraft)>,
    pending_apply_request: Option<(String, TransformDraft)>,
    _pending_animator_request: Option<String>,
    object_fios_controller: HashMap<String, FiosControllerDraft>,
    object_rigidbody: HashMap<String, RigidbodyDraft>,
    object_animator: HashMap<String, AnimatorDraft>,
    object_light: HashMap<String, LightDraft>,
    apply_loading_until: Option<Instant>,
}

#[derive(Clone, Copy)]
enum InspectorDockSide {
    Left,
    Right,
}

fn load_png_as_texture(
    ctx: &egui::Context,
    png_path: &str,
    tint: Option<Color32>,
) -> Option<TextureHandle> {
    let bytes = std::fs::read(png_path).ok()?;
    let rgba_img = image::load_from_memory(&bytes).ok()?.to_rgba8();
    let size = [rgba_img.width() as usize, rgba_img.height() as usize];
    let mut rgba = rgba_img.into_raw();

    if let Some(tint) = tint {
        for px in rgba.chunks_exact_mut(4) {
            if px[3] > 0 {
                px[0] = tint.r();
                px[1] = tint.g();
                px[2] = tint.b();
            }
        }
    }

    let color_image = ColorImage::from_rgba_unmultiplied(size, &rgba);
    Some(ctx.load_texture(
        png_path.to_owned(),
        color_image,
        egui::TextureOptions::LINEAR,
    ))
}

impl InspectorWindow {
    pub fn new() -> Self {
        Self {
            open: true,
            menu_icon_texture: None,
            lock_icon_texture: None,
            unlock_icon_texture: None,
            add_icon_texture: None,
            is_locked: true,
            dock_side: Some(InspectorDockSide::Left),
            window_pos: None,
            window_width: INSPECTOR_MIN_WIDTH,
            dragging_from_header: false,
            resizing_width: false,
            fonts_initialized: false,
            object_transforms: HashMap::new(),
            object_transform_enabled: HashMap::new(),
            last_selected_object: String::new(),
            pending_live_request: None,
            pending_apply_request: None,
            _pending_animator_request: None,
            object_fios_controller: HashMap::new(),
            object_rigidbody: HashMap::new(),
            object_animator: HashMap::new(),
            object_light: HashMap::new(),
            apply_loading_until: None,
        }
    }

    pub fn fios_controller_targets(&self) -> Vec<(String, FiosControllerDraft)> {
        self.object_fios_controller
            .iter()
            .filter_map(|(name, cfg)| {
                if cfg.enabled {
                    Some((name.clone(), cfg.clone()))
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn rigidbody_targets(&self) -> Vec<(String, RigidbodyDraft)> {
        self.object_rigidbody
            .iter()
            .filter_map(|(name, cfg)| {
                if cfg.enabled {
                    Some((name.clone(), *cfg))
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn animator_targets(&self) -> Vec<(String, AnimatorDraft)> {
        self.object_animator
            .iter()
            .filter_map(|(name, cfg)| {
                if cfg.enabled {
                    Some((name.clone(), cfg.clone()))
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn remove_object_data(&mut self, object_name: &str) {
        self.object_transforms.remove(object_name);
        self.object_transform_enabled.remove(object_name);
        self.object_fios_controller.remove(object_name);
        self.object_rigidbody.remove(object_name);
        self.object_animator.remove(object_name);
        self.object_light.remove(object_name);
    }

    pub fn get_object_light(&self, object_name: &str) -> Option<LightDraft> {
        self.object_light.get(object_name).cloned()
    }

    pub fn take_transform_apply_request(
        &mut self,
    ) -> Option<(String, [f32; 3], [f32; 3], [f32; 3])> {
        let (name, draft) = self.pending_apply_request.take()?;
        Some((name, draft.position, draft.rotation, draft.scale))
    }

    pub fn take_transform_live_request(
        &mut self,
    ) -> Option<(String, [f32; 3], [f32; 3], [f32; 3])> {
        let (name, draft) = self.pending_live_request.take()?;
        Some((name, draft.position, draft.rotation, draft.scale))
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        left_reserved: f32,
        right_reserved: f32,
        bottom_reserved: f32,
        language: EngineLanguage,
        selected_object: &str,
        selected_transform: Option<([f32; 3], [f32; 3], [f32; 3])>,
        animation_controllers: &[String],
        animation_modules: &[String],
        fbx_animation_clips: &[String],
        light_yaw: &mut f32,
        light_pitch: &mut f32,
        light_color: &mut [f32; 3],
        light_intensity: &mut f32,
        light_enabled: &mut bool,
    ) {
        if !self.open {
            return;
        }

        if self.menu_icon_texture.is_none() {
            self.menu_icon_texture = load_png_as_texture(ctx, "src/assets/icons/more.png", None);
        }
        if self.lock_icon_texture.is_none() {
            self.lock_icon_texture = load_png_as_texture(ctx, "src/assets/icons/lock.png", None);
        }
        if self.unlock_icon_texture.is_none() {
            self.unlock_icon_texture =
                load_png_as_texture(ctx, "src/assets/icons/unlock.png", None);
        }
        if self.add_icon_texture.is_none() {
            self.add_icon_texture = load_png_as_texture(
                ctx,
                "src/assets/icons/add.png",
                Some(Color32::from_rgb(55, 55, 55)),
            );
        }

        let module_default_clip = |module_name: &str| -> Option<String> {
            if module_name.trim().is_empty() {
                return None;
            }
            let path = Path::new("Assets")
                .join("Animations")
                .join("Modules")
                .join(module_name);
            let raw = fs::read_to_string(path).ok()?;
            let mut walk: Option<String> = None;
            let mut idle: Option<String> = None;
            let mut any_clip: Option<String> = None;
            for line in raw.lines() {
                let l = line.trim();
                if let Some(v) = l.strip_prefix("state.walk=") {
                    let s = v.trim();
                    if !s.is_empty() {
                        walk = Some(s.to_string());
                    }
                } else if let Some(v) = l.strip_prefix("state.idle=") {
                    let s = v.trim();
                    if !s.is_empty() {
                        idle = Some(s.to_string());
                    }
                } else if let Some(v) = l.strip_prefix("clip=") {
                    let s = v.trim();
                    if !s.is_empty() && any_clip.is_none() {
                        any_clip = Some(s.to_string());
                    }
                }
            }
            walk.or(idle).or(any_clip)
        };

        if !self.fonts_initialized {
            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert(
                "Roboto".to_owned(),
                Arc::new(egui::FontData::from_static(include_bytes!(
                    "assets/fonts/roboto.ttf"
                ))),
            );
            if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
                family.insert(0, "Roboto".to_owned());
            }
            ctx.set_fonts(fonts);
            self.fonts_initialized = true;
        }

        let dock_rect = ctx.available_rect();
        let usable_bottom = (dock_rect.bottom() - bottom_reserved).max(dock_rect.top() + 120.0);
        let usable_height = (usable_bottom - dock_rect.top()).max(120.0);
        let pointer_down = ctx.input(|i| i.pointer.primary_down());

        let height = if self.dock_side.is_some() {
            usable_height
        } else {
            (usable_height * 0.85).max(520.0)
        };
        let max_width =
            (dock_rect.width() - left_reserved - right_reserved - 40.0).max(INSPECTOR_MIN_WIDTH);
        self.window_width = self
            .window_width
            .clamp(INSPECTOR_MIN_WIDTH, max_width.min(INSPECTOR_MAX_WIDTH));
        let window_size = egui::vec2(self.window_width, height);
        let left_snap_x = dock_rect.left() + left_reserved;
        let right_snap_right = dock_rect.right() - right_reserved;

        if self.window_pos.is_none() {
            self.window_pos = Some(egui::pos2(left_snap_x, dock_rect.top()));
        }

        if let Some(side) = self.dock_side {
            if !self.dragging_from_header && !self.resizing_width && !pointer_down {
                let x = match side {
                    InspectorDockSide::Left => left_snap_x,
                    InspectorDockSide::Right => right_snap_right - self.window_width,
                };
                self.window_pos = Some(egui::pos2(x, dock_rect.top()));
            }
        }

        let pos = self
            .window_pos
            .unwrap_or(egui::pos2(left_snap_x, dock_rect.top()));

        let mut header_drag_started = false;
        let mut header_drag_stopped = false;
        let mut resize_started = false;
        let mut resize_stopped = false;
        let mut panel_rect = Rect::from_min_size(pos, window_size);
        let selected_changed = self.last_selected_object != selected_object;
        if selected_changed {
            self.last_selected_object = selected_object.to_string();
            if !selected_object.is_empty() {
                if let Some((position, rotation, scale)) = selected_transform {
                    self.object_transforms.insert(
                        selected_object.to_string(),
                        TransformDraft {
                            position,
                            rotation,
                            scale,
                        },
                    );
                } else {
                    self.object_transforms
                        .entry(selected_object.to_string())
                        .or_default();
                }
                self.object_transform_enabled
                    .entry(selected_object.to_string())
                    .or_insert(true);
            }
        }
        if !selected_object.is_empty() {
            if let Some((position, rotation, scale)) = selected_transform {
                self.object_transforms.insert(
                    selected_object.to_string(),
                    TransformDraft {
                        position,
                        rotation,
                        scale,
                    },
                );
            }
        }

        egui::Area::new(Id::new("inspetor_window_id"))
            .order(Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                let (rect, _) = ui.allocate_exact_size(window_size, egui::Sense::hover());
                panel_rect = rect;

                // Detect resizing area (5px on the left if docked right, or on the right if docked left)
                let resize_edge_w = 6.0;
                let is_docked_right = matches!(self.dock_side, Some(InspectorDockSide::Right));
                let resize_rect = if is_docked_right {
                    Rect::from_min_max(
                        egui::pos2(rect.left(), rect.top()),
                        egui::pos2(rect.left() + resize_edge_w, rect.bottom()),
                    )
                } else {
                    Rect::from_min_max(
                        egui::pos2(rect.right() - resize_edge_w, rect.top()),
                        egui::pos2(rect.right(), rect.bottom()),
                    )
                };

                let resize_resp = ui.interact(
                    resize_rect,
                    ui.id().with("resize_width"),
                    egui::Sense::drag(),
                );
                if resize_resp.drag_started() {
                    resize_started = true;
                    self.resizing_width = true;
                }
                if resize_resp.drag_stopped() {
                    resize_stopped = true;
                }
                if self.resizing_width {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                    let delta = ctx.input(|i| i.pointer.delta().x);
                    if is_docked_right {
                        self.window_width = (self.window_width - delta)
                            .clamp(INSPECTOR_MIN_WIDTH, max_width.min(INSPECTOR_MAX_WIDTH));
                    } else {
                        self.window_width = (self.window_width + delta)
                            .clamp(INSPECTOR_MIN_WIDTH, max_width.min(INSPECTOR_MAX_WIDTH));
                    }
                } else if resize_resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                }

                ui.painter()
                    .rect_filled(rect, 6.0, Color32::from_rgb(28, 28, 28));
                ui.painter().rect_stroke(
                    rect,
                    6.0,
                    Stroke::new(1.0, Color32::from_gray(60)),
                    egui::StrokeKind::Outside,
                );

                let inner = rect.shrink2(egui::vec2(8.0, 6.0));
                let header_h = 18.0;
                let header_rect =
                    Rect::from_min_max(inner.min, egui::pos2(inner.max.x, inner.min.y + header_h));
                let icon_side = 16.0;
                let icon_gap = 5.0;
                let icon_block = (icon_side * 2.0) + icon_gap;
                let drag_rect = Rect::from_min_max(
                    header_rect.min,
                    egui::pos2(header_rect.max.x - icon_block - 4.0, header_rect.max.y),
                );

                let drag_resp = ui.interact(
                    drag_rect,
                    ui.id().with("header_drag"),
                    egui::Sense::click_and_drag(),
                );
                if drag_resp.drag_started() {
                    header_drag_started = true;
                }
                if drag_resp.drag_stopped() {
                    header_drag_stopped = true;
                }

                ui.painter().text(
                    drag_rect.center(),
                    Align2::CENTER_CENTER,
                    match language {
                        EngineLanguage::Pt => "Inspetor",
                        EngineLanguage::En => "Inspector",
                        EngineLanguage::Es => "Inspector",
                    },
                    FontId::new(13.0, FontFamily::Proportional),
                    Color32::WHITE,
                );

                let lock_tex = if self.is_locked {
                    self.lock_icon_texture.as_ref()
                } else {
                    self.unlock_icon_texture.as_ref()
                };

                let lock_rect = Rect::from_min_size(
                    egui::pos2(header_rect.max.x - icon_block, header_rect.min.y + 1.0),
                    egui::vec2(icon_side, icon_side),
                );
                if let Some(lock_tex) = lock_tex {
                    let lock_resp = ui.put(
                        lock_rect,
                        egui::Image::new(lock_tex)
                            .fit_to_exact_size(egui::vec2(icon_side, icon_side))
                            .sense(egui::Sense::click()),
                    );
                    if lock_resp.hovered() {
                        ui.painter().rect_filled(
                            lock_rect.expand2(egui::vec2(2.0, 2.0)),
                            4.0,
                            Color32::from_rgba_unmultiplied(255, 255, 255, 28),
                        );
                    }
                    if lock_resp.clicked() {
                        self.is_locked = !self.is_locked;
                    }
                }

                let menu_rect = Rect::from_min_size(
                    egui::pos2(lock_rect.max.x + icon_gap, header_rect.min.y + 1.0),
                    egui::vec2(icon_side, icon_side),
                );
                if let Some(menu_tex) = &self.menu_icon_texture {
                    let menu_resp = ui.put(
                        menu_rect,
                        egui::Image::new(menu_tex)
                            .fit_to_exact_size(egui::vec2(icon_side, icon_side))
                            .sense(egui::Sense::click()),
                    );
                    if menu_resp.hovered() {
                        ui.painter().rect_filled(
                            menu_rect.expand2(egui::vec2(2.0, 2.0)),
                            4.0,
                            Color32::from_rgba_unmultiplied(255, 255, 255, 28),
                        );
                    }

                    egui::Popup::menu(&menu_resp)
                        .id(egui::Id::new("inspector_menu_popup"))
                        .width(220.0)
                        .frame(
                            egui::Frame::new()
                                .fill(Color32::from_rgb(45, 45, 45))
                                .corner_radius(8)
                                .stroke(Stroke::new(1.0, Color32::from_gray(78))),
                        )
                        .show(|ui| {
                            let copy_clicked = ui
                                .add_sized(
                                    [208.0, 26.0],
                                    egui::Button::new(
                                        egui::RichText::new(match language {
                                            EngineLanguage::Pt => "Copiar cadeia de componentes",
                                            EngineLanguage::En => "Copy component chain",
                                            EngineLanguage::Es => "Copiar cadena de componentes",
                                        })
                                        .color(Color32::WHITE),
                                    )
                                    .fill(Color32::from_rgb(62, 62, 62))
                                    .corner_radius(6),
                                )
                                .clicked();
                            if copy_clicked {
                                ui.ctx().copy_text("cadeia de componentes".to_owned());
                                ui.close();
                            }

                            let send_clicked = ui
                                .add_sized(
                                    [208.0, 26.0],
                                    egui::Button::new(
                                        egui::RichText::new(match language {
                                            EngineLanguage::Pt => "Enviar cadeia para...",
                                            EngineLanguage::En => "Send chain to...",
                                            EngineLanguage::Es => "Enviar cadena a...",
                                        })
                                        .color(Color32::WHITE),
                                    )
                                    .fill(Color32::from_rgb(62, 62, 62))
                                    .corner_radius(6),
                                )
                                .clicked();
                            if send_clicked {
                                ui.close();
                            }
                        });
                }

                let sep_y = header_rect.max.y + 5.0;
                ui.painter().line_segment(
                    [
                        egui::pos2(inner.min.x, sep_y),
                        egui::pos2(inner.max.x, sep_y),
                    ],
                    Stroke::new(1.0, Color32::from_gray(60)),
                );

                let button_h = 32.0;
                let button_rect = Rect::from_min_max(
                    egui::pos2(inner.min.x, rect.bottom() - 12.0 - button_h),
                    egui::pos2(inner.max.x, rect.bottom() - 12.0),
                );
                let content_rect = Rect::from_min_max(
                    egui::pos2(inner.min.x, sep_y + 8.0),
                    egui::pos2(inner.max.x, button_rect.min.y - 8.0),
                );

                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(content_rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                    |ui| {
                        if selected_object == "Directional Light" {
                            let light_draft = self
                                .object_light
                                .entry(selected_object.to_string())
                                .or_default();
                            light_draft.enabled = *light_enabled;
                            light_draft.color = *light_color;
                            light_draft.intensity = *light_intensity;
                        }

                        if selected_object.is_empty() || selected_object == "Directional Light" {
                            // Interface de Iluminação
                            egui::Frame::new()
                                .fill(Color32::from_rgb(33, 33, 33))
                                .stroke(Stroke::new(1.0, Color32::from_gray(60)))
                                .corner_radius(6)
                                .inner_margin(egui::Margin::same(10))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            egui::RichText::new(match language {
                                                EngineLanguage::Pt => "Iluminação Global",
                                                EngineLanguage::En => "Global Lighting",
                                                EngineLanguage::Es => "Iluminación Global",
                                            })
                                            .strong()
                                            .size(14.0)
                                            .color(Color32::WHITE),
                                        );
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                ui.checkbox(light_enabled, "");
                                            },
                                        );
                                    });
                                    ui.add_space(8.0);

                                    egui::Grid::new("lighting_grid")
                                        .num_columns(2)
                                        .spacing([10.0, 10.0])
                                        .show(ui, |ui| {
                                            ui.label(match language {
                                                EngineLanguage::Pt => "Yaw:",
                                                EngineLanguage::En => "Yaw:",
                                                EngineLanguage::Es => "Yaw:",
                                            });
                                            ui.add(
                                                egui::Slider::new(light_yaw, 0.0..=6.28)
                                                    .show_value(false),
                                            );
                                            ui.end_row();

                                            ui.label(match language {
                                                EngineLanguage::Pt => "Pitch:",
                                                EngineLanguage::En => "Pitch:",
                                                EngineLanguage::Es => "Pitch:",
                                            });
                                            ui.add(
                                                egui::Slider::new(light_pitch, 0.0..=1.57)
                                                    .show_value(false),
                                            );
                                            ui.end_row();

                                            ui.label(match language {
                                                EngineLanguage::Pt => "Intensidade:",
                                                EngineLanguage::En => "Intensity:",
                                                EngineLanguage::Es => "Intensidad:",
                                            });
                                            ui.add(
                                                egui::Slider::new(light_intensity, 0.0..=5.0)
                                                    .show_value(false),
                                            );
                                            ui.end_row();

                                            ui.label(match language {
                                                EngineLanguage::Pt => "Cor:",
                                                EngineLanguage::En => "Color:",
                                                EngineLanguage::Es => "Color:",
                                            });
                                            ui.color_edit_button_rgb(light_color);
                                            if selected_object == "Directional Light" {
                                                if let Some(light_draft) =
                                                    self.object_light.get_mut(selected_object)
                                                {
                                                    light_draft.color = *light_color;
                                                    light_draft.intensity = *light_intensity;
                                                    light_draft.enabled = *light_enabled;
                                                }
                                            }
                                            ui.end_row();
                                        });
                                });
                            return;
                        }

                        egui::ScrollArea::vertical()
                            .id_salt("inspector_scroll")
                            .show(ui, |ui| {
                                let draft = self
                                    .object_transforms
                                    .entry(selected_object.to_string())
                                    .or_default();
                                let enabled = self
                                    .object_transform_enabled
                                    .entry(selected_object.to_string())
                                    .or_insert(true);

                                let title = match language {
                                    EngineLanguage::Pt => "Transformação",
                                    EngineLanguage::En => "Transform",
                                    EngineLanguage::Es => "Transformación",
                                };
                                let apply_text = match language {
                                    EngineLanguage::Pt => "Aplicar Transformações",
                                    EngineLanguage::En => "Apply Transformations",
                                    EngineLanguage::Es => "Aplicar Transformaciones",
                                };
                                let loading_text = match language {
                                    EngineLanguage::Pt => "Aplicando...",
                                    EngineLanguage::En => "Applying...",
                                    EngineLanguage::Es => "Aplicando...",
                                };

                                egui::Frame::new()
                                    .fill(Color32::from_rgb(36, 36, 36))
                                    .stroke(Stroke::new(1.0, Color32::from_gray(62)))
                                    .corner_radius(6)
                                    .inner_margin(egui::Margin::same(8))
                                    .show(ui, |ui| {
                                        let header_h = 22.0;
                                        ui.horizontal(|ui| {
                                            ui.set_height(header_h);
                                            ui.checkbox(enabled, "");
                                            ui.label(
                                                egui::RichText::new(title)
                                                    .strong()
                                                    .color(Color32::WHITE),
                                            );
                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    if ui
                                                        .button("↺")
                                                        .on_hover_text("Reset Transform")
                                                        .clicked()
                                                    {
                                                        *draft = TransformDraft::default();
                                                    }
                                                },
                                            );
                                        });
                                        ui.add_space(4.0);
                                        ui.painter().line_segment(
                                            [
                                                ui.cursor().min,
                                                egui::pos2(
                                                    ui.max_rect().right() - 8.0,
                                                    ui.cursor().min.y,
                                                ),
                                            ],
                                            Stroke::new(1.0, Color32::from_gray(50)),
                                        );
                                        ui.add_space(8.0);

                                        let mut transform_changed = false;
                                        let mut numeric_dragging = false;

                                        let axis_labels = ["X", "Y", "Z"];
                                        egui::Grid::new("transform_grid")
                                            .num_columns(2)
                                            .spacing([12.0, 8.0])
                                            .show(ui, |ui| {
                                                // Posição
                                                ui.label(match language {
                                                    EngineLanguage::Pt => "Posição",
                                                    EngineLanguage::En => "Position",
                                                    EngineLanguage::Es => "Posición",
                                                });
                                                ui.horizontal(|ui| {
                                                    for i in 0..3 {
                                                        ui.label(
                                                            egui::RichText::new(axis_labels[i])
                                                                .size(9.0)
                                                                .color(Color32::GRAY),
                                                        );
                                                        let resp = ui.add(
                                                            egui::DragValue::new(
                                                                &mut draft.position[i],
                                                            )
                                                            .speed(0.1),
                                                        );
                                                        if resp.changed() {
                                                            transform_changed = true;
                                                        }
                                                        if resp.dragged() {
                                                            numeric_dragging = true;
                                                        }
                                                    }
                                                });
                                                ui.end_row();

                                                // Rotação
                                                ui.label(match language {
                                                    EngineLanguage::Pt => "Rotação",
                                                    EngineLanguage::En => "Rotation",
                                                    EngineLanguage::Es => "Rotación",
                                                });
                                                ui.horizontal(|ui| {
                                                    for i in 0..3 {
                                                        ui.label(
                                                            egui::RichText::new(axis_labels[i])
                                                                .size(9.0)
                                                                .color(Color32::GRAY),
                                                        );
                                                        let resp = ui.add(
                                                            egui::DragValue::new(
                                                                &mut draft.rotation[i],
                                                            )
                                                            .speed(0.1),
                                                        );
                                                        if resp.changed() {
                                                            transform_changed = true;
                                                        }
                                                        if resp.dragged() {
                                                            numeric_dragging = true;
                                                        }
                                                    }
                                                });
                                                ui.end_row();

                                                // Escala
                                                ui.label(match language {
                                                    EngineLanguage::Pt => "Escala",
                                                    EngineLanguage::En => "Scale",
                                                    EngineLanguage::Es => "Escala",
                                                });
                                                ui.horizontal(|ui| {
                                                    for i in 0..3 {
                                                        ui.label(
                                                            egui::RichText::new(axis_labels[i])
                                                                .size(9.0)
                                                                .color(Color32::GRAY),
                                                        );
                                                        let resp = ui.add(
                                                            egui::DragValue::new(
                                                                &mut draft.scale[i],
                                                            )
                                                            .speed(0.05),
                                                        );
                                                        if resp.changed() {
                                                            transform_changed = true;
                                                        }
                                                        if resp.dragged() {
                                                            numeric_dragging = true;
                                                        }
                                                    }
                                                });
                                                ui.end_row();
                                            });

                                        ui.add_space(10.0);
                                        let is_loading = self
                                            .apply_loading_until
                                            .is_some_and(|until| Instant::now() < until);
                                        let button_label =
                                            if is_loading { loading_text } else { apply_text };
                                        let button_resp = ui
                                            .add_enabled_ui(*enabled, |ui| {
                                                ui.add_sized(
                                                    [ui.available_width() - 4.0, 30.0],
                                                    egui::Button::new(
                                                        egui::RichText::new(button_label)
                                                            .size(13.0)
                                                            .color(Color32::from_rgb(55, 55, 55))
                                                            .strong(),
                                                    )
                                                    .fill(Color32::from_rgb(148, 116, 186))
                                                    .corner_radius(6),
                                                )
                                            })
                                            .inner;

                                        if button_resp.clicked() {
                                            self.pending_apply_request =
                                                Some((selected_object.to_string(), *draft));
                                            self.apply_loading_until =
                                                Some(Instant::now() + Duration::from_millis(900));
                                        }

                                        if *enabled && transform_changed {
                                            self.pending_live_request =
                                                Some((selected_object.to_string(), *draft));
                                        }
                                    });

                                ui.add_space(10.0);

                                // Botão Adicionar Componente
                                let add_btn = egui::Button::image_and_text(
                                    egui::Image::new(self.add_icon_texture.as_ref().unwrap())
                                        .fit_to_exact_size(egui::vec2(12.0, 12.0)),
                                    egui::RichText::new(match language {
                                        EngineLanguage::Pt => "Adicionar Componente",
                                        EngineLanguage::En => "Add Component",
                                        EngineLanguage::Es => "Añadir Componente",
                                    })
                                    .strong()
                                    .color(Color32::from_rgb(55, 55, 55)),
                                )
                                .fill(Color32::from_rgb(0x0F, 0xE8, 0x79))
                                .corner_radius(6)
                                .min_size(egui::vec2(ui.available_width() - 4.0, 26.0));

                                let add_id = Id::new("add_comp_menu");
                                let add_resp = ui.add(add_btn);
                                if add_resp.clicked() {
                                    egui::Popup::toggle_id(ui.ctx(), add_id);
                                }

                                egui::Popup::menu(&add_resp).id(add_id).show(|ui| {
                                    ui.set_width(220.0);

                                    ui.menu_button("💡 Iluminação", |ui: &mut egui::Ui| {
                                        if ui.button("Point Light").clicked() {
                                            self.object_light
                                                .entry(selected_object.to_string())
                                                .or_insert(LightDraft {
                                                    enabled: true,
                                                    color: [1.0, 1.0, 1.0],
                                                    intensity: 1.0,
                                                    range: 10.0,
                                                });
                                            ui.close();
                                        }
                                        if ui.button("Spot Light").clicked() {
                                            self.object_light
                                                .entry(selected_object.to_string())
                                                .or_insert(LightDraft {
                                                    enabled: true,
                                                    color: [1.0, 1.0, 0.8],
                                                    intensity: 2.0,
                                                    range: 15.0,
                                                });
                                            ui.close();
                                        }
                                        if ui.button("Directional Light").clicked() {
                                            self.object_light
                                                .entry(selected_object.to_string())
                                                .or_insert(LightDraft {
                                                    enabled: true,
                                                    color: [1.0, 1.0, 1.0],
                                                    intensity: 0.8,
                                                    range: 100.0,
                                                });
                                            ui.close();
                                        }
                                    });

                                    ui.menu_button(
                                        "🎮 Controles de Teclado",
                                        |ui: &mut egui::Ui| {
                                            if ui.button("Fios Controller").clicked() {
                                                self.object_fios_controller
                                                    .entry(selected_object.to_string())
                                                    .or_default();
                                                ui.close();
                                            }
                                        },
                                    );

                                    ui.menu_button("🔌 Módulos Fios", |ui: &mut egui::Ui| {
                                        for module in animation_modules {
                                            if ui.button(module).clicked() {
                                                let ctrl = self
                                                    .object_fios_controller
                                                    .entry(selected_object.to_string())
                                                    .or_default();
                                                ctrl.module_ref = module.clone();
                                                if let Some(clip) = module_default_clip(module) {
                                                    ctrl.primary_clip = clip;
                                                }
                                                ui.close();
                                            }
                                        }
                                    });

                                    ui.menu_button("⚖ Física", |ui: &mut egui::Ui| {
                                        if ui.button("Rigidbody").clicked() {
                                            self.object_rigidbody
                                                .entry(selected_object.to_string())
                                                .or_default();
                                            ui.close();
                                        }
                                    });

                                    ui.menu_button("🎬 Animação", |ui: &mut egui::Ui| {
                                        if ui.button("Animator").clicked() {
                                            self.object_animator
                                                .entry(selected_object.to_string())
                                                .or_default();
                                            ui.close();
                                        }
                                    });

                                    ui.menu_button("💡 Iluminação", |ui: &mut egui::Ui| {
                                        if ui.button("Luz").clicked() {
                                            self.object_light
                                                .entry(selected_object.to_string())
                                                .or_default();
                                            ui.close();
                                        }
                                    });
                                });

                                ui.add_space(10.0);

                                // Outros Componentes
                                let mut remove_fios = false;
                                if let Some(ctrl) =
                                    self.object_fios_controller.get_mut(selected_object)
                                {
                                    egui::Frame::new()
                                        .fill(Color32::from_rgb(36, 36, 36))
                                        .stroke(Stroke::new(1.0, Color32::from_gray(62)))
                                        .corner_radius(6)
                                        .inner_margin(egui::Margin::same(8))
                                        .show(ui, |ui| {
                                            ui.horizontal(|ui| {
                                                ui.label(
                                                    egui::RichText::new("Fios Controller")
                                                        .strong()
                                                        .color(Color32::WHITE),
                                                );
                                                ui.with_layout(
                                                    egui::Layout::right_to_left(
                                                        egui::Align::Center,
                                                    ),
                                                    |ui| {
                                                        if ui.button("×").clicked() {
                                                            remove_fios = true;
                                                        }
                                                    },
                                                );
                                            });
                                            ui.add_space(4.0);
                                            egui::Grid::new("fios_grid")
                                                .num_columns(2)
                                                .spacing([10.0, 8.0])
                                                .show(ui, |ui| {
                                                    ui.label("Modulo:");
                                                    egui::ComboBox::from_id_salt(
                                                        "fios_module_combo",
                                                    )
                                                    .selected_text(&ctrl.module_ref)
                                                    .show_ui(ui, |ui| {
                                                        for m in animation_modules {
                                                            ui.selectable_value(
                                                                &mut ctrl.module_ref,
                                                                m.clone(),
                                                                m,
                                                            );
                                                        }
                                                    });
                                                    ui.end_row();

                                                    ui.label("Clip:");
                                                    egui::ComboBox::from_id_salt("fios_clip_combo")
                                                        .selected_text(&ctrl.primary_clip)
                                                        .show_ui(ui, |ui| {
                                                            for c in fbx_animation_clips {
                                                                ui.selectable_value(
                                                                    &mut ctrl.primary_clip,
                                                                    c.clone(),
                                                                    c,
                                                                );
                                                            }
                                                        });
                                                    ui.end_row();

                                                    ui.label("Velocidade:");
                                                    ui.add(
                                                        egui::DragValue::new(&mut ctrl.move_speed)
                                                            .speed(0.1),
                                                    );
                                                    ui.end_row();
                                                });
                                        });
                                    ui.add_space(8.0);
                                }
                                if remove_fios {
                                    self.object_fios_controller.remove(selected_object);
                                }

                                let mut remove_rb = false;
                                if let Some(rb) = self.object_rigidbody.get_mut(selected_object) {
                                    egui::Frame::new()
                                        .fill(Color32::from_rgb(36, 36, 36))
                                        .stroke(Stroke::new(1.0, Color32::from_gray(62)))
                                        .corner_radius(6)
                                        .inner_margin(egui::Margin::same(8))
                                        .show(ui, |ui| {
                                            ui.horizontal(|ui| {
                                                ui.label(
                                                    egui::RichText::new("Rigidbody")
                                                        .strong()
                                                        .color(Color32::WHITE),
                                                );
                                                ui.with_layout(
                                                    egui::Layout::right_to_left(
                                                        egui::Align::Center,
                                                    ),
                                                    |ui| {
                                                        if ui.button("×").clicked() {
                                                            remove_rb = true;
                                                        }
                                                    },
                                                );
                                            });
                                            ui.add_space(4.0);
                                            egui::Grid::new("rb_grid")
                                                .num_columns(2)
                                                .spacing([10.0, 8.0])
                                                .show(ui, |ui| {
                                                    ui.label("Massa:");
                                                    ui.add(
                                                        egui::DragValue::new(&mut rb.mass)
                                                            .speed(0.1),
                                                    );
                                                    ui.end_row();

                                                    ui.label("Gravidade:");
                                                    ui.checkbox(&mut rb.use_gravity, "");
                                                    ui.end_row();
                                                });
                                        });
                                    ui.add_space(8.0);
                                }
                                if remove_rb {
                                    self.object_rigidbody.remove(selected_object);
                                }

                                let mut remove_anim = false;
                                if let Some(anim) = self.object_animator.get_mut(selected_object) {
                                    egui::Frame::new()
                                        .fill(Color32::from_rgb(36, 36, 36))
                                        .stroke(Stroke::new(1.0, Color32::from_gray(62)))
                                        .corner_radius(6)
                                        .inner_margin(egui::Margin::same(8))
                                        .show(ui, |ui| {
                                            ui.horizontal(|ui| {
                                                ui.label(
                                                    egui::RichText::new("Animator")
                                                        .strong()
                                                        .color(Color32::WHITE),
                                                );
                                                ui.with_layout(
                                                    egui::Layout::right_to_left(
                                                        egui::Align::Center,
                                                    ),
                                                    |ui| {
                                                        if ui.button("×").clicked() {
                                                            remove_anim = true;
                                                        }
                                                    },
                                                );
                                            });
                                            ui.add_space(4.0);
                                            egui::Grid::new("anim_grid")
                                                .num_columns(2)
                                                .spacing([10.0, 8.0])
                                                .show(ui, |ui| {
                                                    ui.label("Controller:");
                                                    egui::ComboBox::from_id_salt("anim_ctrl_combo")
                                                        .selected_text(&anim.controller_ref)
                                                        .show_ui(ui, |ui| {
                                                            for c in animation_controllers {
                                                                ui.selectable_value(
                                                                    &mut anim.controller_ref,
                                                                    c.clone(),
                                                                    c,
                                                                );
                                                            }
                                                        });
                                                    ui.end_row();

                                                    ui.label("Clip:");
                                                    egui::ComboBox::from_id_salt("anim_clip_combo")
                                                        .selected_text(&anim.clip_ref)
                                                        .show_ui(ui, |ui| {
                                                            for c in fbx_animation_clips {
                                                                ui.selectable_value(
                                                                    &mut anim.clip_ref,
                                                                    c.clone(),
                                                                    c,
                                                                );
                                                            }
                                                        });
                                                    ui.end_row();
                                                });
                                        });
                                    ui.add_space(8.0);
                                }
                                if remove_anim {
                                    self.object_animator.remove(selected_object);
                                }

                                let mut remove_light = false;
                                if let Some(light) = self.object_light.get_mut(selected_object) {
                                    egui::Frame::new()
                                        .fill(Color32::from_rgb(36, 36, 36))
                                        .stroke(Stroke::new(1.0, Color32::from_gray(62)))
                                        .corner_radius(6)
                                        .inner_margin(egui::Margin::same(8))
                                        .show(ui, |ui| {
                                            ui.horizontal(|ui| {
                                                ui.label(
                                                    egui::RichText::new("Luz")
                                                        .strong()
                                                        .color(Color32::WHITE),
                                                );
                                                ui.with_layout(
                                                    egui::Layout::right_to_left(
                                                        egui::Align::Center,
                                                    ),
                                                    |ui| {
                                                        if ui.button("×").clicked() {
                                                            remove_light = true;
                                                        }
                                                    },
                                                );
                                            });
                                            ui.add_space(4.0);
                                            egui::Grid::new("light_grid")
                                                .num_columns(2)
                                                .spacing([10.0, 8.0])
                                                .show(ui, |ui| {
                                                    ui.label("Cor:");
                                                    ui.color_edit_button_rgb(&mut light.color);
                                                    ui.end_row();

                                                    ui.label("Intensidade:");
                                                    ui.add(
                                                        egui::DragValue::new(&mut light.intensity)
                                                            .speed(0.05)
                                                            .range(0.0..=10.0),
                                                    );
                                                    ui.end_row();

                                                    ui.label("Alcance:");
                                                    ui.add(
                                                        egui::DragValue::new(&mut light.range)
                                                            .speed(0.1)
                                                            .range(0.1..=100.0),
                                                    );
                                                    ui.end_row();
                                                });
                                        });
                                    ui.add_space(8.0);
                                }
                                if remove_light {
                                    self.object_light.remove(selected_object);
                                }
                            });
                    },
                );
            });

        let near_left = (panel_rect.left() - left_snap_x).abs() <= 28.0;
        let near_right = (right_snap_right - panel_rect.right()).abs() <= 28.0;

        if self.dragging_from_header && pointer_down && (near_left || near_right) {
            let hint_w = 14.0;
            let hint_rect = if near_left {
                Rect::from_min_max(
                    egui::pos2(left_snap_x, dock_rect.top()),
                    egui::pos2(left_snap_x + hint_w, usable_bottom),
                )
            } else {
                Rect::from_min_max(
                    egui::pos2(right_snap_right - hint_w, dock_rect.top()),
                    egui::pos2(right_snap_right, usable_bottom),
                )
            };
            ctx.layer_painter(egui::LayerId::new(Order::Foreground, Id::new("dock_hint")))
                .rect_filled(
                    hint_rect,
                    6.0,
                    Color32::from_rgba_unmultiplied(15, 232, 121, 110),
                );
        }

        if header_drag_stopped || (self.dragging_from_header && !pointer_down) {
            self.dragging_from_header = false;
            if near_left {
                self.dock_side = Some(InspectorDockSide::Left);
            } else if near_right {
                self.dock_side = Some(InspectorDockSide::Right);
            } else {
                self.dock_side = None;
            }
        }

        if resize_stopped || (self.resizing_width && !pointer_down) {
            self.resizing_width = false;
        }

        if self.dragging_from_header {
            let delta = ctx.input(|i| i.pointer.delta());
            self.window_pos = Some(pos + delta);
            self.dock_side = None;
        }

        if header_drag_started {
            self.dragging_from_header = true;
        }

        if resize_started {
            self.resizing_width = true;
        }
    }

    pub fn docked_left_width(&self) -> f32 {
        if self.open && matches!(self.dock_side, Some(InspectorDockSide::Left)) {
            self.window_width
        } else {
            0.0
        }
    }

    pub fn docked_right_width(&self) -> f32 {
        if self.open && matches!(self.dock_side, Some(InspectorDockSide::Right)) {
            self.window_width
        } else {
            0.0
        }
    }
}
