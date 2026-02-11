use eframe::egui::{
    self, Align2, Color32, FontFamily, FontId, LayerId, Order, Stroke, Vec2, TextureHandle,
    TextureOptions,
};
use std::sync::Arc;
use resvg::tiny_skia;
use epaint::ColorImage;

pub struct InspectorWindow {
    pub open: bool,
    menu_icon_texture: Option<TextureHandle>,
    lock_icon_texture: Option<TextureHandle>,
    unlock_icon_texture: Option<TextureHandle>,
    add_icon_texture: Option<TextureHandle>,
    is_locked: bool,
    dock_side: Option<InspectorDockSide>,
}

#[derive(Clone, Copy)]
enum InspectorDockSide {
    Left,
    Right,
}

// Função para carregar SVG como textura
fn load_svg_as_texture(
    ctx: &egui::Context,
    svg_path: &str,
    tint: Option<Color32>,
) -> Option<TextureHandle> {
    // Lê o conteúdo do arquivo SVG
    let svg_data = std::fs::read_to_string(svg_path).ok()?;
    
    // Configura opções para parsing do SVG
    let options = usvg::Options::default();
    
    // Parse do SVG
    let tree = usvg::Tree::from_str(&svg_data, &options).ok()?;
    
    // Define o tamanho para renderização
    let pixmap_size = tiny_skia::IntSize::from_wh(tree.size().width() as u32, tree.size().height() as u32)?;
    
    // Cria um pixmap para renderizar o SVG
    let mut pixmap = tiny_skia::Pixmap::new(pixmap_size.width(), pixmap_size.height())?;
    
    // Renderiza o SVG no pixmap
    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());
    
    // Converte o pixmap para ColorImage
    let img = pixmap.take();
    let mut rgba = img.as_slice().to_vec();
    if let Some(tint) = tint {
        for px in rgba.chunks_exact_mut(4) {
            if px[3] > 0 {
                px[0] = tint.r();
                px[1] = tint.g();
                px[2] = tint.b();
            }
        }
    }

    let color_image = ColorImage::from_rgba_unmultiplied(
        [pixmap_size.width() as usize, pixmap_size.height() as usize],
        &rgba,
    );
    
    // Cria a textura no contexto do egui
    Some(ctx.load_texture(
        svg_path.to_owned(),
        color_image,
        TextureOptions::LINEAR,
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
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        // Carrega os ícones se ainda não estiverem carregados
        if self.menu_icon_texture.is_none() {
            self.menu_icon_texture = load_svg_as_texture(ctx, "src/assets/icons/more.svg", None);
        }
        
        if self.lock_icon_texture.is_none() {
            self.lock_icon_texture = load_svg_as_texture(ctx, "src/assets/icons/lock.svg", None);
        }

        if self.unlock_icon_texture.is_none() {
            self.unlock_icon_texture = load_svg_as_texture(ctx, "src/assets/icons/unlock.svg", None);
        }

        if self.add_icon_texture.is_none() {
            self.add_icon_texture = load_svg_as_texture(
                ctx,
                "src/assets/icons/add.svg",
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
        let window_size = egui::Vec2::new(210.0, dock_rect.height().max(120.0));
        let mut inspector_window = egui::Window::new("inspetor_window_id")
            .open(&mut self.open)
            .title_bar(false)
            .movable(true)
            .resizable(false)
            .collapsible(false)
            .fixed_size(window_size)
            .default_height(window_size.y);

        if let Some(dock_side) = self.dock_side {
            if !ctx.input(|i| i.pointer.primary_down()) {
                let y_offset = dock_rect.top();
                let x = match dock_side {
                    InspectorDockSide::Left => dock_rect.left(),
                    InspectorDockSide::Right => dock_rect.right() - window_size.x,
                };
                inspector_window = inspector_window.current_pos(egui::pos2(x, y_offset));
            }
        }

        let window_response = inspector_window.show(ctx, |ui| {
                ui.spacing_mut().item_spacing.y = 4.0;

                // Título com ícones
                ui.horizontal(|ui| {
                    let side_width = 40.0;
                    let title_width = (ui.available_width() - (side_width * 2.0)).max(0.0);

                    // Espaço à esquerda espelhando a largura dos ícones, para centralizar o título.
                    ui.allocate_space(egui::Vec2::new(side_width, 0.0));

                    // Título centralizado no centro real da janela.
                    ui.allocate_ui_with_layout(
                        egui::Vec2::new(title_width, 16.0),
                        egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                        |ui| {
                            ui.label(
                                egui::RichText::new("Inspetor")
                                    .font(FontId::new(13.0, FontFamily::Proportional))
                                    .strong(),
                            );
                        },
                    );

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
            let rect = window_response.response.rect;
            let content_rect = dock_rect;
            let snap_distance = 28.0;
            let near_left = (rect.left() - content_rect.left()).abs() <= snap_distance;
            let near_right = (content_rect.right() - rect.right()).abs() <= snap_distance;

            let has_pointer_motion = ctx.input(|i| i.pointer.delta().length_sq() > 0.0);
            let is_active_drag = window_response.response.dragged()
                && ctx.input(|i| i.pointer.primary_down())
                && has_pointer_motion;

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
                painter.text(
                    hint_rect.center_top() + egui::vec2(0.0, -18.0),
                    Align2::CENTER_CENTER,
                    "Solte para encaixar",
                    FontId::new(11.0, FontFamily::Proportional),
                    Color32::from_rgb(195, 255, 220),
                );
            }

            if window_response.response.drag_stopped() {
                if near_left {
                    self.dock_side = Some(InspectorDockSide::Left);
                } else if near_right {
                    self.dock_side = Some(InspectorDockSide::Right);
                } else {
                    self.dock_side = None;
                }
            }
        }
    }
}
