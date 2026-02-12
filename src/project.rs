use eframe::egui::{
    self, Align2, Color32, FontFamily, FontId, Id, Order, Pos2, Rect, Stroke, Vec2,
};
use crate::EngineLanguage;

pub struct ProjectWindow {
    pub open: bool,
    dock_side: Option<ProjectDockSide>,
    window_pos: Option<Pos2>,
    floating_size: Vec2,
    dock_height: f32,
    dragging_from_header: bool,
    resizing: bool,
    selected_folder: &'static str,
    search: String,
    language: EngineLanguage,
}

#[derive(Clone, Copy)]
enum ProjectDockSide {
    Bottom,
    Left,
    Right,
}

impl ProjectWindow {
    pub fn new() -> Self {
        Self {
            open: true,
            dock_side: Some(ProjectDockSide::Bottom),
            window_pos: None,
            floating_size: egui::vec2(760.0, 230.0),
            dock_height: 210.0,
            dragging_from_header: false,
            resizing: false,
            selected_folder: "Assets",
            search: String::new(),
            language: EngineLanguage::Pt,
        }
    }

    fn tr(&self, key: &'static str) -> &'static str {
        match (self.language, key) {
            (EngineLanguage::Pt, "title") => "Projeto",
            (EngineLanguage::En, "title") => "Project",
            (EngineLanguage::Es, "title") => "Proyecto",
            (EngineLanguage::Pt, "search") => "Buscar",
            (EngineLanguage::En, "search") => "Search",
            (EngineLanguage::Es, "search") => "Buscar",
            (EngineLanguage::Pt, "assets_root") => "Assets",
            (EngineLanguage::En, "assets_root") => "Assets",
            (EngineLanguage::Es, "assets_root") => "Recursos",
            _ => key,
        }
    }

    fn folder_label(&self, folder: &'static str) -> &'static str {
        match (self.language, folder) {
            (EngineLanguage::Pt, "Assets") => "Assets",
            (EngineLanguage::En, "Assets") => "Assets",
            (EngineLanguage::Es, "Assets") => "Recursos",
            (EngineLanguage::Pt, "Scenes") => "Cenas",
            (EngineLanguage::En, "Scenes") => "Scenes",
            (EngineLanguage::Es, "Scenes") => "Escenas",
            (EngineLanguage::Pt, "Scripts") => "Scripts",
            (EngineLanguage::En, "Scripts") => "Scripts",
            (EngineLanguage::Es, "Scripts") => "Scripts",
            (EngineLanguage::Pt, "Materials") => "Materiais",
            (EngineLanguage::En, "Materials") => "Materials",
            (EngineLanguage::Es, "Materials") => "Materiales",
            (EngineLanguage::Pt, "Textures") => "Texturas",
            (EngineLanguage::En, "Textures") => "Textures",
            (EngineLanguage::Es, "Textures") => "Texturas",
            (EngineLanguage::Pt, "Prefabs") => "Prefabs",
            (EngineLanguage::En, "Prefabs") => "Prefabs",
            (EngineLanguage::Es, "Prefabs") => "Prefabs",
            (EngineLanguage::Pt, "Audio") => "Audio",
            (EngineLanguage::En, "Audio") => "Audio",
            (EngineLanguage::Es, "Audio") => "Audio",
            _ => folder,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, language: EngineLanguage) {
        if !self.open {
            return;
        }
        self.language = language;

        let dock_rect = ctx.available_rect();
        let pointer_down = ctx.input(|i| i.pointer.primary_down());

        self.dock_height = self.dock_height.clamp(140.0, dock_rect.height() - 40.0);
        self.floating_size.x = self.floating_size.x.clamp(360.0, dock_rect.width() - 20.0);
        self.floating_size.y = self.floating_size.y.clamp(180.0, dock_rect.height() - 20.0);

        let window_size = match self.dock_side {
            Some(ProjectDockSide::Bottom) => egui::vec2(dock_rect.width(), self.dock_height),
            Some(ProjectDockSide::Left) | Some(ProjectDockSide::Right) | None => self.floating_size,
        };

        if self.window_pos.is_none() {
            self.window_pos = Some(egui::pos2(
                dock_rect.left(),
                dock_rect.bottom() - window_size.y,
            ));
        }

        if let Some(side) = self.dock_side {
            if !self.dragging_from_header && !self.resizing && !pointer_down {
                let pos = match side {
                    ProjectDockSide::Bottom => {
                        egui::pos2(dock_rect.left(), dock_rect.bottom() - window_size.y)
                    }
                    ProjectDockSide::Left => egui::pos2(dock_rect.left(), dock_rect.top()),
                    ProjectDockSide::Right => {
                        egui::pos2(dock_rect.right() - window_size.x, dock_rect.top())
                    }
                };
                self.window_pos = Some(pos);
            }
        }

        let pos = self.window_pos.unwrap_or(egui::pos2(
            dock_rect.left(),
            dock_rect.bottom() - window_size.y,
        ));

        let mut header_drag_started = false;
        let mut header_drag_stopped = false;
        let mut resize_started = false;
        let mut resize_stopped = false;
        let mut panel_rect = Rect::from_min_size(pos, window_size);

        egui::Area::new(Id::new("project_window_id"))
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
                let header_h = 20.0;
                let header_rect =
                    Rect::from_min_max(inner.min, egui::pos2(inner.max.x, inner.min.y + header_h));

                let drag_resp = ui.interact(
                    header_rect,
                    ui.id().with("project_header_drag"),
                    egui::Sense::click_and_drag(),
                );
                if drag_resp.drag_started() {
                    header_drag_started = true;
                }
                if drag_resp.drag_stopped() {
                    header_drag_stopped = true;
                }

                ui.painter().text(
                    header_rect.center(),
                    Align2::CENTER_CENTER,
                    self.tr("title"),
                    FontId::new(13.0, FontFamily::Proportional),
                    Color32::WHITE,
                );

                let sep_y = header_rect.max.y + 4.0;
                ui.painter().line_segment(
                    [egui::pos2(inner.min.x, sep_y), egui::pos2(inner.max.x, sep_y)],
                    Stroke::new(1.0, Color32::from_gray(60)),
                );

                let content_rect = Rect::from_min_max(
                    egui::pos2(inner.min.x, sep_y + 6.0),
                    egui::pos2(inner.max.x, inner.max.y),
                );
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(content_rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                    |ui| {
                        let topbar_h = 24.0;
                        let bar_rect = Rect::from_min_max(
                            ui.min_rect().min,
                            egui::pos2(ui.max_rect().max.x, ui.min_rect().min.y + topbar_h),
                        );
                        ui.scope_builder(
                            egui::UiBuilder::new()
                                .max_rect(bar_rect)
                                .layout(egui::Layout::left_to_right(egui::Align::Center)),
                            |ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;
                                ui.label(
                                    egui::RichText::new(self.tr("assets_root"))
                                        .color(Color32::from_gray(190)),
                                );
                                ui.label(egui::RichText::new(">").color(Color32::from_gray(120)));
                                ui.label(
                                    egui::RichText::new(self.folder_label(self.selected_folder))
                                        .color(Color32::from_gray(190)),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        let search_hint = self.tr("search");
                                        ui.add_sized(
                                            [150.0, 22.0],
                                            egui::TextEdit::singleline(&mut self.search)
                                                .hint_text(search_hint),
                                        );
                                    },
                                );
                            },
                        );

                        let body_rect = Rect::from_min_max(
                            egui::pos2(ui.min_rect().min.x, bar_rect.max.y + 6.0),
                            ui.max_rect().max,
                        );
                        let left_w = 170.0;
                        let folders_rect = Rect::from_min_max(
                            body_rect.min,
                            egui::pos2(body_rect.min.x + left_w, body_rect.max.y),
                        );
                        let assets_rect = Rect::from_min_max(
                            egui::pos2(folders_rect.max.x + 8.0, body_rect.min.y),
                            body_rect.max,
                        );

                        ui.painter().rect_filled(
                            folders_rect,
                            4.0,
                            Color32::from_rgba_unmultiplied(36, 36, 36, 210),
                        );
                        ui.painter().rect_filled(
                            assets_rect,
                            4.0,
                            Color32::from_rgba_unmultiplied(32, 32, 32, 210),
                        );

                        ui.scope_builder(
                            egui::UiBuilder::new()
                                .max_rect(folders_rect.shrink(6.0))
                                .layout(egui::Layout::top_down(egui::Align::Min)),
                            |ui| {
                                let folders = [
                                    "Assets",
                                    "Scenes",
                                    "Scripts",
                                    "Materials",
                                    "Textures",
                                    "Prefabs",
                                    "Audio",
                                ];
                                for folder in folders {
                                    let selected = self.selected_folder == folder;
                                    let label = format!("  {}", self.folder_label(folder));
                                    if ui.selectable_label(selected, label).clicked() {
                                        self.selected_folder = folder;
                                    }
                                }
                            },
                        );

                        let filter = self.search.to_lowercase();
                        let assets: &[&str] = match self.selected_folder {
                            "Scenes" => &["Main.scene", "Menu.scene", "Debug.scene"],
                            "Scripts" => &["player.rs", "camera.rs", "ai.rs", "inventory.rs"],
                            "Materials" => &["metal.mat", "wood.mat", "glass.mat"],
                            "Textures" => &["grass.png", "stone.png", "ui_atlas.png", "sky.hdr"],
                            "Prefabs" => &["Enemy.prefab", "Chest.prefab", "Door.prefab"],
                            "Audio" => &["theme.ogg", "hit.wav", "ambient.ogg"],
                            _ => &[
                                "Scenes",
                                "Scripts",
                                "Materials",
                                "Textures",
                                "Prefabs",
                                "Audio",
                            ],
                        };

                        ui.scope_builder(
                            egui::UiBuilder::new()
                                .max_rect(assets_rect.shrink(6.0))
                                .layout(egui::Layout::top_down(egui::Align::Min)),
                            |ui| {
                                let cell_w = 88.0;
                                let cell_h = 74.0;
                                let cols = ((ui.available_width() + 8.0) / (cell_w + 8.0))
                                    .floor()
                                    .max(1.0) as usize;
                                let filtered: Vec<&str> = assets
                                    .iter()
                                    .copied()
                                    .filter(|a| filter.is_empty() || a.to_lowercase().contains(&filter))
                                    .collect();

                                egui::ScrollArea::vertical().show(ui, |ui| {
                                    let mut idx = 0;
                                    while idx < filtered.len() {
                                        ui.horizontal(|ui| {
                                            ui.spacing_mut().item_spacing.x = 8.0;
                                            for _ in 0..cols {
                                                if idx >= filtered.len() {
                                                    break;
                                                }
                                                let name = filtered[idx];
                                                ui.scope(|ui| {
                                                    ui.set_width(cell_w);
                                                    let (thumb, _) = ui.allocate_exact_size(
                                                        egui::vec2(cell_w, cell_h - 20.0),
                                                        egui::Sense::click(),
                                                    );
                                                    ui.painter().rect_filled(
                                                        thumb,
                                                        3.0,
                                                        Color32::from_rgb(52, 52, 52),
                                                    );
                                                    ui.painter().rect_stroke(
                                                        thumb,
                                                        3.0,
                                                        Stroke::new(1.0, Color32::from_gray(78)),
                                                        egui::StrokeKind::Outside,
                                                    );
                                                    ui.painter().text(
                                                        thumb.center(),
                                                        Align2::CENTER_CENTER,
                                                        if name.contains('.') { "A" } else { "P" },
                                                        FontId::new(16.0, FontFamily::Proportional),
                                                        Color32::from_gray(180),
                                                    );
                                                    ui.label(
                                                        egui::RichText::new(name)
                                                            .size(11.0)
                                                            .color(Color32::from_gray(200)),
                                                    );
                                                });
                                                idx += 1;
                                            }
                                        });
                                        ui.add_space(4.0);
                                    }
                                });
                            },
                        );
                    },
                );

                let handle_rect = match self.dock_side {
                    Some(ProjectDockSide::Bottom) => Rect::from_min_max(
                        egui::pos2(rect.left(), rect.top()),
                        egui::pos2(rect.right(), rect.top() + 8.0),
                    ),
                    Some(ProjectDockSide::Left) => Rect::from_min_max(
                        egui::pos2(rect.right() - 8.0, rect.top()),
                        egui::pos2(rect.right(), rect.bottom()),
                    ),
                    Some(ProjectDockSide::Right) => Rect::from_min_max(
                        egui::pos2(rect.left(), rect.top()),
                        egui::pos2(rect.left() + 8.0, rect.bottom()),
                    ),
                    None => Rect::from_min_max(
                        egui::pos2(rect.right() - 12.0, rect.bottom() - 12.0),
                        rect.max,
                    ),
                };
                let resize_resp = ui.interact(
                    handle_rect,
                    ui.id().with("project_resize_handle"),
                    egui::Sense::click_and_drag(),
                );
                if resize_resp.hovered() || resize_resp.dragged() {
                    ui.output_mut(|o| {
                        o.cursor_icon = match self.dock_side {
                            Some(ProjectDockSide::Bottom) => egui::CursorIcon::ResizeVertical,
                            Some(ProjectDockSide::Left) | Some(ProjectDockSide::Right) => {
                                egui::CursorIcon::ResizeHorizontal
                            }
                            None => egui::CursorIcon::ResizeNwSe,
                        }
                    });
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
            self.resizing = false;
            self.dock_side = None;
        }
        if resize_started {
            self.resizing = true;
            self.dragging_from_header = false;
        }

        let delta = ctx.input(|i| i.pointer.delta());
        if pointer_down {
            if self.dragging_from_header && delta != Vec2::ZERO {
                if let Some(p) = self.window_pos {
                    self.window_pos = Some(p + delta);
                }
            } else if self.resizing {
                match self.dock_side {
                    Some(ProjectDockSide::Bottom) => {
                        self.dock_height = (self.dock_height - delta.y).clamp(140.0, dock_rect.height() - 40.0);
                    }
                    Some(ProjectDockSide::Left) => {
                        self.floating_size.x = (self.floating_size.x + delta.x).clamp(300.0, dock_rect.width() - 40.0);
                    }
                    Some(ProjectDockSide::Right) => {
                        self.floating_size.x = (self.floating_size.x - delta.x).clamp(300.0, dock_rect.width() - 40.0);
                        if let Some(p) = self.window_pos {
                            self.window_pos = Some(egui::pos2(p.x + delta.x, p.y));
                        }
                    }
                    None => {
                        self.floating_size.x = (self.floating_size.x + delta.x).clamp(320.0, dock_rect.width() - 20.0);
                        self.floating_size.y = (self.floating_size.y + delta.y).clamp(180.0, dock_rect.height() - 20.0);
                    }
                }
            }
        }

        let near_bottom = (dock_rect.bottom() - panel_rect.bottom()).abs() <= 24.0;
        let near_left = (panel_rect.left() - dock_rect.left()).abs() <= 24.0;
        let near_right = (dock_rect.right() - panel_rect.right()).abs() <= 24.0;
        if self.dragging_from_header && pointer_down {
            let hint_rect = if near_bottom {
                Some(Rect::from_min_max(
                    egui::pos2(dock_rect.left(), dock_rect.bottom() - 12.0),
                    egui::pos2(dock_rect.right(), dock_rect.bottom()),
                ))
            } else if near_left {
                Some(Rect::from_min_max(
                    egui::pos2(dock_rect.left(), dock_rect.top()),
                    egui::pos2(dock_rect.left() + 12.0, dock_rect.bottom()),
                ))
            } else if near_right {
                Some(Rect::from_min_max(
                    egui::pos2(dock_rect.right() - 12.0, dock_rect.top()),
                    egui::pos2(dock_rect.right(), dock_rect.bottom()),
                ))
            } else {
                None
            };
            if let Some(hint_rect) = hint_rect {
                ctx.layer_painter(egui::LayerId::new(Order::Foreground, Id::new("project_dock_hint")))
                    .rect_filled(
                        hint_rect,
                        4.0,
                        Color32::from_rgba_unmultiplied(15, 232, 121, 110),
                    );
            }
        }

        if header_drag_stopped || (self.dragging_from_header && !pointer_down) {
            self.dragging_from_header = false;
            if near_bottom {
                self.dock_side = Some(ProjectDockSide::Bottom);
            } else if near_left {
                self.dock_side = Some(ProjectDockSide::Left);
            } else if near_right {
                self.dock_side = Some(ProjectDockSide::Right);
            } else {
                self.dock_side = None;
            }
        }

        if resize_stopped || (self.resizing && !pointer_down) {
            self.resizing = false;
        }
    }
}
