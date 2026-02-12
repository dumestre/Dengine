// src/main.rs
mod inspector;

use eframe::egui::{TextureHandle, TextureOptions};
use eframe::{App, Frame, NativeOptions, egui};
use epaint::ColorImage;
use inspector::InspectorWindow;

struct EditorApp {
    inspector: InspectorWindow,
    win_close_icon: Option<TextureHandle>,
    cena_icon: Option<TextureHandle>,
    game_icon: Option<TextureHandle>,
    play_icon: Option<TextureHandle>,
    pause_icon: Option<TextureHandle>,
    stop_icon: Option<TextureHandle>,
    is_playing: bool,
    is_window_maximized: bool,
    selected_mode: ToolbarMode,
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

impl EditorApp {
    fn ensure_toolbar_icons_loaded(&mut self, ctx: &egui::Context) {
        if self.win_close_icon.is_none() {
            self.win_close_icon = load_png_as_texture(ctx, "src/assets/icons/fechar.png");
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
    }
}

impl App for EditorApp {
    fn update(&mut self, ctx: &egui::Context, _: &mut Frame) {
        // Dark theme
        ctx.set_visuals(egui::Visuals::dark());
        self.ensure_toolbar_icons_loaded(ctx);

        // Barra de título customizada
        egui::TopBottomPanel::top("window_controls_bar")
            .exact_height(30.0)
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::from_rgba_unmultiplied(22, 22, 22, 120))
                    .stroke(egui::Stroke::new(
                        1.0,
                        egui::Color32::from_rgba_unmultiplied(90, 90, 90, 70),
                    )),
            )
            .show(ctx, |ui| {
                let title_rect = ui.max_rect();
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

                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    if let Some(app_icon) = &self.win_close_icon {
                        ui.add(
                            egui::Image::new(app_icon).fit_to_exact_size(egui::Vec2::new(14.0, 14.0)),
                        );
                        ui.add_space(6.0);
                    }
                    ui.label(
                        egui::RichText::new("Dengine")
                            .strong()
                            .color(egui::Color32::from_gray(220)),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(8.0);

                        let (close_rect, close_resp) =
                            ui.allocate_exact_size(egui::Vec2::new(24.0, 24.0), egui::Sense::click());
                        ui.painter().circle_filled(
                            close_rect.center(),
                            6.0,
                            egui::Color32::from_rgb(0xD0, 0x24, 0x24),
                        );
                        if close_resp.clicked() {
                            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                        }

                        let (max_rect, max_resp) =
                            ui.allocate_exact_size(egui::Vec2::new(24.0, 24.0), egui::Sense::click());
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

                        let (min_rect, min_resp) =
                            ui.allocate_exact_size(egui::Vec2::new(24.0, 24.0), egui::Sense::click());
                        ui.painter().circle_filled(
                            min_rect.center(),
                            6.0,
                            egui::Color32::from_rgb(0xD5, 0x3C, 0x0D),
                        );
                        if min_resp.clicked() {
                            ui.ctx()
                                .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                        }
                    });
                });
            });

        // Top menu
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("Arquivo", |ui| {
                    if ui.button("Novo").clicked() {
                        ui.close();
                    }
                    if ui.button("Salvar").clicked() {
                        ui.close();
                    }
                    if ui.button("Sair").clicked() {
                        ui.close();
                    }
                });

                ui.menu_button("Editar", |ui| {
                    if ui.button("Undo").clicked() {}
                    if ui.button("Redo").clicked() {}
                });

                ui.menu_button("Ajuda", |ui| if ui.button("Sobre").clicked() {});
            });
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
                                egui::RichText::new("Cena"),
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
                                    egui::Button::new("Cena")
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
                                egui::RichText::new("Game"),
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
                                    egui::Button::new("Game")
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

        // Janela Inspetor
        self.inspector.show(ctx);
    }
}

fn main() -> eframe::Result<()> {
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Dengine Editor")
            .with_decorations(false)
            .with_maximized(true),
        ..Default::default()
    };

    eframe::run_native(
        "Dengine Editor",
        options,
        Box::new(|_cc| {
            Ok(Box::new(EditorApp {
                inspector: InspectorWindow::new(),
                win_close_icon: None,
                cena_icon: None,
                game_icon: None,
                play_icon: None,
                pause_icon: None,
                stop_icon: None,
                is_playing: false,
                is_window_maximized: true,
                selected_mode: ToolbarMode::Cena,
            }))
        }),
    )
}
