use eframe::egui::{
    self, Color32, FontFamily, FontId, LayerId, Order, Pos2, Stroke, Vec2, TextureHandle,
    TextureOptions,
};
use epaint::ColorImage;
use std::sync::Arc;

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
    drag_anchor_window_pos: Option<Pos2>,
    force_window_pos: bool,
}

#[derive(Clone, Copy)]
enum InspectorDockSide {
    Left,
    Right,
}

// Função para carregar PNG como textura
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

    Some(ctx.load_texture(png_path.to_owned(), color_image, TextureOptions::LINEAR))
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
            drag_anchor_window_pos: None,
            force_window_pos: true,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        // Carrega os ícones se ainda não estiverem carregados
        if self.menu_icon_texture.is_none() {
            self.menu_icon_texture = load_png_as_texture(ctx, "src/assets/icons/more.png", None);
        }
        
        if self.lock_icon_texture.is_none() {
            self.lock_icon_texture = load_png_as_texture(ctx, "src/assets/icons/lock.png", None);
        }

        if self.unlock_icon_texture.is_none() {
            self.unlock_icon_texture = load_png_as_texture(ctx, "src/assets/icons/unlock.png", None);
        }

        if self.add_icon_texture.is_none() {
            self.add_icon_texture = load_png_as_texture(
                ctx,
                "src/assets/icons/add.png",
                Some(Color32::from_rgb(55, 55, 55)),
            );
        }

        // === Configura Roboto ===
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "Roboto".to_owned(),
            Arc::new(egui::FontData::from_static(include_bytes!(
                "assets/fonts/roboto.ttf"
            ))),
        );
        fonts
            .families
            .insert(FontFamily::Proportional, vec!["Roboto".to_owned()]);
        ctx.set_fonts(fonts);

        // === Janela do inspetor ===
        let dock_rect = ctx.available_rect();
        let is_docked = self.dock_side.is_some();
        let docked_height = dock_rect.height().max(120.0);
        let floating_height = (dock_rect.height() * 0.85).max(520.0);
        let window_width = self.window_width.clamp(180.0, 520.0);
        let window_size = egui::Vec2::new(
            window_width,
            if is_docked {
                docked_height
            } else {
                floating_height
            },
        );

        if self.window_pos.is_none() {
            self.window_pos = Some(egui::pos2(dock_rect.left(), dock_rect.top()));
            self.force_window_pos = true;
        }

        let mut inspector_window = egui::Window::new("inspetor_window_id")
            .open(&mut self.open)
            .title_bar(false)
            .movable(false)
            .resizable([true, false])
            .collapsible(false)
            .min_width(180.0)
            .max_width(520.0)
            .min_height(window_size.y)
            .max_height(window_size.y)
            .default_width(window_size.x)
            .default_height(window_size.y);

        let applied_forced_pos = self.dragging_from_header || self.force_window_pos;
        if applied_forced_pos {
            if let Some(pos) = self.window_pos {
                inspector_window = inspector_window.current_pos(pos);
            }
        }

        let mut header_drag_started = false;
        let mut header_drag_stopped = false;
        let mut header_drag_delta = egui::Vec2::ZERO;

        let window_response = inspector_window.show(ctx, |ui| {
                ui.spacing_mut().item_spacing.y = 4.0;

                // Título com ícones
                ui.horizontal(|ui| {
                    let side_width = 40.0;
                    let title_width = (ui.available_width() - (side_width * 2.0)).max(0.0);
                    let drag_zone_start = ui.cursor().min;

                    // Espaço à esquerda espelhando a largura dos ícones, para centralizar o título.
                    ui.allocate_space(egui::Vec2::new(side_width, 0.0));

                    // Título centralizado no centro real da janela.
                    let title_ir = ui.allocate_ui_with_layout(
                        egui::Vec2::new(title_width, 16.0),
                        egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                        |ui| {
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new("Inspetor")
                                        .font(FontId::new(13.0, FontFamily::Proportional))
                                        .strong(),
                                )
                                .sense(egui::Sense::hover())
                                .selectable(false),
                            );
                        },
                    );
                    let drag_zone = egui::Rect::from_min_max(
                        egui::pos2(drag_zone_start.x, title_ir.response.rect.top()),
                        egui::pos2(title_ir.response.rect.right(), title_ir.response.rect.bottom()),
                    );
                    let header_drag_response =
                        ui.interact(drag_zone, ui.id().with("header_drag"), egui::Sense::click_and_drag());
                    if header_drag_response.drag_started() {
                        header_drag_started = true;
                    }
                    if header_drag_response.drag_stopped() {
                        header_drag_stopped = true;
                    }
                    header_drag_delta = header_drag_response.drag_delta();

                    // Área dos ícones à direita.
                    ui.allocate_ui_with_layout(
                        egui::Vec2::new(side_width, 16.0),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            // Ícone lock (à esquerda do ícone menu)
                            let lock_texture = if self.is_locked {
                                self.lock_icon_texture.as_ref()
                            } else {
                                self.unlock_icon_texture.as_ref()
                            };

                            if let Some(lock_texture) = lock_texture {
                                let lock_response = ui.add(
                                    egui::Image::new(lock_texture)
                                        .fit_to_exact_size(egui::Vec2::new(16.0, 16.0))
                                        .sense(egui::Sense::click()),
                                );
                                if lock_response.clicked() {
                                    self.is_locked = !self.is_locked;
                                }
                            } else {
                                ui.allocate_space(egui::Vec2::new(16.0, 16.0));
                            }

                            ui.allocate_space(egui::Vec2::new(5.0, 0.0));

                            // Ícone menu (à direita)
                            if let Some(menu_texture) = &self.menu_icon_texture {
                                let menu_response = ui.add(
                                    egui::Image::new(menu_texture)
                                        .fit_to_exact_size(egui::Vec2::new(16.0, 16.0))
                                        .sense(egui::Sense::click()),
                                );

                                egui::Popup::menu(&menu_response)
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
                                                    egui::RichText::new("Copiar cadeia de componentes")
                                                        .color(Color32::WHITE),
                                                )
                                                .fill(Color32::from_rgb(62, 62, 62))
                                                .corner_radius(6),
                                            )
                                            .clicked();
                                        if copy_clicked {
                                            ui.ctx().copy_text("cadeia de componentes".to_owned());
                                            println!("Cadeia de componentes copiada.");
                                            ui.close();
                                        }

                                        let send_clicked = ui
                                            .add_sized(
                                                [208.0, 26.0],
                                                egui::Button::new(
                                                    egui::RichText::new("Enviar cadeia para...")
                                                        .color(Color32::WHITE),
                                                )
                                                .fill(Color32::from_rgb(62, 62, 62))
                                                .corner_radius(6),
                                            )
                                            .clicked();
                                        if send_clicked {
                                            println!("Ação: Enviar cadeia para...");
                                            ui.close();
                                        }
                                    });
                            } else {
                                ui.allocate_space(egui::Vec2::new(16.0, 16.0));
                            }
                        },
                    );
                });

                ui.separator();

                // Botão centralizado, verde, maior e arredondado
                ui.vertical_centered(|ui| {
                    let add_icon = self
                        .add_icon_texture
                        .as_ref()
                        .map(|texture| egui::Image::new(texture).fit_to_exact_size(egui::Vec2::new(12.0, 12.0)));

                    let mut button = egui::Button::new(
                        egui::RichText::new("Componente")
                            .font(FontId::new(14.0, FontFamily::Proportional))
                            .strong()
                            .color(Color32::from_rgb(55, 55, 55)),
                    );

                    if let Some(icon) = add_icon {
                        button = egui::Button::image_and_text(
                            icon,
                            egui::RichText::new("Componente")
                                .font(FontId::new(14.0, FontFamily::Proportional))
                                .strong()
                                .color(Color32::from_rgb(55, 55, 55)),
                        );
                    }

                    let button = button
                        .min_size(Vec2::new(0.0, 25.0)) // altura maior
                        .corner_radius(3) // borda arredondada pequena
                        .fill(Color32::from_rgb(0x0F, 0xE8, 0x79)); // verde

                    let response = ui.add(button);

                    if response.clicked() {
                        println!("Botão Componente clicado!");
                    }
                });
            });

        if let Some(window_response) = window_response {
            if header_drag_started {
                self.dragging_from_header = true;
                self.dock_side = None;
                self.drag_anchor_window_pos = Some(window_response.response.rect.min);
                self.window_pos = self.drag_anchor_window_pos;
                self.force_window_pos = true;
            }

            if self.dragging_from_header && ctx.input(|i| i.pointer.primary_down()) {
                if let Some(anchor) = self.drag_anchor_window_pos {
                    self.window_pos = Some(anchor + header_drag_delta);
                    self.force_window_pos = true;
                }
            }

            let rect = window_response.response.rect;
            self.window_width = rect.width().clamp(180.0, 520.0);
            if !self.dragging_from_header {
                self.window_pos = Some(rect.min);
            }
            let content_rect = dock_rect;

            if let Some(dock_side) = self.dock_side {
                if !self.dragging_from_header && !ctx.input(|i| i.pointer.primary_down()) {
                    let x = match dock_side {
                        InspectorDockSide::Left => content_rect.left(),
                        InspectorDockSide::Right => content_rect.right() - rect.width(),
                    };
                    let docked_pos = egui::pos2(x, content_rect.top());
                    if rect.min.distance(docked_pos) > 0.5 {
                        self.window_pos = Some(docked_pos);
                        self.force_window_pos = true;
                    }
                }
            }

            let snap_distance = 28.0;
            let near_left = (rect.left() - content_rect.left()).abs() <= snap_distance;
            let near_right = (content_rect.right() - rect.right()).abs() <= snap_distance;

            let is_active_drag = self.dragging_from_header && ctx.input(|i| i.pointer.primary_down());

            if is_active_drag && (near_left || near_right) {
                let painter =
                    ctx.layer_painter(LayerId::new(Order::Foreground, egui::Id::new("dock_hint")));
                let hint_width = 14.0;
                let hint_rect = if near_left {
                    egui::Rect::from_min_max(
                        egui::pos2(content_rect.left(), content_rect.top()),
                        egui::pos2(content_rect.left() + hint_width, content_rect.bottom()),
                    )
                } else {
                    egui::Rect::from_min_max(
                        egui::pos2(content_rect.right() - hint_width, content_rect.top()),
                        egui::pos2(content_rect.right(), content_rect.bottom()),
                    )
                };

                painter.rect_filled(
                    hint_rect,
                    6.0,
                    Color32::from_rgba_unmultiplied(15, 232, 121, 110),
                );
            }

            if header_drag_stopped || (self.dragging_from_header && !ctx.input(|i| i.pointer.primary_down())) {
                self.dragging_from_header = false;
                if near_left {
                    self.dock_side = Some(InspectorDockSide::Left);
                } else if near_right {
                    self.dock_side = Some(InspectorDockSide::Right);
                } else {
                    self.dock_side = None;
                }
                self.drag_anchor_window_pos = None;
                self.force_window_pos = true;
            }
        }

        if applied_forced_pos && !self.dragging_from_header {
            self.force_window_pos = false;
        }
    }
}
