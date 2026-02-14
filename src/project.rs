use std::collections::{BTreeMap, HashSet, VecDeque};
use std::fs::{self, File};
use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;

use eframe::egui::{self, Align2, Color32, FontFamily, FontId, Id, Order, Rect, Sense, Stroke, TextureHandle, Vec2};
use epaint::ColorImage;

use crate::EngineLanguage;

pub struct ProjectWindow {
    pub open: bool,
    panel_height: f32,
    resizing_height: bool,
    selected_folder: &'static str,
    selected_asset: Option<String>,
    search_query: String,
    icon_scale: f32,
    deleted_assets: HashSet<String>,
    status_text: String,
    arrow_icon_texture: Option<TextureHandle>,
    assets_open: bool,
    packages_open: bool,
    hover_roll_asset: Option<String>,
    hover_still_since: f64,
    imported_assets: BTreeMap<&'static str, Vec<String>>,
    preview_cache: BTreeMap<String, TextureHandle>,
    preview_lru: VecDeque<String>,
    dragging_asset: Option<String>,
    image_preview_tx: Sender<ImagePreviewDecoded>,
    image_preview_rx: Receiver<ImagePreviewDecoded>,
    mesh_preview_tx: Sender<MeshPreviewDecoded>,
    mesh_preview_rx: Receiver<MeshPreviewDecoded>,
    image_preview_pending: HashSet<String>,
    mesh_preview_pending: HashSet<String>,
    image_preview_workers: Arc<AtomicUsize>,
    mesh_preview_workers: Arc<AtomicUsize>,
}

struct MeshPreview {
    lines: Vec<([f32; 2], [f32; 2])>,
}

struct ImagePreviewDecoded {
    key: String,
    image: Option<([usize; 2], Vec<u8>)>,
}

struct MeshPreviewDecoded {
    key: String,
    image: Option<([usize; 2], Vec<u8>)>,
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
    const MAX_IMAGE_PREVIEWS: usize = 128;
    const MAX_IMAGE_PREVIEW_WORKERS: usize = 4;
    const MAX_MESH_PREVIEW_WORKERS: usize = 2;
    const MESH_THUMB_SIZE: [usize; 2] = [176, 124];

    pub fn new() -> Self {
        let (img_tx, img_rx) = mpsc::channel();
        let (mesh_tx, mesh_rx) = mpsc::channel();
        let image_preview_workers = Arc::new(AtomicUsize::new(0));
        let mesh_preview_workers = Arc::new(AtomicUsize::new(0));
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
            imported_assets: BTreeMap::new(),
            preview_cache: BTreeMap::new(),
            preview_lru: VecDeque::new(),
            dragging_asset: None,
            image_preview_tx: img_tx,
            image_preview_rx: img_rx,
            mesh_preview_tx: mesh_tx,
            mesh_preview_rx: mesh_rx,
            image_preview_pending: HashSet::new(),
            mesh_preview_pending: HashSet::new(),
            image_preview_workers,
            mesh_preview_workers,
        }
    }

    fn lru_touch(queue: &mut VecDeque<String>, key: &str) {
        if let Some(idx) = queue.iter().position(|k| k == key) {
            queue.remove(idx);
        }
        queue.push_back(key.to_string());
    }

    fn evict_preview_cache_if_needed(&mut self) {
        while self.preview_cache.len() > Self::MAX_IMAGE_PREVIEWS {
            let Some(old_key) = self.preview_lru.pop_front() else {
                break;
            };
            self.preview_cache.remove(&old_key);
        }
    }

    fn poll_preview_jobs(&mut self, ctx: &egui::Context) {
        while let Ok(decoded) = self.image_preview_rx.try_recv() {
            if let Some((size, rgba)) = decoded.image {
                let color_image = ColorImage::from_rgba_unmultiplied(size, &rgba);
                let tex = ctx.load_texture(
                    decoded.key.clone(),
                    color_image,
                    egui::TextureOptions::LINEAR,
                );
                self.preview_cache.insert(decoded.key.clone(), tex);
                Self::lru_touch(&mut self.preview_lru, &decoded.key);
                self.evict_preview_cache_if_needed();
            }
            self.image_preview_pending.remove(&decoded.key);
        }
        while let Ok(decoded) = self.mesh_preview_rx.try_recv() {
            if let Some((size, rgba)) = decoded.image {
                let color_image = ColorImage::from_rgba_unmultiplied(size, &rgba);
                let tex = ctx.load_texture(
                    decoded.key.clone(),
                    color_image,
                    egui::TextureOptions::LINEAR,
                );
                self.preview_cache.insert(decoded.key.clone(), tex);
                Self::lru_touch(&mut self.preview_lru, &decoded.key);
                self.evict_preview_cache_if_needed();
            }
            self.mesh_preview_pending.remove(&decoded.key);
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
            (EngineLanguage::Pt, "import") => "Importar",
            (EngineLanguage::En, "import") => "Import",
            (EngineLanguage::Es, "import") => "Importar",
            (EngineLanguage::Pt, "save") => "Salvar",
            (EngineLanguage::En, "save") => "Save",
            (EngineLanguage::Es, "save") => "Guardar",
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

    fn assets_for_folder(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        if let Some(folder_path) = self.selected_folder_path() {
            if let Ok(entries) = fs::read_dir(folder_path) {
                for entry in entries.flatten() {
                    let Some(name) = entry.file_name().to_str().map(|s| s.to_string()) else {
                        continue;
                    };
                    if name.starts_with('.') {
                        continue;
                    }
                    out.push(name);
                }
            }
        }

        if let Some(extra) = self.imported_assets.get(self.selected_folder) {
            for name in extra {
                if !out.iter().any(|n| n == name) {
                    out.push(name.clone());
                }
            }
        }
        out.sort_by_key(|s| s.to_ascii_lowercase());
        out
    }

    fn should_show_folder(&self, folder: &'static str) -> bool {
        if let Some(path) = match folder {
            "Animations" => Some(PathBuf::from("Assets/Animations")),
            "Materials" => Some(PathBuf::from("Assets/Materials")),
            "Meshes" => Some(PathBuf::from("Assets/Meshes")),
            "Mold" => Some(PathBuf::from("Assets/Mold")),
            "Scripts" => Some(PathBuf::from("Assets/Scripts")),
            "TextMeshPro" => Some(PathBuf::from("Packages/TextMeshPro")),
            "InputSystem" => Some(PathBuf::from("Packages/InputSystem")),
            _ => None,
        } {
            if path.exists() {
                return true;
            }
        }
        self.imported_assets
            .get(folder)
            .is_some_and(|v| !v.is_empty())
    }

    fn import_target_folder_for_ext(ext: &str) -> &'static str {
        match ext {
            "fbx" | "obj" | "glb" | "gltf" => "Meshes",
            "cs" => "Scripts",
            // Sem restrição: qualquer formato não mapeado cai em Assets.
            _ => "Assets",
        }
    }

    fn unique_destination_path(dest_dir: &Path, base_name: &str) -> PathBuf {
        let candidate = dest_dir.join(base_name);
        if !candidate.exists() {
            return candidate;
        }

        let stem = Path::new(base_name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("asset");
        let ext = Path::new(base_name)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("");

        for idx in 1..10_000 {
            let file_name = if ext.is_empty() {
                format!("{stem}_{idx}")
            } else {
                format!("{stem}_{idx}.{ext}")
            };
            let path = dest_dir.join(&file_name);
            if !path.exists() {
                return path;
            }
        }

        dest_dir.join(base_name)
    }

    fn import_model_path(&mut self, src_path: &Path, language: EngineLanguage) {
        if !src_path.is_file() {
            self.status_text = format!("{}: selecione um arquivo válido", self.tr(language, "import"));
            return;
        }
        let Some(file_name) = src_path.file_name().and_then(|n| n.to_str()) else {
            self.status_text = format!("{}: caminho inválido", self.tr(language, "import"));
            return;
        };

        let ext = src_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();

        let target_folder = Self::import_target_folder_for_ext(&ext);

        let dest_dir = Path::new("Assets").join(target_folder);
        if let Err(err) = std::fs::create_dir_all(&dest_dir) {
            self.status_text = format!("{}: erro ao criar pasta ({err})", self.tr(language, "import"));
            return;
        }

        let dest_path = Self::unique_destination_path(&dest_dir, file_name);
        if let Err(err) = std::fs::copy(src_path, &dest_path) {
            self.status_text = format!("{}: erro ao copiar arquivo ({err})", self.tr(language, "import"));
            return;
        }

        let imported_name = dest_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(file_name)
            .to_string();
        let imported = self.imported_assets.entry(target_folder).or_default();
        if !imported.iter().any(|n| n == &imported_name) {
            imported.push(imported_name.clone());
        }
        if self.selected_folder == target_folder {
            self.selected_asset = Some(imported_name.clone());
        } else {
            self.selected_asset = None;
        }
        self.deleted_assets.remove(&imported_name);
        self.status_text = format!("{}: {}", self.tr(language, "import"), imported_name);
    }

    pub fn import_file_path(&mut self, src_path: &Path, language: EngineLanguage) {
        self.import_model_path(src_path, language);
    }

    pub fn import_asset_dialog(&mut self, language: EngineLanguage) {
        let file = rfd::FileDialog::new().pick_file();

        if let Some(path) = file {
            self.import_model_path(&path, language);
        }
    }

    pub fn save_project_dialog(&mut self, language: EngineLanguage) -> Option<PathBuf> {
        let picked = rfd::FileDialog::new()
            .add_filter("Dengine Project", &["deng"])
            .set_file_name("project.deng")
            .save_file();

        let Some(mut path) = picked else {
            return None;
        };
        if path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("deng"))
            != Some(true)
        {
            path.set_extension("deng");
        }

        match self.save_project_file(&path) {
            Ok(()) => {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("project.deng");
                self.status_text = format!("{}: {}", self.tr(language, "save"), name);
                Some(path)
            }
            Err(err) => {
                self.status_text = format!("{}: erro ao salvar ({err})", self.tr(language, "save"));
                None
            }
        }
    }

    pub fn save_project_to_path(&mut self, path: &Path, language: EngineLanguage) -> bool {
        match self.save_project_file(path) {
            Ok(()) => {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("project.deng");
                self.status_text = format!("{}: {}", self.tr(language, "save"), name);
                true
            }
            Err(err) => {
                self.status_text = format!("{}: erro ao salvar ({err})", self.tr(language, "save"));
                false
            }
        }
    }

    fn save_project_file(&self, path: &Path) -> Result<(), String> {
        let mut files = Vec::<String>::new();
        collect_project_files_recursive(Path::new("Assets"), Path::new("Assets"), &mut files)?;
        files.sort_by_key(|s| s.to_ascii_lowercase());

        let mut f = File::create(path).map_err(|e| e.to_string())?;
        f.write_all(b"DENG1\n").map_err(|e| e.to_string())?;
        for rel in files {
            let line = format!("asset={rel}\n");
            f.write_all(line.as_bytes()).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn icon_style(asset: &str) -> (Color32, &'static str) {
        if asset.ends_with(".mold") {
            (Color32::from_rgb(56, 95, 166), "PF")
        } else if asset.ends_with(".cs") {
            (Color32::from_rgb(184, 104, 51), "C#")
        } else if asset.ends_with(".png")
            || asset.ends_with(".jpg")
            || asset.ends_with(".jpeg")
            || asset.ends_with(".webp")
        {
            (Color32::from_rgb(64, 146, 112), "IMG")
        } else if asset.ends_with(".wav")
            || asset.ends_with(".mp3")
            || asset.ends_with(".ogg")
            || asset.ends_with(".flac")
        {
            (Color32::from_rgb(132, 96, 178), "SND")
        } else if asset.ends_with(".anim") || asset.ends_with(".controller") {
            (Color32::from_rgb(154, 72, 167), "AN")
        } else if asset.ends_with(".mat") {
            (Color32::from_rgb(179, 137, 57), "MAT")
        } else if asset.ends_with(".fbx") || asset.ends_with(".obj") || asset.ends_with(".glb") || asset.ends_with(".gltf") {
            (Color32::from_rgb(86, 132, 176), "MESH")
        } else if asset.ends_with(".json") {
            (Color32::from_rgb(127, 127, 127), "{}")
        } else {
            (Color32::from_rgb(88, 88, 88), "AS")
        }
    }

    fn selected_folder_path(&self) -> Option<PathBuf> {
        match self.selected_folder {
            "Assets" => Some(PathBuf::from("Assets")),
            "Animations" => Some(PathBuf::from("Assets/Animations")),
            "Materials" => Some(PathBuf::from("Assets/Materials")),
            "Meshes" => Some(PathBuf::from("Assets/Meshes")),
            "Mold" => Some(PathBuf::from("Assets/Mold")),
            "Scripts" => Some(PathBuf::from("Assets/Scripts")),
            "Packages" => Some(PathBuf::from("Packages")),
            "TextMeshPro" => Some(PathBuf::from("Packages/TextMeshPro")),
            "InputSystem" => Some(PathBuf::from("Packages/InputSystem")),
            _ => None,
        }
    }

    fn asset_path_in_selected_folder(&self, asset_name: &str) -> Option<PathBuf> {
        self.selected_folder_path().map(|p| p.join(asset_name))
    }

    fn asset_preview_texture<'a>(
        &'a mut self,
        _ctx: &egui::Context,
        asset_name: &str,
    ) -> Option<&'a TextureHandle> {
        let asset_path = self.asset_path_in_selected_folder(asset_name)?;
        let ext = asset_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .unwrap_or_default();
        let is_image = ext == "png" || ext == "jpg" || ext == "jpeg" || ext == "webp";
        let is_mesh = ext == "obj" || ext == "glb" || ext == "gltf" || ext == "fbx";
        if !is_image && !is_mesh {
            return None;
        }
        if !asset_path.exists() {
            return None;
        }

        let key = asset_path.to_string_lossy().to_string();
        if !self.preview_cache.contains_key(&key) {
            if is_image && !self.image_preview_pending.contains(&key) {
                let active = self.image_preview_workers.load(Ordering::Relaxed);
                if active >= Self::MAX_IMAGE_PREVIEW_WORKERS {
                    return None;
                }
                self.image_preview_pending.insert(key.clone());
                let tx = self.image_preview_tx.clone();
                let key_clone = key.clone();
                let workers = Arc::clone(&self.image_preview_workers);
                workers.fetch_add(1, Ordering::Relaxed);
                std::thread::spawn(move || {
                    let decoded = match std::fs::read(&asset_path)
                        .ok()
                        .and_then(|bytes| image::load_from_memory(&bytes).ok())
                        .map(|img| img.to_rgba8())
                    {
                        Some(rgba) => ImagePreviewDecoded {
                            key: key_clone,
                            image: Some(([rgba.width() as usize, rgba.height() as usize], rgba.into_raw())),
                        },
                        None => ImagePreviewDecoded {
                            key: key_clone,
                            image: None,
                        },
                    };
                    let _ = tx.send(decoded);
                    workers.fetch_sub(1, Ordering::Relaxed);
                });
            } else if is_mesh && !self.mesh_preview_pending.contains(&key) {
                let active = self.mesh_preview_workers.load(Ordering::Relaxed);
                if active >= Self::MAX_MESH_PREVIEW_WORKERS {
                    return None;
                }
                self.mesh_preview_pending.insert(key.clone());
                let tx = self.mesh_preview_tx.clone();
                let key_clone = key.clone();
                let workers = Arc::clone(&self.mesh_preview_workers);
                workers.fetch_add(1, Ordering::Relaxed);
                std::thread::spawn(move || {
                    let image = build_mesh_preview(&asset_path)
                        .ok()
                        .map(|preview| {
                            let size = Self::MESH_THUMB_SIZE;
                            let rgba = rasterize_mesh_preview(&preview, size);
                            (size, rgba)
                        });
                    let _ = tx.send(MeshPreviewDecoded {
                        key: key_clone,
                        image,
                    });
                    workers.fetch_sub(1, Ordering::Relaxed);
                });
            }
        }
        if self.preview_cache.contains_key(&key) {
            Self::lru_touch(&mut self.preview_lru, &key);
            self.evict_preview_cache_if_needed();
        }
        self.preview_cache.get(&key)
    }

    pub fn dragging_asset_name(&self) -> Option<&str> {
        self.dragging_asset.as_deref()
    }

    pub fn dragging_asset_path(&self) -> Option<PathBuf> {
        let name = self.dragging_asset_name()?;
        self.asset_path_in_selected_folder(name)
    }

    pub fn clear_dragging_asset(&mut self) {
        self.dragging_asset = None;
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
        bottom_offset: f32,
    ) -> bool {
        if !self.open {
            return false;
        }

        if self.arrow_icon_texture.is_none() {
            self.arrow_icon_texture = load_png_as_texture(ctx, "src/assets/icons/seta.png");
        }
        self.poll_preview_jobs(ctx);

        let dock_rect_full = ctx.available_rect();
        let dock_rect = Rect::from_min_max(
            dock_rect_full.min,
            egui::pos2(dock_rect_full.max.x, dock_rect_full.max.y - bottom_offset.max(0.0)),
        );
        let pointer_down = ctx.input(|i| i.pointer.primary_down());

        let min_h = 185.0;
        let max_h = (dock_rect.height() - 120.0).max(min_h);
        self.panel_height = self.panel_height.clamp(min_h, max_h);

        let panel_rect = Rect::from_min_max(
            egui::pos2(dock_rect.left(), dock_rect.bottom() - self.panel_height),
            egui::pos2(dock_rect.right(), dock_rect.bottom()),
        );

        let mut request_collapse = false;
        let mut request_import = false;
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
                let breadcrumb = self.breadcrumb_segments(language);
                let collapse_btn_rect = Rect::from_center_size(
                    egui::pos2(header_rect.right() - 10.0, header_rect.center().y),
                    egui::vec2(16.0, 16.0),
                );
                let collapse_resp = ui.interact(
                    collapse_btn_rect,
                    ui.id().with("project_minimize"),
                    Sense::click(),
                );
                if collapse_resp.hovered() {
                    ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                    ui.painter().rect_filled(
                        collapse_btn_rect.expand(2.0),
                        3.0,
                        Color32::from_rgb(58, 58, 58),
                    );
                }
                if collapse_resp.clicked() {
                    request_collapse = true;
                }
                if let Some(arrow_tex) = &self.arrow_icon_texture {
                    let _ = ui.put(
                        collapse_btn_rect,
                        egui::Image::new(arrow_tex)
                            .fit_to_exact_size(egui::vec2(10.0, 10.0))
                            .rotate(std::f32::consts::FRAC_PI_2, Vec2::splat(0.5)),
                    );
                } else {
                    ui.painter().text(
                        collapse_btn_rect.center(),
                        Align2::CENTER_CENTER,
                        "▾",
                        FontId::new(11.0, FontFamily::Proportional),
                        Color32::from_gray(190),
                    );
                }

                let left_header_rect = Rect::from_min_max(
                    header_rect.min,
                    egui::pos2(collapse_btn_rect.left() - 8.0, header_rect.bottom()),
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

                let splitter_y = header_rect.bottom() + 4.0;
                    let search_hint = self.tr(language, "search");
                    let desired_search_w: f32 = 220.0;
                    let min_search_w: f32 = 80.0;
                    let import_w = 88.0;
                    let search_right = collapse_btn_rect.left() - 10.0 - import_w;
                    let search_min_x = header_rect.left() + 6.0;
                    let search_space = (search_right - search_min_x).max(0.0);
                    let search_w = desired_search_w.min(search_space).max(min_search_w.min(search_space));
                    let search_x = if search_w <= 0.0 {
                        search_min_x
                    } else {
                        let search_max_x = (search_right - search_w).max(search_min_x);
                        (header_rect.center().x - search_w * 0.5 - 36.0).clamp(search_min_x, search_max_x)
                    };
                    let search_rect = Rect::from_min_max(
                        egui::pos2(search_x, header_rect.top()),
                        egui::pos2(search_x + search_w, header_rect.bottom()),
                    );
                    let import_left = (collapse_btn_rect.left() - 8.0 - import_w).max(search_rect.right() + 6.0);
                    let import_right = (collapse_btn_rect.left() - 8.0).max(import_left);
                    let import_rect = Rect::from_min_max(
                        egui::pos2(import_left, header_rect.top()),
                        egui::pos2(import_right, header_rect.bottom()),
                    );

                    if search_w > 0.0 {
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
                    }
                    ui.scope_builder(
                        egui::UiBuilder::new()
                            .max_rect(import_rect)
                            .layout(
                                egui::Layout::left_to_right(egui::Align::Center)
                                    .with_main_align(egui::Align::Center),
                            ),
                        |ui| {
                            if ui
                                .add_sized(
                                    [import_w, 22.0],
                                    egui::Button::new(self.tr(language, "import"))
                                        .corner_radius(6)
                                        .stroke(Stroke::new(1.0, Color32::from_rgb(15, 232, 121))),
                                )
                                .clicked()
                            {
                                request_import = true;
                            }
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
                                        if !self.should_show_folder(folder) {
                                            continue;
                                        }
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
                                        if !self.should_show_folder(folder) {
                                            continue;
                                        }
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
                    let filtered_assets: Vec<&String> = assets
                        .iter()
                        .filter(|asset| {
                            !self.deleted_assets.contains(*asset)
                                && (filter.is_empty() || asset.to_lowercase().contains(&filter))
                        })
                        .collect();

                    ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(grid_rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                    |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("project_grid")
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                let spacing = egui::vec2(8.0, 8.0);
                                ui.spacing_mut().item_spacing = spacing;
                                let tile_w = self.icon_scale.clamp(56.0, 98.0);
                                let tile_name_h = 18.0;
                                let tile_pad = 6.0;
                                let tile_size = Vec2::new(tile_w, tile_w + tile_name_h + tile_pad * 2.0);
                                let cols = (((ui.available_width() + spacing.x) / (tile_size.x + spacing.x))
                                    .floor() as usize)
                                    .max(1);
                                let now = ui.ctx().input(|i| i.time);
                                let mut hovered_any = false;

                                egui::Grid::new("project_asset_grid_fixed")
                                    .num_columns(cols)
                                    .spacing(spacing)
                                    .show(ui, |ui| {
                                        for (idx, asset) in filtered_assets.iter().enumerate() {
                                        let asset = *asset;
                                        let tile_size = Vec2::new(tile_w, tile_w + tile_name_h + tile_pad * 2.0);
                                        let selected = self.selected_asset.as_ref() == Some(asset);
                                        let (tile_rect, tile_resp) =
                                            ui.allocate_exact_size(tile_size, Sense::click_and_drag());

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

                                        let preview_rect = Rect::from_min_max(
                                            tile_rect.min + egui::vec2(tile_pad, tile_pad),
                                            egui::pos2(
                                                tile_rect.max.x - tile_pad,
                                                tile_rect.max.y - tile_name_h - tile_pad,
                                            ),
                                        );
                                        ui.painter().rect_filled(
                                            preview_rect,
                                            3.0,
                                            Color32::from_rgb(38, 40, 42),
                                        );
                                        if let Some(tex) = self.asset_preview_texture(ui.ctx(), asset) {
                                            let image_rect = preview_rect.shrink(1.0);
                                            let _ = ui.put(
                                                image_rect,
                                                egui::Image::new(tex).fit_to_exact_size(image_rect.size()),
                                            );
                                            ui.painter().rect_stroke(
                                                preview_rect,
                                                2.0,
                                                Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 26)),
                                                egui::StrokeKind::Outside,
                                            );
                                        } else {
                                            let (icon_color, icon_tag) = Self::icon_style(asset);
                                            ui.painter().rect_filled(preview_rect.shrink(1.0), 2.0, icon_color);
                                            ui.painter().text(
                                                preview_rect.center(),
                                                egui::Align2::CENTER_CENTER,
                                                icon_tag,
                                                FontId::proportional(10.0),
                                                Color32::from_gray(245),
                                            );
                                        }
                                        let name_font = FontId::proportional(11.0);
                                        let name_color = Color32::from_gray(210);
                                        let name_rect = Rect::from_min_max(
                                            egui::pos2(
                                                tile_rect.left() + tile_pad,
                                                tile_rect.bottom() - tile_name_h - 2.0,
                                            ),
                                            egui::pos2(tile_rect.right() - tile_pad, tile_rect.bottom() - 2.0),
                                        );
                                        let clipped_painter = ui.painter().with_clip_rect(name_rect);
                                        let full_w = ui
                                            .painter()
                                            .layout_no_wrap(asset.to_string(), name_font.clone(), name_color)
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
                                            if self.hover_roll_asset.as_ref() != Some(asset) {
                                                self.hover_roll_asset = Some(asset.clone());
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
                                            self.selected_asset = Some(asset.clone());
                                            self.status_text =
                                                format!("{}: {}", self.tr(language, "open"), asset);
                                        }
                                        if reveal_clicked {
                                            self.status_text =
                                                format!("{}: {}", self.tr(language, "reveal"), asset);
                                        }
                                        if delete_clicked {
                                            self.deleted_assets.insert(asset.clone());
                                            if self.selected_asset.as_ref() == Some(asset) {
                                                self.selected_asset = None;
                                            }
                                            self.status_text =
                                                format!("{}: {}", self.tr(language, "delete"), asset);
                                        }

                                        if tile_resp.clicked() {
                                            self.selected_asset = Some(asset.clone());
                                            self.status_text = asset.to_string();
                                        }
                                        if tile_resp.drag_started() || tile_resp.dragged() {
                                            self.dragging_asset = Some(asset.clone());
                                        }
                                        if tile_resp.hovered()
                                            && ui.input(|i| i.pointer.primary_down() && i.pointer.delta().length_sq() > 0.0)
                                        {
                                            self.dragging_asset = Some(asset.clone());
                                        }
                                            if (idx + 1) % cols == 0 {
                                                ui.end_row();
                                            }
                                    }
                                    });

                                    if !hovered_any {
                                        self.hover_roll_asset = None;
                                    }
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
                        let count = filtered_assets.len();
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
        if request_import {
            self.import_asset_dialog(language);
        }

        request_collapse
    }

    pub fn docked_bottom_height(&self) -> f32 {
        if self.open {
            self.panel_height
        } else {
            0.0
        }
    }
}

fn build_mesh_preview(path: &Path) -> Result<MeshPreview, String> {
    let (mut vertices, triangles) = load_preview_mesh_cached(path)?;
    normalize_preview_vertices(&mut vertices);

    // Isometric-ish thumbnail projection.
    let yaw = 0.65_f32;
    let pitch = 0.52_f32;
    let (sy, cy) = yaw.sin_cos();
    let (sp, cp) = pitch.sin_cos();
    let mut p2: Vec<[f32; 2]> = Vec::with_capacity(vertices.len());
    for v in &vertices {
        let x1 = v.x * cy - v.z * sy;
        let z1 = v.x * sy + v.z * cy;
        let y2 = v.y * cp - z1 * sp;
        p2.push([x1, y2]);
    }

    let mut edges = HashSet::<(u32, u32)>::new();
    let mut lines: Vec<([f32; 2], [f32; 2])> = Vec::new();
    for tri in &triangles {
        for (a, b) in [(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])] {
            let key = if a < b { (a, b) } else { (b, a) };
            if !edges.insert(key) {
                continue;
            }
            let ai = a as usize;
            let bi = b as usize;
            if ai < p2.len() && bi < p2.len() {
                lines.push((p2[ai], p2[bi]));
            }
        }
    }
    if lines.is_empty() {
        return Err("sem arestas para preview".to_string());
    }
    Ok(MeshPreview { lines })
}

fn rasterize_mesh_preview(preview: &MeshPreview, size: [usize; 2]) -> Vec<u8> {
    let w = size[0].max(1);
    let h = size[1].max(1);
    let mut rgba = vec![0_u8; w * h * 4];

    for px in rgba.chunks_exact_mut(4) {
        px[0] = 33;
        px[1] = 39;
        px[2] = 46;
        px[3] = 255;
    }

    let cx = w as f32 * 0.5;
    let cy = h as f32 * 0.5;
    let sx = w as f32 * 0.42;
    let sy = h as f32 * 0.42;

    for (a, b) in &preview.lines {
        let x0 = (cx + a[0] * sx).round() as i32;
        let y0 = (cy - a[1] * sy).round() as i32;
        let x1 = (cx + b[0] * sx).round() as i32;
        let y1 = (cy - b[1] * sy).round() as i32;
        draw_line_rgba(
            &mut rgba,
            w,
            h,
            x0,
            y0,
            x1,
            y1,
            [145, 198, 236, 255],
        );
    }

    // Outer border to improve readability on dark tiles.
    for x in 0..w {
        put_pixel_rgba(&mut rgba, w, h, x as i32, 0, [82, 112, 136, 255]);
        put_pixel_rgba(&mut rgba, w, h, x as i32, h as i32 - 1, [82, 112, 136, 255]);
    }
    for y in 0..h {
        put_pixel_rgba(&mut rgba, w, h, 0, y as i32, [82, 112, 136, 255]);
        put_pixel_rgba(&mut rgba, w, h, w as i32 - 1, y as i32, [82, 112, 136, 255]);
    }

    rgba
}

fn put_pixel_rgba(
    rgba: &mut [u8],
    w: usize,
    h: usize,
    x: i32,
    y: i32,
    color: [u8; 4],
) {
    if x < 0 || y < 0 {
        return;
    }
    let xu = x as usize;
    let yu = y as usize;
    if xu >= w || yu >= h {
        return;
    }
    let idx = (yu * w + xu) * 4;
    rgba[idx] = color[0];
    rgba[idx + 1] = color[1];
    rgba[idx + 2] = color[2];
    rgba[idx + 3] = color[3];
}

fn draw_line_rgba(
    rgba: &mut [u8],
    w: usize,
    h: usize,
    mut x0: i32,
    mut y0: i32,
    x1: i32,
    y1: i32,
    color: [u8; 4],
) {
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        put_pixel_rgba(rgba, w, h, x0, y0, color);
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = err * 2;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

fn collect_project_files_recursive(
    root: &Path,
    current: &Path,
    out: &mut Vec<String>,
) -> Result<(), String> {
    if !current.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(current).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            collect_project_files_recursive(root, &path, out)?;
        } else if path.is_file() {
            let rel = path
                .strip_prefix(root)
                .ok()
                .and_then(|p| p.to_str())
                .unwrap_or(name)
                .replace('\\', "/");
            out.push(format!("Assets/{rel}"));
        }
    }
    Ok(())
}

fn load_preview_mesh_cached(path: &Path) -> Result<(Vec<glam::Vec3>, Vec<[u32; 3]>), String> {
    let stamp = source_stamp_preview(path).unwrap_or((0, 0));
    if let Some(mesh) = read_dmesh_cache_preview(path, stamp).ok().flatten() {
        return Ok(mesh);
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .ok_or_else(|| "extensão inválida".to_string())?;

    let mesh = match ext.as_str() {
        "obj" => load_obj_preview_mesh(path)?,
        "glb" | "gltf" => load_gltf_preview_mesh(path)?,
        "fbx" => load_fbx_ascii_preview_mesh(path)?,
        _ => return Err("formato não suportado".to_string()),
    };
    let _ = write_dmesh_cache_preview(path, &mesh, stamp);
    Ok(mesh)
}

fn source_stamp_preview(path: &Path) -> Result<(u64, u64), String> {
    let meta = fs::metadata(path).map_err(|e| e.to_string())?;
    let len = meta.len();
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Ok((len, mtime))
}

fn cache_file_path_preview(source: &Path) -> Result<PathBuf, String> {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    source.to_string_lossy().hash(&mut hasher);
    let key = hasher.finish();
    let cache_dir = Path::new("Assets").join(".cache").join("meshes");
    fs::create_dir_all(&cache_dir).map_err(|e| e.to_string())?;
    Ok(cache_dir.join(format!("{key:016x}.dmesh")))
}

fn write_dmesh_cache_preview(
    source: &Path,
    mesh: &(Vec<glam::Vec3>, Vec<[u32; 3]>),
    stamp: (u64, u64),
) -> Result<(), String> {
    let cache = cache_file_path_preview(source)?;
    let mut f = File::create(cache).map_err(|e| e.to_string())?;
    f.write_all(b"DMSH1").map_err(|e| e.to_string())?;
    f.write_all(&stamp.0.to_le_bytes()).map_err(|e| e.to_string())?;
    f.write_all(&stamp.1.to_le_bytes()).map_err(|e| e.to_string())?;
    let vcount = mesh.0.len() as u32;
    let tcount = mesh.1.len() as u32;
    f.write_all(&vcount.to_le_bytes()).map_err(|e| e.to_string())?;
    f.write_all(&tcount.to_le_bytes()).map_err(|e| e.to_string())?;
    for v in &mesh.0 {
        f.write_all(&v.x.to_le_bytes()).map_err(|e| e.to_string())?;
        f.write_all(&v.y.to_le_bytes()).map_err(|e| e.to_string())?;
        f.write_all(&v.z.to_le_bytes()).map_err(|e| e.to_string())?;
    }
    for tri in &mesh.1 {
        f.write_all(&tri[0].to_le_bytes()).map_err(|e| e.to_string())?;
        f.write_all(&tri[1].to_le_bytes()).map_err(|e| e.to_string())?;
        f.write_all(&tri[2].to_le_bytes()).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn read_dmesh_cache_preview(
    source: &Path,
    stamp: (u64, u64),
) -> Result<Option<(Vec<glam::Vec3>, Vec<[u32; 3]>)>, String> {
    let cache = cache_file_path_preview(source)?;
    if !cache.exists() {
        return Ok(None);
    }
    let mut f = File::open(cache).map_err(|e| e.to_string())?;
    let mut magic = [0_u8; 5];
    f.read_exact(&mut magic).map_err(|e| e.to_string())?;
    if &magic != b"DMSH1" {
        return Ok(None);
    }

    let mut buf8 = [0_u8; 8];
    f.read_exact(&mut buf8).map_err(|e| e.to_string())?;
    let src_len = u64::from_le_bytes(buf8);
    f.read_exact(&mut buf8).map_err(|e| e.to_string())?;
    let src_mtime = u64::from_le_bytes(buf8);
    if src_len != stamp.0 || src_mtime != stamp.1 {
        return Ok(None);
    }

    let mut buf4 = [0_u8; 4];
    f.read_exact(&mut buf4).map_err(|e| e.to_string())?;
    let vcount = u32::from_le_bytes(buf4) as usize;
    f.read_exact(&mut buf4).map_err(|e| e.to_string())?;
    let tcount = u32::from_le_bytes(buf4) as usize;

    let mut vertices = Vec::with_capacity(vcount);
    for _ in 0..vcount {
        let mut fb = [0_u8; 4];
        f.read_exact(&mut fb).map_err(|e| e.to_string())?;
        let x = f32::from_le_bytes(fb);
        f.read_exact(&mut fb).map_err(|e| e.to_string())?;
        let y = f32::from_le_bytes(fb);
        f.read_exact(&mut fb).map_err(|e| e.to_string())?;
        let z = f32::from_le_bytes(fb);
        vertices.push(glam::Vec3::new(x, y, z));
    }
    let mut triangles = Vec::with_capacity(tcount);
    for _ in 0..tcount {
        f.read_exact(&mut buf4).map_err(|e| e.to_string())?;
        let a = u32::from_le_bytes(buf4);
        f.read_exact(&mut buf4).map_err(|e| e.to_string())?;
        let b = u32::from_le_bytes(buf4);
        f.read_exact(&mut buf4).map_err(|e| e.to_string())?;
        let c = u32::from_le_bytes(buf4);
        triangles.push([a, b, c]);
    }
    Ok(Some((vertices, triangles)))
}

fn load_obj_preview_mesh(path: &Path) -> Result<(Vec<glam::Vec3>, Vec<[u32; 3]>), String> {
    let opt = tobj::LoadOptions {
        triangulate: true,
        single_index: true,
        ..Default::default()
    };
    let (models, _mats) = tobj::load_obj(path, &opt).map_err(|e| e.to_string())?;
    let mut vertices = Vec::new();
    let mut triangles = Vec::new();
    for m in models {
        let base = vertices.len() as u32;
        let mesh = m.mesh;
        for p in mesh.positions.chunks_exact(3) {
            vertices.push(glam::Vec3::new(p[0], p[1], p[2]));
        }
        for idx in mesh.indices.chunks_exact(3) {
            triangles.push([base + idx[0], base + idx[1], base + idx[2]]);
        }
    }
    if vertices.is_empty() || triangles.is_empty() {
        return Err("OBJ vazio".to_string());
    }
    Ok((vertices, triangles))
}

fn load_gltf_preview_mesh(path: &Path) -> Result<(Vec<glam::Vec3>, Vec<[u32; 3]>), String> {
    let gltf = gltf::Gltf::open(path).map_err(|e| e.to_string())?;
    let buffers = load_gltf_buffers_mesh_only_preview(path, &gltf)?;
    let mut vertices = Vec::new();
    let mut triangles = Vec::new();
    if let Some(scene) = gltf
        .document
        .default_scene()
        .or_else(|| gltf.document.scenes().next())
    {
        for node in scene.nodes() {
            append_gltf_preview_node_meshes(
                node,
                glam::Mat4::IDENTITY,
                &buffers,
                &mut vertices,
                &mut triangles,
            );
        }
    } else {
        for node in gltf.document.nodes() {
            append_gltf_preview_node_meshes(
                node,
                glam::Mat4::IDENTITY,
                &buffers,
                &mut vertices,
                &mut triangles,
            );
        }
    }
    if vertices.is_empty() || triangles.is_empty() {
        return Err("GLTF/GLB vazio".to_string());
    }
    Ok((vertices, triangles))
}

fn append_gltf_preview_node_meshes(
    node: gltf::Node<'_>,
    parent: glam::Mat4,
    buffers: &[Vec<u8>],
    vertices: &mut Vec<glam::Vec3>,
    triangles: &mut Vec<[u32; 3]>,
) {
    let local = glam::Mat4::from_cols_array_2d(&node.transform().matrix());
    let world = parent * local;

    if let Some(mesh) = node.mesh() {
        for primitive in mesh.primitives() {
            if primitive.mode() != gltf::mesh::Mode::Triangles {
                continue;
            }
            let reader = primitive.reader(|buf| buffers.get(buf.index()).map(|b| b.as_slice()));
            let Some(positions) = reader.read_positions() else {
                continue;
            };
            let base = vertices.len() as u32;
            let local: Vec<glam::Vec3> = positions
                .map(|p| world.transform_point3(glam::Vec3::new(p[0], p[1], p[2])))
                .collect();
            let vcount = local.len() as u32;
            vertices.extend(local);

            if let Some(indices) = reader.read_indices() {
                let idx_u32: Vec<u32> = indices.into_u32().collect();
                for tri in idx_u32.chunks_exact(3) {
                    triangles.push([base + tri[0], base + tri[1], base + tri[2]]);
                }
            } else {
                let mut i = 0;
                while i + 2 < vcount {
                    triangles.push([base + i, base + i + 1, base + i + 2]);
                    i += 3;
                }
            }
        }
    }

    for child in node.children() {
        append_gltf_preview_node_meshes(child, world, buffers, vertices, triangles);
    }
}

fn load_gltf_buffers_mesh_only_preview(path: &Path, gltf: &gltf::Gltf) -> Result<Vec<Vec<u8>>, String> {
    let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
    let mut out = Vec::new();
    for buf in gltf.document.buffers() {
        match buf.source() {
            gltf::buffer::Source::Bin => {
                let blob = gltf
                    .blob
                    .as_ref()
                    .ok_or_else(|| "GLB sem bloco binário".to_string())?;
                out.push(blob.clone());
            }
            gltf::buffer::Source::Uri(uri) => {
                if uri.starts_with("data:") {
                    return Err("GLTF com data-uri não suportado no preview".to_string());
                }
                let p = base_dir.join(uri);
                let bytes = std::fs::read(&p)
                    .map_err(|e| format!("falha ao ler buffer GLTF '{}': {e}", p.display()))?;
                out.push(bytes);
            }
        }
    }
    Ok(out)
}

fn load_fbx_ascii_preview_mesh(path: &Path) -> Result<(Vec<glam::Vec3>, Vec<[u32; 3]>), String> {
    use fbxcel_dom::any::AnyDocument;
    use fbxcel_dom::v7400::object::{TypedObjectHandle, geometry::TypedGeometryHandle};
    use std::io::BufReader;

    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let reader = BufReader::new(file);
    let doc = match AnyDocument::from_seekable_reader(reader).map_err(|e| e.to_string())? {
        AnyDocument::V7400(_, doc) => doc,
        _ => return Err("versão FBX não suportada".to_string()),
    };

    let mut vertices = Vec::<glam::Vec3>::new();
    let mut triangles = Vec::<[u32; 3]>::new();
    for obj in doc.objects() {
        let TypedObjectHandle::Geometry(TypedGeometryHandle::Mesh(mesh)) = obj.get_typed() else {
            continue;
        };
        let poly_verts = mesh.polygon_vertices().map_err(|e| e.to_string())?;
        let cps: Vec<_> = poly_verts
            .raw_control_points()
            .map_err(|e| e.to_string())?
            .collect();
        if cps.is_empty() {
            continue;
        }
        let base = vertices.len() as u32;
        vertices.extend(cps.iter().map(|p| glam::Vec3::new(p.x as f32, p.y as f32, p.z as f32)));

        let mut poly: Vec<u32> = Vec::new();
        for raw in poly_verts.raw_polygon_vertices() {
            let is_end = *raw < 0;
            let local_idx = if is_end { (-raw - 1) as u32 } else { *raw as u32 };
            if (local_idx as usize) < cps.len() {
                poly.push(base + local_idx);
            }
            if is_end {
                if poly.len() >= 3 {
                    for i in 1..(poly.len() - 1) {
                        triangles.push([poly[0], poly[i], poly[i + 1]]);
                    }
                }
                poly.clear();
            }
        }
    }
    if vertices.is_empty() || triangles.is_empty() {
        return Err("FBX sem malha suportada".to_string());
    }
    Ok((vertices, triangles))
}

fn normalize_preview_vertices(vertices: &mut [glam::Vec3]) {
    let mut min = glam::Vec3::splat(f32::INFINITY);
    let mut max = glam::Vec3::splat(f32::NEG_INFINITY);
    for v in vertices.iter().copied() {
        min = min.min(v);
        max = max.max(v);
    }
    let center = (min + max) * 0.5;
    let ext = (max - min).max(glam::Vec3::splat(1e-5));
    let s = 1.7 / ext.x.max(ext.y).max(ext.z);
    for v in vertices {
        *v = (*v - center) * s;
    }
}
