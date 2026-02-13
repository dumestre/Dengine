use eframe::egui::{
    self, Align2, Color32, FontFamily, FontId, Id, Order, Pos2, Rect, Stroke, TextureHandle, Vec2,
};
use epaint::ColorImage;
use std::sync::Arc;
use crate::EngineLanguage;

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
            window_width: 190.0,
            dragging_from_header: false,
            resizing_width: false,
            fonts_initialized: false,
        }
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        left_reserved: f32,
        right_reserved: f32,
        bottom_reserved: f32,
        language: EngineLanguage,
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
        let max_width = (dock_rect.width() - left_reserved - right_reserved - 40.0).max(180.0);
        self.window_width = self.window_width.clamp(180.0, max_width.min(520.0));
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

                let button_rect = Rect::from_min_max(
                    egui::pos2(inner.min.x, sep_y + 8.0),
                    egui::pos2(inner.max.x, sep_y + 40.0),
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

                        let _ = ui.add(button);
                    },
                );

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
                        let new_w = (old_w - delta.x).clamp(180.0, 520.0);
                        let applied = old_w - new_w;
                        self.window_width = new_w;
                        if let Some(p) = self.window_pos {
                            self.window_pos = Some(egui::pos2(p.x + applied, p.y));
                        }
                    }
                    _ => {
                        self.window_width = (self.window_width + delta.x).clamp(180.0, 520.0);
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
