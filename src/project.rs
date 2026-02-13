use std::collections::HashSet;

use eframe::egui::{self, Align2, Color32, FontFamily, FontId, Id, Order, Rect, Sense, Stroke, TextureHandle, Vec2};
use epaint::ColorImage;

use crate::EngineLanguage;

pub struct ProjectWindow {
    pub open: bool,
    panel_height: f32,
    resizing_height: bool,
    selected_folder: &'static str,
    selected_asset: Option<&'static str>,
    search_query: String,
    icon_scale: f32,
    deleted_assets: HashSet<&'static str>,
    status_text: String,
    arrow_icon_texture: Option<TextureHandle>,
    assets_open: bool,
    packages_open: bool,
    hover_roll_asset: Option<&'static str>,
    hover_still_since: f64,
}

fn load_png_as_texture(ctx: &egui::Context, png_path: &str) -> Option<TextureHandle> {
    let bytes = std::fs::read(png_path).ok()?;
    let rgba = image::load_from_memory(&bytes).ok()?.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let color_image = ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
    Some(ctx.load_texture(
        png_path.to_owned(),
        color_image,
        egui::TextureOptions::LINEAR,
    ))
}

impl ProjectWindow {
    pub fn new() -> Self {
        Self {
            open: true,
            panel_height: 260.0,
            resizing_height: false,
            selected_folder: "Assets",
            selected_asset: None,
            search_query: String::new(),
            icon_scale: 72.0,
            deleted_assets: HashSet::new(),
            status_text: String::new(),
            arrow_icon_texture: None,
            assets_open: true,
            packages_open: true,
            hover_roll_asset: None,
            hover_still_since: 0.0,
        }
    }

    fn tr(&self, lang: EngineLanguage, key: &'static str) -> &'static str {
        match (lang, key) {
            (EngineLanguage::Pt, "title") => "Projeto",
            (EngineLanguage::En, "title") => "Project",
            (EngineLanguage::Es, "title") => "Proyecto",
            (EngineLanguage::Pt, "assets") => "Assets",
            (EngineLanguage::En, "assets") => "Assets",
            (EngineLanguage::Es, "assets") => "Assets",
            (EngineLanguage::Pt, "packages") => "Pacotes",
            (EngineLanguage::En, "packages") => "Packages",
            (EngineLanguage::Es, "packages") => "Paquetes",
            (EngineLanguage::Pt, "search") => "Buscar em Assets",
            (EngineLanguage::En, "search") => "Search in Assets",
            (EngineLanguage::Es, "search") => "Buscar en Assets",
            (EngineLanguage::Pt, "count") => "itens",
            (EngineLanguage::En, "count") => "items",
            (EngineLanguage::Es, "count") => "elementos",
            (EngineLanguage::Pt, "open") => "Abrir",
            (EngineLanguage::En, "open") => "Open",
            (EngineLanguage::Es, "open") => "Abrir",
            (EngineLanguage::Pt, "reveal") => "Mostrar no Explorer",
            (EngineLanguage::En, "reveal") => "Show in Explorer",
            (EngineLanguage::Es, "reveal") => "Mostrar en Explorer",
            (EngineLanguage::Pt, "delete") => "Excluir",
            (EngineLanguage::En, "delete") => "Delete",
            (EngineLanguage::Es, "delete") => "Eliminar",
            _ => key,
        }
    }

    fn is_package_folder(folder: &str) -> bool {
        matches!(folder, "Packages" | "TextMeshPro" | "InputSystem")
    }

    fn breadcrumb_segments(&self, language: EngineLanguage) -> Vec<(&'static str, String)> {
        if self.selected_folder == "Packages" {
            vec![("Packages", self.tr(language, "packages").to_string())]
        } else if self.selected_folder == "Assets" {
            vec![("Assets", self.tr(language, "assets").to_string())]
        } else if Self::is_package_folder(self.selected_folder) {
            vec![
                ("Packages", self.tr(language, "packages").to_string()),
                (self.selected_folder, self.selected_folder.to_string()),
            ]
        } else {
            vec![
                ("Assets", self.tr(language, "assets").to_string()),
                (self.selected_folder, self.selected_folder.to_string()),
            ]
        }
    }

    fn assets_for_folder(&self) -> &'static [&'static str] {
        match self.selected_folder {
            "Assets" => &[
                "Player.mold",
                "Main Camera.mold",
                "Environment.mold",
                "UIAtlas.png",
                "AudioMixer.asset",
                "LightingSettings.asset",
            ],
            "Animations" => &["Idle.anim", "Run.anim", "Jump.anim", "BlendTree.controller"],
            "Materials" => &["Terrain.mat", "Character.mat", "Water.mat"],
            "Meshes" => &["Hero.fbx", "Tree_A.fbx", "Rock_01.fbx"],
            "Mold" => &["Enemy.mold", "HUD.mold", "Spawner.mold"],
            "Scripts" => &["PlayerController.cs", "EnemyAI.cs", "GameBootstrap.cs"],
            "Packages" => &["manifest.json", "packages-lock.json"],
            "TextMeshPro" => &["TMP Settings.asset", "TMP Essentials"],
            "InputSystem" => &["InputActions.inputactions"],
            _ => &[],
        }
    }

    fn icon_style(asset: &str) -> (Color32, &'static str) {
        if asset.ends_with(".mold") {
            (Color32::from_rgb(56, 95, 166), "PF")
        } else if asset.ends_with(".cs") {
            (Color32::from_rgb(184, 104, 51), "C#")
        } else if asset.ends_with(".png") {
            (Color32::from_rgb(64, 146, 112), "IMG")
        } else if asset.ends_with(".anim") || asset.ends_with(".controller") {
            (Color32::from_rgb(154, 72, 167), "AN")
        } else if asset.ends_with(".mat") {
            (Color32::from_rgb(179, 137, 57), "MAT")
        } else if asset.ends_with(".fbx") {
            (Color32::from_rgb(86, 132, 176), "MESH")
        } else if asset.ends_with(".json") {
            (Color32::from_rgb(127, 127, 127), "{}")
        } else {
            (Color32::from_rgb(88, 88, 88), "AS")
        }
    }

    fn truncate_with_ellipsis(
        painter: &egui::Painter,
        text: &str,
        font: &FontId,
        max_width: f32,
    ) -> String {
        let full = painter.layout_no_wrap(text.to_owned(), font.clone(), Color32::WHITE);
        if full.size().x <= max_width {
            return text.to_owned();
        }

        let ellipsis = "...";
        let ellipsis_w = painter
            .layout_no_wrap(ellipsis.to_owned(), font.clone(), Color32::WHITE)
            .size()
            .x;
        if ellipsis_w >= max_width {
            return ellipsis.to_owned();
        }

        let chars: Vec<char> = text.chars().collect();
        for keep in (0..chars.len()).rev() {
            let mut candidate: String = chars.iter().take(keep).collect();
            candidate.push_str(ellipsis);
            let w = painter
                .layout_no_wrap(candidate.clone(), font.clone(), Color32::WHITE)
                .size()
                .x;
            if w <= max_width {
                return candidate;
            }
        }

        ellipsis.to_owned()
    }

    fn draw_icon_size_slider(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let min = 56.0;
        let max = 98.0;
        let t = ((self.icon_scale - min) / (max - min)).clamp(0.0, 1.0);

        let resp = ui.interact(rect, ui.id().with("project_icon_size_slider"), Sense::click_and_drag());
        if resp.clicked() || resp.dragged() {
            if let Some(pos) = ui.ctx().input(|i| i.pointer.interact_pos()) {
                let k = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
                self.icon_scale = min + k * (max - min);
            }
        }

        let track_rect = Rect::from_center_size(rect.center(), egui::vec2(rect.width(), 4.0));
        ui.painter()
            .rect_filled(track_rect, 6.0, Color32::from_rgb(74, 74, 74));

        let fill_rect = Rect::from_min_max(
            track_rect.min,
            egui::pos2(track_rect.left() + track_rect.width() * t, track_rect.bottom()),
        );
        ui.painter()
            .rect_filled(fill_rect, 6.0, Color32::from_rgb(15, 232, 121));

        let knob_center = egui::pos2(track_rect.left() + track_rect.width() * t, track_rect.center().y);
        ui.painter()
            .circle_filled(knob_center, 5.0, Color32::from_rgb(34, 34, 34));
        ui.painter().circle_stroke(
            knob_center,
            5.0,
            Stroke::new(1.4, Color32::from_rgb(15, 232, 121)),
        );
    }

    fn draw_tree_leaf_row(
        ui: &mut egui::Ui,
        id: &str,
        label: &str,
        indent: f32,
        selected: bool,
    ) -> egui::Response {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 20.0), Sense::click());
        let resp = ui.interact(rect, ui.id().with(("project_tree_leaf", id)), Sense::click());

        ui.painter().text(
            egui::pos2(rect.left() + indent + 6.0, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            FontId::new(12.0, FontFamily::Proportional),
            if selected {
                Color32::from_rgb(15, 232, 121)
            } else if resp.hovered() {
                Color32::from_gray(225)
            } else {
                Color32::from_gray(195)
            },
        );

        resp
    }

    fn draw_tree_parent_row(
        &mut self,
        ui: &mut egui::Ui,
        id: &str,
        label: &str,
        indent: f32,
        is_open: &mut bool,
        selected: bool,
    ) -> egui::Response {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 20.0), Sense::click());
        let row_resp = ui.interact(rect, ui.id().with(("project_tree_parent", id)), Sense::click());

        let arrow_rect = Rect::from_center_size(
            egui::pos2(rect.left() + indent + 10.0, rect.center().y),
            egui::vec2(12.0, 12.0),
        );
        let arrow_resp = ui.interact(
            arrow_rect,
            ui.id().with(("project_tree_arrow", id)),
            Sense::click(),
        );
        if arrow_resp.clicked() {
            *is_open = !*is_open;
        }

        if let Some(arrow_tex) = &self.arrow_icon_texture {
            let angle = if *is_open { std::f32::consts::FRAC_PI_2 } else { 0.0 };
            let _ = ui.put(
                arrow_rect,
                egui::Image::new(arrow_tex)
                    .fit_to_exact_size(egui::vec2(9.0, 9.0))
                    .rotate(angle, Vec2::splat(0.5)),
            );
        } else {
            ui.painter().text(
                arrow_rect.center(),
                Align2::CENTER_CENTER,
                if *is_open { "▾" } else { "▸" },
                FontId::new(11.0, FontFamily::Proportional),
                Color32::from_gray(140),
            );
        }

        ui.painter().text(
            egui::pos2(rect.left() + indent + 22.0, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            FontId::new(12.0, FontFamily::Proportional),
            if selected {
                Color32::from_rgb(15, 232, 121)
            } else if row_resp.hovered() {
                Color32::from_gray(225)
            } else {
                Color32::from_gray(195)
            },
        );

        row_resp
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        language: EngineLanguage,
    ) {
        if !self.open {
            return;
        }

        if self.arrow_icon_texture.is_none() {
            self.arrow_icon_texture = load_png_as_texture(ctx, "src/assets/icons/seta.png");
        }

        let dock_rect = ctx.available_rect();
        let pointer_down = ctx.input(|i| i.pointer.primary_down());

        let min_h = 185.0;
        let max_h = (dock_rect.height() - 120.0).max(min_h);
        self.panel_height = self.panel_height.clamp(min_h, max_h);

        let panel_rect = Rect::from_min_max(
            egui::pos2(dock_rect.left(), dock_rect.bottom() - self.panel_height),
            egui::pos2(dock_rect.right(), dock_rect.bottom()),
        );

        let mut resize_started = false;
        let mut resize_stopped = false;

        egui::Area::new(Id::new("project_window"))
            .order(Order::Foreground)
            .fixed_pos(panel_rect.min)
            .show(ctx, |ui| {
                let (rect, _) = ui.allocate_exact_size(panel_rect.size(), Sense::hover());

                ui.painter()
                    .rect_filled(rect, 0.0, Color32::from_rgb(35, 35, 35));
                ui.painter().rect_stroke(
                    rect,
                    0.0,
                    Stroke::new(1.0, Color32::from_rgb(58, 58, 58)),
                    egui::StrokeKind::Outside,
                );

                let resize_rect = Rect::from_min_max(
                    egui::pos2(rect.left(), rect.top() - 4.0),
                    egui::pos2(rect.right(), rect.top() + 5.0),
                );
                let resize_resp = ui.interact(
                    resize_rect,
                    ui.id().with("project_resize"),
                    Sense::click_and_drag(),
                );
                if resize_resp.hovered() || resize_resp.dragged() {
                    ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeVertical);
                    ui.painter().line_segment(
                        [egui::pos2(rect.left(), rect.top()), egui::pos2(rect.right(), rect.top())],
                        Stroke::new(2.0, Color32::from_rgb(15, 232, 121)),
                    );
                }
                if resize_resp.drag_started() {
                    resize_started = true;
                }
                if resize_resp.drag_stopped() {
                    resize_stopped = true;
                }

                let inner = rect.shrink2(egui::vec2(8.0, 6.0));
                let header_rect =
                    Rect::from_min_max(inner.min, egui::pos2(inner.max.x, inner.min.y + 24.0));
                let splitter_y = header_rect.bottom() + 4.0;
                let search_hint = self.tr(language, "search");
                let breadcrumb = self.breadcrumb_segments(language);
                let search_w = 220.0;
                let search_x = (header_rect.center().x - search_w * 0.5 - 36.0)
                    .clamp(header_rect.left() + 6.0, header_rect.right() - search_w);
                let search_rect = Rect::from_min_max(
                    egui::pos2(search_x, header_rect.top()),
                    egui::pos2(search_x + search_w, header_rect.bottom()),
                );
                let left_header_rect = Rect::from_min_max(
                    header_rect.min,
                    egui::pos2(search_rect.left() - 6.0, header_rect.bottom()),
                );

                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(left_header_rect)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    |ui| {
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new(self.tr(language, "title"))
                                .size(12.0)
                                .color(Color32::from_gray(175)),
                        );
                        ui.add_space(6.0);
                        ui.label(egui::RichText::new("|").size(12.0).color(Color32::from_gray(110)));
                        ui.add_space(8.0);

                        for (idx, (folder_id, folder_label)) in breadcrumb.iter().enumerate() {
                            let is_current = *folder_id == self.selected_folder;
                            let crumb = ui.add(
                                egui::Label::new(
                                    egui::RichText::new(folder_label)
                                        .size(12.0)
                                        .color(Color32::from_gray(if is_current { 220 } else { 190 })),
                                )
                                .sense(Sense::click()),
                            );
                            if crumb.hovered() {
                                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                                ui.painter().line_segment(
                                    [
                                        egui::pos2(crumb.rect.left(), crumb.rect.bottom() + 1.0),
                                        egui::pos2(crumb.rect.right(), crumb.rect.bottom() + 1.0),
                                    ],
                                    Stroke::new(1.0, Color32::from_rgb(15, 232, 121)),
                                );
                            }
                            if crumb.clicked() {
                                self.selected_folder = *folder_id;
                                self.selected_asset = None;
                                if self.selected_folder == "Assets" {
                                    self.assets_open = true;
                                } else if self.selected_folder == "Packages" {
                                    self.packages_open = true;
                                }
                            }

                            if idx + 1 < breadcrumb.len() {
                                ui.label(
                                    egui::RichText::new(">").size(12.0).color(Color32::from_gray(150)),
                                );
                            }
                        }
                    },
                );

                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(search_rect)
                        .layout(
                            egui::Layout::left_to_right(egui::Align::Center)
                                .with_main_align(egui::Align::Center),
                        ),
                    |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.search_query)
                                .desired_width(search_w)
                                .hint_text(search_hint),
                        );
                    },
                );

                ui.painter().line_segment(
                    [
                        egui::pos2(inner.left(), splitter_y),
                        egui::pos2(inner.right(), splitter_y),
                    ],
                    Stroke::new(1.0, Color32::from_rgb(62, 62, 62)),
                );

                let content_rect = Rect::from_min_max(
                    egui::pos2(inner.left(), splitter_y + 6.0),
                    egui::pos2(inner.right(), inner.bottom() - 20.0),
                );
                let sidebar_w = 212.0;
                let sidebar_rect = Rect::from_min_max(
                    content_rect.min,
                    egui::pos2(content_rect.left() + sidebar_w, content_rect.bottom()),
                );
                let grid_rect = Rect::from_min_max(
                    egui::pos2(sidebar_rect.right() + 8.0, content_rect.top()),
                    content_rect.max,
                );

                ui.painter().line_segment(
                    [
                        egui::pos2(sidebar_rect.right() + 4.0, content_rect.top()),
                        egui::pos2(sidebar_rect.right() + 4.0, content_rect.bottom()),
                    ],
                    Stroke::new(1.0, Color32::from_rgb(60, 60, 60)),
                );

                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(sidebar_rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                    |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("project_sidebar")
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                let assets_selected = self.selected_folder == "Assets";
                                let mut assets_open = self.assets_open;
                                let assets_resp = self.draw_tree_parent_row(
                                    ui,
                                    "assets_root",
                                    self.tr(language, "assets"),
                                    0.0,
                                    &mut assets_open,
                                    assets_selected,
                                );
                                self.assets_open = assets_open;
                                if assets_resp.clicked() {
                                    self.selected_folder = "Assets";
                                    self.selected_asset = None;
                                }

                                if self.assets_open {
                                    for folder in ["Animations", "Materials", "Meshes", "Mold", "Scripts"] {
                                        let leaf = Self::draw_tree_leaf_row(
                                            ui,
                                            folder,
                                            folder,
                                            18.0,
                                            self.selected_folder == folder,
                                        );
                                        if leaf.clicked() {
                                            self.selected_folder = folder;
                                            self.selected_asset = None;
                                        }
                                    }
                                }

                                ui.add_space(2.0);

                                let packages_selected = self.selected_folder == "Packages";
                                let mut packages_open = self.packages_open;
                                let pkg_resp = self.draw_tree_parent_row(
                                    ui,
                                    "packages_root",
                                    self.tr(language, "packages"),
                                    0.0,
                                    &mut packages_open,
                                    packages_selected,
                                );
                                self.packages_open = packages_open;
                                if pkg_resp.clicked() {
                                    self.selected_folder = "Packages";
                                    self.selected_asset = None;
                                }

                                if self.packages_open {
                                    for folder in ["TextMeshPro", "InputSystem"] {
                                        let leaf = Self::draw_tree_leaf_row(
                                            ui,
                                            folder,
                                            folder,
                                            18.0,
                                            self.selected_folder == folder,
                                        );
                                        if leaf.clicked() {
                                            self.selected_folder = folder;
                                            self.selected_asset = None;
                                        }
                                    }
                                }
                            });
                    },
                );

                let assets = self.assets_for_folder();
                let filter = self.search_query.to_lowercase();

                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(grid_rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                    |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("project_grid")
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                ui.horizontal_wrapped(|ui| {
                                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
                                    let now = ui.ctx().input(|i| i.time);
                                    let mut hovered_any = false;

                                    for asset in assets {
                                        if self.deleted_assets.contains(asset) {
                                            continue;
                                        }
                                        if !filter.is_empty() && !asset.to_lowercase().contains(&filter) {
                                            continue;
                                        }

                                        let tile_w = self.icon_scale.clamp(56.0, 98.0);
                                        let tile_size = Vec2::new(tile_w, tile_w + 20.0);
                                        let selected = self.selected_asset == Some(asset);
                                        let (tile_rect, tile_resp) =
                                            ui.allocate_exact_size(tile_size, Sense::click());

                                        ui.painter().rect_filled(
                                            tile_rect,
                                            4.0,
                                            if selected {
                                                Color32::from_rgb(64, 64, 64)
                                            } else {
                                                Color32::from_rgb(44, 44, 44)
                                            },
                                        );
                                        ui.painter().rect_stroke(
                                            tile_rect,
                                            4.0,
                                            if selected {
                                                Stroke::new(1.0, Color32::from_rgb(15, 232, 121))
                                            } else {
                                                Stroke::new(1.0, Color32::from_rgb(58, 58, 58))
                                            },
                                            egui::StrokeKind::Outside,
                                        );

                                        let (icon_color, icon_tag) = Self::icon_style(asset);
                                        let preview_rect = Rect::from_min_max(
                                            tile_rect.min + egui::vec2(7.0, 7.0),
                                            egui::pos2(tile_rect.max.x - 7.0, tile_rect.max.y - 20.0),
                                        );
                                        ui.painter().rect_filled(preview_rect, 2.0, icon_color);
                                        ui.painter().text(
                                            preview_rect.center(),
                                            egui::Align2::CENTER_CENTER,
                                            icon_tag,
                                            FontId::proportional(10.0),
                                            Color32::from_gray(245),
                                        );
                                        let name_font = FontId::proportional(11.0);
                                        let name_color = Color32::from_gray(210);
                                        let name_rect = Rect::from_min_max(
                                            egui::pos2(tile_rect.left() + 5.0, tile_rect.bottom() - 16.0),
                                            egui::pos2(tile_rect.right() - 5.0, tile_rect.bottom() - 2.0),
                                        );
                                        let clipped_painter = ui.painter().with_clip_rect(name_rect);
                                        let full_w = ui
                                            .painter()
                                            .layout_no_wrap((*asset).to_string(), name_font.clone(), name_color)
                                            .size()
                                            .x;

                                        if full_w <= name_rect.width() {
                                            clipped_painter.text(
                                                name_rect.center(),
                                                egui::Align2::CENTER_CENTER,
                                                asset,
                                                name_font.clone(),
                                                name_color,
                                            );
                                        } else if tile_resp.hovered() {
                                            hovered_any = true;
                                            if self.hover_roll_asset != Some(asset) {
                                                self.hover_roll_asset = Some(asset);
                                                self.hover_still_since = now;
                                            }

                                            let hover_elapsed = now - self.hover_still_since;
                                            if hover_elapsed < 0.18 {
                                                let short = Self::truncate_with_ellipsis(
                                                    ui.painter(),
                                                    asset,
                                                    &name_font,
                                                    name_rect.width(),
                                                );
                                                clipped_painter.text(
                                                    name_rect.center(),
                                                    egui::Align2::CENTER_CENTER,
                                                    short,
                                                    name_font.clone(),
                                                    name_color,
                                                );
                                            } else {
                                                ui.ctx().request_repaint();
                                                let anim_time = (hover_elapsed - 0.18) as f32;
                                                let tail_pad = 8.0;
                                                let overflow = (full_w - name_rect.width() + tail_pad).max(0.0);
                                                let speed = 12.0;
                                                let start_pause = 0.65;
                                                let end_pause = 1.0;
                                                let run_time = overflow / speed;
                                                let cycle = start_pause + run_time + end_pause;
                                                let phase = anim_time % cycle;
                                                let scroll_x = if phase < start_pause {
                                                    0.0
                                                } else if phase < start_pause + run_time {
                                                    (phase - start_pause) * speed
                                                } else {
                                                    overflow
                                                };
                                                let base_x = name_rect.center().x - full_w * 0.5;

                                                clipped_painter.text(
                                                    egui::pos2(base_x - scroll_x, name_rect.center().y),
                                                    egui::Align2::LEFT_CENTER,
                                                    asset,
                                                    name_font.clone(),
                                                    name_color,
                                                );
                                            }
                                        } else {
                                            let short = Self::truncate_with_ellipsis(
                                                ui.painter(),
                                                asset,
                                                &name_font,
                                                name_rect.width(),
                                            );
                                            clipped_painter.text(
                                                name_rect.center(),
                                                egui::Align2::CENTER_CENTER,
                                                short,
                                                name_font.clone(),
                                                name_color,
                                            );
                                        }

                                        let mut open_clicked = false;
                                        let mut reveal_clicked = false;
                                        let mut delete_clicked = false;
                                        tile_resp.context_menu(|ui| {
                                            if ui.button(self.tr(language, "open")).clicked() {
                                                open_clicked = true;
                                                ui.close();
                                            }
                                            if ui.button(self.tr(language, "reveal")).clicked() {
                                                reveal_clicked = true;
                                                ui.close();
                                            }
                                            ui.separator();
                                            if ui
                                                .add(
                                                    egui::Button::new(self.tr(language, "delete"))
                                                        .fill(Color32::from_rgb(74, 38, 38)),
                                                )
                                                .clicked()
                                            {
                                                delete_clicked = true;
                                                ui.close();
                                            }
                                        });

                                        if open_clicked {
                                            self.selected_asset = Some(asset);
                                            self.status_text =
                                                format!("{}: {}", self.tr(language, "open"), asset);
                                        }
                                        if reveal_clicked {
                                            self.status_text =
                                                format!("{}: {}", self.tr(language, "reveal"), asset);
                                        }
                                        if delete_clicked {
                                            self.deleted_assets.insert(asset);
                                            if self.selected_asset == Some(asset) {
                                                self.selected_asset = None;
                                            }
                                            self.status_text =
                                                format!("{}: {}", self.tr(language, "delete"), asset);
                                        }

                                        if tile_resp.clicked() {
                                            self.selected_asset = Some(asset);
                                            self.status_text = asset.to_string();
                                        }
                                    }

                                    if !hovered_any {
                                        self.hover_roll_asset = None;
                                    }
                                });
                            });
                    },
                );

                let footer_rect = Rect::from_min_max(
                    egui::pos2(inner.left(), inner.bottom() - 18.0),
                    inner.max,
                );
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(footer_rect)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    |ui| {
                        let count = assets
                            .iter()
                            .filter(|asset| {
                                !self.deleted_assets.contains(*asset)
                                    && (filter.is_empty() || asset.to_lowercase().contains(&filter))
                            })
                            .count();
                        let status = if self.status_text.is_empty() {
                            format!("{} {}", count, self.tr(language, "count"))
                        } else {
                            format!("{} {} | {}", count, self.tr(language, "count"), self.status_text)
                        };
                        ui.label(
                            egui::RichText::new(status)
                                .size(11.0)
                                .color(Color32::from_gray(165)),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let (slider_rect, _) =
                                ui.allocate_exact_size(egui::vec2(140.0, 14.0), Sense::hover());
                            self.draw_icon_size_slider(ui, slider_rect);
                        });
                    },
                );
            });

        if resize_started {
            self.resizing_height = true;
        }

        if self.resizing_height && pointer_down {
            let delta = ctx.input(|i| i.pointer.delta());
            if delta.y != 0.0 {
                self.panel_height = (self.panel_height - delta.y).clamp(min_h, max_h);
            }
        }

        if resize_stopped || (self.resizing_height && !pointer_down) {
            self.resizing_height = false;
        }
    }

    pub fn docked_bottom_height(&self) -> f32 {
        if self.open {
            self.panel_height
        } else {
            0.0
        }
    }
}
