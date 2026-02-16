use std::collections::{BTreeMap, HashSet, VecDeque};
use std::fs::{self, File};
use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;

use eframe::egui::{
    self, Align2, Color32, FontFamily, FontId, Id, Order, Rect, Sense, Stroke, TextureHandle, Vec2,
};
use epaint::ColorImage;

use crate::EngineLanguage;

pub struct ProjectWindow {
    pub open: bool,
    panel_height: f32,
    resizing_height: bool,
    selected_folder: &'static str,
    selected_asset: Option<String>,
    selected_sub_asset: Option<String>,
    search_query: String,
    icon_scale: f32,
    deleted_assets: HashSet<String>,
    status_text: String,
    arrow_icon_texture: Option<TextureHandle>,
    rig_icon_texture: Option<TextureHandle>,
    animador_icon_texture: Option<TextureHandle>,
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
    fbx_meta_cache: BTreeMap<String, FbxMetaCacheEntry>,
    fbx_expanded_assets: HashSet<String>,
}

struct MeshPreview {
    lines: Vec<([f32; 2], [f32; 2])>,
}

#[derive(Clone, Default)]
struct FbxAssetMeta {
    has_mesh: bool,
    has_skeleton: bool,
    animations: Vec<String>,
}

#[derive(Clone)]
struct FbxMetaCacheEntry {
    stamp: (u64, u64),
    meta: FbxAssetMeta,
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
            selected_sub_asset: None,
            search_query: String::new(),
            icon_scale: 72.0,
            deleted_assets: HashSet::new(),
            status_text: String::new(),
            arrow_icon_texture: None,
            rig_icon_texture: None,
            animador_icon_texture: None,
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
            fbx_meta_cache: BTreeMap::new(),
            fbx_expanded_assets: HashSet::new(),
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
            (EngineLanguage::Pt, "create") => "Criar",
            (EngineLanguage::En, "create") => "Create",
            (EngineLanguage::Es, "create") => "Crear",
            (EngineLanguage::Pt, "create_script") => "Script C#",
            (EngineLanguage::En, "create_script") => "C# Script",
            (EngineLanguage::Es, "create_script") => "Script C#",
            (EngineLanguage::Pt, "create_material") => "Material",
            (EngineLanguage::En, "create_material") => "Material",
            (EngineLanguage::Es, "create_material") => "Material",
            (EngineLanguage::Pt, "create_folder") => "Pasta",
            (EngineLanguage::En, "create_folder") => "Folder",
            (EngineLanguage::Es, "create_folder") => "Carpeta",
            (EngineLanguage::Pt, "created") => "Criado",
            (EngineLanguage::En, "created") => "Created",
            (EngineLanguage::Es, "created") => "Creado",
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
        self.assets_for_folder_id(self.selected_folder)
    }

    fn assets_for_folder_id(&self, folder: &'static str) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        if let Some(folder_path) = Self::folder_path_from_id(folder) {
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

        if let Some(extra) = self.imported_assets.get(folder) {
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
            self.status_text = format!(
                "{}: selecione um arquivo válido",
                self.tr(language, "import")
            );
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
            self.status_text = format!(
                "{}: erro ao criar pasta ({err})",
                self.tr(language, "import")
            );
            return;
        }

        let dest_path = Self::unique_destination_path(&dest_dir, file_name);
        if let Err(err) = std::fs::copy(src_path, &dest_path) {
            self.status_text = format!(
                "{}: erro ao copiar arquivo ({err})",
                self.tr(language, "import")
            );
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
        if ext == "fbx" {
            match self.upsert_default_animation_module_for_fbx(&imported_name, &dest_path) {
                Ok(Some(module)) => {
                    self.status_text = format!(
                        "{}: {} | módulo padrão: {}",
                        self.tr(language, "import"),
                        imported_name,
                        module
                    );
                }
                Ok(None) => {
                    self.status_text =
                        format!("{}: {}", self.tr(language, "import"), imported_name);
                }
                Err(err) => {
                    self.status_text = format!(
                        "{}: {} | aviso módulo: {}",
                        self.tr(language, "import"),
                        imported_name,
                        err
                    );
                }
            }
        } else {
            self.status_text = format!("{}: {}", self.tr(language, "import"), imported_name);
        }
    }

    fn unique_named_file_path(dir: &Path, base_stem: &str, ext: &str) -> PathBuf {
        let first = dir.join(format!("{base_stem}.{ext}"));
        if !first.exists() {
            return first;
        }
        for idx in 1..10_000 {
            let candidate = dir.join(format!("{base_stem}_{idx}.{ext}"));
            if !candidate.exists() {
                return candidate;
            }
        }
        dir.join(format!("{base_stem}.{ext}"))
    }

    fn create_text_asset(
        &mut self,
        language: EngineLanguage,
        target_folder: &'static str,
        base_stem: &str,
        ext: &str,
        content: &str,
    ) {
        let dir = Path::new("Assets").join(target_folder);
        if let Err(err) = fs::create_dir_all(&dir) {
            self.status_text = format!(
                "{}: erro ao criar pasta ({err})",
                self.tr(language, "create")
            );
            return;
        }
        let target = Self::unique_named_file_path(&dir, base_stem, ext);
        if let Err(err) = fs::write(&target, content.as_bytes()) {
            self.status_text = format!(
                "{}: erro ao criar arquivo ({err})",
                self.tr(language, "create")
            );
            return;
        }
        let Some(name) = target
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
        else {
            self.status_text = format!("{}: erro ao resolver nome", self.tr(language, "create"));
            return;
        };
        let imported = self.imported_assets.entry(target_folder).or_default();
        if !imported.iter().any(|n| n == &name) {
            imported.push(name.clone());
        }
        self.deleted_assets.remove(&name);
        self.selected_folder = target_folder;
        self.selected_asset = Some(name.clone());
        self.status_text = format!("{}: {}", self.tr(language, "created"), name);
    }

    fn unique_named_folder_path(dir: &Path, base_name: &str) -> PathBuf {
        let first = dir.join(base_name);
        if !first.exists() {
            return first;
        }
        for idx in 1..10_000 {
            let candidate = dir.join(format!("{base_name}_{idx}"));
            if !candidate.exists() {
                return candidate;
            }
        }
        dir.join(base_name)
    }

    fn create_folder_in_selected(&mut self, language: EngineLanguage) {
        let Some(parent) = self.selected_folder_path() else {
            self.status_text = format!("{}: pasta alvo inválida", self.tr(language, "create"));
            return;
        };
        if let Err(err) = fs::create_dir_all(&parent) {
            self.status_text = format!(
                "{}: erro ao criar pasta alvo ({err})",
                self.tr(language, "create")
            );
            return;
        }
        let target = Self::unique_named_folder_path(&parent, "NovaPasta");
        if let Err(err) = fs::create_dir(&target) {
            self.status_text = format!(
                "{}: erro ao criar pasta ({err})",
                self.tr(language, "create")
            );
            return;
        }
        let name = target
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("NovaPasta")
            .to_string();
        self.status_text = format!("{}: {}", self.tr(language, "created"), name);
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
        let path = Self::normalize_project_save_path(&path);

        match self.save_project_file(&path) {
            Ok(()) => {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("project.deng");
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
        let path = Self::normalize_project_save_path(path);
        match self.save_project_file(&path) {
            Ok(()) => {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("project.deng");
                self.status_text = format!("{}: {}", self.tr(language, "save"), name);
                true
            }
            Err(err) => {
                self.status_text = format!("{}: erro ao salvar ({err})", self.tr(language, "save"));
                false
            }
        }
    }

    fn normalize_project_save_path(path: &Path) -> PathBuf {
        let mut p = path.to_path_buf();
        if p.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("deng"))
            != Some(true)
        {
            p.set_extension("deng");
        }
        let parent = p.parent().unwrap_or_else(|| Path::new("."));
        let stem = p
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("project")
            .to_string();
        if parent.join("Assets").is_dir() {
            return p;
        }
        let root = parent.join(&stem);
        let _ = fs::create_dir_all(root.join("Assets"));
        root.join(format!("{stem}.deng"))
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
        } else if asset.ends_with(".fbx")
            || asset.ends_with(".obj")
            || asset.ends_with(".glb")
            || asset.ends_with(".gltf")
        {
            (Color32::from_rgb(86, 132, 176), "MESH")
        } else if asset.ends_with(".json") {
            (Color32::from_rgb(127, 127, 127), "{}")
        } else {
            (Color32::from_rgb(88, 88, 88), "AS")
        }
    }

    fn selected_folder_path(&self) -> Option<PathBuf> {
        Self::folder_path_from_id(self.selected_folder)
    }

    fn folder_path_from_id(folder: &'static str) -> Option<PathBuf> {
        match folder {
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

    fn source_stamp(path: &Path) -> Option<(u64, u64)> {
        let meta = fs::metadata(path).ok()?;
        let len = meta.len();
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Some((len, mtime))
    }

    fn parse_fbx_animation_names(raw: &str) -> Vec<String> {
        let mut out = Vec::<String>::new();
        let mut push_unique = |name: &str| {
            let clean = name.trim();
            if clean.is_empty() {
                return;
            }
            if !out.iter().any(|x| x.eq_ignore_ascii_case(clean)) {
                out.push(clean.to_string());
            }
        };
        for prefix in ["AnimationStack::", "AnimStack::"] {
            let mut offset = 0usize;
            while let Some(found) = raw[offset..].find(prefix) {
                let mut i = offset + found + prefix.len();
                while i < raw.len() && raw.as_bytes()[i].is_ascii_whitespace() {
                    i += 1;
                }
                if i < raw.len() && raw.as_bytes()[i] == b'"' {
                    i += 1;
                }
                let start = i;
                while i < raw.len() {
                    let c = raw.as_bytes()[i];
                    if c == b'"' || c == b',' || c == b'\r' || c == b'\n' || c == 0 {
                        break;
                    }
                    i += 1;
                }
                push_unique(&raw[start..i]);
                offset = i.saturating_add(1);
                if offset >= raw.len() {
                    break;
                }
            }
        }
        let mut offset = 0usize;
        while let Some(found) = raw[offset..].find("Take:") {
            let mut i = offset + found + "Take:".len();
            while i < raw.len() && raw.as_bytes()[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < raw.len() && raw.as_bytes()[i] == b'"' {
                i += 1;
                let start = i;
                while i < raw.len() && raw.as_bytes()[i] != b'"' && raw.as_bytes()[i] != 0 {
                    i += 1;
                }
                push_unique(&raw[start..i]);
            }
            offset = i.saturating_add(1);
            if offset >= raw.len() {
                break;
            }
        }
        out
    }

    fn parse_fbx_skeleton_bones(raw: &str) -> Vec<String> {
        let mut out = Vec::<String>::new();
        for prefix in ["LimbNode::", "Skeleton::"] {
            let mut offset = 0usize;
            while let Some(found) = raw[offset..].find(prefix) {
                let mut i = offset + found + prefix.len();
                while i < raw.len() && raw.as_bytes()[i].is_ascii_whitespace() {
                    i += 1;
                }
                if i < raw.len() && raw.as_bytes()[i] == b'"' {
                    i += 1;
                }
                let start = i;
                while i < raw.len() {
                    let c = raw.as_bytes()[i];
                    if c == b'"' || c == b',' || c == b'\r' || c == b'\n' || c == 0 {
                        break;
                    }
                    i += 1;
                }
                let name = raw[start..i].trim();
                if !name.is_empty() {
                    out.push(name.to_ascii_lowercase());
                }
                offset = i.saturating_add(1);
                if offset >= raw.len() {
                    break;
                }
            }
        }
        out.sort();
        out.dedup();
        out
    }

    fn parse_fbx_binary_animation_names(bytes: &[u8]) -> Vec<String> {
        let mut out = Vec::<String>::new();

        // FBX binary header is 27 bytes, then nodes start
        if bytes.len() < 27 {
            return out;
        }

        // Check for FBX binary magic
        let header = String::from_utf8_lossy(&bytes[0..20]);
        if !header.contains("Kaydara FBX Binary") {
            return out;
        }

        // Scan for AnimationStack nodes in binary format
        // In binary FBX, nodes have format:
        // - 4 bytes: end offset
        // - 4 bytes: num properties
        // - 4 bytes: property list length
        // - 1 byte: name length
        // - name bytes
        // - padding to 4-byte align
        // - property data

        let mut pos = 27; // Skip header

        while pos + 4 < bytes.len() {
            let record_len =
                u32::from_le_bytes([bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]])
                    as usize;

            if record_len == 0 || record_len > bytes.len().saturating_sub(pos) {
                break;
            }

            let end_pos = pos + record_len;
            pos += 4;

            if pos + 4 >= bytes.len() {
                break;
            }

            // Skip num properties and property list length
            let _num_props =
                u32::from_le_bytes([bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]]);
            pos += 4;

            if pos + 4 >= bytes.len() {
                break;
            }

            let _prop_list_len =
                u32::from_le_bytes([bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]]);
            pos += 4;

            if pos >= bytes.len() {
                break;
            }

            // Get name length
            let name_len = bytes[pos] as usize;
            pos += 1;

            // Align to 4 bytes
            if pos % 4 != 0 {
                pos += 4 - (pos % 4);
            }

            if pos + name_len > bytes.len() {
                break;
            }

            // Get node name
            let node_name = String::from_utf8_lossy(&bytes[pos..pos + name_len]);
            pos += name_len;

            // Align after name
            while pos % 4 != 0 && pos < bytes.len() {
                pos += 1;
            }

            // Check if this is an AnimationStack node
            if node_name.contains("AnimationStack") || node_name.contains("AnimStack") {
                // Try to find the name property (usually first string property)
                // Skip the properties and look for the name
                let mut search_pos = pos;

                // Search for string properties that contain animation names
                // Look for common animation patterns
                while search_pos + 10 < end_pos && search_pos + 100 < bytes.len() {
                    // Look for string length marker followed by text
                    if bytes[search_pos] == 0
                        && bytes[search_pos + 1] == 0
                        && bytes[search_pos + 2] == 0
                    {
                        // Could be a string, try to parse
                        let remaining = &bytes[search_pos + 4..];
                        if let Ok(s) = String::from_utf8(
                            remaining
                                .iter()
                                .take_while(|b| **b != 0)
                                .cloned()
                                .collect::<Vec<_>>(),
                        ) {
                            if !s.is_empty() && s.len() > 1 && s.len() < 100 {
                                // Check if it looks like an animation name
                                if s.chars().all(|c| {
                                    c.is_alphanumeric() || c == '_' || c == '-' || c == ' '
                                }) {
                                    if !out.iter().any(|x| x.eq_ignore_ascii_case(&s)) {
                                        out.push(s);
                                    }
                                }
                            }
                        }
                    }
                    search_pos += 1;
                }
            }

            pos = end_pos;
        }

        // Also look for "Take" nodes in binary
        let bytes_str = String::from_utf8_lossy(bytes);
        let mut search_offset = 0;
        while let Some(start) = bytes_str[search_offset..].find("Take") {
            let ctx_start = search_offset.saturating_add(start).saturating_sub(20);
            let ctx = &bytes_str[ctx_start
                ..search_offset
                    .saturating_add(start + 50)
                    .min(bytes_str.len())];

            // Look for "Take" followed by a name in quotes or as length-prefixed
            if ctx.contains("Take") {
                // Extract what comes after "Take"
                if let Some(after_take) = ctx.split("Take").last() {
                    // Try to extract the name
                    let name: String = after_take
                        .chars()
                        .skip_while(|c| !c.is_alphanumeric())
                        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
                        .collect();
                    if !name.is_empty() && name.len() > 1 {
                        if !out.iter().any(|x| x.eq_ignore_ascii_case(&name)) {
                            out.push(name);
                        }
                    }
                }
            }
            search_offset += start + 4;
            if search_offset >= bytes_str.len() {
                break;
            }
        }

        out.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        out
    }

    fn infer_default_animation_state(clips: &[String], keys: &[&str]) -> Option<String> {
        clips
            .iter()
            .find(|clip| {
                let c = clip.to_ascii_lowercase();
                keys.iter().any(|k| c.contains(k))
            })
            .cloned()
    }

    fn upsert_default_animation_module_for_fbx(
        &mut self,
        imported_fbx_name: &str,
        fbx_path: &Path,
    ) -> Result<Option<String>, String> {
        let bytes = fs::read(fbx_path).map_err(|e| e.to_string())?;
        let raw = String::from_utf8_lossy(&bytes);
        let clips = Self::parse_fbx_animation_names(&raw);
        if clips.is_empty() {
            return Ok(None);
        }

        let bones = Self::parse_fbx_skeleton_bones(&raw);
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        if bones.is_empty() {
            imported_fbx_name.to_ascii_lowercase().hash(&mut hasher);
            clips
                .iter()
                .for_each(|c| c.to_ascii_lowercase().hash(&mut hasher));
        } else {
            bones.iter().for_each(|b| b.hash(&mut hasher));
        }
        let skeleton_key = format!("{:016x}", hasher.finish());

        let idle = Self::infer_default_animation_state(&clips, &["idle", "stand", "breath"]);
        let walk = Self::infer_default_animation_state(&clips, &["walk"]);
        let run = Self::infer_default_animation_state(&clips, &["run", "sprint"]);
        let jump = Self::infer_default_animation_state(&clips, &["jump", "leap"]);

        let module_dir = Path::new("Assets").join("Animations").join("Modules");
        fs::create_dir_all(&module_dir).map_err(|e| e.to_string())?;
        let module_name = format!("default_{skeleton_key}.animodule");
        let module_path = module_dir.join(&module_name);
        let mut content = String::new();
        content.push_str("ANIMODULE1\n");
        content.push_str(&format!("name=Default_{skeleton_key}\n"));
        content.push_str(&format!("skeleton_key={skeleton_key}\n"));
        content.push_str(&format!("source_fbx={imported_fbx_name}\n"));
        if let Some(v) = &idle {
            content.push_str(&format!("state.idle={v}\n"));
        }
        if let Some(v) = &walk {
            content.push_str(&format!("state.walk={v}\n"));
        }
        if let Some(v) = &run {
            content.push_str(&format!("state.run={v}\n"));
        }
        if let Some(v) = &jump {
            content.push_str(&format!("state.jump={v}\n"));
        }
        for clip in &clips {
            content.push_str(&format!("clip={imported_fbx_name}::{clip}\n"));
        }
        fs::write(&module_path, content.as_bytes()).map_err(|e| e.to_string())?;

        let imported = self.imported_assets.entry("Animations").or_default();
        if !imported.iter().any(|n| n == &module_name) {
            imported.push(module_name.clone());
        }
        Ok(Some(module_name))
    }

    fn parse_fbx_meta(path: &Path) -> FbxAssetMeta {
        let bytes = fs::read(path).unwrap_or_default();
        let raw = String::from_utf8_lossy(&bytes);

        // Check if binary FBX
        let is_binary = bytes.len() > 20
            && String::from_utf8_lossy(&bytes[0..20]).contains("Kaydara FBX Binary");

        let animations = if is_binary {
            Self::parse_fbx_binary_animation_names(&bytes)
        } else {
            Self::parse_fbx_animation_names(&raw)
        };

        let has_skeleton = raw.contains("LimbNode")
            || raw.contains("Skeleton::")
            || raw.contains("Deformer::")
            || raw.contains("Cluster::");
        let has_mesh = load_fbx_ascii_preview_mesh(path).is_ok()
            || raw.contains("Geometry::")
            || raw.contains("Mesh");
        FbxAssetMeta {
            has_mesh,
            has_skeleton,
            animations,
        }
    }

    fn fbx_meta_for_path(&mut self, path: &Path) -> FbxAssetMeta {
        let key = path.to_string_lossy().to_string();
        let stamp = Self::source_stamp(path).unwrap_or((0, 0));
        if let Some(entry) = self.fbx_meta_cache.get(&key) {
            if entry.stamp == stamp {
                return entry.meta.clone();
            }
        }
        let meta = Self::parse_fbx_meta(path);
        self.fbx_meta_cache.insert(
            key,
            FbxMetaCacheEntry {
                stamp,
                meta: meta.clone(),
            },
        );
        meta
    }

    fn fbx_assets_in_meshes_folder(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .assets_for_folder_id("Meshes")
            .into_iter()
            .filter(|a| a.to_ascii_lowercase().ends_with(".fbx"))
            .collect();
        out.sort_by_key(|s| s.to_ascii_lowercase());
        out
    }

    pub fn list_animation_controller_assets(&self) -> Vec<String> {
        let mut out = Vec::<String>::new();
        let base = Path::new("Assets").join("Animations");
        if let Ok(entries) = fs::read_dir(base) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_ascii_lowercase())
                    .unwrap_or_default();
                if ext != "animctrl" && ext != "controller" {
                    continue;
                }
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    out.push(name.to_string());
                }
            }
        }
        out.sort_by_key(|s| s.to_ascii_lowercase());
        out.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        out
    }

    pub fn list_animation_modules(&self) -> Vec<String> {
        let mut out = Vec::<String>::new();
        let dir = Path::new("Assets").join("Animations").join("Modules");
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if !p.is_file() {
                    continue;
                }
                let is_animodule = p
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.eq_ignore_ascii_case("animodule"))
                    .unwrap_or(false);
                if !is_animodule {
                    continue;
                }
                if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                    out.push(name.to_string());
                }
            }
        }
        out.sort_by_key(|s| s.to_ascii_lowercase());
        out
    }

    pub fn list_fbx_animation_clips(&mut self) -> Vec<String> {
        let mut out = Vec::<String>::new();
        let Some(meshes_dir) = Self::folder_path_from_id("Meshes") else {
            return out;
        };

        // Get clips from FBX files in Meshes folder
        let fbx_assets = self.fbx_assets_in_meshes_folder();

        for asset in fbx_assets {
            let path = meshes_dir.join(&asset);
            if !path.is_file() {
                continue;
            }

            let meta = self.fbx_meta_for_path(&path);
            for clip in meta.animations {
                out.push(format!("{asset}::{clip}"));
            }
        }

        // Also get clips from .anim files in Assets/Animations folder
        if let Some(anim_dir) = Self::folder_path_from_id("Animations") {
            if let Ok(entries) = fs::read_dir(anim_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if !path.is_file() {
                        continue;
                    }
                    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                    if ext.eq_ignore_ascii_case("anim") {
                        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                            out.push(name.to_string());
                        }
                    }
                }
            }
        }

        out.sort_by_key(|s| s.to_ascii_lowercase());
        out.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        out
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
                            image: Some((
                                [rgba.width() as usize, rgba.height() as usize],
                                rgba.into_raw(),
                            )),
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
                    let image = build_mesh_preview(&asset_path).ok().map(|preview| {
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

        let resp = ui.interact(
            rect,
            ui.id().with("project_icon_size_slider"),
            Sense::click_and_drag(),
        );
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
            egui::pos2(
                track_rect.left() + track_rect.width() * t,
                track_rect.bottom(),
            ),
        );
        ui.painter()
            .rect_filled(fill_rect, 6.0, Color32::from_rgb(15, 232, 121));

        let knob_center = egui::pos2(
            track_rect.left() + track_rect.width() * t,
            track_rect.center().y,
        );
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
        Self::draw_tree_leaf_row_with_icon(ui, id, label, indent, selected, None)
    }

    fn draw_tree_leaf_row_with_icon(
        ui: &mut egui::Ui,
        id: &str,
        label: &str,
        indent: f32,
        selected: bool,
        icon: Option<&TextureHandle>,
    ) -> egui::Response {
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 20.0), Sense::click());
        let resp = ui.interact(
            rect,
            ui.id().with(("project_tree_leaf", id)),
            Sense::click(),
        );
        let mut text_x = rect.left() + indent + 6.0;
        if let Some(icon) = icon {
            let icon_rect = Rect::from_center_size(
                egui::pos2(rect.left() + indent + 6.0, rect.center().y),
                egui::vec2(11.0, 11.0),
            );
            let _ = ui.put(
                icon_rect,
                egui::Image::new(icon).fit_to_exact_size(egui::vec2(11.0, 11.0)),
            );
            text_x += 14.0;
        }

        ui.painter().text(
            egui::pos2(text_x, rect.center().y),
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
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 20.0), Sense::click());
        let row_resp = ui.interact(
            rect,
            ui.id().with(("project_tree_parent", id)),
            Sense::click(),
        );

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
            let angle = if *is_open {
                std::f32::consts::FRAC_PI_2
            } else {
                0.0
            };
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
        if self.rig_icon_texture.is_none() {
            self.rig_icon_texture = load_png_as_texture(ctx, "src/assets/icons/rig.png");
        }
        if self.animador_icon_texture.is_none() {
            self.animador_icon_texture = load_png_as_texture(ctx, "src/assets/icons/animador.png");
        }
        self.poll_preview_jobs(ctx);

        let dock_rect_full = ctx.available_rect();
        let dock_rect = Rect::from_min_max(
            dock_rect_full.min,
            egui::pos2(
                dock_rect_full.max.x,
                dock_rect_full.max.y - bottom_offset.max(0.0),
            ),
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
        let mut request_create_script = false;
        let mut request_create_material = false;
        let mut request_create_folder = false;
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
                        [
                            egui::pos2(rect.left(), rect.top()),
                            egui::pos2(rect.right(), rect.top()),
                        ],
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
                        ui.label(
                            egui::RichText::new("|")
                                .size(12.0)
                                .color(Color32::from_gray(110)),
                        );
                        ui.add_space(8.0);

                        for (idx, (folder_id, folder_label)) in breadcrumb.iter().enumerate() {
                            let is_current = *folder_id == self.selected_folder;
                            let crumb = ui.add(
                                egui::Label::new(
                                    egui::RichText::new(folder_label).size(12.0).color(
                                        Color32::from_gray(if is_current { 220 } else { 190 }),
                                    ),
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
                                    egui::RichText::new(">")
                                        .size(12.0)
                                        .color(Color32::from_gray(150)),
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
                let search_w = desired_search_w
                    .min(search_space)
                    .max(min_search_w.min(search_space));
                let search_x = if search_w <= 0.0 {
                    search_min_x
                } else {
                    let search_max_x = (search_right - search_w).max(search_min_x);
                    (header_rect.center().x - search_w * 0.5 - 36.0)
                        .clamp(search_min_x, search_max_x)
                };
                let search_rect = Rect::from_min_max(
                    egui::pos2(search_x, header_rect.top()),
                    egui::pos2(search_x + search_w, header_rect.bottom()),
                );
                let import_left =
                    (collapse_btn_rect.left() - 8.0 - import_w).max(search_rect.right() + 6.0);
                let import_right = (collapse_btn_rect.left() - 8.0).max(import_left);
                let import_rect = Rect::from_min_max(
                    egui::pos2(import_left, header_rect.top()),
                    egui::pos2(import_right, header_rect.bottom()),
                );

                if search_w > 0.0 {
                    ui.scope_builder(
                        egui::UiBuilder::new().max_rect(search_rect).layout(
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
                    egui::UiBuilder::new().max_rect(import_rect).layout(
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
                                    for folder in
                                        ["Animations", "Materials", "Meshes", "Mold", "Scripts"]
                                    {
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
                                        if folder == "Meshes" && self.selected_folder == "Meshes" {
                                            let Some(meshes_dir) =
                                                Self::folder_path_from_id("Meshes")
                                            else {
                                                continue;
                                            };
                                            for fbx_asset in self.fbx_assets_in_meshes_folder() {
                                                let mut opened =
                                                    self.fbx_expanded_assets.contains(&fbx_asset);
                                                let row = self.draw_tree_parent_row(
                                                    ui,
                                                    &format!("mesh_fbx_{fbx_asset}"),
                                                    &fbx_asset,
                                                    34.0,
                                                    &mut opened,
                                                    self.selected_asset.as_ref()
                                                        == Some(&fbx_asset),
                                                );
                                                if opened {
                                                    self.fbx_expanded_assets
                                                        .insert(fbx_asset.clone());
                                                } else {
                                                    self.fbx_expanded_assets.remove(&fbx_asset);
                                                }
                                                if row.clicked() {
                                                    self.selected_folder = "Meshes";
                                                    self.selected_asset = Some(fbx_asset.clone());
                                                    self.status_text = fbx_asset.clone();
                                                }
                                                if !opened {
                                                    continue;
                                                }

                                                let asset_path = meshes_dir.join(&fbx_asset);
                                                let meta = self.fbx_meta_for_path(&asset_path);
                                                let mesh_label = if meta.has_mesh {
                                                    "Mesh"
                                                } else {
                                                    "Mesh (indisponível)"
                                                };
                                                let _ = Self::draw_tree_leaf_row(
                                                    ui,
                                                    &format!("mesh_node_{fbx_asset}"),
                                                    mesh_label,
                                                    52.0,
                                                    false,
                                                );

                                                let skeleton_label = if meta.has_skeleton {
                                                    "Esqueleto"
                                                } else {
                                                    "Esqueleto (não detectado)"
                                                };
                                                let _ = Self::draw_tree_leaf_row_with_icon(
                                                    ui,
                                                    &format!("skeleton_node_{fbx_asset}"),
                                                    skeleton_label,
                                                    52.0,
                                                    false,
                                                    self.rig_icon_texture.as_ref(),
                                                );

                                                if meta.animations.is_empty() {
                                                    let _ = Self::draw_tree_leaf_row_with_icon(
                                                        ui,
                                                        &format!("anim_none_{fbx_asset}"),
                                                        "Animações (0)",
                                                        52.0,
                                                        false,
                                                        self.animador_icon_texture.as_ref(),
                                                    );
                                                } else {
                                                    let _ = Self::draw_tree_leaf_row_with_icon(
                                                        ui,
                                                        &format!("anim_count_{fbx_asset}"),
                                                        &format!(
                                                            "Animações ({})",
                                                            meta.animations.len()
                                                        ),
                                                        52.0,
                                                        false,
                                                        self.animador_icon_texture.as_ref(),
                                                    );
                                                    for clip in &meta.animations {
                                                        let clip_ref =
                                                            format!("{fbx_asset}::{clip}");
                                                        let resp =
                                                            Self::draw_tree_leaf_row_with_icon(
                                                                ui,
                                                                &format!(
                                                                    "anim_clip_{fbx_asset}_{clip}"
                                                                ),
                                                                clip,
                                                                70.0,
                                                                false,
                                                                self.animador_icon_texture.as_ref(),
                                                            );
                                                        if resp.clicked() {
                                                            self.selected_folder = "Meshes";
                                                            self.selected_asset =
                                                                Some(fbx_asset.clone());
                                                            self.status_text =
                                                                format!("Clipe: {clip_ref}");
                                                        }
                                                    }
                                                }
                                            }
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
                        let grid_ctx_resp = ui.interact(
                            ui.max_rect(),
                            ui.id().with("project_grid_context_menu"),
                            Sense::click(),
                        );
                        grid_ctx_resp.context_menu(|ui| {
                            ui.menu_button(self.tr(language, "create"), |ui| {
                                if ui.button(self.tr(language, "create_script")).clicked() {
                                    request_create_script = true;
                                    ui.close();
                                }
                                if ui.button(self.tr(language, "create_material")).clicked() {
                                    request_create_material = true;
                                    ui.close();
                                }
                                if ui.button(self.tr(language, "create_folder")).clicked() {
                                    request_create_folder = true;
                                    ui.close();
                                }
                            });
                            ui.separator();
                            if ui.button(self.tr(language, "import")).clicked() {
                                request_import = true;
                                ui.close();
                            }
                        });
                        egui::ScrollArea::vertical()
                            .id_salt("project_grid")
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                let spacing = egui::vec2(8.0, 8.0);
                                ui.spacing_mut().item_spacing = spacing;
                                let tile_w = self.icon_scale.clamp(56.0, 98.0);
                                let tile_name_h = 18.0;
                                let tile_pad = 6.0;
                                let tile_size =
                                    Vec2::new(tile_w, tile_w + tile_name_h + tile_pad * 2.0);
                                let cols = (((ui.available_width() + spacing.x)
                                    / (tile_size.x + spacing.x))
                                    .floor() as usize)
                                    .max(1);
                                let now = ui.ctx().input(|i| i.time);
                                let mut hovered_any = false;

                                egui::Grid::new("project_asset_grid_fixed")
                                    .num_columns(cols)
                                    .spacing(spacing)
                                    .show(ui, |ui| {
                                        let mut col = 0usize;
                                        for asset in filtered_assets.iter() {
                                            let asset = *asset;
                                            let tile_size = Vec2::new(
                                                tile_w,
                                                tile_w + tile_name_h + tile_pad * 2.0,
                                            );
                                            let selected =
                                                self.selected_asset.as_ref() == Some(asset);
                                            let (tile_rect, tile_resp) = ui.allocate_exact_size(
                                                tile_size,
                                                Sense::click_and_drag(),
                                            );

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
                                                    Stroke::new(
                                                        1.0,
                                                        Color32::from_rgb(15, 232, 121),
                                                    )
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
                                            if let Some(tex) =
                                                self.asset_preview_texture(ui.ctx(), asset)
                                            {
                                                let image_rect = preview_rect.shrink(1.0);
                                                let _ = ui.put(
                                                    image_rect,
                                                    egui::Image::new(tex)
                                                        .fit_to_exact_size(image_rect.size()),
                                                );
                                                ui.painter().rect_stroke(
                                                    preview_rect,
                                                    2.0,
                                                    Stroke::new(
                                                        1.0,
                                                        Color32::from_rgba_unmultiplied(
                                                            255, 255, 255, 26,
                                                        ),
                                                    ),
                                                    egui::StrokeKind::Outside,
                                                );
                                            } else {
                                                let (icon_color, icon_tag) =
                                                    Self::icon_style(asset);
                                                ui.painter().rect_filled(
                                                    preview_rect.shrink(1.0),
                                                    2.0,
                                                    icon_color,
                                                );
                                                ui.painter().text(
                                                    preview_rect.center(),
                                                    egui::Align2::CENTER_CENTER,
                                                    icon_tag,
                                                    FontId::proportional(10.0),
                                                    Color32::from_gray(245),
                                                );
                                            }
                                            let mut expanded_fbx = false;
                                            if self.selected_folder == "Meshes"
                                                && asset.to_ascii_lowercase().ends_with(".fbx")
                                            {
                                                let expand_rect = Rect::from_center_size(
                                                    egui::pos2(
                                                        preview_rect.right() - 8.0,
                                                        preview_rect.center().y,
                                                    ),
                                                    egui::vec2(14.0, 14.0),
                                                );
                                                let expand_resp = ui.interact(
                                                    expand_rect,
                                                    ui.id().with(("mesh_tile_expand", asset)),
                                                    Sense::click(),
                                                );
                                                let expanded =
                                                    self.fbx_expanded_assets.contains(asset);
                                                expanded_fbx = expanded;
                                                if expand_resp.hovered() {
                                                    ui.painter().circle_filled(
                                                        expand_rect.center(),
                                                        8.0,
                                                        Color32::from_rgba_unmultiplied(
                                                            255, 255, 255, 26,
                                                        ),
                                                    );
                                                }
                                                if let Some(arrow_tex) = &self.arrow_icon_texture {
                                                    let angle = if expanded {
                                                        std::f32::consts::FRAC_PI_2
                                                    } else {
                                                        0.0
                                                    };
                                                    let _ = ui.put(
                                                        expand_rect,
                                                        egui::Image::new(arrow_tex)
                                                            .fit_to_exact_size(egui::vec2(
                                                                10.0, 10.0,
                                                            ))
                                                            .rotate(angle, Vec2::splat(0.5)),
                                                    );
                                                } else {
                                                    ui.painter().text(
                                                        expand_rect.center(),
                                                        Align2::CENTER_CENTER,
                                                        if expanded { "▾" } else { "▸" },
                                                        FontId::new(11.0, FontFamily::Proportional),
                                                        Color32::from_gray(230),
                                                    );
                                                }
                                                if expand_resp.clicked() {
                                                    if expanded {
                                                        self.fbx_expanded_assets.remove(asset);
                                                        expanded_fbx = false;
                                                    } else {
                                                        self.fbx_expanded_assets
                                                            .insert(asset.clone());
                                                        self.selected_asset = Some(asset.clone());
                                                        expanded_fbx = true;
                                                    }
                                                }
                                            }
                                            let name_font = FontId::proportional(11.0);
                                            let name_color = Color32::from_gray(210);
                                            let name_rect = Rect::from_min_max(
                                                egui::pos2(
                                                    tile_rect.left() + tile_pad,
                                                    tile_rect.bottom() - tile_name_h - 2.0,
                                                ),
                                                egui::pos2(
                                                    tile_rect.right() - tile_pad,
                                                    tile_rect.bottom() - 2.0,
                                                ),
                                            );
                                            let clipped_painter =
                                                ui.painter().with_clip_rect(name_rect);
                                            let full_w = ui
                                                .painter()
                                                .layout_no_wrap(
                                                    asset.to_string(),
                                                    name_font.clone(),
                                                    name_color,
                                                )
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
                                                    let overflow = (full_w - name_rect.width()
                                                        + tail_pad)
                                                        .max(0.0);
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
                                                    let base_x =
                                                        name_rect.center().x - full_w * 0.5;

                                                    clipped_painter.text(
                                                        egui::pos2(
                                                            base_x - scroll_x,
                                                            name_rect.center().y,
                                                        ),
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
                                                if ui.button(self.tr(language, "reveal")).clicked()
                                                {
                                                    reveal_clicked = true;
                                                    ui.close();
                                                }
                                                ui.separator();
                                                if ui
                                                    .add(
                                                        egui::Button::new(
                                                            self.tr(language, "delete"),
                                                        )
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
                                                self.status_text = format!(
                                                    "{}: {}",
                                                    self.tr(language, "open"),
                                                    asset
                                                );
                                            }
                                            if reveal_clicked {
                                                self.status_text = format!(
                                                    "{}: {}",
                                                    self.tr(language, "reveal"),
                                                    asset
                                                );
                                            }
                                            if delete_clicked {
                                                self.deleted_assets.insert(asset.clone());
                                                if self.selected_asset.as_ref() == Some(asset) {
                                                    self.selected_asset = None;
                                                }
                                                if self.selected_sub_asset.as_ref().is_some_and(
                                                    |s| s.starts_with(&format!("{asset}::")),
                                                ) {
                                                    self.selected_sub_asset = None;
                                                }
                                                self.status_text = format!(
                                                    "{}: {}",
                                                    self.tr(language, "delete"),
                                                    asset
                                                );
                                            }

                                            if tile_resp.clicked() {
                                                self.selected_asset = Some(asset.clone());
                                                self.selected_sub_asset = None;
                                                self.status_text = asset.to_string();
                                            }
                                            if tile_resp.drag_started() || tile_resp.dragged() {
                                                if !self
                                                    .dragging_asset
                                                    .as_ref()
                                                    .is_some_and(|v| v.contains("::"))
                                                {
                                                    self.dragging_asset = Some(asset.clone());
                                                }
                                            }
                                            if tile_resp.hovered()
                                                && ui.input(|i| {
                                                    i.pointer.primary_down()
                                                        && i.pointer.delta().length_sq() > 0.0
                                                })
                                            {
                                                if !self
                                                    .dragging_asset
                                                    .as_ref()
                                                    .is_some_and(|v| v.contains("::"))
                                                {
                                                    self.dragging_asset = Some(asset.clone());
                                                }
                                            }
                                            col += 1;
                                            if col % cols == 0 {
                                                ui.end_row();
                                            }

                                            if expanded_fbx {
                                                let Some(meshes_dir) =
                                                    Self::folder_path_from_id("Meshes")
                                                else {
                                                    continue;
                                                };
                                                let meta =
                                                    self.fbx_meta_for_path(&meshes_dir.join(asset));
                                                let child_tile_w = (tile_w * 0.82).max(48.0);
                                                let child_tile_name_h = 16.0;
                                                let child_tile_pad = 5.0;
                                                let child_tile_size = Vec2::new(
                                                    child_tile_w,
                                                    child_tile_w
                                                        + child_tile_name_h
                                                        + child_tile_pad * 2.0,
                                                );
                                                let mut children: Vec<(
                                                    String,
                                                    Option<&TextureHandle>,
                                                    Option<String>,
                                                    String,
                                                )> = Vec::new();
                                                children.push((
                                                    if meta.has_mesh {
                                                        "Mesh".to_string()
                                                    } else {
                                                        "Mesh (indisponível)".to_string()
                                                    },
                                                    None,
                                                    None,
                                                    format!("{asset}::mesh"),
                                                ));
                                                children.push((
                                                    if meta.has_skeleton {
                                                        "Esqueleto".to_string()
                                                    } else {
                                                        "Esqueleto (não detectado)".to_string()
                                                    },
                                                    self.rig_icon_texture.as_ref(),
                                                    None,
                                                    format!("{asset}::skeleton"),
                                                ));
                                                children.push((
                                                    format!(
                                                        "Animações ({})",
                                                        meta.animations.len()
                                                    ),
                                                    self.animador_icon_texture.as_ref(),
                                                    Some(asset.clone()),
                                                    format!("{asset}::animations"),
                                                ));
                                                for clip in &meta.animations {
                                                    let clip_ref = format!("{asset}::{clip}");
                                                    children.push((
                                                        format!("Anim: {clip}"),
                                                        self.animador_icon_texture.as_ref(),
                                                        Some(clip_ref.clone()),
                                                        clip_ref,
                                                    ));
                                                }

                                                for (label, icon_opt, drag_payload, child_key) in
                                                    children
                                                {
                                                    let (c_rect, c_resp) = ui.allocate_exact_size(
                                                        child_tile_size,
                                                        Sense::click_and_drag(),
                                                    );
                                                    let selected_sub = self
                                                        .selected_sub_asset
                                                        .as_ref()
                                                        .is_some_and(|k| k == &child_key);
                                                    if selected_sub {
                                                        ui.painter().rect_filled(
                                                            c_rect.expand(1.0),
                                                            5.0,
                                                            Color32::from_rgba_unmultiplied(
                                                                15, 232, 121, 24,
                                                            ),
                                                        );
                                                    }
                                                    ui.painter().rect_stroke(
                                                        c_rect,
                                                        4.0,
                                                        if selected_sub {
                                                            Stroke::new(
                                                                1.2,
                                                                Color32::from_rgb(15, 232, 121),
                                                            )
                                                        } else {
                                                            Stroke::new(
                                                                1.0,
                                                                Color32::from_rgb(70, 70, 78),
                                                            )
                                                        },
                                                        egui::StrokeKind::Outside,
                                                    );
                                                    let c_preview = Rect::from_min_max(
                                                        c_rect.min
                                                            + egui::vec2(
                                                                child_tile_pad,
                                                                child_tile_pad,
                                                            ),
                                                        egui::pos2(
                                                            c_rect.max.x - child_tile_pad,
                                                            c_rect.max.y
                                                                - child_tile_name_h
                                                                - child_tile_pad,
                                                        ),
                                                    );
                                                    ui.painter().rect_stroke(
                                                        c_preview,
                                                        3.0,
                                                        Stroke::new(
                                                            1.0,
                                                            Color32::from_rgb(88, 96, 108),
                                                        ),
                                                        egui::StrokeKind::Outside,
                                                    );
                                                    if let Some(icon) = icon_opt
                                                        .or(self.arrow_icon_texture.as_ref())
                                                    {
                                                        let icon_rect = Rect::from_center_size(
                                                            c_preview.center(),
                                                            egui::vec2(
                                                                c_preview.width().min(20.0),
                                                                c_preview.height().min(20.0),
                                                            ),
                                                        );
                                                        let _ = ui.put(
                                                            icon_rect,
                                                            egui::Image::new(icon)
                                                                .fit_to_exact_size(
                                                                    icon_rect.size(),
                                                                ),
                                                        );
                                                    }
                                                    let c_name_rect = Rect::from_min_max(
                                                        egui::pos2(
                                                            c_rect.left() + child_tile_pad,
                                                            c_rect.bottom()
                                                                - child_tile_name_h
                                                                - 2.0,
                                                        ),
                                                        egui::pos2(
                                                            c_rect.right() - child_tile_pad,
                                                            c_rect.bottom() - 2.0,
                                                        ),
                                                    );
                                                    let short = Self::truncate_with_ellipsis(
                                                        ui.painter(),
                                                        &label,
                                                        &FontId::proportional(11.0),
                                                        c_name_rect.width(),
                                                    );
                                                    ui.painter().text(
                                                        c_name_rect.center(),
                                                        Align2::CENTER_CENTER,
                                                        short,
                                                        FontId::proportional(11.0),
                                                        Color32::from_gray(210),
                                                    );
                                                    if c_resp.clicked() {
                                                        self.selected_asset = Some(asset.clone());
                                                        self.selected_sub_asset =
                                                            Some(child_key.clone());
                                                        self.status_text =
                                                            format!("{asset} > {label}");
                                                    }
                                                    if let Some(payload) = &drag_payload {
                                                        if c_resp.drag_started() || c_resp.dragged()
                                                        {
                                                            self.dragging_asset =
                                                                Some(payload.clone());
                                                        }
                                                        if c_resp.hovered()
                                                            && ui.input(|i| {
                                                                i.pointer.primary_down()
                                                                    && i.pointer.delta().length_sq()
                                                                        > 0.0
                                                            })
                                                        {
                                                            self.dragging_asset =
                                                                Some(payload.clone());
                                                        }
                                                    }
                                                    col += 1;
                                                    if col % cols == 0 {
                                                        ui.end_row();
                                                    }
                                                }
                                            }
                                        }
                                    });

                                if !hovered_any {
                                    self.hover_roll_asset = None;
                                }
                            });
                    },
                );

                let footer_rect =
                    Rect::from_min_max(egui::pos2(inner.left(), inner.bottom() - 18.0), inner.max);
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(footer_rect)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    |ui| {
                        let count = filtered_assets.len();
                        let status = if self.status_text.is_empty() {
                            format!("{} {}", count, self.tr(language, "count"))
                        } else {
                            format!(
                                "{} {} | {}",
                                count,
                                self.tr(language, "count"),
                                self.status_text
                            )
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
        if request_create_script {
            self.create_text_asset(
                language,
                "Scripts",
                "NovoScript",
                "cs",
                "using UnityEngine;\n\npublic class NovoScript : MonoBehaviour\n{\n    void Start()\n    {\n    }\n\n    void Update()\n    {\n    }\n}\n",
            );
        }
        if request_create_material {
            self.create_text_asset(
                language,
                "Materials",
                "NovoMaterial",
                "mat",
                "# Dengine Material\nshader=Standard\nalbedo=1,1,1,1\nmetallic=0.0\nsmoothness=0.5\n",
            );
        }
        if request_create_folder {
            self.create_folder_in_selected(language);
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
        draw_line_rgba(&mut rgba, w, h, x0, y0, x1, y1, [145, 198, 236, 255]);
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

fn put_pixel_rgba(rgba: &mut [u8], w: usize, h: usize, x: i32, y: i32, color: [u8; 4]) {
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
    f.write_all(&stamp.0.to_le_bytes())
        .map_err(|e| e.to_string())?;
    f.write_all(&stamp.1.to_le_bytes())
        .map_err(|e| e.to_string())?;
    let vcount = mesh.0.len() as u32;
    let tcount = mesh.1.len() as u32;
    f.write_all(&vcount.to_le_bytes())
        .map_err(|e| e.to_string())?;
    f.write_all(&tcount.to_le_bytes())
        .map_err(|e| e.to_string())?;
    for v in &mesh.0 {
        f.write_all(&v.x.to_le_bytes()).map_err(|e| e.to_string())?;
        f.write_all(&v.y.to_le_bytes()).map_err(|e| e.to_string())?;
        f.write_all(&v.z.to_le_bytes()).map_err(|e| e.to_string())?;
    }
    for tri in &mesh.1 {
        f.write_all(&tri[0].to_le_bytes())
            .map_err(|e| e.to_string())?;
        f.write_all(&tri[1].to_le_bytes())
            .map_err(|e| e.to_string())?;
        f.write_all(&tri[2].to_le_bytes())
            .map_err(|e| e.to_string())?;
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

fn load_gltf_buffers_mesh_only_preview(
    path: &Path,
    gltf: &gltf::Gltf,
) -> Result<Vec<Vec<u8>>, String> {
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
    use fbxcel_dom::v7400::object::{geometry::TypedGeometryHandle, TypedObjectHandle};
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
        // FBX forward correction kept consistent with runtime mesh import.
        vertices.extend(
            cps.iter()
                .map(|p| glam::Vec3::new(-(p.x as f32), p.y as f32, -(p.z as f32))),
        );

        let mut poly: Vec<u32> = Vec::new();
        for raw in poly_verts.raw_polygon_vertices() {
            let is_end = *raw < 0;
            let local_idx = if is_end {
                (-raw - 1) as u32
            } else {
                *raw as u32
            };
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
