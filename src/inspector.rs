use eframe::egui::{
    self, Align2, Color32, FontFamily, FontId, Id, Order, Pos2, Rect, Stroke, TextureHandle, Vec2,
};
use epaint::ColorImage;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use crate::EngineLanguage;

const INSPECTOR_MIN_WIDTH: f32 = 260.0;
const INSPECTOR_MAX_WIDTH: f32 = 520.0;

#[derive(Clone, Copy)]
struct TransformDraft {
    position: [f32; 3],
    rotation: [f32; 3],
    scale: [f32; 3],
}

#[derive(Clone, Copy)]
pub struct FiosControllerDraft {
    pub enabled: bool,
    pub move_speed: f32,
    pub rotate_speed: f32,
    pub action_speed: f32,
}

#[derive(Clone, Copy)]
pub struct RigidbodyDraft {
    pub enabled: bool,
    pub gravity: f32,
    pub jump_impulse: f32,
}

#[derive(Clone)]
pub struct AnimatorDraft {
    pub enabled: bool,
    pub module_asset: String,
    pub controller_asset: String,
    pub clip_ref: String,
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
        }
    }
}

impl Default for RigidbodyDraft {
    fn default() -> Self {
        Self {
            enabled: true,
            gravity: -9.81,
            jump_impulse: 5.2,
        }
    }
}

impl Default for AnimatorDraft {
    fn default() -> Self {
        Self {
            enabled: true,
            module_asset: String::new(),
            controller_asset: String::new(),
            clip_ref: String::new(),
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
    object_fios_controller: HashMap<String, FiosControllerDraft>,
    object_rigidbody: HashMap<String, RigidbodyDraft>,
    object_animator: HashMap<String, AnimatorDraft>,
    apply_loading_until: Option<Instant>,
    last_cursor_wrap: Option<Instant>,
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
            object_fios_controller: HashMap::new(),
            object_rigidbody: HashMap::new(),
            object_animator: HashMap::new(),
            apply_loading_until: None,
            last_cursor_wrap: None,
        }
    }

    pub fn fios_controller_targets(&self) -> Vec<(String, FiosControllerDraft)> {
        self.object_fios_controller
            .iter()
            .filter_map(|(name, cfg)| if cfg.enabled { Some((name.clone(), *cfg)) } else { None })
            .collect()
    }

    pub fn rigidbody_targets(&self) -> Vec<(String, RigidbodyDraft)> {
        self.object_rigidbody
            .iter()
            .filter_map(|(name, cfg)| if cfg.enabled { Some((name.clone(), *cfg)) } else { None })
            .collect()
    }

    pub fn animator_targets(&self) -> Vec<(String, AnimatorDraft)> {
        self.object_animator
            .iter()
            .filter_map(|(name, cfg)| if cfg.enabled { Some((name.clone(), cfg.clone())) } else { None })
            .collect()
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
        let max_width = (dock_rect.width() - left_reserved - right_reserved - 40.0)
            .max(INSPECTOR_MIN_WIDTH);
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
                        if selected_object.is_empty() {
                            return;
                        }

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
                                let header_rect = Rect::from_min_size(
                                    ui.min_rect().min,
                                    egui::vec2(ui.available_width(), header_h),
                                );
                                ui.scope_builder(egui::UiBuilder::new().max_rect(header_rect), |ui| {
                                    let toggle_text = if *enabled { "ON" } else { "OFF" };
                                    let toggle_fill = if *enabled {
                                        Color32::from_rgb(58, 118, 84)
                                    } else {
                                        Color32::from_rgb(78, 52, 52)
                                    };
                                    let toggle_stroke = if *enabled {
                                        Stroke::new(1.0, Color32::from_rgb(110, 178, 132))
                                    } else {
                                        Stroke::new(1.0, Color32::from_rgb(144, 84, 84))
                                    };
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            let clicked = ui
                                                .add_sized(
                                                    [12.0, 7.0],
                                                    egui::Button::new(
                                                        egui::RichText::new(toggle_text).size(7.0),
                                                    )
                                                        .min_size(egui::vec2(12.0, 7.0))
                                                        .fill(toggle_fill)
                                                        .stroke(toggle_stroke)
                                                        .corner_radius(6),
                                                )
                                                .clicked();
                                            if clicked {
                                                *enabled = !*enabled;
                                            }
                                        },
                                    );
                                    ui.painter().text(
                                        header_rect.center(),
                                        Align2::CENTER_CENTER,
                                        title,
                                        FontId::new(13.0, FontFamily::Proportional),
                                        Color32::from_gray(220),
                                    );
                                });
                                ui.add_space(6.0);
                                ui.label(
                                    egui::RichText::new(selected_object)
                                        .size(11.0)
                                        .color(Color32::from_gray(160)),
                                );
                                ui.add_space(6.0);

                                let label_w = 52.0_f32;
                                let row_spacing = ui.spacing().item_spacing.x;
                                let fields_total =
                                    (ui.available_width() - label_w - row_spacing * 3.0).max(72.0);
                                let field_w = (fields_total / 3.0).max(20.0);

                                let mut transform_changed = false;
                                let mut numeric_dragging = false;
                                ui.add_enabled_ui(*enabled, |ui| {
                                    let axis_labels = ["X", "Y", "Z"];
                                    ui.horizontal(|ui| {
                                        ui.add_sized([52.0, 18.0], egui::Label::new("Posição"));
                                        for i in 0..3 {
                                            ui.label(
                                                egui::RichText::new(axis_labels[i]).size(10.0),
                                            );
                                            let resp = ui.add_sized(
                                                [field_w - 10.0, 20.0],
                                                egui::DragValue::new(&mut draft.position[i]).speed(0.1),
                                            );
                                            let changed = resp.changed();
                                            numeric_dragging |= resp.dragged();
                                            transform_changed |= changed;
                                        }
                                    });
                                    ui.horizontal(|ui| {
                                        ui.add_sized([52.0, 18.0], egui::Label::new("Rotação"));
                                        for i in 0..3 {
                                            ui.label(
                                                egui::RichText::new(axis_labels[i]).size(10.0),
                                            );
                                            let resp = ui.add_sized(
                                                [field_w - 10.0, 20.0],
                                                egui::DragValue::new(&mut draft.rotation[i]).speed(0.1),
                                            );
                                            let changed = resp.changed();
                                            numeric_dragging |= resp.dragged();
                                            transform_changed |= changed;
                                        }
                                    });
                                    ui.horizontal(|ui| {
                                        ui.add_sized([52.0, 18.0], egui::Label::new("Escala"));
                                        for i in 0..3 {
                                            ui.label(
                                                egui::RichText::new(axis_labels[i]).size(10.0),
                                            );
                                            let resp = ui.add_sized(
                                                [field_w - 10.0, 20.0],
                                                egui::DragValue::new(&mut draft.scale[i]).speed(0.05),
                                            );
                                            let changed = resp.changed();
                                            numeric_dragging |= resp.dragged();
                                            transform_changed |= changed;
                                        }
                                    });
                                });
                                if numeric_dragging {
                                    let now = Instant::now();
                                    let can_wrap = self
                                        .last_cursor_wrap
                                        .is_none_or(|t| now.duration_since(t) >= Duration::from_millis(16));
                                    if can_wrap {
                                        let pointer_pos = ui.ctx().input(|i| i.pointer.hover_pos());
                                        let vp_rect = ui.ctx().input(|i| {
                                            i.viewport()
                                                .inner_rect
                                                .or(i.viewport().outer_rect)
                                        });
                                        if let (Some(mut p), Some(vr)) = (pointer_pos, vp_rect) {
                                            let margin = 2.0_f32;
                                            let mut wrapped = false;
                                            if p.x <= vr.left() + margin {
                                                p.x = vr.right() - margin;
                                                wrapped = true;
                                            } else if p.x >= vr.right() - margin {
                                                p.x = vr.left() + margin;
                                                wrapped = true;
                                            }
                                            if p.y <= vr.top() + margin {
                                                p.y = vr.bottom() - margin;
                                                wrapped = true;
                                            } else if p.y >= vr.bottom() - margin {
                                                p.y = vr.top() + margin;
                                                wrapped = true;
                                            }
                                            if wrapped {
                                                ui.ctx().send_viewport_cmd(
                                                    egui::ViewportCommand::CursorPosition(p),
                                                );
                                                self.last_cursor_wrap = Some(now);
                                            }
                                        }
                                    }
                                }
                                if *enabled && transform_changed {
                                    self.pending_live_request =
                                        Some((selected_object.to_string(), *draft));
                                }

                                ui.add_space(10.0);
                                let is_loading = self
                                    .apply_loading_until
                                    .is_some_and(|until| Instant::now() < until);
                                let button_label = if is_loading {
                                    loading_text
                                } else {
                                    apply_text
                                };
                                let button_resp = ui
                                    .add_enabled_ui(*enabled, |ui| {
                                        ui.add_sized(
                                            [ui.available_width(), 30.0],
                                            egui::Button::new(
                                                egui::RichText::new(button_label)
                                                    .size(13.0)
                                                    .color(Color32::from_rgb(55, 55, 55))
                                                    .strong(),
                                            )
                                            .fill(Color32::from_rgb(148, 116, 186))
                                            .stroke(Stroke::new(1.0, Color32::from_rgb(173, 140, 208)))
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
                                if is_loading {
                                    ui.ctx().request_repaint_after(Duration::from_millis(16));
                                }
                            });
                    },
                );
                ui.scope_builder(
                    egui::UiBuilder::new().max_rect(button_rect).layout(
                        egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                    ),
                    |ui| {
                        let mut button = egui::Button::new(
                            egui::RichText::new(match language {
                                EngineLanguage::Pt => "Componente",
                                EngineLanguage::En => "Component",
                                EngineLanguage::Es => "Componente",
                            })
                            .text_style(egui::TextStyle::Button)
                            .font(FontId::new(14.0, FontFamily::Proportional))
                            .strong()
                            .color(Color32::from_rgb(55, 55, 55)),
                        )
                        .fill(Color32::from_rgb(0x0F, 0xE8, 0x79))
                        .corner_radius(3)
                        .min_size(egui::vec2(82.0, 16.0));

                        if let Some(add_tex) = &self.add_icon_texture {
                            button = egui::Button::image_and_text(
                                egui::Image::new(add_tex).fit_to_exact_size(egui::vec2(12.0, 12.0)),
                                egui::RichText::new(match language {
                                    EngineLanguage::Pt => "Componente",
                                    EngineLanguage::En => "Component",
                                    EngineLanguage::Es => "Componente",
                                })
                                .font(FontId::new(14.0, FontFamily::Proportional))
                                .strong()
                                .color(Color32::from_rgb(55, 55, 55)),
                            )
                            .fill(Color32::from_rgb(0x0F, 0xE8, 0x79))
                            .corner_radius(3)
                            .min_size(egui::vec2(82.0, 16.0));
                        }

                        let add_resp = ui.add(button);
                        if !selected_object.is_empty() {
                            add_resp.context_menu(|ui| {
                                if ui.button("Fios Controller").clicked() {
                                    self.object_fios_controller
                                        .entry(selected_object.to_string())
                                        .or_default();
                                    ui.close();
                                }
                                if ui.button("Rigidbody").clicked() {
                                    self.object_rigidbody
                                        .entry(selected_object.to_string())
                                        .or_default();
                                    ui.close();
                                }
                                if ui.button("Animator").clicked() {
                                    self.object_animator
                                        .entry(selected_object.to_string())
                                        .or_default();
                                    ui.close();
                                }
                            });
                            if add_resp.clicked() {
                                self.object_fios_controller
                                    .entry(selected_object.to_string())
                                    .or_default();
                            }
                        }
                    },
                );

                if !selected_object.is_empty() {
                    if let Some(ctrl) = self.object_fios_controller.get_mut(selected_object) {
                        let mut remove_component = false;
                        let card_height = 178.0_f32;
                        let card_rect = Rect::from_min_max(
                            egui::pos2(inner.min.x, (button_rect.min.y - card_height - 10.0).max(sep_y + 220.0)),
                            egui::pos2(inner.max.x, button_rect.min.y - 10.0),
                        );
                        if card_rect.height() > 70.0 {
                            ui.scope_builder(
                                egui::UiBuilder::new()
                                    .max_rect(card_rect)
                                    .layout(egui::Layout::top_down(egui::Align::Min)),
                                |ui| {
                                    egui::Frame::new()
                                        .fill(Color32::from_rgb(36, 36, 36))
                                        .stroke(Stroke::new(1.0, Color32::from_gray(62)))
                                        .corner_radius(6)
                                        .inner_margin(egui::Margin::same(8))
                                        .show(ui, |ui| {
                                            ui.horizontal(|ui| {
                                                ui.label(
                                                    egui::RichText::new("Fios Controller")
                                                        .size(13.0)
                                                        .strong()
                                                        .color(Color32::from_gray(220)),
                                                );
                                                ui.with_layout(
                                                    egui::Layout::right_to_left(egui::Align::Center),
                                                    |ui| {
                                                        let rm = ui
                                                            .add_sized([44.0, 18.0], egui::Button::new("Remover"))
                                                            .clicked();
                                                        if rm {
                                                            remove_component = true;
                                                        }
                                                    },
                                                );
                                            });
                                            ui.add_space(6.0);
                                            ui.horizontal(|ui| {
                                                ui.label("Ativo");
                                                let txt = if ctrl.enabled { "ON" } else { "OFF" };
                                                let fill = if ctrl.enabled {
                                                    Color32::from_rgb(58, 118, 84)
                                                } else {
                                                    Color32::from_rgb(78, 52, 52)
                                                };
                                                if ui
                                                    .add_sized(
                                                        [40.0, 18.0],
                                                        egui::Button::new(txt)
                                                            .fill(fill)
                                                            .stroke(Stroke::new(1.0, Color32::from_gray(90))),
                                                    )
                                                    .clicked()
                                                {
                                                    ctrl.enabled = !ctrl.enabled;
                                                }
                                            });
                                            ui.add_space(4.0);
                                            ui.horizontal(|ui| {
                                                ui.label("Move Speed");
                                                let _ = ui.add_sized(
                                                    [90.0, 20.0],
                                                    egui::DragValue::new(&mut ctrl.move_speed)
                                                        .range(0.1..=100.0)
                                                        .speed(0.1),
                                                );
                                            });
                                            ui.horizontal(|ui| {
                                                ui.label("Rotate Speed");
                                                let _ = ui.add_sized(
                                                    [90.0, 20.0],
                                                    egui::DragValue::new(&mut ctrl.rotate_speed)
                                                        .range(1.0..=720.0)
                                                        .speed(1.0),
                                                );
                                            });
                                            ui.horizontal(|ui| {
                                                ui.label("Action Speed");
                                                let _ = ui.add_sized(
                                                    [90.0, 20.0],
                                                    egui::DragValue::new(&mut ctrl.action_speed)
                                                        .range(0.1..=20.0)
                                                        .speed(0.1),
                                                );
                                            });
                                            ui.add_space(6.0);
                                            ui.horizontal(|ui| {
                                                ui.label(
                                                    egui::RichText::new(match language {
                                                        EngineLanguage::Pt => "Entradas",
                                                        EngineLanguage::En => "Inputs",
                                                        EngineLanguage::Es => "Entradas",
                                                    })
                                                    .size(11.0)
                                                    .strong()
                                                    .color(Color32::from_gray(210)),
                                                );
                                                ui.label(
                                                    egui::RichText::new("MoveAxis, LookAxis, Action")
                                                        .size(10.0)
                                                        .color(Color32::from_gray(165)),
                                                );
                                            });
                                            ui.horizontal(|ui| {
                                                ui.label(
                                                    egui::RichText::new(match language {
                                                        EngineLanguage::Pt => "Saidas",
                                                        EngineLanguage::En => "Outputs",
                                                        EngineLanguage::Es => "Salidas",
                                                    })
                                                    .size(11.0)
                                                    .strong()
                                                    .color(Color32::from_gray(210)),
                                                );
                                                ui.label(
                                                    egui::RichText::new("Move, Rotate, Action")
                                                        .size(10.0)
                                                        .color(Color32::from_gray(165)),
                                                );
                                            });
                                        });
                                },
                            );
                        }
                        if remove_component {
                            self.object_fios_controller.remove(selected_object);
                        }
                    }
                    if let Some(rb) = self.object_rigidbody.get_mut(selected_object) {
                        let mut remove_component = false;
                        let card_height = 138.0_f32;
                        let card_rect = Rect::from_min_max(
                            egui::pos2(inner.min.x, (button_rect.min.y - card_height - 196.0).max(sep_y + 38.0)),
                            egui::pos2(inner.max.x, button_rect.min.y - 196.0),
                        );
                        if card_rect.height() > 70.0 {
                            ui.scope_builder(
                                egui::UiBuilder::new()
                                    .max_rect(card_rect)
                                    .layout(egui::Layout::top_down(egui::Align::Min)),
                                |ui| {
                                    egui::Frame::new()
                                        .fill(Color32::from_rgb(34, 38, 42))
                                        .stroke(Stroke::new(1.0, Color32::from_gray(62)))
                                        .corner_radius(6)
                                        .inner_margin(egui::Margin::same(8))
                                        .show(ui, |ui| {
                                            ui.horizontal(|ui| {
                                                ui.label(
                                                    egui::RichText::new("Rigidbody")
                                                        .size(13.0)
                                                        .strong()
                                                        .color(Color32::from_gray(220)),
                                                );
                                                ui.with_layout(
                                                    egui::Layout::right_to_left(egui::Align::Center),
                                                    |ui| {
                                                        let rm = ui
                                                            .add_sized([44.0, 18.0], egui::Button::new("Remover"))
                                                            .clicked();
                                                        if rm {
                                                            remove_component = true;
                                                        }
                                                    },
                                                );
                                            });
                                            ui.add_space(6.0);
                                            ui.horizontal(|ui| {
                                                ui.label("Ativo");
                                                let txt = if rb.enabled { "ON" } else { "OFF" };
                                                let fill = if rb.enabled {
                                                    Color32::from_rgb(58, 118, 84)
                                                } else {
                                                    Color32::from_rgb(78, 52, 52)
                                                };
                                                if ui
                                                    .add_sized(
                                                        [40.0, 18.0],
                                                        egui::Button::new(txt)
                                                            .fill(fill)
                                                            .stroke(Stroke::new(1.0, Color32::from_gray(90))),
                                                    )
                                                    .clicked()
                                                {
                                                    rb.enabled = !rb.enabled;
                                                }
                                            });
                                            ui.horizontal(|ui| {
                                                ui.label("Gravity");
                                                let _ = ui.add_sized(
                                                    [90.0, 20.0],
                                                    egui::DragValue::new(&mut rb.gravity)
                                                        .range(-80.0..=80.0)
                                                        .speed(0.1),
                                                );
                                            });
                                            ui.horizontal(|ui| {
                                                ui.label("Jump Impulse");
                                                let _ = ui.add_sized(
                                                    [90.0, 20.0],
                                                    egui::DragValue::new(&mut rb.jump_impulse)
                                                        .range(0.1..=40.0)
                                                        .speed(0.1),
                                                );
                                            });
                                        });
                                },
                            );
                        }
                        if remove_component {
                            self.object_rigidbody.remove(selected_object);
                        }
                    }
                    if let Some(animator) = self.object_animator.get_mut(selected_object) {
                        let mut remove_component = false;
                        let card_height = 164.0_f32;
                        let card_rect = Rect::from_min_max(
                            egui::pos2(inner.min.x, (button_rect.min.y - card_height - 348.0).max(sep_y + 38.0)),
                            egui::pos2(inner.max.x, button_rect.min.y - 348.0),
                        );
                        if card_rect.height() > 70.0 {
                            ui.scope_builder(
                                egui::UiBuilder::new()
                                    .max_rect(card_rect)
                                    .layout(egui::Layout::top_down(egui::Align::Min)),
                                |ui| {
                                    egui::Frame::new()
                                        .fill(Color32::from_rgb(38, 34, 44))
                                        .stroke(Stroke::new(1.0, Color32::from_gray(62)))
                                        .corner_radius(6)
                                        .inner_margin(egui::Margin::same(8))
                                        .show(ui, |ui| {
                                            ui.horizontal(|ui| {
                                                ui.label(
                                                    egui::RichText::new("Animator")
                                                        .size(13.0)
                                                        .strong()
                                                        .color(Color32::from_gray(220)),
                                                );
                                                ui.with_layout(
                                                    egui::Layout::right_to_left(egui::Align::Center),
                                                    |ui| {
                                                        let rm = ui
                                                            .add_sized([60.0, 18.0], egui::Button::new("Remover"))
                                                            .clicked();
                                                        if rm {
                                                            remove_component = true;
                                                        }
                                                    },
                                                );
                                            });
                                            ui.add_space(6.0);
                                            ui.horizontal(|ui| {
                                                ui.label("Ativo");
                                                let txt = if animator.enabled { "ON" } else { "OFF" };
                                                let fill = if animator.enabled {
                                                    Color32::from_rgb(58, 118, 84)
                                                } else {
                                                    Color32::from_rgb(78, 52, 52)
                                                };
                                                if ui
                                                    .add_sized(
                                                        [40.0, 18.0],
                                                        egui::Button::new(txt)
                                                            .fill(fill)
                                                            .stroke(Stroke::new(1.0, Color32::from_gray(90))),
                                                    )
                                                    .clicked()
                                                {
                                                    animator.enabled = !animator.enabled;
                                                }
                                            });
                                            ui.add_space(6.0);
                                            ui.horizontal(|ui| {
                                                ui.label("Módulo");
                                                egui::ComboBox::from_id_salt(("anim_module_combo", selected_object))
                                                    .width(180.0)
                                                    .selected_text(if animator.module_asset.is_empty() {
                                                        "Nenhum".to_string()
                                                    } else {
                                                        animator.module_asset.clone()
                                                    })
                                                    .show_ui(ui, |ui| {
                                                        ui.selectable_value(
                                                            &mut animator.module_asset,
                                                            String::new(),
                                                            "Nenhum",
                                                        );
                                                        for module in animation_modules {
                                                            ui.selectable_value(
                                                                &mut animator.module_asset,
                                                                module.clone(),
                                                                module,
                                                            );
                                                        }
                                                    });
                                            });
                                            ui.horizontal(|ui| {
                                                if ui.button("Aplicar módulo").clicked() {
                                                    if let Some(clip) =
                                                        module_default_clip(&animator.module_asset)
                                                    {
                                                        if fbx_animation_clips.iter().any(|c| c.eq_ignore_ascii_case(&clip)) {
                                                            animator.clip_ref = clip;
                                                        } else {
                                                            animator.clip_ref = clip;
                                                        }
                                                    }
                                                }
                                            });
                                            ui.horizontal(|ui| {
                                                ui.label("Controller");
                                                egui::ComboBox::from_id_salt(("anim_ctrl_combo", selected_object))
                                                    .width(180.0)
                                                    .selected_text(if animator.controller_asset.is_empty() {
                                                        "Nenhum".to_string()
                                                    } else {
                                                        animator.controller_asset.clone()
                                                    })
                                                    .show_ui(ui, |ui| {
                                                        ui.selectable_value(
                                                            &mut animator.controller_asset,
                                                            String::new(),
                                                            "Nenhum",
                                                        );
                                                        for controller in animation_controllers {
                                                            ui.selectable_value(
                                                                &mut animator.controller_asset,
                                                                controller.clone(),
                                                                controller,
                                                            );
                                                        }
                                                    });
                                            });
                                            ui.horizontal(|ui| {
                                                ui.label("Animação");
                                                egui::ComboBox::from_id_salt(("anim_clip_combo", selected_object))
                                                    .width(180.0)
                                                    .selected_text(if animator.clip_ref.is_empty() {
                                                        "Nenhuma".to_string()
                                                    } else {
                                                        animator.clip_ref.clone()
                                                    })
                                                    .show_ui(ui, |ui| {
                                                        ui.selectable_value(
                                                            &mut animator.clip_ref,
                                                            String::new(),
                                                            "Nenhuma",
                                                        );
                                                        for clip in fbx_animation_clips {
                                                            ui.selectable_value(
                                                                &mut animator.clip_ref,
                                                                clip.clone(),
                                                                clip,
                                                            );
                                                        }
                                                    });
                                            });
                                        });
                                },
                            );
                        }
                        if remove_component {
                            self.object_animator.remove(selected_object);
                        }
                    }
                }

                let handle_w = 10.0;
                let handle_rect = match self.dock_side {
                    Some(InspectorDockSide::Right) => Rect::from_min_max(
                        egui::pos2(rect.left(), rect.top()),
                        egui::pos2(rect.left() + handle_w, rect.bottom()),
                    ),
                    _ => Rect::from_min_max(
                        egui::pos2(rect.right() - handle_w, rect.top()),
                        egui::pos2(rect.right(), rect.bottom()),
                    ),
                };
                let resize_resp = ui.interact(
                    handle_rect,
                    ui.id().with("width_resize_handle"),
                    egui::Sense::click_and_drag(),
                );
                if resize_resp.hovered() || resize_resp.dragged() {
                    ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeHorizontal);
                }
                if resize_resp.drag_started() {
                    resize_started = true;
                }
                if resize_resp.drag_stopped() {
                    resize_stopped = true;
                }
            });

        if header_drag_started {
            self.dragging_from_header = true;
            self.resizing_width = false;
            self.dock_side = None;
        }
        if resize_started {
            self.resizing_width = true;
            self.dragging_from_header = false;
        }

        let delta = ctx.input(|i| i.pointer.delta());
        if pointer_down {
            if self.dragging_from_header && delta != Vec2::ZERO {
                if let Some(p) = self.window_pos {
                    self.window_pos = Some(p + delta);
                }
            } else if self.resizing_width && delta.x != 0.0 {
                match self.dock_side {
                    Some(InspectorDockSide::Right) => {
                        let old_w = self.window_width;
                        let new_w =
                            (old_w - delta.x).clamp(INSPECTOR_MIN_WIDTH, INSPECTOR_MAX_WIDTH);
                        let applied = old_w - new_w;
                        self.window_width = new_w;
                        if let Some(p) = self.window_pos {
                            self.window_pos = Some(egui::pos2(p.x + applied, p.y));
                        }
                    }
                    _ => {
                        self.window_width = (self.window_width + delta.x)
                            .clamp(INSPECTOR_MIN_WIDTH, INSPECTOR_MAX_WIDTH);
                    }
                }
            }
        }

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
