// src/main.rs
mod inspector;
mod hierarchy;
mod project;

use eframe::egui::{TextureHandle, TextureOptions};
use eframe::{App, Frame, NativeOptions, egui};
use epaint::ColorImage;
use hierarchy::HierarchyWindow;
use inspector::InspectorWindow;
use project::ProjectWindow;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::sync::Arc;
#[cfg(target_os = "windows")]
use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EngineLanguage {
    Pt,
    En,
    Es,
}

impl EngineLanguage {
}

struct EditorApp {
    inspector: InspectorWindow,
    hierarchy: HierarchyWindow,
    project: ProjectWindow,
    app_icon_texture: Option<TextureHandle>,
    cena_icon: Option<TextureHandle>,
    game_icon: Option<TextureHandle>,
    play_icon: Option<TextureHandle>,
    pause_icon: Option<TextureHandle>,
    stop_icon: Option<TextureHandle>,
    files_icon: Option<TextureHandle>,
    lang_pt_icon: Option<TextureHandle>,
    lang_en_icon: Option<TextureHandle>,
    lang_es_icon: Option<TextureHandle>,
    is_playing: bool,
    is_window_maximized: bool,
    selected_mode: ToolbarMode,
    language: EngineLanguage,
    project_collapsed: bool,
    windows_blur_initialized: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ToolbarMode {
    Cena,
    Game,
}

fn load_png_as_texture(ctx: &egui::Context, png_path: &str) -> Option<TextureHandle> {
    let bytes = std::fs::read(png_path).ok()?;
    let rgba = image::load_from_memory(&bytes).ok()?.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let color_image = ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
    Some(ctx.load_texture(png_path.to_owned(), color_image, TextureOptions::LINEAR))
}

fn load_icon_data_from_png(png_path: &str) -> Option<Arc<egui::IconData>> {
    let bytes = std::fs::read(png_path).ok()?;
    let rgba = image::load_from_memory(&bytes).ok()?.to_rgba8();
    let width = rgba.width();
    let height = rgba.height();
    Some(Arc::new(egui::IconData {
        rgba: rgba.into_raw(),
        width,
        height,
    }))
}

impl EditorApp {
    fn language_name(&self, lang: EngineLanguage) -> &'static str {
        match lang {
            EngineLanguage::Pt => "Português",
            EngineLanguage::En => "English",
            EngineLanguage::Es => "Español",
        }
    }

    fn language_icon(&self, lang: EngineLanguage) -> Option<&TextureHandle> {
        match lang {
            EngineLanguage::Pt => self.lang_pt_icon.as_ref(),
            EngineLanguage::En => self.lang_en_icon.as_ref(),
            EngineLanguage::Es => self.lang_es_icon.as_ref(),
        }
    }

    fn tr(&self, key: &'static str) -> &'static str {
        match (self.language, key) {
            (EngineLanguage::Pt, "menu_file") => "Arquivo",
            (EngineLanguage::En, "menu_file") => "File",
            (EngineLanguage::Es, "menu_file") => "Archivo",

            (EngineLanguage::Pt, "menu_edit") => "Editar",
            (EngineLanguage::En, "menu_edit") => "Edit",
            (EngineLanguage::Es, "menu_edit") => "Editar",

            (EngineLanguage::Pt, "menu_help") => "Ajuda",
            (EngineLanguage::En, "menu_help") => "Help",
            (EngineLanguage::Es, "menu_help") => "Ayuda",

            (EngineLanguage::Pt, "new") => "Novo",
            (EngineLanguage::En, "new") => "New",
            (EngineLanguage::Es, "new") => "Nuevo",

            (EngineLanguage::Pt, "save") => "Salvar",
            (EngineLanguage::En, "save") => "Save",
            (EngineLanguage::Es, "save") => "Guardar",

            (EngineLanguage::Pt, "exit") => "Sair",
            (EngineLanguage::En, "exit") => "Exit",
            (EngineLanguage::Es, "exit") => "Salir",

            (EngineLanguage::Pt, "about") => "Sobre",
            (EngineLanguage::En, "about") => "About",
            (EngineLanguage::Es, "about") => "Acerca de",

            (EngineLanguage::Pt, "scene") => "Cena",
            (EngineLanguage::En, "scene") => "Scene",
            (EngineLanguage::Es, "scene") => "Escena",

            (EngineLanguage::Pt, "game") => "Game",
            (EngineLanguage::En, "game") => "Game",
            (EngineLanguage::Es, "game") => "Juego",
            _ => key,
        }
    }

    fn ensure_toolbar_icons_loaded(&mut self, ctx: &egui::Context) {
        if self.app_icon_texture.is_none() {
            self.app_icon_texture = load_png_as_texture(ctx, "src/assets/icons/icon.png");
        }
        if self.cena_icon.is_none() {
            self.cena_icon = load_png_as_texture(ctx, "src/assets/icons/cena.png");
        }
        if self.game_icon.is_none() {
            self.game_icon = load_png_as_texture(ctx, "src/assets/icons/game.png");
        }
        if self.play_icon.is_none() {
            self.play_icon = load_png_as_texture(ctx, "src/assets/icons/play.png");
        }
        if self.pause_icon.is_none() {
            self.pause_icon = load_png_as_texture(ctx, "src/assets/icons/pause.png");
        }
        if self.stop_icon.is_none() {
            self.stop_icon = load_png_as_texture(ctx, "src/assets/icons/stop.png");
        }
        if self.files_icon.is_none() {
            self.files_icon = load_png_as_texture(ctx, "src/assets/icons/files.png");
        }
        if self.lang_pt_icon.is_none() {
            self.lang_pt_icon = load_png_as_texture(ctx, "src/assets/icons/portugues.png");
        }
        if self.lang_en_icon.is_none() {
            self.lang_en_icon = load_png_as_texture(ctx, "src/assets/icons/ingles.png");
        }
        if self.lang_es_icon.is_none() {
            self.lang_es_icon = load_png_as_texture(ctx, "src/assets/icons/espanhol.png");
        }
    }
}

impl App for EditorApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut Frame) {
        // Dark theme
        ctx.set_visuals(egui::Visuals::dark());
        self.ensure_toolbar_icons_loaded(ctx);
        if !self.windows_blur_initialized {
            self.windows_blur_initialized = true;
            let _ = enable_windows_backdrop_blur(frame);
        }

        // Barra de título customizada
        egui::TopBottomPanel::top("window_controls_bar")
            .exact_height(30.0)
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::from_rgba_unmultiplied(24, 31, 30, 56))
                    .stroke(egui::Stroke::new(
                        1.0,
                        egui::Color32::from_rgba_unmultiplied(210, 228, 222, 42),
                    )),
            )
            .show(ctx, |ui| {
                let title_rect = ui.max_rect();
                ui.painter().rect_filled(
                    title_rect,
                    0.0,
                    egui::Color32::from_rgba_unmultiplied(245, 252, 249, 14),
                );

                let drag_rect = egui::Rect::from_min_max(
                    title_rect.min,
                    egui::pos2(title_rect.max.x - 116.0, title_rect.max.y),
                );
                let drag_resp = ui.interact(
                    drag_rect,
                    ui.id().with("window_drag_zone"),
                    egui::Sense::click_and_drag(),
                );
                if drag_resp.drag_started() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }

                let controls_w = 104.0;
                let lang_w = 124.0;
                let gap = 8.0;
                let controls_rect = egui::Rect::from_min_max(
                    egui::pos2(title_rect.max.x - controls_w, title_rect.min.y),
                    title_rect.max,
                );
                let lang_rect = egui::Rect::from_min_max(
                    egui::pos2(controls_rect.min.x - lang_w - gap, title_rect.min.y),
                    egui::pos2(controls_rect.min.x - gap, title_rect.max.y),
                );
                let main_rect = egui::Rect::from_min_max(
                    title_rect.min,
                    egui::pos2(lang_rect.min.x - gap, title_rect.max.y),
                );

                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(main_rect)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    |ui| {
                        ui.add_space(10.0);
                        if let Some(app_icon) = &self.app_icon_texture {
                            ui.add(
                                egui::Image::new(app_icon)
                                    .fit_to_exact_size(egui::Vec2::new(14.0, 14.0)),
                            );
                            ui.add_space(6.0);
                        }
                        ui.label(
                            egui::RichText::new("Dengine")
                                .strong()
                                .color(egui::Color32::from_gray(220)),
                        );
                        ui.add_space(10.0);

                        egui::MenuBar::new().ui(ui, |ui| {
                            ui.menu_button(self.tr("menu_file"), |ui| {
                                if ui.button(self.tr("new")).clicked() {
                                    ui.close();
                                }
                                if ui.button(self.tr("save")).clicked() {
                                    ui.close();
                                }
                                if ui.button(self.tr("exit")).clicked() {
                                    ui.close();
                                }
                            });

                            ui.menu_button(self.tr("menu_edit"), |ui| {
                                if ui.button("Undo").clicked() {}
                                if ui.button("Redo").clicked() {}
                            });

                            ui.menu_button(self.tr("menu_help"), |ui| {
                                if ui.button(self.tr("about")).clicked() {}
                            });
                        });
                    },
                );

                let mut lang_resp_opt: Option<egui::Response> = None;
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(lang_rect)
                        .layout(
                            egui::Layout::left_to_right(egui::Align::Center)
                                .with_main_align(egui::Align::Center),
                        ),
                    |ui| {
                        let current_lang = self.language;
                        let current_lang_name = self.language_name(current_lang);
                        let lang_resp = if let Some(lang_icon) = self.language_icon(current_lang) {
                            ui.add_sized(
                                [116.0, 24.0],
                                egui::Button::image_and_text(
                                    egui::Image::new(lang_icon)
                                        .fit_to_exact_size(egui::vec2(14.0, 14.0)),
                                    egui::RichText::new(current_lang_name).size(12.0),
                                )
                                .corner_radius(6)
                                .fill(egui::Color32::from_rgb(44, 44, 44))
                                .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(70))),
                            )
                        } else {
                            ui.add_sized(
                                [116.0, 24.0],
                                egui::Button::new(current_lang_name)
                                    .corner_radius(6)
                                    .fill(egui::Color32::from_rgb(44, 44, 44))
                                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(70))),
                            )
                        };
                        lang_resp_opt = Some(lang_resp);
                    },
                );

                if let Some(lang_resp) = &lang_resp_opt {
                    egui::Popup::menu(lang_resp)
                        .id(egui::Id::new("language_menu_popup"))
                        .width(150.0)
                        .show(|ui| {
                            let languages = [EngineLanguage::Pt, EngineLanguage::En, EngineLanguage::Es];
                            for lang in languages {
                                let name = self.language_name(lang);
                                let selected = self.language == lang;
                                let clicked = if let Some(icon) = self.language_icon(lang) {
                                    ui.add_sized(
                                        [138.0, 24.0],
                                        egui::Button::image_and_text(
                                            egui::Image::new(icon)
                                                .fit_to_exact_size(egui::vec2(14.0, 14.0)),
                                            egui::RichText::new(name),
                                        )
                                        .fill(if selected {
                                            egui::Color32::from_rgb(62, 62, 62)
                                        } else {
                                            egui::Color32::from_rgb(44, 44, 44)
                                        })
                                        .stroke(if selected {
                                            egui::Stroke::new(1.0, egui::Color32::from_rgb(15, 232, 121))
                                        } else {
                                            egui::Stroke::new(1.0, egui::Color32::from_gray(70))
                                        })
                                        .corner_radius(6),
                                    )
                                    .clicked()
                                } else {
                                    ui.add_sized(
                                        [138.0, 24.0],
                                        egui::Button::new(name)
                                            .fill(if selected {
                                                egui::Color32::from_rgb(62, 62, 62)
                                            } else {
                                                egui::Color32::from_rgb(44, 44, 44)
                                            })
                                            .stroke(if selected {
                                                egui::Stroke::new(
                                                    1.0,
                                                    egui::Color32::from_rgb(15, 232, 121),
                                                )
                                            } else {
                                                egui::Stroke::new(1.0, egui::Color32::from_gray(70))
                                            })
                                            .corner_radius(6),
                                    )
                                    .clicked()
                                };
                                if clicked {
                                    self.language = lang;
                                    ui.close();
                                }
                            }
                        });
                }

                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(controls_rect)
                        .layout(egui::Layout::right_to_left(egui::Align::Center)),
                    |ui| {
                        ui.add_space(8.0);

                        let (close_rect, close_resp) = ui.allocate_exact_size(
                            egui::Vec2::new(30.0, 30.0),
                            egui::Sense::click(),
                        );
                        if close_resp.hovered() {
                            ui.painter().circle_filled(
                                close_rect.center(),
                                12.0,
                                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 28),
                            );
                        }
                        ui.painter().circle_filled(
                            close_rect.center(),
                            6.0,
                            egui::Color32::from_rgb(0xD0, 0x24, 0x24),
                        );
                        if close_resp.clicked() {
                            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                        }

                        let (max_rect, max_resp) = ui.allocate_exact_size(
                            egui::Vec2::new(30.0, 30.0),
                            egui::Sense::click(),
                        );
                        if max_resp.hovered() {
                            ui.painter().circle_filled(
                                max_rect.center(),
                                12.0,
                                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 28),
                            );
                        }
                        ui.painter().circle_filled(
                            max_rect.center(),
                            6.0,
                            egui::Color32::from_rgb(0x04, 0xBA, 0x6C),
                        );
                        if max_resp.clicked() {
                            self.is_window_maximized = !self.is_window_maximized;
                            ui.ctx()
                                .send_viewport_cmd(egui::ViewportCommand::Maximized(
                                    self.is_window_maximized,
                                ));
                        }

                        let (min_rect, min_resp) = ui.allocate_exact_size(
                            egui::Vec2::new(30.0, 30.0),
                            egui::Sense::click(),
                        );
                        if min_resp.hovered() {
                            ui.painter().circle_filled(
                                min_rect.center(),
                                12.0,
                                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 28),
                            );
                        }
                        ui.painter().circle_filled(
                            min_rect.center(),
                            6.0,
                            egui::Color32::from_rgb(0xD5, 0x3C, 0x0D),
                        );
                        if min_resp.clicked() {
                            ui.ctx()
                                .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                        }
                    },
                );
            });

        // Toolbar logo abaixo do menu
        egui::TopBottomPanel::top("toolbar_row")
            .exact_height(36.0)
            .show(ctx, |ui| {
                let side_width = 220.0;
                let row_height = ui.available_height();
                let control_size = egui::Vec2::new(26.0, 26.0);

                ui.spacing_mut().item_spacing.y = 0.0;
                ui.spacing_mut().item_spacing.x = 8.0;
                let row_rect = ui.max_rect();

                let left_rect = egui::Rect::from_min_size(
                    row_rect.min,
                    egui::Vec2::new(side_width, row_height),
                );
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(left_rect)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    |ui| {
                        if let Some(cena_icon) = &self.cena_icon {
                            let cena_button = egui::Button::image_and_text(
                                egui::Image::new(cena_icon)
                                    .fit_to_exact_size(egui::Vec2::new(16.0, 16.0)),
                                egui::RichText::new(self.tr("scene")),
                            )
                            .corner_radius(8)
                            .fill(if self.selected_mode == ToolbarMode::Cena {
                                egui::Color32::from_rgb(62, 62, 62)
                            } else {
                                egui::Color32::from_rgb(44, 44, 44)
                            })
                            .stroke(if self.selected_mode == ToolbarMode::Cena {
                                egui::Stroke::new(1.0, egui::Color32::from_rgb(15, 232, 121))
                            } else {
                                egui::Stroke::new(1.0, egui::Color32::from_gray(70))
                            });
                            let cena_clicked = ui.add_sized([88.0, 28.0], cena_button).clicked();
                            if cena_clicked {
                                self.selected_mode = ToolbarMode::Cena;
                            }
                        } else {
                            let cena_clicked = ui
                                .add_sized(
                                    [88.0, 28.0],
                                    egui::Button::new(self.tr("scene"))
                                        .corner_radius(8)
                                        .fill(if self.selected_mode == ToolbarMode::Cena {
                                            egui::Color32::from_rgb(62, 62, 62)
                                        } else {
                                            egui::Color32::from_rgb(44, 44, 44)
                                        })
                                        .stroke(if self.selected_mode == ToolbarMode::Cena {
                                            egui::Stroke::new(1.0, egui::Color32::from_rgb(15, 232, 121))
                                        } else {
                                            egui::Stroke::new(1.0, egui::Color32::from_gray(70))
                                        }),
                                )
                                .clicked();
                            if cena_clicked {
                                self.selected_mode = ToolbarMode::Cena;
                            }
                        }

                        if let Some(game_icon) = &self.game_icon {
                            let game_button = egui::Button::image_and_text(
                                egui::Image::new(game_icon)
                                    .fit_to_exact_size(egui::Vec2::new(16.0, 16.0)),
                                egui::RichText::new(self.tr("game")),
                            )
                            .corner_radius(8)
                            .fill(if self.selected_mode == ToolbarMode::Game {
                                egui::Color32::from_rgb(62, 62, 62)
                            } else {
                                egui::Color32::from_rgb(44, 44, 44)
                            })
                            .stroke(if self.selected_mode == ToolbarMode::Game {
                                egui::Stroke::new(1.0, egui::Color32::from_rgb(15, 232, 121))
                            } else {
                                egui::Stroke::new(1.0, egui::Color32::from_gray(70))
                            });
                            let game_clicked = ui.add_sized([88.0, 28.0], game_button).clicked();
                            if game_clicked {
                                self.selected_mode = ToolbarMode::Game;
                            }
                        } else {
                            let game_clicked = ui
                                .add_sized(
                                    [88.0, 28.0],
                                    egui::Button::new(self.tr("game"))
                                        .corner_radius(8)
                                        .fill(if self.selected_mode == ToolbarMode::Game {
                                            egui::Color32::from_rgb(62, 62, 62)
                                        } else {
                                            egui::Color32::from_rgb(44, 44, 44)
                                        })
                                        .stroke(if self.selected_mode == ToolbarMode::Game {
                                            egui::Stroke::new(1.0, egui::Color32::from_rgb(15, 232, 121))
                                        } else {
                                            egui::Stroke::new(1.0, egui::Color32::from_gray(70))
                                        }),
                                )
                                .clicked();
                            if game_clicked {
                                self.selected_mode = ToolbarMode::Game;
                            }
                        }
                    },
                );

                let controls_width = control_size.x * 2.0 + ui.spacing().item_spacing.x;
                let controls_rect = egui::Rect::from_center_size(
                    row_rect.center(),
                    egui::Vec2::new(controls_width, row_height),
                );
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(controls_rect)
                        .layout(
                            egui::Layout::left_to_right(egui::Align::Center)
                                .with_main_align(egui::Align::Center),
                        ),
                    |ui| {
                        let play_pause_texture = if self.is_playing {
                            self.pause_icon.as_ref()
                        } else {
                            self.play_icon.as_ref()
                        };

                        if let Some(play_pause_texture) = play_pause_texture {
                            let play_pause_clicked = ui
                                .add_sized(
                                    control_size,
                                    egui::Button::image(
                                        egui::Image::new(play_pause_texture)
                                            .fit_to_exact_size(egui::Vec2::new(14.0, 14.0)),
                                    )
                                    .corner_radius(8),
                                )
                                .clicked();
                            if play_pause_clicked {
                                self.is_playing = !self.is_playing;
                            }
                        }

                        if let Some(stop_icon) = &self.stop_icon {
                            let stop_clicked = ui
                                .add_sized(
                                    control_size,
                                    egui::Button::image(
                                        egui::Image::new(stop_icon)
                                            .fit_to_exact_size(egui::Vec2::new(14.0, 14.0)),
                                    )
                                    .corner_radius(8),
                                )
                                .clicked();
                            if stop_clicked {
                                self.is_playing = false;
                            }
                        }
                    },
                );
            });

        let dock_bar_h = 34.0;
        let project_bottom = if self.project_collapsed {
            dock_bar_h
        } else {
            self.project.docked_bottom_height()
        };

        // Janela Inspetor
        self.inspector
            .show(ctx, 0.0, 0.0, project_bottom, self.language);
        let i_left = self.inspector.docked_left_width();
        let i_right = self.inspector.docked_right_width();
        self.hierarchy
            .show(ctx, i_left, i_right, project_bottom, self.language);
        let engine_busy = self.is_playing;

        if self.project_collapsed {
            let dock_rect = ctx.available_rect();
            let bar_rect = egui::Rect::from_min_max(
                egui::pos2(dock_rect.left(), dock_rect.bottom() - dock_bar_h),
                egui::pos2(dock_rect.right(), dock_rect.bottom()),
            );

            egui::Area::new(egui::Id::new("bottom_multi_dock_bar"))
                .order(egui::Order::Foreground)
                .fixed_pos(bar_rect.min)
                .show(ctx, |ui| {
                    let (rect, _) = ui.allocate_exact_size(bar_rect.size(), egui::Sense::hover());
                    ui.painter()
                        .rect_filled(rect, 0.0, egui::Color32::from_rgb(35, 35, 35));
                    ui.painter().rect_stroke(
                        rect,
                        0.0,
                        egui::Stroke::new(1.0, egui::Color32::from_rgb(58, 58, 58)),
                        egui::StrokeKind::Outside,
                    );

                    let icon_rect = egui::Rect::from_center_size(
                        egui::pos2(rect.left() + 16.0, rect.center().y),
                        egui::vec2(28.0, 22.0),
                    );
                    let icon_resp = ui.interact(
                        icon_rect,
                        ui.id().with("restore_project_from_dock"),
                        egui::Sense::click(),
                    );
                    if icon_resp.hovered() {
                        ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                        ui.painter().rect_filled(
                            icon_rect.expand(2.0),
                            3.0,
                            egui::Color32::from_rgb(58, 58, 58),
                        );
                    }
                    if icon_resp.clicked() {
                        self.project_collapsed = false;
                    }

                    if let Some(files_icon) = &self.files_icon {
                        let _ = ui.put(
                            icon_rect,
                            egui::Image::new(files_icon)
                                .fit_to_exact_size(egui::Vec2::new(20.0, 20.0)),
                        );
                    }

                    if engine_busy {
                        ui.ctx().request_repaint();
                        let spinner_rect = egui::Rect::from_center_size(
                            rect.center(),
                            egui::vec2(20.0, 20.0),
                        );
                        ui.scope_builder(
                            egui::UiBuilder::new()
                                .max_rect(spinner_rect)
                                .layout(
                                    egui::Layout::left_to_right(egui::Align::Center)
                                        .with_main_align(egui::Align::Center),
                                ),
                            |ui| {
                                ui.add(
                                    egui::Spinner::new()
                                        .size(16.0)
                                        .color(egui::Color32::from_rgb(15, 232, 121)),
                                );
                            },
                        );
                    }
                });
        } else if self.project.show(ctx, self.language) {
            self.project_collapsed = true;
        }
    }
}

fn enable_windows_backdrop_blur(frame: &Frame) -> bool {
    #[cfg(target_os = "windows")]
    {
        let Ok(window_handle) = frame.window_handle() else {
            return false;
        };
        let RawWindowHandle::Win32(win) = window_handle.as_raw() else {
            return false;
        };

        // Windows 11 backdrop types: 2 = Mica, 3 = Acrylic-like transient blur.
        const DWMWA_SYSTEMBACKDROP_TYPE: u32 = 38;
        const DWMSBT_TRANSIENTWINDOW: i32 = 3;

        let hwnd = win.hwnd.get() as *mut core::ffi::c_void;
        let backdrop = DWMSBT_TRANSIENTWINDOW;
        let hr = unsafe {
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_SYSTEMBACKDROP_TYPE,
                &backdrop as *const _ as *const core::ffi::c_void,
                std::mem::size_of::<i32>() as u32,
            )
        };
        return hr >= 0;
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = frame;
        false
    }
}

fn main() -> eframe::Result<()> {
    let app_icon = load_icon_data_from_png("src/assets/icons/icon.png");
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Dengine Editor")
            .with_decorations(false)
            .with_transparent(true)
            .with_maximized(true)
            .with_icon(
                app_icon.unwrap_or_else(|| {
                    Arc::new(egui::IconData {
                        rgba: vec![0, 0, 0, 0],
                        width: 1,
                        height: 1,
                    })
                }),
            ),
        ..Default::default()
    };

    eframe::run_native(
        "Dengine Editor",
        options,
        Box::new(|_cc| {
            Ok(Box::new(EditorApp {
                inspector: InspectorWindow::new(),
                hierarchy: HierarchyWindow::new(),
                project: ProjectWindow::new(),
                app_icon_texture: None,
                cena_icon: None,
                game_icon: None,
                play_icon: None,
                pause_icon: None,
                stop_icon: None,
                files_icon: None,
                lang_pt_icon: None,
                lang_en_icon: None,
                lang_es_icon: None,
                is_playing: false,
                is_window_maximized: true,
                selected_mode: ToolbarMode::Cena,
                language: EngineLanguage::Pt,
                project_collapsed: false,
                windows_blur_initialized: false,
            }))
        }),
    )
}
