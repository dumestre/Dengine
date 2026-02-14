// src/main.rs
mod inspector;
mod hierarchy;
mod project;
mod viewport;
mod viewport_gpu;

use eframe::egui::{TextureHandle, TextureOptions};
use eframe::{App, Frame, NativeOptions, egui};
use epaint::ColorImage;
use hierarchy::HierarchyWindow;
use inspector::InspectorWindow;
use project::ProjectWindow;
use viewport::ViewportPanel;
use viewport_gpu::ViewportGpuRenderer;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
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
    viewport: ViewportPanel,
    viewport_gpu: Option<ViewportGpuRenderer>,
    app_icon_texture: Option<TextureHandle>,
    cena_icon: Option<TextureHandle>,
    game_icon: Option<TextureHandle>,
    play_icon: Option<TextureHandle>,
    pause_icon: Option<TextureHandle>,
    stop_icon: Option<TextureHandle>,
    files_icon: Option<TextureHandle>,
    rig_icon: Option<TextureHandle>,
    animador_icon: Option<TextureHandle>,
    fios_icon: Option<TextureHandle>,
    lang_pt_icon: Option<TextureHandle>,
    lang_en_icon: Option<TextureHandle>,
    lang_es_icon: Option<TextureHandle>,
    is_playing: bool,
    is_window_maximized: bool,
    selected_mode: ToolbarMode,
    rig_enabled: bool,
    animator_enabled: bool,
    fios_enabled: bool,
    language: EngineLanguage,
    project_collapsed: bool,
    windows_blur_initialized: bool,
    last_pointer_pos: Option<egui::Pos2>,
    show_hub: bool,
    hub_projects: Vec<PathBuf>,
    hub_selected: Option<usize>,
    current_project: Option<PathBuf>,
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
    fn hub_registry_path() -> PathBuf {
        PathBuf::from(".dengine_hub_projects.txt")
    }

    fn normalize_project_path(path: &Path) -> PathBuf {
        fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
    }

    fn load_hub_registry() -> Vec<PathBuf> {
        let mut out = Vec::new();
        let path = Self::hub_registry_path();
        let Ok(content) = fs::read_to_string(path) else {
            return out;
        };
        for line in content.lines() {
            let item = line.trim();
            if item.is_empty() {
                continue;
            }
            let p = PathBuf::from(item);
            if p.exists()
                && p.extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.eq_ignore_ascii_case("deng"))
                    == Some(true)
            {
                out.push(Self::normalize_project_path(&p));
            }
        }
        out
    }

    fn save_hub_registry(&self) {
        let mut lines = String::new();
        for p in &self.hub_projects {
            lines.push_str(&p.to_string_lossy());
            lines.push('\n');
        }
        let _ = fs::write(Self::hub_registry_path(), lines);
    }

    fn sort_and_dedupe_paths(paths: &mut Vec<PathBuf>) {
        paths.sort_by_key(|p| p.to_string_lossy().to_ascii_lowercase());
        paths.dedup_by(|a, b| {
            a.to_string_lossy()
                .eq_ignore_ascii_case(b.to_string_lossy().as_ref())
        });
    }

    fn register_hub_project(&mut self, project_path: &Path) {
        let normalized = Self::normalize_project_path(project_path);
        self.hub_projects.push(normalized);
        Self::sort_and_dedupe_paths(&mut self.hub_projects);
        self.save_hub_registry();
    }

    fn current_project_label(&self) -> String {
        self.current_project
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "Sem projeto".to_string())
    }

    fn refresh_hub_projects(&mut self) {
        let mut out = Vec::<PathBuf>::new();
        let root = Path::new(".");
        collect_deng_files(root, root, 0, &mut out);
        for p in Self::load_hub_registry() {
            out.push(p);
        }
        Self::sort_and_dedupe_paths(&mut out);
        self.hub_projects = out;
        self.save_hub_registry();
        if let Some(sel) = self.hub_selected {
            if sel >= self.hub_projects.len() {
                self.hub_selected = None;
            }
        }
    }

    fn create_project_dialog(&mut self) {
        let picked = rfd::FileDialog::new()
            .add_filter("Dengine Project", &["deng"])
            .set_file_name("NovoProjeto.deng")
            .save_file();
        let Some(mut path) = picked else {
            return;
        };
        if path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("deng"))
            != Some(true)
        {
            path.set_extension("deng");
        }

        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("NovoProjeto")
            .to_string();
        let project_dir = parent.join(&stem);
        let _ = std::fs::create_dir_all(project_dir.join("Assets"));
        let project_file = project_dir.join(format!("{stem}.deng"));

        if let Ok(mut f) = File::create(&project_file) {
            let _ = f.write_all(b"DENG1\n");
        }
        let normalized = Self::normalize_project_path(&project_file);
        self.current_project = Some(normalized.clone());
        self.register_hub_project(&normalized);
        self.show_hub = false;
        self.refresh_hub_projects();
    }

    fn open_project_dialog(&mut self) {
        let picked = rfd::FileDialog::new()
            .add_filter("Dengine Project", &["deng"])
            .pick_file();
        let Some(path) = picked else {
            return;
        };
        let normalized = Self::normalize_project_path(&path);
        self.current_project = Some(normalized.clone());
        self.register_hub_project(&normalized);
        self.show_hub = false;
    }

    fn draw_hub(&mut self, ctx: &egui::Context) {
        let bg = egui::Color32::from_rgb(20, 23, 24);
        let panel_fill = egui::Color32::from_rgb(28, 33, 34);
        let panel_stroke = egui::Color32::from_rgba_unmultiplied(210, 228, 222, 42);
        let accent = egui::Color32::from_rgb(15, 232, 121);
        let muted = egui::Color32::from_gray(170);

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(bg))
            .show(ctx, |ui| {
                ui.add_space(14.0);
                egui::Frame::new()
                    .fill(panel_fill)
                    .stroke(egui::Stroke::new(1.0, panel_stroke))
                    .corner_radius(8)
                    .inner_margin(egui::Margin::same(12))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if let Some(icon) = &self.app_icon_texture {
                                ui.add(
                                    egui::Image::new(icon)
                                        .fit_to_exact_size(egui::Vec2::new(18.0, 18.0)),
                                );
                                ui.add_space(6.0);
                            }
                            ui.vertical(|ui| {
                                ui.label(
                                    egui::RichText::new("Dengine Hub")
                                        .strong()
                                        .size(18.0)
                                        .color(egui::Color32::from_gray(230)),
                                );
                                ui.label(
                                    egui::RichText::new("Projetos locais")
                                        .size(12.0)
                                        .color(muted),
                                );
                            });
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let control_size = egui::Vec2::new(22.0, 22.0);

                                    let (close_rect, close_resp) =
                                        ui.allocate_exact_size(control_size, egui::Sense::click());
                                    if close_resp.hovered() {
                                        ui.painter().circle_filled(
                                            close_rect.center(),
                                            9.0,
                                            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 28),
                                        );
                                    }
                                    ui.painter().circle_filled(
                                        close_rect.center(),
                                        5.0,
                                        egui::Color32::from_rgb(0xD0, 0x24, 0x24),
                                    );
                                    if close_resp.clicked() {
                                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                                    }

                                    let (max_rect, max_resp) =
                                        ui.allocate_exact_size(control_size, egui::Sense::click());
                                    if max_resp.hovered() {
                                        ui.painter().circle_filled(
                                            max_rect.center(),
                                            9.0,
                                            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 28),
                                        );
                                    }
                                    ui.painter().circle_filled(
                                        max_rect.center(),
                                        5.0,
                                        egui::Color32::from_rgb(0x04, 0xBA, 0x6C),
                                    );
                                    if max_resp.clicked() {
                                        self.is_window_maximized = !self.is_window_maximized;
                                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(
                                            self.is_window_maximized,
                                        ));
                                    }

                                    let (min_rect, min_resp) =
                                        ui.allocate_exact_size(control_size, egui::Sense::click());
                                    if min_resp.hovered() {
                                        ui.painter().circle_filled(
                                            min_rect.center(),
                                            9.0,
                                            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 28),
                                        );
                                    }
                                    ui.painter().circle_filled(
                                        min_rect.center(),
                                        5.0,
                                        egui::Color32::from_rgb(0xD5, 0x3C, 0x0D),
                                    );
                                    if min_resp.clicked() {
                                        ui.ctx()
                                            .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                                    }

                                    ui.add_space(8.0);
                                    let version = format!("Engine {}", env!("CARGO_PKG_VERSION"));
                                    ui.label(egui::RichText::new(version).size(11.0).color(accent));
                                },
                            );
                        });
                    });

                ui.add_space(10.0);
                ui.horizontal_top(|ui| {
                    ui.set_height(ui.available_height());
                    ui.allocate_ui_with_layout(
                        egui::Vec2::new(260.0, ui.available_height()),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            egui::Frame::new()
                                .fill(panel_fill)
                                .stroke(egui::Stroke::new(1.0, panel_stroke))
                                .corner_radius(8)
                                .inner_margin(egui::Margin::same(12))
                                .show(ui, |ui| {
                                    ui.label(
                                        egui::RichText::new("Acoes")
                                            .size(13.0)
                                            .color(egui::Color32::from_gray(220)),
                                    );
                                    ui.add_space(8.0);
                                    if ui
                                        .add_sized(
                                            [ui.available_width(), 30.0],
                                            egui::Button::new("Novo Projeto")
                                                .corner_radius(6)
                                                .fill(egui::Color32::from_rgb(44, 44, 44))
                                                .stroke(egui::Stroke::new(1.0, accent)),
                                        )
                                        .clicked()
                                    {
                                        self.create_project_dialog();
                                    }
                                    if ui
                                        .add_sized(
                                            [ui.available_width(), 30.0],
                                            egui::Button::new("Abrir .deng")
                                                .corner_radius(6)
                                                .fill(egui::Color32::from_rgb(44, 44, 44))
                                                .stroke(egui::Stroke::new(
                                                    1.0,
                                                    egui::Color32::from_gray(70),
                                                )),
                                        )
                                        .clicked()
                                    {
                                        self.open_project_dialog();
                                    }
                                    if ui
                                        .add_sized(
                                            [ui.available_width(), 28.0],
                                            egui::Button::new("Atualizar Lista")
                                                .corner_radius(6)
                                                .fill(egui::Color32::from_rgb(40, 45, 46))
                                                .stroke(egui::Stroke::new(
                                                    1.0,
                                                    egui::Color32::from_gray(70),
                                                )),
                                        )
                                        .clicked()
                                    {
                                        self.refresh_hub_projects();
                                    }
                                    ui.add_space(10.0);
                                    ui.separator();
                                    ui.add_space(10.0);
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "{} projeto(s) encontrado(s)",
                                            self.hub_projects.len()
                                        ))
                                        .size(11.0)
                                        .color(muted),
                                    );
                                    ui.label(
                                        egui::RichText::new(self.current_project_label())
                                            .size(11.0)
                                            .color(egui::Color32::from_gray(140)),
                                    );
                                });
                        },
                    );

                    ui.add_space(10.0);
                    egui::Frame::new()
                        .fill(panel_fill)
                        .stroke(egui::Stroke::new(1.0, panel_stroke))
                        .corner_radius(8)
                        .inner_margin(egui::Margin::same(12))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new("Projetos")
                                        .size(13.0)
                                        .color(egui::Color32::from_gray(220)),
                                );
                                ui.label(
                                    egui::RichText::new(".deng")
                                        .size(11.0)
                                        .color(egui::Color32::from_gray(150)),
                                );
                            });
                            ui.add_space(8.0);

                            egui::ScrollArea::vertical()
                                .max_height((ui.available_height() - 46.0).max(80.0))
                                .show(ui, |ui| {
                                    if self.hub_projects.is_empty() {
                                        ui.label(
                                            egui::RichText::new(
                                                "Nenhum projeto encontrado. Crie ou abra um .deng.",
                                            )
                                            .color(muted),
                                        );
                                    }

                                    for (idx, path) in self.hub_projects.iter().enumerate() {
                                        let selected = self.hub_selected == Some(idx);
                                        let name = path
                                            .file_stem()
                                            .and_then(|s| s.to_str())
                                            .unwrap_or("Projeto");
                                        let full = path.to_string_lossy();
                                        let stroke = if selected {
                                            egui::Stroke::new(1.0, accent)
                                        } else {
                                            egui::Stroke::new(1.0, egui::Color32::from_gray(66))
                                        };
                                        let fill = if selected {
                                            egui::Color32::from_rgb(40, 51, 46)
                                        } else {
                                            egui::Color32::from_rgb(34, 38, 39)
                                        };

                                        let button = egui::Button::new(
                                            egui::RichText::new(name).color(egui::Color32::WHITE),
                                        )
                                        .fill(fill)
                                        .stroke(stroke)
                                        .corner_radius(6);
                                        let resp = ui
                                            .add_sized([ui.available_width(), 28.0], button)
                                            .on_hover_text(full.as_ref());
                                        if resp.clicked() {
                                            self.hub_selected = Some(idx);
                                        }
                                    }
                                });

                            ui.add_space(8.0);
                            let can_enter = self.hub_selected.is_some();
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .add_enabled(
                                            can_enter,
                                            egui::Button::new("Entrar no Projeto")
                                                .corner_radius(6)
                                                .fill(egui::Color32::from_rgb(44, 44, 44))
                                                .stroke(egui::Stroke::new(1.0, accent)),
                                        )
                                        .clicked()
                                    {
                                        if let Some(idx) = self.hub_selected {
                                            if let Some(path) = self.hub_projects.get(idx) {
                                                self.current_project = Some(path.clone());
                                                self.show_hub = false;
                                            }
                                        }
                                    }
                                },
                            );
                        });
                });
            });
    }

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

            (EngineLanguage::Pt, "import") => "Importar",
            (EngineLanguage::En, "import") => "Import",
            (EngineLanguage::Es, "import") => "Importar",

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
        if self.rig_icon.is_none() {
            self.rig_icon = load_png_as_texture(ctx, "src/assets/icons/rig.png");
        }
        if self.animador_icon.is_none() {
            self.animador_icon = load_png_as_texture(ctx, "src/assets/icons/animador.png");
        }
        if self.fios_icon.is_none() {
            self.fios_icon = load_png_as_texture(ctx, "src/assets/icons/fios.png");
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
        if self.show_hub {
            self.draw_hub(ctx);
            return;
        }
        let undo_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::Z);
        let redo_shortcut =
            egui::KeyboardShortcut::new(egui::Modifiers::CTRL | egui::Modifiers::SHIFT, egui::Key::Z);
        let redo_shortcut_alt = egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::Y);
        let undo_pressed = ctx.input_mut(|i| i.consume_shortcut(&undo_shortcut));
        let redo_pressed = ctx.input_mut(|i| i.consume_shortcut(&redo_shortcut))
            || ctx.input_mut(|i| i.consume_shortcut(&redo_shortcut_alt));
        if undo_pressed {
            self.viewport.undo();
        }
        if redo_pressed {
            self.viewport.redo();
        }
        if !self.windows_blur_initialized {
            self.windows_blur_initialized = true;
            let _ = enable_windows_backdrop_blur(frame);
        }

        // Barra de título customizada
        egui::TopBottomPanel::top("window_controls_bar")
            .exact_height(30.0)
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::from_rgba_unmultiplied(24, 31, 30, 76))
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
                    egui::Color32::from_rgba_unmultiplied(245, 252, 249, 18),
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
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new(self.current_project_label())
                                .size(11.0)
                                .color(egui::Color32::from_gray(170)),
                        );
                        ui.add_space(10.0);
                        if ui
                            .add_sized([54.0, 22.0], egui::Button::new("Hub").corner_radius(6))
                            .clicked()
                        {
                            self.show_hub = true;
                            self.refresh_hub_projects();
                        }
                        ui.add_space(8.0);

                        egui::MenuBar::new().ui(ui, |ui| {
                            ui.menu_button(self.tr("menu_file"), |ui| {
                                if ui.button(self.tr("new")).clicked() {
                                    ui.close();
                                }
                                if ui.button(self.tr("save")).clicked() {
                                    if let Some(path) = self.current_project.clone() {
                                        let _ = self.project.save_project_to_path(&path, self.language);
                                    } else if let Some(path) = self.project.save_project_dialog(self.language) {
                                        self.current_project = Some(path);
                                    }
                                    ui.close();
                                }
                                if ui.button(self.tr("import")).clicked() {
                                    self.project.import_asset_dialog(self.language);
                                    ui.close();
                                }
                                if ui.button(self.tr("exit")).clicked() {
                                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                                    ui.close();
                                }
                            });

                            ui.menu_button(self.tr("menu_edit"), |ui| {
                                if ui
                                    .add_enabled(self.viewport.can_undo(), egui::Button::new("Undo (Ctrl+Z)"))
                                    .clicked()
                                {
                                    self.viewport.undo();
                                    ui.close();
                                }
                                if ui
                                    .add_enabled(
                                        self.viewport.can_redo(),
                                        egui::Button::new("Redo (Ctrl+Shift+Z)"),
                                    )
                                    .clicked()
                                {
                                    self.viewport.redo();
                                    ui.close();
                                }
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

        let dock_bar_h = 48.0;
        let project_bottom = if self.project_collapsed {
            dock_bar_h
        } else {
            self.project.docked_bottom_height()
        };
        let left_reserved = self.inspector.docked_left_width() + self.hierarchy.docked_left_width();
        let right_reserved = self.inspector.docked_right_width() + self.hierarchy.docked_right_width();
        let mode_label = if self.selected_mode == ToolbarMode::Cena {
            "Cena"
        } else {
            "Game"
        };
        let hierarchy_selected = self.hierarchy.selected_object_name().to_string();
        self.viewport.set_selected_object(&hierarchy_selected);
        self.viewport
            .show(
                ctx,
                mode_label,
                left_reserved,
                right_reserved,
                project_bottom,
                self.viewport_gpu.as_ref(),
            );

        // Janela Inspetor
        self.inspector
            .show(
                ctx,
                0.0,
                0.0,
                project_bottom,
                self.language,
                self.hierarchy.selected_object_name(),
            );
        let i_left = self.inspector.docked_left_width();
        let i_right = self.inspector.docked_right_width();
        self.hierarchy
            .show(ctx, i_left, i_right, project_bottom, self.language);
        if let Some(selected_in_viewport) = self.viewport.selected_object_name() {
            self.hierarchy.set_selected_object(selected_in_viewport);
        }

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

                    let icon_center_y = rect.top() + 15.0;
                    let button_start_x = 28.0;
                    let button_spacing = 46.0;
                    let files_rect = egui::Rect::from_center_size(
                        egui::pos2(rect.left() + button_start_x, icon_center_y),
                        egui::vec2(28.0, 22.0),
                    );
                    let files_resp = ui.interact(
                        files_rect,
                        ui.id().with("restore_project_from_dock"),
                        egui::Sense::click(),
                    );
                    if files_resp.hovered() {
                        ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                        ui.painter().rect_filled(
                            files_rect.expand(2.0),
                            3.0,
                            egui::Color32::from_rgb(58, 58, 58),
                        );
                    }
                    if files_resp.clicked() {
                        self.project_collapsed = false;
                    }

                    if let Some(files_icon) = &self.files_icon {
                        let _ = ui.put(
                            files_rect,
                            egui::Image::new(files_icon)
                                .fit_to_exact_size(egui::Vec2::new(20.0, 20.0)),
                        );
                    }

                    let rig_rect = egui::Rect::from_center_size(
                        egui::pos2(rect.left() + button_start_x + button_spacing, icon_center_y),
                        egui::vec2(28.0, 22.0),
                    );
                    let rig_resp = ui.interact(
                        rig_rect,
                        ui.id().with("toggle_rig_mode"),
                        egui::Sense::click(),
                    );
                    if rig_resp.hovered() || self.rig_enabled {
                        ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                        ui.painter().rect_filled(
                            rig_rect.expand(2.0),
                            3.0,
                            if self.rig_enabled {
                                egui::Color32::from_rgb(58, 84, 64)
                            } else {
                                egui::Color32::from_rgb(58, 58, 58)
                            },
                        );
                    }
                    if rig_resp.clicked() {
                        self.rig_enabled = !self.rig_enabled;
                    }
                    if let Some(rig_icon) = &self.rig_icon {
                        let _ = ui.put(
                            rig_rect,
                            egui::Image::new(rig_icon)
                                .fit_to_exact_size(egui::Vec2::new(20.0, 20.0)),
                        );
                    }

                    let animator_rect = egui::Rect::from_center_size(
                        egui::pos2(
                            rect.left() + button_start_x + button_spacing * 2.0,
                            icon_center_y,
                        ),
                        egui::vec2(28.0, 22.0),
                    );
                    let animator_resp = ui.interact(
                        animator_rect,
                        ui.id().with("toggle_animator_mode"),
                        egui::Sense::click(),
                    );
                    if animator_resp.hovered() || self.animator_enabled {
                        ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                        ui.painter().rect_filled(
                            animator_rect.expand(2.0),
                            3.0,
                            if self.animator_enabled {
                                egui::Color32::from_rgb(58, 84, 64)
                            } else {
                                egui::Color32::from_rgb(58, 58, 58)
                            },
                        );
                    }
                    if animator_resp.clicked() {
                        self.animator_enabled = !self.animator_enabled;
                    }
                    if let Some(animador_icon) = &self.animador_icon {
                        let _ = ui.put(
                            animator_rect,
                            egui::Image::new(animador_icon)
                                .fit_to_exact_size(egui::Vec2::new(20.0, 20.0)),
                        );
                    }

                    let fios_rect = egui::Rect::from_center_size(
                        egui::pos2(
                            rect.left() + button_start_x + button_spacing * 3.0,
                            icon_center_y,
                        ),
                        egui::vec2(28.0, 22.0),
                    );
                    let fios_resp = ui.interact(
                        fios_rect,
                        ui.id().with("toggle_fios_mode"),
                        egui::Sense::click(),
                    );
                    if fios_resp.hovered() || self.fios_enabled {
                        ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                        ui.painter().rect_filled(
                            fios_rect.expand(2.0),
                            3.0,
                            if self.fios_enabled {
                                egui::Color32::from_rgb(58, 84, 64)
                            } else {
                                egui::Color32::from_rgb(58, 58, 58)
                            },
                        );
                    }
                    if fios_resp.clicked() {
                        self.fios_enabled = !self.fios_enabled;
                    }
                    if let Some(fios_icon) = &self.fios_icon {
                        let _ = ui.put(
                            fios_rect,
                            egui::Image::new(fios_icon)
                                .fit_to_exact_size(egui::Vec2::new(20.0, 20.0)),
                        );
                    }

                    let label_y = rect.bottom() - 10.0;
                    ui.painter().text(
                        egui::pos2(files_rect.center().x, label_y),
                        egui::Align2::CENTER_CENTER,
                        "Projeto",
                        egui::FontId::proportional(12.0),
                        egui::Color32::from_gray(190),
                    );
                    ui.painter().text(
                        egui::pos2(rig_rect.center().x, label_y),
                        egui::Align2::CENTER_CENTER,
                        "Rig",
                        egui::FontId::proportional(12.0),
                        egui::Color32::from_gray(190),
                    );
                    ui.painter().text(
                        egui::pos2(animator_rect.center().x, label_y),
                        egui::Align2::CENTER_CENTER,
                        "Animador",
                        egui::FontId::proportional(12.0),
                        egui::Color32::from_gray(190),
                    );
                    ui.painter().text(
                        egui::pos2(fios_rect.center().x, label_y),
                        egui::Align2::CENTER_CENTER,
                        "Fios",
                        egui::FontId::proportional(12.0),
                        egui::Color32::from_gray(190),
                    );

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

        let pointer_pos = ctx.input(|i| i.pointer.hover_pos().or(i.pointer.latest_pos()));
        if pointer_pos.is_some() {
            self.last_pointer_pos = pointer_pos;
        }
        let drop_pos = pointer_pos.or(self.last_pointer_pos);
        let pointer_down = ctx.input(|i| i.pointer.primary_down());
        if !pointer_down {
            if let (Some(asset_name), Some(pos)) = (self.project.dragging_asset_name(), drop_pos) {
                if self.viewport.contains_point(pos) {
                    let object_name = self.hierarchy.on_asset_dropped(asset_name);
                    if let Some(path) = self.project.dragging_asset_path() {
                        self.viewport.on_asset_file_dropped_named(&path, &object_name);
                    } else {
                        self.viewport.on_asset_dropped(&object_name);
                    }
                } else if self.hierarchy.contains_point(pos) {
                    self.hierarchy.on_asset_dropped(asset_name);
                }
            }
            self.project.clear_dragging_asset();
        }

        let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
        if !dropped_files.is_empty() {
            if let Some(pos) = drop_pos {
                for file in dropped_files {
                    let asset_name = if let Some(path) = &file.path {
                        path.file_name()
                            .and_then(|n| n.to_str())
                            .map(|s| s.to_owned())
                            .unwrap_or_else(|| file.name.clone())
                    } else {
                        file.name.clone()
                    };
                    if asset_name.is_empty() {
                        continue;
                    }
                    if let Some(path) = &file.path {
                        self.project.import_file_path(path, self.language);
                    }
                    if self.viewport.contains_point(pos) {
                        if let Some(path) = &file.path {
                            let object_name = self.hierarchy.on_asset_dropped(&asset_name);
                            self.viewport.on_asset_file_dropped_named(path, &object_name);
                        } else {
                            let object_name = self.hierarchy.on_asset_dropped(&asset_name);
                            self.viewport.on_asset_dropped(&object_name);
                        }
                    } else if self.hierarchy.contains_point(pos) {
                        self.hierarchy.on_asset_dropped(&asset_name);
                    }
                }
            } else {
                for file in dropped_files {
                    if let Some(path) = &file.path {
                        self.project.import_file_path(path, self.language);
                        let asset_name = path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("Imported");
                        let object_name = self.hierarchy.on_asset_dropped(asset_name);
                        self.viewport.on_asset_file_dropped_named(path, &object_name);
                    }
                }
            }
        }

        if let (Some(asset_name), Some(pos)) = (self.project.dragging_asset_name(), drop_pos) {
            let painter =
                ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("asset_drag_overlay")));
            let preview_rect = egui::Rect::from_min_size(pos + egui::vec2(14.0, 12.0), egui::vec2(170.0, 24.0));
            painter.rect_filled(
                preview_rect,
                4.0,
                egui::Color32::from_rgba_unmultiplied(26, 32, 34, 220),
            );
            painter.rect_stroke(
                preview_rect,
                4.0,
                egui::Stroke::new(1.0, egui::Color32::from_rgb(80, 120, 100)),
                egui::StrokeKind::Outside,
            );
            painter.text(
                preview_rect.left_center() + egui::vec2(8.0, 0.0),
                egui::Align2::LEFT_CENTER,
                asset_name,
                egui::FontId::proportional(12.0),
                egui::Color32::from_gray(230),
            );

            if self.viewport.contains_point(pos) {
                if let Some(rect) = self.viewport.panel_rect() {
                    painter.rect_stroke(
                        rect.shrink(2.0),
                        4.0,
                        egui::Stroke::new(2.0, egui::Color32::from_rgb(15, 232, 121)),
                        egui::StrokeKind::Outside,
                    );
                }
            } else if self.hierarchy.contains_point(pos) {
                if let Some(rect) = self.hierarchy.panel_rect() {
                    painter.rect_stroke(
                        rect.shrink(2.0),
                        6.0,
                        egui::Stroke::new(2.0, egui::Color32::from_rgb(15, 232, 121)),
                        egui::StrokeKind::Outside,
                    );
                }
            }
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

fn collect_deng_files(root: &Path, current: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > 6 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(current) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.starts_with('.') || name == "target" {
            continue;
        }
        if path.is_dir() {
            collect_deng_files(root, &path, depth + 1, out);
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("deng"))
            == Some(true)
        {
            let rel = path
                .strip_prefix(root)
                .map(PathBuf::from)
                .unwrap_or(path.clone());
            out.push(rel);
        }
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
        Box::new(|cc| {
            let mut app = EditorApp {
                inspector: InspectorWindow::new(),
                hierarchy: HierarchyWindow::new(),
                project: ProjectWindow::new(),
                viewport: ViewportPanel::new(),
                viewport_gpu: cc
                    .wgpu_render_state
                    .clone()
                    .map(ViewportGpuRenderer::new),
                app_icon_texture: None,
                cena_icon: None,
                game_icon: None,
                play_icon: None,
                pause_icon: None,
                stop_icon: None,
                files_icon: None,
                rig_icon: None,
                animador_icon: None,
                fios_icon: None,
                lang_pt_icon: None,
                lang_en_icon: None,
                lang_es_icon: None,
                is_playing: false,
                is_window_maximized: true,
                selected_mode: ToolbarMode::Cena,
                rig_enabled: false,
                animator_enabled: false,
                fios_enabled: false,
                language: EngineLanguage::Pt,
                project_collapsed: false,
                windows_blur_initialized: false,
                last_pointer_pos: None,
                show_hub: true,
                hub_projects: Vec::new(),
                hub_selected: None,
                current_project: None,
            };
            app.refresh_hub_projects();
            Ok(Box::new(app))
        }),
    )
}
