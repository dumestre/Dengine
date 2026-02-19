use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};

use crate::hierarchy::Primitive3DKind;
use crate::inspector;
use crate::viewport_gpu::ViewportGpuRenderer;
use eframe::egui::{
    self, Align2, Color32, FontId, PointerButton, Pos2, Rect, Sense, Stroke, TextureHandle,
    TextureOptions, Vec2,
};
use egui_gizmo::{Gizmo, GizmoMode, GizmoOrientation};
use epaint::ColorImage;
use glam::{EulerRot, Mat4, Quat, Vec3};

const MAX_RUNTIME_TRIANGLES: usize = 90_000;
const MAX_RUNTIME_VERTICES: usize = 120_000;
const MAX_IMPORT_FILE_BYTES: u64 = 350 * 1024 * 1024;
const MAX_PARSED_TRIANGLES: usize = 6_000_000;
const MAX_PARSED_VERTICES: usize = 3_000_000;
const VIEWPORT_PROXY_TRIANGLES: usize = 12_000;
const VIEWPORT_PROXY_VERTICES: usize = 24_000;
const VIEWPORT_NAV_TRIANGLES: usize = 18_000;
const VIEWPORT_NAV_VERTICES: usize = 36_000;

/// Normaliza um path removendo o prefixo verbatim do Windows (\\?\)
fn normalize_path_string(path: &str) -> String {
    if path.starts_with("\\\\?\\") {
        path[4..].to_string()
    } else {
        path.to_string()
    }
}

pub struct ViewportPanel {
    is_3d: bool,
    is_ortho: bool,
    gizmo_mode: GizmoMode,
    gizmo_orientation: GizmoOrientation,
    model_matrix: Mat4,
    camera_yaw: f32,
    camera_pitch: f32,
    camera_distance: f32,
    camera_target: Vec3,
    object_selected: bool,
    scene_entries: Vec<SceneEntry>,
    selected_scene_object: Option<String>,
    pending_mesh_name: Option<String>,
    rotation_icon: Option<TextureHandle>,
    scale_icon: Option<TextureHandle>,
    transform_icon: Option<TextureHandle>,
    move_icon: Option<TextureHandle>,
    move_view_mode: bool,
    last_viewport_rect: Option<Rect>,
    dropped_asset_label: Option<String>,
    mesh_status: Option<String>,
    mesh_loading: bool,
    pending_delete_object: Option<String>,
    import_pipeline: AssetImportPipeline,
    pending_mesh_job: Option<u64>,
    next_import_job_id: u64,
    undo_stack: Vec<ViewportSnapshot>,
    redo_stack: Vec<ViewportSnapshot>,
    pub light_yaw: f32,
    pub light_pitch: f32,
    pub light_color: [f32; 3],
    pub light_intensity: f32,
    pub light_enabled: bool,
    pending_gizmo_undo: bool,
    gizmo_interacting: bool,
    texture_cache: HashMap<String, TextureHandle>,
}

#[derive(Clone, PartialEq)]
struct SceneEntry {
    name: String,
    transform: Mat4,
    full: MeshData,
    proxy: MeshData,
}

#[derive(Clone, PartialEq)]
struct MeshData {
    name: String,
    vertices: Vec<Vec3>,
    normals: Vec<Vec3>,
    uvs: Vec<[f32; 2]>,
    triangles: Vec<[u32; 3]>,
    texture_path: Option<String>,
    material_path: Option<String>,
}

/// Parse um arquivo .mat e extrai o caminho da textura (albedo/diffuse)
fn parse_material_texture_path(mat_path: &str) -> Option<String> {
    let content = std::fs::read_to_string(mat_path).ok()?;
    let mat_dir = std::path::Path::new(mat_path).parent()?;

    for line in content.lines() {
        let line = line.trim();
        // Procura por albedo_texture, diffuse_texture, texture ou texture_path
        if let Some(val) = line.strip_prefix("albedo_texture=") {
            let mut path = val.trim().trim_matches('"').to_string();

            // Remove prefixo Windows \\?\ se existir
            if path.starts_with("\\\\?\\") {
                path = path[4..].to_string();
            }

            // Se for caminho relativo ou nome interno (ex: base_color_texture), procura em caches
            if !std::path::Path::new(&path).exists()
                || (!path.contains('/') && !path.contains('\\'))
            {
                // Tenta extrair apenas o nome do arquivo
                let file_name = std::path::Path::new(&path)
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.clone());

                // Tenta em Assets/Textures/
                let textures_path = format!("Assets/Textures/{}", file_name);
                if std::path::Path::new(&textures_path).exists() {
                    return Some(textures_path);
                }
                // Tenta relativo ao material
                let rel_path = mat_dir.join(&file_name);
                if rel_path.exists() {
                    if let Ok(abs) = std::fs::canonicalize(&rel_path) {
                        return Some(normalize_path_string(&abs.to_string_lossy()));
                    }
                }
                // Tenta em Assets/Assets/ (legado)
                let assets_path = format!("Assets/Assets/{}", file_name);
                if std::path::Path::new(&assets_path).exists() {
                    return Some(assets_path);
                }
                // Tenta em .cache/textures/ (GLB)
                let cache_dir = Path::new("Assets").join(".cache").join("textures");
                if cache_dir.exists() {
                    if let Ok(entries) = std::fs::read_dir(&cache_dir) {
                        for entry in entries.flatten() {
                            let entry_path = entry.path();
                            if entry_path
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                == Some(file_name.clone())
                            {
                                if let Ok(abs) = std::fs::canonicalize(&entry_path) {
                                    return Some(normalize_path_string(&abs.to_string_lossy()));
                                }
                            }
                        }
                    }
                    // Fallback: primeira textura em cache
                    for entry in std::fs::read_dir(&cache_dir).ok()?.flatten() {
                        let entry_path = entry.path();
                        if let Some(ext) = entry_path.extension().and_then(|e| e.to_str()) {
                            if matches!(ext.to_lowercase().as_str(), "png" | "jpg" | "jpeg") {
                                if let Ok(abs) = std::fs::canonicalize(&entry_path) {
                                    eprintln!("[MATERIAL] Fallback textura cache: {:?}", abs);
                                    return Some(normalize_path_string(&abs.to_string_lossy()));
                                }
                            }
                        }
                    }
                }
            }
            return Some(path);
        }
        if let Some(val) = line.strip_prefix("diffuse_texture=") {
            let mut path = val.trim().trim_matches('"').to_string();
            if path.starts_with("\\\\?\\") {
                path = path[4..].to_string();
            }
            if !std::path::Path::new(&path).exists() {
                let file_name = std::path::Path::new(&path)
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.clone());
                let textures_path = format!("Assets/Textures/{}", file_name);
                if std::path::Path::new(&textures_path).exists() {
                    return Some(textures_path);
                }
            }
            return Some(path);
        }
        if let Some(val) = line.strip_prefix("texture=") {
            return Some(val.trim().trim_matches('"').to_string());
        }
        if let Some(val) = line.strip_prefix("texture_path=") {
            return Some(val.trim().trim_matches('"').to_string());
        }
    }

    // Fallback: procura textura com nome similar ao material
    let mat_name = std::path::Path::new(mat_path)
        .file_stem()?
        .to_string_lossy()
        .to_string();
    let base_name = mat_name.strip_suffix("_Mat").unwrap_or(&mat_name);

    for ext in &["png", "jpg", "jpeg"] {
        let tex_path = format!("Assets/Textures/{}_{}.{}", base_name, base_name, ext);
        if std::path::Path::new(&tex_path).exists() {
            eprintln!(
                "[MATERIAL] Textura encontrada por nome similar: {}",
                tex_path
            );
            return Some(tex_path);
        }
    }

    None
}

fn material_name_variations(name: &str) -> Vec<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let mut variations = Vec::new();
    variations.push(trimmed.to_string());

    if let Some(idx) = trimmed.find(" (Mesh") {
        let base = trimmed[..idx].trim_end();
        if !base.is_empty() {
            variations.push(base.to_string());
        }
    }

    if let Some(stem) = Path::new(trimmed).file_stem().and_then(|s| s.to_str()) {
        let stem_trimmed = stem.trim();
        if !stem_trimmed.is_empty() {
            variations.push(stem_trimmed.to_string());
        }
    }

    variations
}

fn find_material_path_for_names<Names, Name>(names: Names) -> Option<String>
where
    Names: IntoIterator<Item = Name>,
    Name: AsRef<str>,
{
    let materials_dir = Path::new("Assets").join("Materials");
    let mut seen = HashSet::new();
    for name in names {
        for candidate in material_name_variations(name.as_ref()) {
            if !seen.insert(candidate.clone()) {
                continue;
            }
            let material_path = materials_dir.join(format!("{candidate}_Mat.mat"));
            if material_path.exists() {
                if let Ok(abs) = std::fs::canonicalize(&material_path) {
                    return Some(normalize_path_string(&abs.to_string_lossy()));
                }
                return Some(normalize_path_string(&material_path.to_string_lossy()));
            }
        }
    }
    None
}

#[derive(Clone, PartialEq)]
struct ViewportSnapshot {
    scene_entries: Vec<SceneEntry>,
    selected_scene_object: Option<String>,
    object_selected: bool,
    dropped_asset_label: Option<String>,
}

enum MeshLoadEvent {
    Proxy(MeshData),
    Full(Result<MeshData, String>),
}

enum ImportRequest {
    LoadMesh { job_id: u64, path: PathBuf },
}

enum ImportEvent {
    Mesh { job_id: u64, event: MeshLoadEvent },
}

struct ViewportMeshAsset {
    full: MeshData,
    proxy: MeshData,
}

struct AssetImportPipeline {
    tx: Sender<ImportRequest>,
    rx: Receiver<ImportEvent>,
}

impl AssetImportPipeline {
    fn new() -> Self {
        let (tx_req, rx_req) = mpsc::channel::<ImportRequest>();
        let (tx_evt, rx_evt) = mpsc::channel::<ImportEvent>();
        std::thread::spawn(move || {
            while let Ok(req) = rx_req.recv() {
                match req {
                    ImportRequest::LoadMesh { job_id, path } => {
                        match load_viewport_mesh_asset_cached(&path) {
                            Ok(asset) => {
                                let _ = tx_evt.send(ImportEvent::Mesh {
                                    job_id,
                                    event: MeshLoadEvent::Proxy(asset.proxy),
                                });
                                let _ = tx_evt.send(ImportEvent::Mesh {
                                    job_id,
                                    event: MeshLoadEvent::Full(Ok(asset.full)),
                                });
                            }
                            Err(err) => {
                                let _ = tx_evt.send(ImportEvent::Mesh {
                                    job_id,
                                    event: MeshLoadEvent::Full(Err(err)),
                                });
                            }
                        }
                    }
                }
            }
        });
        Self {
            tx: tx_req,
            rx: rx_evt,
        }
    }

    fn enqueue_mesh(&self, job_id: u64, path: PathBuf) {
        let _ = self.tx.send(ImportRequest::LoadMesh { job_id, path });
    }
}

impl ViewportPanel {
    fn scene_entry_world_center(entry: &SceneEntry) -> Vec3 {
        let verts = &entry.proxy.vertices;
        if verts.is_empty() {
            return entry.transform.transform_point3(Vec3::ZERO);
        }
        let sample = verts.len().min(256);
        let mut acc = Vec3::ZERO;
        for v in verts.iter().take(sample) {
            acc += *v;
        }
        let local_center = acc / sample as f32;
        entry.transform.transform_point3(local_center)
    }

    fn scene_entry_world_radius(entry: &SceneEntry, world_center: Vec3) -> f32 {
        let verts = &entry.proxy.vertices;
        if verts.is_empty() {
            return 0.5;
        }
        let mut max_d2 = 0.0_f32;
        for v in verts.iter().take(2048) {
            let w = entry.transform.transform_point3(*v);
            let d2 = w.distance_squared(world_center);
            if d2.is_finite() && d2 > max_d2 {
                max_d2 = d2;
            }
        }
        max_d2.sqrt().clamp(0.2, 50.0)
    }

    fn scene_entry_screen_hit_info(
        entry: &SceneEntry,
        viewport_rect: Rect,
        mvp: Mat4,
    ) -> Option<(Pos2, f32)> {
        let world_center = Self::scene_entry_world_center(entry);
        let screen_center = project_point(viewport_rect, mvp, world_center)?;
        let radius_world = Self::scene_entry_world_radius(entry, world_center);
        if radius_world <= 0.0 {
            return Some((screen_center, 20.0));
        }
        let offsets = [
            Vec3::new(radius_world, 0.0, 0.0),
            Vec3::new(0.0, radius_world, 0.0),
            Vec3::new(0.0, 0.0, radius_world),
            Vec3::new(radius_world, radius_world, 0.0),
            Vec3::new(radius_world, 0.0, radius_world),
        ];
        let mut max_radius = 0.0;
        for offset in offsets {
            if let Some(point) = project_point(viewport_rect, mvp, world_center + offset) {
                let dist = screen_center.distance(point);
                if dist > max_radius {
                    max_radius = dist;
                }
            }
        }
        let screen_radius = (max_radius * 1.25).max(18.0);
        Some((screen_center, screen_radius))
    }

    fn focus_selected_or_origin(&mut self) {
        if let Some(name) = self.selected_scene_object.clone() {
            if let Some(entry) = self.scene_entries.iter().find(|o| o.name == name) {
                let center = Self::scene_entry_world_center(entry);
                let radius = Self::scene_entry_world_radius(entry, center);
                self.camera_target = center;
                self.camera_distance = (radius * 3.0).clamp(0.8, 80.0);
                return;
            }
        }
        self.camera_target = Vec3::ZERO;
        self.camera_distance = self.camera_distance.clamp(0.8, 80.0);
    }

    pub fn new() -> Self {
        let import_pipeline = AssetImportPipeline::new();
        let mut s = Self {
            is_3d: true,
            is_ortho: false,
            gizmo_mode: GizmoMode::Translate,
            gizmo_orientation: GizmoOrientation::Local,
            model_matrix: Mat4::IDENTITY,
            camera_yaw: 0.78,
            camera_pitch: 0.42,
            camera_distance: 4.8,
            camera_target: Vec3::ZERO,
            object_selected: false,
            scene_entries: Vec::new(),
            selected_scene_object: None,
            pending_mesh_name: None,
            rotation_icon: None,
            scale_icon: None,
            transform_icon: None,
            move_icon: None,
            move_view_mode: false,
            last_viewport_rect: None,
            dropped_asset_label: None,
            mesh_status: None,
            mesh_loading: false,
            pending_delete_object: None,
            import_pipeline,
            pending_mesh_job: None,
            next_import_job_id: 1,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            light_yaw: 0.78,
            light_pitch: 0.42,
            light_color: [1.0, 1.0, 1.0],
            light_intensity: 1.0,
            light_enabled: true,
            pending_gizmo_undo: false,
            gizmo_interacting: false,
            texture_cache: HashMap::new(),
        };
        s.push_undo_snapshot();
        s
    }

    fn alloc_import_job_id(&mut self) -> u64 {
        let id = self.next_import_job_id;
        self.next_import_job_id = self.next_import_job_id.wrapping_add(1).max(1);
        id
    }

    pub fn contains_point(&self, p: Pos2) -> bool {
        self.last_viewport_rect.is_some_and(|r| r.contains(p))
    }

    pub fn panel_rect(&self) -> Option<Rect> {
        self.last_viewport_rect
    }

    pub fn request_delete_selected_object(&mut self) {
        if self.pending_delete_object.is_some() {
            return;
        }
        if self.object_selected {
            if let Some(name) = self.selected_scene_object.clone() {
                self.pending_delete_object = Some(name);
            }
        }
    }

    pub fn take_pending_delete_object(&mut self) -> Option<String> {
        self.pending_delete_object.take()
    }

    pub fn selected_object_name(&self) -> Option<&str> {
        self.selected_scene_object.as_deref()
    }

    pub fn scene_object_names(&self) -> Vec<String> {
        self.scene_entries.iter().map(|o| o.name.clone()).collect()
    }

    fn gpu_scene_mesh_id(&self, use_proxy: bool) -> u64 {
        let mut hasher = DefaultHasher::new();
        use_proxy.hash(&mut hasher);
        self.scene_entries.len().hash(&mut hasher);
        for entry in &self.scene_entries {
            entry.name.hash(&mut hasher);
            let mesh = if use_proxy { &entry.proxy } else { &entry.full };
            mesh.vertices.len().hash(&mut hasher);
            mesh.triangles.len().hash(&mut hasher);
            for col in entry.transform.to_cols_array_2d() {
                for f in col {
                    f.to_bits().hash(&mut hasher);
                }
            }
        }
        hasher.finish().max(1)
    }

    fn build_gpu_scene_mesh(&self, use_proxy: bool) -> (MeshData, bool) {
        let mut vertices: Vec<Vec3> = Vec::new();
        let mut normals: Vec<Vec3> = Vec::new();
        let mut uvs: Vec<[f32; 2]> = Vec::new();
        let mut triangles: Vec<[u32; 3]> = Vec::new();
        let mut texture_path: Option<String> = None;
        let mut texture_conflict = false;
        let mut unique_texture: Option<String> = None;

        for entry in &self.scene_entries {
            let mesh = if use_proxy { &entry.proxy } else { &entry.full };
            let base = vertices.len() as u32;
            vertices.extend(
                mesh.vertices
                    .iter()
                    .map(|v| entry.transform.transform_point3(*v)),
            );
            // Transform normals by model matrix (direction only, normalize after)
            normals.extend(
                mesh.normals
                    .iter()
                    .map(|n| entry.transform.transform_vector3(*n).normalize_or_zero()),
            );
            // Pad normals if mesh has fewer normals than vertices
            while normals.len() < vertices.len() {
                normals.push(Vec3::Y);
            }
            // Pad UVs if mesh has fewer UVs than vertices
            uvs.extend(mesh.uvs.iter().cloned());
            while uvs.len() < vertices.len() {
                uvs.push([0.0, 0.0]);
            }
            for tri in &mesh.triangles {
                if (tri[0] as usize) < mesh.vertices.len()
                    && (tri[1] as usize) < mesh.vertices.len()
                    && (tri[2] as usize) < mesh.vertices.len()
                {
                    triangles.push([base + tri[0], base + tri[1], base + tri[2]]);
                }
            }
            let entry_texture = mesh.texture_path.clone().or_else(|| {
                mesh.material_path
                    .as_ref()
                    .and_then(|mp| parse_material_texture_path(mp))
            });
            if texture_path.is_none() {
                texture_path = entry_texture.clone();
            }
            if let Some(tex) = entry_texture {
                match &unique_texture {
                    Some(prev) if prev != &tex => texture_conflict = true,
                    None => unique_texture = Some(tex.clone()),
                    _ => {}
                }
            }
        }
        if texture_conflict {
            texture_path = None;
        } else {
            texture_path = unique_texture.clone().or(texture_path);
        }
        let mesh_summary = MeshData {
            name: "SceneBatch".to_string(),
            vertices,
            normals,
            uvs,
            triangles,
            texture_path,
            material_path: None,
        };
        (mesh_summary, texture_conflict)
    }

    pub fn set_selected_object(&mut self, object_name: &str) {
        if self.scene_entries.iter().any(|o| o.name == object_name) {
            self.selected_scene_object = Some(object_name.to_string());
            self.object_selected = true;
        } else {
            self.selected_scene_object = None;
            self.object_selected = false;
        }
    }

    pub fn remove_scene_object(&mut self, object_name: &str) -> bool {
        let Some(idx) = self
            .scene_entries
            .iter()
            .position(|o| o.name == object_name)
        else {
            return false;
        };
        self.push_undo_snapshot();
        self.scene_entries.remove(idx);
        if self
            .selected_scene_object
            .as_ref()
            .is_some_and(|n| n == object_name)
        {
            self.selected_scene_object = None;
            self.object_selected = false;
        }
        true
    }

    pub fn object_transform_components(
        &self,
        object_name: &str,
    ) -> Option<([f32; 3], [f32; 3], [f32; 3])> {
        let entry = self.scene_entries.iter().find(|o| o.name == object_name)?;
        let (scale, rotation, translation) = entry.transform.to_scale_rotation_translation();
        let (rx, ry, rz) = rotation.to_euler(EulerRot::XYZ);
        Some((
            [translation.x, translation.y, translation.z],
            [rx.to_degrees(), ry.to_degrees(), rz.to_degrees()],
            [
                if scale.x.is_finite() { scale.x } else { 1.0 },
                if scale.y.is_finite() { scale.y } else { 1.0 },
                if scale.z.is_finite() { scale.z } else { 1.0 },
            ],
        ))
    }

    pub fn apply_object_transform_components(
        &mut self,
        object_name: &str,
        position: [f32; 3],
        rotation_deg: [f32; 3],
        scale: [f32; 3],
    ) -> bool {
        let Some(idx) = self
            .scene_entries
            .iter()
            .position(|o| o.name == object_name)
        else {
            return false;
        };
        let pos = Vec3::new(position[0], position[1], position[2]);
        let scl = Vec3::new(scale[0], scale[1], scale[2]);
        let rot = Quat::from_euler(
            EulerRot::XYZ,
            rotation_deg[0].to_radians(),
            rotation_deg[1].to_radians(),
            rotation_deg[2].to_radians(),
        );
        let new_transform = Mat4::from_scale_rotation_translation(scl, rot, pos);
        let old_transform = self.scene_entries[idx].transform;
        if old_transform == new_transform {
            return false;
        }
        self.push_undo_snapshot();
        {
            let entry = &mut self.scene_entries[idx];
            for v in &mut entry.full.vertices {
                *v = new_transform.transform_point3(*v);
            }
            for v in &mut entry.proxy.vertices {
                *v = new_transform.transform_point3(*v);
            }
            entry.transform = Mat4::IDENTITY;
        }
        self.model_matrix = Mat4::IDENTITY;
        self.object_selected = true;
        self.selected_scene_object = Some(object_name.to_string());
        self.mesh_status = Some("Transformacoes aplicadas e zeradas".to_string());
        true
    }

    pub fn set_object_transform_components(
        &mut self,
        object_name: &str,
        position: [f32; 3],
        rotation_deg: [f32; 3],
        scale: [f32; 3],
    ) -> bool {
        let Some(idx) = self
            .scene_entries
            .iter()
            .position(|o| o.name == object_name)
        else {
            return false;
        };
        let pos = Vec3::new(position[0], position[1], position[2]);
        let scl = Vec3::new(scale[0], scale[1], scale[2]);
        let rot = Quat::from_euler(
            EulerRot::XYZ,
            rotation_deg[0].to_radians(),
            rotation_deg[1].to_radians(),
            rotation_deg[2].to_radians(),
        );
        let new_transform = Mat4::from_scale_rotation_translation(scl, rot, pos);
        let old_transform = self.scene_entries[idx].transform;
        if old_transform == new_transform {
            return false;
        }
        self.scene_entries[idx].transform = new_transform;
        self.model_matrix = new_transform;
        self.object_selected = true;
        self.selected_scene_object = Some(object_name.to_string());
        true
    }

    pub fn move_object_by(&mut self, object_name: &str, delta: [f32; 3]) -> bool {
        let Some(idx) = self
            .scene_entries
            .iter()
            .position(|o| o.name == object_name)
        else {
            return false;
        };
        let d = Vec3::new(delta[0], delta[1], delta[2]);
        if d.length_squared() <= 1e-12 {
            return false;
        }
        let (scale, rotation, translation) = self.scene_entries[idx]
            .transform
            .to_scale_rotation_translation();
        let next_t = translation + d;
        self.scene_entries[idx].transform =
            Mat4::from_scale_rotation_translation(scale, rotation, next_t);
        true
    }

    pub fn rotate_object_by(&mut self, object_name: &str, delta_deg: [f32; 3]) -> bool {
        let Some(idx) = self
            .scene_entries
            .iter()
            .position(|o| o.name == object_name)
        else {
            return false;
        };
        let d = Vec3::new(delta_deg[0], delta_deg[1], delta_deg[2]);
        if d.length_squared() <= 1e-12 {
            return false;
        }
        let (scale, rotation, translation) = self.scene_entries[idx]
            .transform
            .to_scale_rotation_translation();
        let dq = Quat::from_euler(
            EulerRot::XYZ,
            delta_deg[0].to_radians(),
            delta_deg[1].to_radians(),
            delta_deg[2].to_radians(),
        );
        self.scene_entries[idx].transform =
            Mat4::from_scale_rotation_translation(scale, rotation * dq, translation);
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo(&mut self) {
        let Some(prev) = self.undo_stack.pop() else {
            return;
        };
        self.redo_stack.push(self.snapshot());
        self.apply_snapshot(prev);
    }

    pub fn redo(&mut self) {
        let Some(next) = self.redo_stack.pop() else {
            return;
        };
        self.undo_stack.push(self.snapshot());
        self.apply_snapshot(next);
    }

    fn snapshot(&self) -> ViewportSnapshot {
        ViewportSnapshot {
            scene_entries: self.scene_entries.clone(),
            selected_scene_object: self.selected_scene_object.clone(),
            object_selected: self.object_selected,
            dropped_asset_label: self.dropped_asset_label.clone(),
        }
    }

    fn apply_snapshot(&mut self, snap: ViewportSnapshot) {
        self.scene_entries = snap.scene_entries;
        self.selected_scene_object = snap.selected_scene_object;
        self.object_selected = snap.object_selected;
        self.dropped_asset_label = snap.dropped_asset_label;
        self.mesh_status = Some("Historico aplicado".to_string());
    }

    fn push_undo_snapshot(&mut self) {
        let snap = self.snapshot();
        if self.undo_stack.last().is_some_and(|s| s == &snap) {
            return;
        }
        self.undo_stack.push(snap);
        if self.undo_stack.len() > 64 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    pub fn on_asset_dropped(&mut self, asset_name: &str) {
        if asset_name.ends_with(".fbx")
            || asset_name.ends_with(".obj")
            || asset_name.ends_with(".glb")
            || asset_name.ends_with(".gltf")
        {
            self.object_selected = true;
            self.dropped_asset_label = Some(asset_name.to_string());
        }
    }

    pub fn spawn_primitive(&mut self, kind: Primitive3DKind, object_name: &str) -> bool {
        let full = make_primitive_mesh(kind);
        if full.vertices.is_empty() || full.triangles.is_empty() {
            return false;
        }
        self.push_undo_snapshot();
        let nav_proxy = make_proxy_mesh(&full, VIEWPORT_NAV_TRIANGLES, VIEWPORT_NAV_VERTICES);
        let target_pos = self.camera_target;
        let rotation = Mat4::from_rotation_y(self.camera_yaw + std::f32::consts::PI);
        let transform = Mat4::from_translation(target_pos) * rotation;
        let name = object_name.to_string();
        self.scene_entries.push(SceneEntry {
            name: name.clone(),
            transform,
            full,
            proxy: nav_proxy,
        });
        self.selected_scene_object = Some(name.clone());
        self.dropped_asset_label = Some(name);
        self.object_selected = true;
        self.mesh_status = Some("Primitiva 3D criada".to_string());
        true
    }

    pub fn spawn_light(&mut self, object_name: &str, light_type: inspector::LightType) -> bool {
        let full = make_light_mesh(light_type);
        if full.vertices.is_empty() || full.triangles.is_empty() {
            return false;
        }
        self.push_undo_snapshot();
        let nav_proxy = make_proxy_mesh(&full, VIEWPORT_NAV_TRIANGLES, VIEWPORT_NAV_VERTICES);
        let target_pos = self.camera_target;
        let rotation = Mat4::from_rotation_y(self.camera_yaw + std::f32::consts::PI);
        let transform = Mat4::from_translation(target_pos) * rotation;
        let name = object_name.to_string();
        self.scene_entries.push(SceneEntry {
            name: name.clone(),
            transform,
            full,
            proxy: nav_proxy,
        });
        self.selected_scene_object = Some(name.clone());
        self.dropped_asset_label = Some(name);
        self.object_selected = true;
        self.mesh_status = Some("Luz adicionada".to_string());
        true
    }

    pub fn on_asset_file_dropped_named(&mut self, path: &Path, object_name: &str) {
        self.pending_mesh_name = Some(object_name.to_string());
        self.on_asset_file_dropped(path);
    }

    pub fn on_asset_file_dropped(&mut self, path: &Path) {
        let asset_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("asset")
            .to_string();
        self.on_asset_dropped(&asset_name);

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .unwrap_or_default();

        match ext.as_str() {
            "fbx" | "obj" | "glb" | "gltf" => {
                if let Ok(meta) = fs::metadata(path) {
                    if meta.len() > MAX_IMPORT_FILE_BYTES {
                        self.mesh_status = Some(
                            "Arquivo muito grande para importacao direta; reduza a malha"
                                .to_string(),
                        );
                        self.mesh_loading = false;
                        self.pending_mesh_job = None;
                        return;
                    }
                }
                let job_id = self.alloc_import_job_id();
                self.pending_mesh_job = Some(job_id);
                self.mesh_loading = true;
                self.mesh_status = Some("Carregando proxy...".to_string());
                if self.pending_mesh_name.is_none() {
                    self.pending_mesh_name = Some(
                        path.file_stem()
                            .and_then(|n| n.to_str())
                            .unwrap_or("Imported Mesh")
                            .to_string(),
                    );
                }
                self.import_pipeline
                    .enqueue_mesh(job_id, path.to_path_buf());
            }
            "png" | "jpg" | "jpeg" | "webp" => {
                self.mesh_status = Some("Viewport em modo sólido: textura desativada".to_string());
            }
            _ => {}
        }
    }

    fn ensure_icons_loaded(&mut self, ctx: &egui::Context) {
        if self.rotation_icon.is_none() {
            self.rotation_icon = load_png_as_texture(ctx, "src/assets/icons/rotation.png");
        }
        if self.scale_icon.is_none() {
            self.scale_icon = load_png_as_texture(ctx, "src/assets/icons/scale.png");
        }
        if self.transform_icon.is_none() {
            self.transform_icon = load_png_as_texture(ctx, "src/assets/icons/transform.png");
        }
        if self.move_icon.is_none() {
            self.move_icon = load_png_as_texture(ctx, "src/assets/icons/move.png");
        }
    }

    fn poll_import_pipeline(&mut self) {
        while let Ok(event) = self.import_pipeline.rx.try_recv() {
            match event {
                ImportEvent::Mesh { job_id, event } => {
                    if self.pending_mesh_job != Some(job_id) {
                        continue;
                    }
                    match event {
                        MeshLoadEvent::Proxy(_mesh) => {
                            self.mesh_status = Some("Proxy carregada... finalizando".to_string());
                        }
                        MeshLoadEvent::Full(Ok(mesh)) => {
                            self.push_undo_snapshot();
                            let is_heavy = mesh.triangles.len() > MAX_RUNTIME_TRIANGLES
                                || mesh.vertices.len() > MAX_RUNTIME_VERTICES;
                            // Full mesh mantém UVs e dados completos
                            let mut full = mesh;
                            // Otimiza se necessário
                            if is_heavy {
                                full = make_proxy_mesh(
                                    &full,
                                    MAX_RUNTIME_TRIANGLES,
                                    MAX_RUNTIME_VERTICES,
                                );
                            }
                            let nav_proxy = make_proxy_mesh(
                                &full,
                                VIEWPORT_NAV_TRIANGLES,
                                VIEWPORT_NAV_VERTICES,
                            );
                            let name = self
                                .pending_mesh_name
                                .take()
                                .unwrap_or_else(|| full.name.clone());
                            let target_pos = self.camera_target;
                            let rotation =
                                Mat4::from_rotation_y(self.camera_yaw + std::f32::consts::PI);
                            let transform = Mat4::from_translation(target_pos) * rotation;
                            self.scene_entries.push(SceneEntry {
                                name: name.clone(),
                                transform,
                                full,
                                proxy: nav_proxy,
                            });
                            if let Some(entry) = self.scene_entries.last_mut() {
                                if entry.full.material_path.is_none() {
                                    let mut name_candidates = vec![name.clone()];
                                    if let Some(stem) = Path::new(&entry.full.name)
                                        .file_stem()
                                        .and_then(|s| s.to_str())
                                    {
                                        name_candidates.push(stem.to_string());
                                    }
                                    if let Some(mat_path) =
                                        find_material_path_for_names(name_candidates.iter())
                                    {
                                        entry.full.material_path = Some(mat_path.clone());
                                        entry.proxy.material_path = Some(mat_path);
                                    }
                                }
                            }
                            self.selected_scene_object = Some(name.clone());
                            self.dropped_asset_label = Some(name);
                            self.object_selected = true;
                            self.mesh_status = Some(if is_heavy {
                                "Mesh carregada com otimização automática".to_string()
                            } else {
                                "Mesh carregada".to_string()
                            });
                            self.mesh_loading = false;
                            self.pending_mesh_job = None;
                        }
                        MeshLoadEvent::Full(Err(err)) => {
                            self.pending_mesh_name = None;
                            self.mesh_status = Some(format!("Falha ao carregar malha: {err}"));
                            self.mesh_loading = false;
                            self.pending_mesh_job = None;
                        }
                    }
                }
            }
        }
    }

    fn gizmo_icon_button(
        ui: &mut egui::Ui,
        texture: Option<&TextureHandle>,
        fallback: &str,
        selected: bool,
        tooltip: &str,
    ) -> bool {
        let button = if let Some(texture) = texture {
            egui::Button::image(egui::Image::new(texture).fit_to_exact_size(egui::vec2(14.0, 14.0)))
        } else {
            egui::Button::new(fallback)
        }
        .corner_radius(6)
        .fill(if selected {
            Color32::from_rgb(64, 64, 68)
        } else {
            Color32::from_rgb(42, 42, 46)
        })
        .stroke(if selected {
            Stroke::new(1.0, Color32::from_rgb(15, 232, 121))
        } else {
            Stroke::new(1.0, Color32::from_rgb(72, 72, 78))
        });

        ui.add_sized([28.0, 24.0], button)
            .on_hover_text(tooltip)
            .clicked()
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        mode_label: &str,
        left_reserved: f32,
        right_reserved: f32,
        bottom_reserved: f32,
        gpu_renderer: Option<&ViewportGpuRenderer>,
    ) {
        self.ensure_icons_loaded(ctx);

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(28, 28, 30))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(48, 48, 52))),
            )
            .show(ctx, |ui| {
                self.poll_import_pipeline();

                if self.mesh_loading {
                    ui.ctx().request_repaint();
                }

                let content = ui.max_rect();
                let viewport_rect = Rect::from_min_max(
                    egui::pos2(content.left() + left_reserved, content.top()),
                    egui::pos2(content.right() - right_reserved, content.bottom() - bottom_reserved),
                );
                if viewport_rect.width() < 80.0 || viewport_rect.height() < 80.0 {
                    self.last_viewport_rect = None;
                    return;
                }
                self.last_viewport_rect = Some(viewport_rect);

                ui.painter()
                    .rect_filled(viewport_rect, 0.0, Color32::from_rgb(22, 22, 24));
                ui.painter().rect_stroke(
                    viewport_rect,
                    0.0,
                    Stroke::new(1.0, Color32::from_rgb(58, 58, 62)),
                    egui::StrokeKind::Outside,
                );

                let grid_step = 24.0;
                let mut x = viewport_rect.left();
                while x <= viewport_rect.right() {
                    ui.painter().line_segment(
                        [egui::pos2(x, viewport_rect.top()), egui::pos2(x, viewport_rect.bottom())],
                        Stroke::new(1.0, Color32::from_rgba_unmultiplied(86, 86, 92, 24)),
                    );
                    x += grid_step;
                }
                let mut y = viewport_rect.top();
                while y <= viewport_rect.bottom() {
                    ui.painter().line_segment(
                        [egui::pos2(viewport_rect.left(), y), egui::pos2(viewport_rect.right(), y)],
                        Stroke::new(1.0, Color32::from_rgba_unmultiplied(86, 86, 92, 24)),
                    );
                    y += grid_step;
                }

                ui.painter().text(
                    egui::pos2(viewport_rect.left() + 12.0, viewport_rect.top() + 10.0),
                    Align2::LEFT_TOP,
                    format!("Viewport - {}", mode_label),
                    FontId::proportional(13.0),
                    Color32::from_gray(210),
                );
                if let Some(asset) = &self.dropped_asset_label {
                    ui.painter().text(
                        egui::pos2(viewport_rect.left() + 12.0, viewport_rect.top() + 28.0),
                        Align2::LEFT_TOP,
                        format!("Asset: {asset}"),
                        FontId::proportional(11.0),
                        Color32::from_rgb(144, 206, 168),
                    );
                }
                if let Some(status) = &self.mesh_status {
                    ui.painter().text(
                        egui::pos2(viewport_rect.left() + 12.0, viewport_rect.top() + 44.0),
                        Align2::LEFT_TOP,
                        status,
                        FontId::proportional(10.0),
                        Color32::from_gray(190),
                    );
                }
                if self.mesh_loading {
                    let loading_rect = Rect::from_center_size(viewport_rect.center(), egui::vec2(160.0, 30.0));
                    ui.painter().rect_filled(
                        loading_rect,
                        6.0,
                        Color32::from_rgba_unmultiplied(25, 30, 33, 220),
                    );
                    ui.painter().rect_stroke(
                        loading_rect,
                        6.0,
                        Stroke::new(1.0, Color32::from_rgb(72, 92, 96)),
                        egui::StrokeKind::Outside,
                    );
                    ui.scope_builder(
                        egui::UiBuilder::new()
                            .max_rect(loading_rect)
                            .layout(
                                egui::Layout::left_to_right(egui::Align::Center)
                                    .with_main_align(egui::Align::Center),
                            ),
                        |ui| {
                            ui.add(egui::Spinner::new().size(14.0));
                            ui.add_space(8.0);
                            ui.label("Carregando malha...");
                        },
                    );
                }

                let viewport_resp =
                    ui.interact(viewport_rect, ui.id().with("scene_viewport_input"), Sense::click_and_drag());

                let controls_rect = Rect::from_min_max(
                    egui::pos2(viewport_rect.right() - 395.0, viewport_rect.top() + 6.0),
                    egui::pos2(viewport_rect.right() - 8.0, viewport_rect.top() + 32.0),
                );
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(controls_rect)
                        .layout(egui::Layout::right_to_left(egui::Align::Center)),
                    |ui| {
                        let proj_label = if self.is_ortho { "Ortho" } else { "Persp" };
                        if ui
                            .add_sized([74.0, 22.0], egui::Button::new(proj_label).corner_radius(6))
                            .clicked()
                            && self.is_3d
                        {
                            self.is_ortho = !self.is_ortho;
                        }
                        ui.add_space(6.0);

                        let dim_label = if self.is_3d { "3D" } else { "2D" };
                        if ui
                            .add_sized(
                                [52.0, 22.0],
                                egui::Button::new(dim_label)
                                    .corner_radius(6)
                                    .stroke(Stroke::new(1.0, Color32::from_rgb(15, 232, 121))),
                            )
                            .clicked()
                        {
                            self.is_3d = !self.is_3d;
                        }
                        ui.add_space(6.0);

                        let local_selected = self.gizmo_orientation == GizmoOrientation::Local;
                        if ui
                            .add_sized(
                                [58.0, 22.0],
                                egui::Button::new("Local")
                                    .corner_radius(6)
                                    .fill(if local_selected {
                                        Color32::from_rgb(62, 62, 62)
                                    } else {
                                        Color32::from_rgb(44, 44, 44)
                                    })
                                    .stroke(if local_selected {
                                        Stroke::new(1.0, Color32::from_rgb(15, 232, 121))
                                    } else {
                                        Stroke::new(1.0, Color32::from_gray(70))
                                    }),
                            )
                            .clicked()
                        {
                            self.gizmo_orientation = GizmoOrientation::Local;
                        }
                        if ui
                            .add_sized(
                                [62.0, 22.0],
                                egui::Button::new("Global")
                                    .corner_radius(6)
                                    .fill(if !local_selected {
                                        Color32::from_rgb(62, 62, 62)
                                    } else {
                                        Color32::from_rgb(44, 44, 44)
                                    })
                                    .stroke(if !local_selected {
                                        Stroke::new(1.0, Color32::from_rgb(15, 232, 121))
                                    } else {
                                        Stroke::new(1.0, Color32::from_gray(70))
                                    }),
                            )
                            .clicked()
                        {
                            self.gizmo_orientation = GizmoOrientation::Global;
                        }
                        ui.add_space(10.0);

                        if Self::gizmo_icon_button(
                            ui,
                            self.move_icon.as_ref(),
                            "M",
                            self.move_view_mode,
                            "Move View",
                        ) {
                            self.move_view_mode = true;
                        }
                        if Self::gizmo_icon_button(
                            ui,
                            self.transform_icon.as_ref(),
                            "T",
                            self.gizmo_mode == GizmoMode::Translate && !self.move_view_mode,
                            "Transform",
                        ) {
                            self.move_view_mode = false;
                            self.gizmo_mode = GizmoMode::Translate;
                            self.object_selected = true;
                        }
                        if Self::gizmo_icon_button(
                            ui,
                            self.scale_icon.as_ref(),
                            "S",
                            self.gizmo_mode == GizmoMode::Scale && !self.move_view_mode,
                            "Scale",
                        ) {
                            self.move_view_mode = false;
                            self.gizmo_mode = GizmoMode::Scale;
                            self.object_selected = true;
                        }
                        if Self::gizmo_icon_button(
                            ui,
                            self.rotation_icon.as_ref(),
                            "R",
                            self.gizmo_mode == GizmoMode::Rotate && !self.move_view_mode,
                            "Rotation",
                        ) {
                            self.move_view_mode = false;
                            self.gizmo_mode = GizmoMode::Rotate;
                            self.object_selected = true;
                        }
                    },
                );

                ui.painter().text(
                    egui::pos2(viewport_rect.left() + 12.0, viewport_rect.bottom() - 10.0),
                    Align2::LEFT_BOTTOM,
                    "Mouse (Unity): Alt+LMB orbitar | RMB arrastar olhar (camera fixa) | MMB pan | Alt+RMB zoom | Scroll zoom | LMB selecionar | Touchpad: clique selecionar | 2 dedos pan | Pinch zoom | Ctrl+2 dedos orbitar",
                    FontId::proportional(11.0),
                    Color32::from_gray(170),
                );

                if self.is_3d {
                    let pointer_delta = ctx.input(|i| i.pointer.delta());
                    let scroll_delta = ctx.input(|i| i.smooth_scroll_delta);
                    let pinch_zoom = ctx.input(|i| i.zoom_delta());
                    let alt_down = ctx.input(|i| i.modifiers.alt);
                    let ctrl_down = ctx.input(|i| i.modifiers.ctrl);
                    let primary_down = ctx.input(|i| i.pointer.primary_down());
                    let middle_down = ctx.input(|i| i.pointer.middle_down());
                    let secondary_down = ctx.input(|i| i.pointer.secondary_down());
                    let pointer_over_controls = ctx.input(|i| {
                        i.pointer
                            .hover_pos()
                            .is_some_and(|p| controls_rect.contains(p))
                    });
                    let view_gizmo_rect = Rect::from_min_size(
                        egui::pos2(viewport_rect.right() - 66.0, viewport_rect.bottom() - 66.0),
                        egui::vec2(56.0, 56.0),
                    );

                    let aspect = (viewport_rect.width() / viewport_rect.height()).max(0.1);
                    let orbit = Vec3::new(
                        self.camera_yaw.cos() * self.camera_pitch.cos(),
                        self.camera_pitch.sin(),
                        self.camera_yaw.sin() * self.camera_pitch.cos(),
                    );
                    let eye = self.camera_target + orbit * self.camera_distance;
                    let view = Mat4::look_at_rh(eye, self.camera_target, Vec3::Y);
                    let proj = if self.is_ortho {
                        Mat4::orthographic_rh_gl(-2.0 * aspect, 2.0 * aspect, -2.0, 2.0, 0.1, 50.0)
                    } else {
                        Mat4::perspective_rh_gl(45.0_f32.to_radians(), aspect, 0.1, 50.0)
                    };
                    if let Some((next_yaw, next_pitch)) = draw_view_orientation_gizmo(ui, view_gizmo_rect, view) {
                        self.camera_yaw = next_yaw;
                        self.camera_pitch = next_pitch;
                        ui.ctx().request_repaint();
                    }

                    let pointer_over_view_gizmo = ctx.input(|i| {
                        i.pointer
                            .hover_pos()
                            .is_some_and(|p| view_gizmo_rect.contains(p))
                    });
                    let can_navigate_camera =
                        viewport_resp.hovered() && !pointer_over_controls && !pointer_over_view_gizmo;
                    let is_navigating = can_navigate_camera
                        && ((alt_down && primary_down)
                            || (self.move_view_mode && primary_down)
                            || (secondary_down && !alt_down)
                            || middle_down
                            || (alt_down && secondary_down && pointer_delta.y.abs() > 0.0)
                            || scroll_delta.length_sq() > 0.0
                            || (pinch_zoom - 1.0).abs() > 1e-4);

                    if can_navigate_camera && self.move_view_mode {
                        ui.output_mut(|o| {
                            o.cursor_icon = if primary_down {
                                egui::CursorIcon::Grabbing
                            } else {
                                egui::CursorIcon::Grab
                            };
                        });
                    }

                    let key_front =
                        ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Num1));
                    let key_side =
                        ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Num3));
                    let key_top =
                        ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Num7));
                    let key_focus =
                        ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F));
                    if key_front {
                        self.camera_yaw = -std::f32::consts::FRAC_PI_2;
                        self.camera_pitch = 0.0;
                        ui.ctx().request_repaint();
                    }
                    if key_side {
                        self.camera_yaw = 0.0;
                        self.camera_pitch = 0.0;
                        ui.ctx().request_repaint();
                    }
                    if key_top {
                        self.camera_yaw = -std::f32::consts::FRAC_PI_2;
                        self.camera_pitch = 1.45;
                        ui.ctx().request_repaint();
                    }
                    if key_focus && viewport_resp.hovered() && !pointer_over_controls {
                        self.focus_selected_or_origin();
                        ui.ctx().request_repaint();
                    }

                    if can_navigate_camera {
                        if self.move_view_mode && primary_down {
                            let right = Vec3::new(self.camera_yaw.sin(), 0.0, -self.camera_yaw.cos());
                            let up = Vec3::Y;
                            let pan_scale = self.camera_distance * 0.0022;
                            self.camera_target += (-pointer_delta.x * pan_scale) * right;
                            self.camera_target += (pointer_delta.y * pan_scale) * up;
                            ui.ctx().request_repaint();
                        }

                        // Alt + LMB:
                        // - Local: orbit around selected object center.
                        // - Global: keep previous orbit behavior around current target.
                        if alt_down && primary_down && !self.move_view_mode {
                            let pivot_local = if self.gizmo_orientation == GizmoOrientation::Local {
                                self.selected_scene_object.as_ref().and_then(|name| {
                                    self.scene_entries
                                        .iter()
                                        .find(|o| &o.name == name)
                                        .map(Self::scene_entry_world_center)
                                })
                            } else {
                                None
                            };

                            if let Some(pivot) = pivot_local {
                                let old_orbit = Vec3::new(
                                    self.camera_yaw.cos() * self.camera_pitch.cos(),
                                    self.camera_pitch.sin(),
                                    self.camera_yaw.sin() * self.camera_pitch.cos(),
                                );
                                let eye = self.camera_target + old_orbit * self.camera_distance;
                                let pivot_to_eye = eye - pivot;
                                let len = pivot_to_eye.length();
                                if len > 1e-4 {
                                    let dir = pivot_to_eye / len;
                                    let base_yaw = dir.z.atan2(dir.x);
                                    let base_pitch = dir.y.clamp(-1.0, 1.0).asin();
                                    self.camera_yaw = base_yaw + pointer_delta.x * 0.012;
                                    self.camera_pitch =
                                        (base_pitch + pointer_delta.y * 0.009).clamp(-1.45, 1.45);
                                    self.camera_target = pivot;
                                    self.camera_distance = len.clamp(0.8, 80.0);
                                } else {
                                    self.camera_yaw -= pointer_delta.x * 0.012;
                                    self.camera_pitch = (self.camera_pitch + pointer_delta.y * 0.009)
                                        .clamp(-1.45, 1.45);
                                }
                            } else {
                                self.camera_yaw -= pointer_delta.x * 0.012;
                                self.camera_pitch = (self.camera_pitch - pointer_delta.y * 0.009)
                                    .clamp(-1.45, 1.45);
                            }
                            ui.ctx().request_repaint();
                        }

                        // RMB drag:
                        // - Local: orbit around selected object center.
                        // - Global: keep previous free-look behavior (fixed eye).
                        if secondary_down && !alt_down {
                            let pivot_local = if self.gizmo_orientation == GizmoOrientation::Local {
                                self.selected_scene_object.as_ref().and_then(|name| {
                                    self.scene_entries
                                        .iter()
                                        .find(|o| &o.name == name)
                                        .map(Self::scene_entry_world_center)
                                })
                            } else {
                                None
                            };

                            if let Some(pivot) = pivot_local {
                                let old_orbit = Vec3::new(
                                    self.camera_yaw.cos() * self.camera_pitch.cos(),
                                    self.camera_pitch.sin(),
                                    self.camera_yaw.sin() * self.camera_pitch.cos(),
                                );
                                let eye = self.camera_target + old_orbit * self.camera_distance;
                                let pivot_to_eye = eye - pivot;
                                let len = pivot_to_eye.length();
                                if len > 1e-4 {
                                    let dir = pivot_to_eye / len;
                                    let base_yaw = dir.z.atan2(dir.x);
                                    let base_pitch = dir.y.clamp(-1.0, 1.0).asin();
                                    self.camera_yaw = base_yaw + pointer_delta.x * 0.012;
                                    self.camera_pitch =
                                        (base_pitch + pointer_delta.y * 0.009).clamp(-1.45, 1.45);
                                    self.camera_target = pivot;
                                    self.camera_distance = len.clamp(0.8, 80.0);
                                } else {
                                    self.camera_yaw -= pointer_delta.x * 0.012;
                                    self.camera_pitch = (self.camera_pitch + pointer_delta.y * 0.009)
                                        .clamp(-1.45, 1.45);
                                }
                            } else {
                                let old_orbit = Vec3::new(
                                    self.camera_yaw.cos() * self.camera_pitch.cos(),
                                    self.camera_pitch.sin(),
                                    self.camera_yaw.sin() * self.camera_pitch.cos(),
                                );
                                let eye = self.camera_target + old_orbit * self.camera_distance;
                                self.camera_yaw -= pointer_delta.x * 0.012;
                                self.camera_pitch = (self.camera_pitch - pointer_delta.y * 0.009)
                                    .clamp(-1.45, 1.45);
                                let new_orbit = Vec3::new(
                                    self.camera_yaw.cos() * self.camera_pitch.cos(),
                                    self.camera_pitch.sin(),
                                    self.camera_yaw.sin() * self.camera_pitch.cos(),
                                );
                                self.camera_target = eye - new_orbit * self.camera_distance;
                            }
                            ui.ctx().request_repaint();
                        }

                        // Unity-like pan: MMB drag.
                        if middle_down {
                            let right = Vec3::new(self.camera_yaw.sin(), 0.0, -self.camera_yaw.cos());
                            let up = Vec3::Y;
                            let pan_scale = self.camera_distance * 0.002;
                            self.camera_target += (-pointer_delta.x * pan_scale) * right;
                            self.camera_target += (pointer_delta.y * pan_scale) * up;
                            ui.ctx().request_repaint();
                        }

                        // Unity-like dolly: Alt + RMB drag.
                        if alt_down && secondary_down && pointer_delta.y.abs() > 0.0 {
                            self.camera_distance =
                                (self.camera_distance + pointer_delta.y * 0.02).clamp(0.8, 80.0);
                            ui.ctx().request_repaint();
                        }

                        // Scroll zoom (mouse wheel / touchpad scroll).
                        // Quando Ctrl estiver pressionado, o gesto vira orbita (não zoom).
                        if scroll_delta.y.abs() > 0.0 && !ctrl_down {
                            self.camera_distance =
                                (self.camera_distance - scroll_delta.y * 0.01).clamp(0.8, 80.0);
                            ui.ctx().request_repaint();
                        }

                        // Touchpad: dois dedos = pan; Ctrl + dois dedos = orbita.
                        if scroll_delta.length_sq() > 0.0 {
                            if ctrl_down {
                                // Local: orbita em torno do objeto selecionado.
                                // Global: orbita livre mantendo o ponto de vista atual.
                                let pivot_local = if self.gizmo_orientation == GizmoOrientation::Local {
                                    self.selected_scene_object.as_ref().and_then(|name| {
                                        self.scene_entries
                                            .iter()
                                            .find(|o| &o.name == name)
                                            .map(Self::scene_entry_world_center)
                                    })
                                } else {
                                    None
                                };
                                if let Some(pivot) = pivot_local {
                                    let old_orbit = Vec3::new(
                                        self.camera_yaw.cos() * self.camera_pitch.cos(),
                                        self.camera_pitch.sin(),
                                        self.camera_yaw.sin() * self.camera_pitch.cos(),
                                    );
                                    let eye = self.camera_target + old_orbit * self.camera_distance;
                                    let pivot_to_eye = eye - pivot;
                                    let len = pivot_to_eye.length();
                                    if len > 1e-4 {
                                        let dir = pivot_to_eye / len;
                                        let base_yaw = dir.z.atan2(dir.x);
                                        let base_pitch = dir.y.clamp(-1.0, 1.0).asin();
                                        self.camera_yaw = base_yaw + scroll_delta.x * 0.008;
                                        self.camera_pitch =
                                            (base_pitch - scroll_delta.y * 0.006).clamp(-1.45, 1.45);
                                        self.camera_target = pivot;
                                        self.camera_distance = len.clamp(0.8, 80.0);
                                    } else {
                                        self.camera_yaw -= scroll_delta.x * 0.008;
                                        self.camera_pitch =
                                            (self.camera_pitch - scroll_delta.y * 0.006).clamp(-1.45, 1.45);
                                    }
                                } else {
                                    let old_orbit = Vec3::new(
                                        self.camera_yaw.cos() * self.camera_pitch.cos(),
                                        self.camera_pitch.sin(),
                                        self.camera_yaw.sin() * self.camera_pitch.cos(),
                                    );
                                    let eye = self.camera_target + old_orbit * self.camera_distance;
                                    self.camera_yaw -= scroll_delta.x * 0.008;
                                    self.camera_pitch =
                                        (self.camera_pitch - scroll_delta.y * 0.006).clamp(-1.45, 1.45);
                                    let new_orbit = Vec3::new(
                                        self.camera_yaw.cos() * self.camera_pitch.cos(),
                                        self.camera_pitch.sin(),
                                        self.camera_yaw.sin() * self.camera_pitch.cos(),
                                    );
                                    self.camera_target = eye - new_orbit * self.camera_distance;
                                }
                            } else {
                                let right = Vec3::new(self.camera_yaw.sin(), 0.0, -self.camera_yaw.cos());
                                let up = Vec3::Y;
                                let pan_scale = self.camera_distance * 0.0016;
                                self.camera_target += (-scroll_delta.x * pan_scale) * right;
                                self.camera_target += (scroll_delta.y * pan_scale) * up;
                            }
                            ui.ctx().request_repaint();
                        }

                        // Touchpad pinch: aproxima/afasta camera.
                        if (pinch_zoom - 1.0).abs() > 1e-4 {
                            self.camera_distance = (self.camera_distance / pinch_zoom).clamp(0.8, 80.0);
                            ui.ctx().request_repaint();
                        }
                    }

                    if viewport_resp.clicked_by(PointerButton::Primary)
                        && !pointer_over_controls
                        && !pointer_over_view_gizmo
                        && !alt_down
                        && !self.move_view_mode
                    {
                        let hover_pos = ctx.input(|i| i.pointer.hover_pos());
                        if let Some(cursor) = hover_pos {
                            let mut best: Option<(f32, String)> = None;
                            let view_proj = proj * view;
                            for entry in &self.scene_entries {
                                if let Some((screen, radius_px)) =
                                    Self::scene_entry_screen_hit_info(entry, viewport_rect, view_proj)
                                {
                                    let dist = cursor.distance(screen);
                                    if dist <= radius_px {
                                        match &best {
                                            Some((best_d, _)) if dist >= *best_d => {}
                                            _ => best = Some((dist, entry.name.clone())),
                                        }
                                    }
                                }
                            }
                            if let Some((_, name)) = best {
                                self.selected_scene_object = Some(name.clone());
                                self.dropped_asset_label = Some(name);
                                self.object_selected = true;
                            } else {
                                self.selected_scene_object = None;
                                self.object_selected = false;
                            }
                        }
                    }

                    if !self.scene_entries.is_empty() {
                        let use_proxy = is_navigating;
                        let mut gpu_drawn = false;
                        if let Some(gpu) = gpu_renderer {
                            let (scene_batch, texture_conflict) =
                                self.build_gpu_scene_mesh(use_proxy);
                            if !texture_conflict {
                                let mesh_id = self.gpu_scene_mesh_id(use_proxy);
                                let light_dir = Vec3::new(
                                    self.light_yaw.cos() * self.light_pitch.cos(),
                                    self.light_pitch.sin(),
                                    self.light_yaw.sin() * self.light_pitch.cos(),
                                );
                                gpu.update_scene(
                                    mesh_id,
                                    &scene_batch.vertices,
                                    &scene_batch.normals,
                                    &scene_batch.uvs,
                                    &scene_batch.triangles,
                                    proj * view,
                                    Mat4::IDENTITY,
                                    eye,
                                    light_dir,
                                    Vec3::from(self.light_color),
                                    self.light_intensity,
                                    self.light_enabled,
                                    scene_batch.texture_path,
                                );
                                let cb = gpu.paint_callback(viewport_rect);
                                ui.painter().add(egui::Shape::Callback(cb));
                                gpu_drawn = true;
                            }
                        }
                        if !gpu_drawn {
                            for entry in &self.scene_entries {
                                let model = entry.transform;
                                let mvp_obj = proj * view * model;
                                let mesh = if is_navigating {
                                    &entry.proxy
                                } else {
                                    &entry.full
                                };
                                eprintln!("[VIEWPORT] Renderizando: {} (proxy={}), material_path={:?}", entry.name, is_navigating, mesh.material_path);
                                draw_solid_mesh(
                                    ui,
                                    viewport_rect,
                                    mvp_obj,
                                    mesh,
                                    &mut self.texture_cache,
                                );
                            }
                        }
                        for entry in &self.scene_entries {
                            let model = entry.transform;
                            let mvp_obj = proj * view * model;
                            let selected = self
                                .selected_scene_object
                                .as_ref()
                                .is_some_and(|name| name == &entry.name);
                            if selected {
                                draw_mesh_silhouette(
                                    ui,
                                    viewport_rect,
                                    mvp_obj,
                                    view * model,
                                    &entry.proxy,
                                );
                            }
                        }
                    }

                    if self.object_selected {
                        let selected_name = self.selected_scene_object.clone();
                        let selected_transform = selected_name
                            .as_ref()
                            .and_then(|name| {
                                self.scene_entries
                                    .iter()
                                    .find(|o| &o.name == name)
                                    .map(|o| o.transform)
                            })
                            .unwrap_or(self.model_matrix);
                        let gizmo = Gizmo::new("scene_transform_gizmo")
                            .view_matrix(view.to_cols_array_2d().into())
                            .projection_matrix(proj.to_cols_array_2d().into())
                            .model_matrix(selected_transform.to_cols_array_2d().into())
                            .mode(self.gizmo_mode)
                            .orientation(self.gizmo_orientation)
                            .viewport(viewport_rect);

                        let gizmo_result = gizmo.interact(ui);
                        let interacting = gizmo_result.is_some();
                        if interacting && !self.gizmo_interacting {
                            self.pending_gizmo_undo = true;
                        }
                        self.gizmo_interacting = interacting;
                        if !interacting {
                            self.pending_gizmo_undo = false;
                        }

                        if let Some(result) = gizmo_result {
                            let new_transform = Mat4::from(result.transform());
                            if let Some(name) = selected_name {
                                if let Some(idx) = self.scene_entries.iter().position(|o| o.name == name) {
                                    let old = self.scene_entries[idx].transform;
                                    if old != new_transform {
                                        if self.pending_gizmo_undo {
                                            self.push_undo_snapshot();
                                            self.pending_gizmo_undo = false;
                                        }
                                        self.scene_entries[idx].transform = new_transform;
                                    }
                                }
                            } else {
                                self.model_matrix = new_transform;
                            }
                        }
                    } else {
                        self.gizmo_interacting = false;
                        self.pending_gizmo_undo = false;
                    }
                }
            });
    }

    pub fn object_texture_path(&self, object_name: &str) -> Option<String> {
        self.scene_entries
            .iter()
            .find(|e| e.name == object_name)
            .and_then(|e| e.full.texture_path.clone())
    }

    pub fn set_object_texture_path(&mut self, object_name: &str, path: Option<String>) -> bool {
        if let Some(entry) = self
            .scene_entries
            .iter_mut()
            .find(|e| e.name == object_name)
        {
            entry.full.texture_path = path;
            true
        } else {
            false
        }
    }

    pub fn set_object_material_path(&mut self, object_name: &str, path: Option<String>) -> bool {
        eprintln!(
            "[VIEWPORT] set_object_material_path: objeto={}, path={:?}",
            object_name, path
        );
        if let Some(entry) = self
            .scene_entries
            .iter_mut()
            .find(|e| e.name == object_name)
        {
            eprintln!(
                "[VIEWPORT] Material definido: {:?} -> {:?}",
                entry.full.material_path, path
            );
            entry.full.material_path = path.clone();
            // Also update proxy mesh
            entry.proxy.material_path = path;
            true
        } else {
            eprintln!("[VIEWPORT] Objeto nao encontrado: {}", object_name);
            false
        }
    }
}

fn load_png_as_texture(ctx: &egui::Context, png_path: &str) -> Option<TextureHandle> {
    let bytes = std::fs::read(png_path).ok()?;
    let rgba = image::load_from_memory(&bytes).ok()?.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let color_image = ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
    Some(ctx.load_texture(png_path.to_owned(), color_image, TextureOptions::LINEAR))
}

fn load_mesh_from_path(path: &Path) -> Result<MeshData, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .ok_or_else(|| "extensão inválida".to_string())?;

    let mut mesh = match ext.as_str() {
        "fbx" => load_fbx_ascii_mesh(path)?,
        "obj" => load_obj_mesh(path)?,
        "glb" | "gltf" => load_gltf_mesh(path)?,
        _ => return Err("formato não suportado".to_string()),
    };
    normalize_mesh(&mut mesh);
    if mesh.triangles.len() > MAX_PARSED_TRIANGLES || mesh.vertices.len() > MAX_PARSED_VERTICES {
        return Err("malha excede limite de complexidade para importacao".to_string());
    }
    Ok(mesh)
}

fn make_proxy_mesh(full: &MeshData, max_tris: usize, max_vertices: usize) -> MeshData {
    if full.triangles.is_empty() || full.vertices.is_empty() {
        return MeshData {
            name: format!("{} [proxy]", full.name),
            vertices: vec![Vec3::ZERO, Vec3::X * 0.2, Vec3::Y * 0.2],
            normals: vec![Vec3::Y; 3],
            uvs: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
            triangles: vec![[0, 1, 2]],
            texture_path: full.texture_path.clone(),
            material_path: full.material_path.clone(),
        };
    }

    let positions: Vec<[f32; 3]> = full.vertices.iter().map(|v| [v.x, v.y, v.z]).collect();
    let src_indices: Vec<u32> = full
        .triangles
        .iter()
        .flat_map(|t| [t[0], t[1], t[2]])
        .collect();

    let target_index_count = (max_tris.saturating_mul(3)).min(src_indices.len()).max(3);
    let simplified = meshopt::simplify_decoder(
        &src_indices,
        &positions,
        target_index_count,
        0.02,
        meshopt::SimplifyOptions::Sparse,
        None,
    );

    let mut optimized_indices = if simplified.len() >= 3 {
        meshopt::optimize_vertex_cache(&simplified, positions.len())
    } else {
        Vec::new()
    };
    let mut working_vertices = positions.clone();
    let compact_vertices =
        meshopt::optimize_vertex_fetch(&mut optimized_indices, &working_vertices);
    working_vertices.clear();

    let vertices: Vec<Vec3> = compact_vertices
        .iter()
        .take(max_vertices)
        .map(|v| Vec3::new(v[0], v[1], v[2]))
        .collect();
    let mut triangles: Vec<[u32; 3]> = Vec::with_capacity(optimized_indices.len() / 3);
    for tri in optimized_indices.chunks_exact(3) {
        if (tri[0] as usize) < vertices.len()
            && (tri[1] as usize) < vertices.len()
            && (tri[2] as usize) < vertices.len()
        {
            triangles.push([tri[0], tri[1], tri[2]]);
        }
        if triangles.len() >= max_tris {
            break;
        }
    }

    if triangles.is_empty() || vertices.is_empty() {
        return MeshData {
            name: format!("{} [proxy]", full.name),
            vertices: vec![Vec3::ZERO, Vec3::X * 0.2, Vec3::Y * 0.2],
            normals: vec![Vec3::Y; 3],
            uvs: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
            triangles: vec![[0, 1, 2]],
            texture_path: full.texture_path.clone(),
            material_path: full.material_path.clone(),
        };
    }

    let mut res = MeshData {
        name: if triangles.len() < full.triangles.len() || vertices.len() < full.vertices.len() {
            format!("{} [proxy]", full.name)
        } else {
            full.name.clone()
        },
        vertices,
        normals: vec![], // Será recalculado
        uvs: vec![],     // Será copiado abaixo
        triangles,
        texture_path: full.texture_path.clone(),
        material_path: full.material_path.clone(),
    };

    // Copiar UVs do mesh original
    // Como os vértices foram simplificados, precisamos mapear as UVs
    // Abordagem simples: usar UVs dos vértices originais baseado na posição
    if !full.uvs.is_empty() && full.vertices.len() == res.vertices.len() {
        // Se o número de vértices é o mesmo, copiar diretamente
        res.uvs = full.uvs.clone();
    } else if !full.uvs.is_empty() {
        // Mapear UVs baseado no índice original (aproximação)
        // Para proxy de navegação, UVs não são críticas
        res.uvs = vec![[0.0, 0.0]; res.vertices.len()];
    }

    normalize_mesh(&mut res); // Garante normais
    res
}

fn make_primitive_mesh(kind: Primitive3DKind) -> MeshData {
    let mut mesh = match kind {
        Primitive3DKind::Cube => make_cube_mesh(),
        Primitive3DKind::Sphere => make_sphere_mesh(14, 20),
        Primitive3DKind::Cone => make_cone_mesh(24),
        Primitive3DKind::Cylinder => make_cylinder_mesh(24),
        Primitive3DKind::Plane => make_plane_mesh(),
    };
    normalize_mesh(&mut mesh);
    mesh
}

fn make_light_mesh(light_type: inspector::LightType) -> MeshData {
    let mut mesh = match light_type {
        inspector::LightType::Point => make_sphere_mesh(14, 24),
        inspector::LightType::Spot => make_cone_mesh(28),
        inspector::LightType::Directional => {
            let mut plane = make_plane_mesh();
            for v in &mut plane.vertices {
                *v *= 0.6;
            }
            plane
        }
    };
    mesh.name = format!("{} Indicator", light_type.as_str());
    normalize_mesh(&mut mesh);
    mesh
}

fn make_cube_mesh() -> MeshData {
    let h = 0.5_f32;
    let mut vertices = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut triangles = Vec::new();

    // Dados para um cubo de 24 vértices (6 faces * 4 vértices)
    let face_normals = [
        Vec3::new(0.0, 0.0, 1.0),  // Frente
        Vec3::new(0.0, 0.0, -1.0), // Trás
        Vec3::new(0.0, 1.0, 0.0),  // Cima
        Vec3::new(0.0, -1.0, 0.0), // Baixo
        Vec3::new(1.0, 0.0, 0.0),  // Direita
        Vec3::new(-1.0, 0.0, 0.0), // Esquerda
    ];

    let face_vertices = [
        // Frente
        [
            Vec3::new(-h, -h, h),
            Vec3::new(h, -h, h),
            Vec3::new(h, h, h),
            Vec3::new(-h, h, h),
        ],
        // Trás
        [
            Vec3::new(h, -h, -h),
            Vec3::new(-h, -h, -h),
            Vec3::new(-h, h, -h),
            Vec3::new(h, h, -h),
        ],
        // Cima
        [
            Vec3::new(-h, h, h),
            Vec3::new(h, h, h),
            Vec3::new(h, h, -h),
            Vec3::new(-h, h, -h),
        ],
        // Baixo
        [
            Vec3::new(-h, -h, -h),
            Vec3::new(h, -h, -h),
            Vec3::new(h, -h, h),
            Vec3::new(-h, -h, h),
        ],
        // Direita
        [
            Vec3::new(h, -h, h),
            Vec3::new(h, -h, -h),
            Vec3::new(h, h, -h),
            Vec3::new(h, h, h),
        ],
        // Esquerda
        [
            Vec3::new(-h, -h, -h),
            Vec3::new(-h, -h, h),
            Vec3::new(-h, h, h),
            Vec3::new(-h, h, -h),
        ],
    ];

    for i in 0..6 {
        let base = vertices.len() as u32;
        vertices.extend(&face_vertices[i]);
        for _ in 0..4 {
            normals.push(face_normals[i]);
        }
        uvs.extend(&[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
        triangles.push([base, base + 1, base + 2]);
        triangles.push([base, base + 2, base + 3]);
    }

    MeshData {
        name: "Cube".to_string(),
        vertices,
        normals,
        uvs,
        triangles,
        texture_path: None,
        material_path: None,
    }
}

fn make_plane_mesh() -> MeshData {
    let h = 0.5_f32;
    let vertices = vec![
        Vec3::new(-h, 0.0, -h),
        Vec3::new(h, 0.0, -h),
        Vec3::new(h, 0.0, h),
        Vec3::new(-h, 0.0, h),
    ];
    let triangles = vec![[0, 1, 2], [0, 2, 3]];
    let v_count = vertices.len();
    MeshData {
        name: "Plane".to_string(),
        vertices,
        normals: vec![Vec3::Y; v_count],
        uvs: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        triangles,
        texture_path: None,
        material_path: None,
    }
}

fn make_cone_mesh(segments: usize) -> MeshData {
    let seg = segments.max(8);
    let r = 0.5_f32;
    let half_h = 0.5_f32;
    let mut vertices = Vec::with_capacity(seg + 2);
    vertices.push(Vec3::new(0.0, half_h, 0.0)); // apex = 0
    for i in 0..seg {
        let a = (i as f32 / seg as f32) * std::f32::consts::TAU;
        vertices.push(Vec3::new(a.cos() * r, -half_h, a.sin() * r));
    }
    vertices.push(Vec3::new(0.0, -half_h, 0.0)); // base center
    let base_center = (vertices.len() - 1) as u32;

    let mut triangles = Vec::with_capacity(seg * 2);
    for i in 0..seg {
        let a = (1 + i) as u32;
        let b = (1 + ((i + 1) % seg)) as u32;
        triangles.push([0, a, b]); // CCW: Apex, Current, Next
        triangles.push([base_center, b, a]); // CCW: Center, Next, Current
    }
    let vcount = vertices.len();
    MeshData {
        name: "Cone".to_string(),
        vertices,
        normals: vec![], // Será computado
        uvs: vec![[0.0, 0.0]; vcount],
        triangles,
        texture_path: None,
        material_path: None,
    }
}

fn make_cylinder_mesh(segments: usize) -> MeshData {
    let seg = segments.max(8);
    let r = 0.5_f32;
    let half_h = 0.5_f32;
    let mut vertices = Vec::with_capacity(seg * 2 + 2);
    for i in 0..seg {
        let a = (i as f32 / seg as f32) * std::f32::consts::TAU;
        vertices.push(Vec3::new(a.cos() * r, -half_h, a.sin() * r)); // bottom ring
    }
    for i in 0..seg {
        let a = (i as f32 / seg as f32) * std::f32::consts::TAU;
        vertices.push(Vec3::new(a.cos() * r, half_h, a.sin() * r)); // top ring
    }
    vertices.push(Vec3::new(0.0, -half_h, 0.0));
    vertices.push(Vec3::new(0.0, half_h, 0.0));
    let bottom_center = (seg * 2) as u32;
    let top_center = (seg * 2 + 1) as u32;

    let mut triangles = Vec::with_capacity(seg * 4);
    for i in 0..seg {
        let n = (i + 1) % seg;
        let b0 = i as u32;
        let b1 = n as u32;
        let t0 = (i + seg) as u32;
        let t1 = (n + seg) as u32;
        triangles.push([b0, b1, t1]);
        triangles.push([b0, t1, t0]);
        triangles.push([bottom_center, b0, b1]);
        triangles.push([top_center, t1, t0]);
    }
    let vcount = vertices.len();
    MeshData {
        name: "Cylinder".to_string(),
        vertices,
        normals: vec![], // Será computado
        uvs: vec![[0.0, 0.0]; vcount],
        triangles,
        texture_path: None,
        material_path: None,
    }
}

fn make_sphere_mesh(stacks: usize, slices: usize) -> MeshData {
    let st = stacks.max(6);
    let sl = slices.max(8);
    let r = 0.5_f32;
    let mut vertices = Vec::with_capacity((st + 1) * (sl + 1));
    for i in 0..=st {
        let v = i as f32 / st as f32;
        let phi = v * std::f32::consts::PI;
        let y = (phi.cos()) * r;
        let ring_r = (phi.sin()) * r;
        for j in 0..=sl {
            let u = j as f32 / sl as f32;
            let theta = u * std::f32::consts::TAU;
            vertices.push(Vec3::new(theta.cos() * ring_r, y, theta.sin() * ring_r));
        }
    }

    let cols = sl + 1;
    let mut triangles = Vec::with_capacity(st * sl * 2);
    for i in 0..st {
        for j in 0..sl {
            let a = (i * cols + j) as u32;
            let b = (i * cols + j + 1) as u32;
            let c = ((i + 1) * cols + j) as u32;
            let d = ((i + 1) * cols + j + 1) as u32;
            triangles.push([a, d, c]); // CCW: TopLeft, BottomRight, BottomLeft
            triangles.push([a, b, d]); // CCW: TopLeft, TopRight, BottomRight
        }
    }
    let vcount = vertices.len();
    MeshData {
        name: "Sphere".to_string(),
        vertices,
        normals: vec![], // Será computado
        uvs: vec![[0.0, 0.0]; vcount],
        triangles,
        texture_path: None,
        material_path: None,
    }
}

fn load_viewport_mesh_asset_cached(path: &Path) -> Result<ViewportMeshAsset, String> {
    let stamp = source_stamp(path).unwrap_or((0, 0));
    if let Some(asset) = read_vmesh_cache(path, stamp).ok().flatten() {
        return Ok(asset);
    }

    let mut full = load_mesh_from_path(path)?;
    if full.material_path.is_none() {
        let mut candidates = Vec::new();
        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
            candidates.push(file_name.to_string());
        }
        if let Some(file_stem) = path.file_stem().and_then(|n| n.to_str()) {
            candidates.push(file_stem.to_string());
        }
        if let Some(mat_path) = find_material_path_for_names(candidates.iter()) {
            full.material_path = Some(mat_path);
        }
    }
    let proxy = make_proxy_mesh(&full, VIEWPORT_PROXY_TRIANGLES, VIEWPORT_PROXY_VERTICES);
    let asset = ViewportMeshAsset { full, proxy };
    let _ = write_vmesh_cache(path, &asset, stamp);
    Ok(asset)
}

fn source_stamp(path: &Path) -> Result<(u64, u64), String> {
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

fn viewport_cache_file_path(source: &Path) -> Result<std::path::PathBuf, String> {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    source.to_string_lossy().hash(&mut hasher);
    let key = hasher.finish();
    let cache_dir = Path::new("Assets").join(".cache").join("viewport_meshes");
    fs::create_dir_all(&cache_dir).map_err(|e| e.to_string())?;
    Ok(cache_dir.join(format!("{key:016x}.vmesh")))
}

fn write_mesh_blob(f: &mut File, mesh: &MeshData) -> Result<(), String> {
    let vcount = mesh.vertices.len() as u32;
    let tcount = mesh.triangles.len() as u32;
    f.write_all(&vcount.to_le_bytes())
        .map_err(|e| e.to_string())?;
    f.write_all(&tcount.to_le_bytes())
        .map_err(|e| e.to_string())?;
    for v in &mesh.vertices {
        f.write_all(&v.x.to_le_bytes()).map_err(|e| e.to_string())?;
        f.write_all(&v.y.to_le_bytes()).map_err(|e| e.to_string())?;
        f.write_all(&v.z.to_le_bytes()).map_err(|e| e.to_string())?;
    }
    for n in &mesh.normals {
        f.write_all(&n.x.to_le_bytes()).map_err(|e| e.to_string())?;
        f.write_all(&n.y.to_le_bytes()).map_err(|e| e.to_string())?;
        f.write_all(&n.z.to_le_bytes()).map_err(|e| e.to_string())?;
    }
    for tri in &mesh.triangles {
        f.write_all(&tri[0].to_le_bytes())
            .map_err(|e| e.to_string())?;
        f.write_all(&tri[1].to_le_bytes())
            .map_err(|e| e.to_string())?;
        f.write_all(&tri[2].to_le_bytes())
            .map_err(|e| e.to_string())?;
    }
    let uv_count = mesh.uvs.len() as u32;
    f.write_all(&uv_count.to_le_bytes())
        .map_err(|e| e.to_string())?;
    for uv in &mesh.uvs {
        f.write_all(&uv[0].to_le_bytes())
            .map_err(|e| e.to_string())?;
        f.write_all(&uv[1].to_le_bytes())
            .map_err(|e| e.to_string())?;
    }
    write_optional_string(f, mesh.texture_path.as_ref())?;
    write_optional_string(f, mesh.material_path.as_ref())?;
    Ok(())
}

fn read_mesh_blob(f: &mut File, name: &str) -> Result<MeshData, String> {
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
        vertices.push(Vec3::new(x, y, z));
    }
    let mut normals = Vec::with_capacity(vcount);
    for _ in 0..vcount {
        let mut fb = [0_u8; 4];
        f.read_exact(&mut fb).map_err(|e| e.to_string())?;
        let x = f32::from_le_bytes(fb);
        f.read_exact(&mut fb).map_err(|e| e.to_string())?;
        let y = f32::from_le_bytes(fb);
        f.read_exact(&mut fb).map_err(|e| e.to_string())?;
        let z = f32::from_le_bytes(fb);
        normals.push(Vec3::new(x, y, z));
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
    f.read_exact(&mut buf4).map_err(|e| e.to_string())?;
    let uv_count = u32::from_le_bytes(buf4) as usize;
    let mut uvs = Vec::with_capacity(uv_count);
    for _ in 0..uv_count {
        let mut fb = [0_u8; 4];
        f.read_exact(&mut fb).map_err(|e| e.to_string())?;
        let u = f32::from_le_bytes(fb);
        f.read_exact(&mut fb).map_err(|e| e.to_string())?;
        let v = f32::from_le_bytes(fb);
        uvs.push([u, v]);
    }
    let texture_path = read_optional_string(f)?;
    let material_path = read_optional_string(f)?;
    Ok(MeshData {
        name: name.to_string(),
        vertices,
        normals,
        uvs,
        triangles,
        texture_path,
        material_path,
    })
}

fn write_optional_string(f: &mut File, value: Option<&String>) -> Result<(), String> {
    let bytes = value.map(|v| v.as_bytes());
    let len = bytes.map(|b| b.len() as u32).unwrap_or(0);
    f.write_all(&len.to_le_bytes()).map_err(|e| e.to_string())?;
    if let Some(data) = bytes {
        f.write_all(data).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn read_optional_string(f: &mut File) -> Result<Option<String>, String> {
    let mut len_buf = [0_u8; 4];
    f.read_exact(&mut len_buf).map_err(|e| e.to_string())?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len == 0 {
        return Ok(None);
    }
    let mut buf = vec![0_u8; len];
    f.read_exact(&mut buf).map_err(|e| e.to_string())?;
    String::from_utf8(buf).map(Some).map_err(|e| e.to_string())
}

fn write_vmesh_cache(
    source: &Path,
    asset: &ViewportMeshAsset,
    stamp: (u64, u64),
) -> Result<(), String> {
    let cache = viewport_cache_file_path(source)?;
    let mut f = File::create(cache).map_err(|e| e.to_string())?;
    f.write_all(b"VMSH4").map_err(|e| e.to_string())?;
    f.write_all(&stamp.0.to_le_bytes())
        .map_err(|e| e.to_string())?;
    f.write_all(&stamp.1.to_le_bytes())
        .map_err(|e| e.to_string())?;
    write_mesh_blob(&mut f, &asset.full)?;
    write_mesh_blob(&mut f, &asset.proxy)?;
    Ok(())
}

fn read_vmesh_cache(source: &Path, stamp: (u64, u64)) -> Result<Option<ViewportMeshAsset>, String> {
    let cache = viewport_cache_file_path(source)?;
    if !cache.exists() {
        return Ok(None);
    }
    let mut f = File::open(cache).map_err(|e| e.to_string())?;
    let mut magic = [0_u8; 5];
    f.read_exact(&mut magic).map_err(|e| e.to_string())?;
    if &magic != b"VMSH4" {
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

    let name = source
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("Mesh")
        .to_string();
    let full = read_mesh_blob(&mut f, &name)?;
    let proxy = read_mesh_blob(&mut f, &format!("{name} [proxy]"))?;
    Ok(Some(ViewportMeshAsset { full, proxy }))
}

fn load_fbx_ascii_mesh(path: &Path) -> Result<MeshData, String> {
    use fbxcel_dom::any::AnyDocument;
    use fbxcel_dom::v7400::object::{TypedObjectHandle, geometry::TypedGeometryHandle};
    use std::io::BufReader;

    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let reader = BufReader::new(file);
    let doc = match AnyDocument::from_seekable_reader(reader).map_err(|e| e.to_string())? {
        AnyDocument::V7400(_, doc) => doc,
        _ => return Err("versão FBX não suportada".to_string()),
    };

    let mut vertices: Vec<Vec3> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut triangles: Vec<[u32; 3]> = Vec::new();
    let mut texture_path: Option<String> = None;

    // Try to find a texture path in the FBX document
    for obj in doc.objects() {
        if let TypedObjectHandle::Texture(tex) = obj.get_typed() {
            if let Some(filename) = tex.name() {
                // Resolve path relative to the FBX file
                let fbx_dir = path.parent().unwrap_or(Path::new(""));
                let full_path = fbx_dir.join(filename);
                texture_path = Some(normalize_path_string(&full_path.to_string_lossy()));
                break; // Take the first texture found for simplicity
            }
        }
    }

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
        // FBX forward correction: align imported meshes to editor forward (+Z).
        vertices.extend(
            cps.iter()
                .map(|p| Vec3::new(-(p.x as f32), p.y as f32, -(p.z as f32))),
        );

        // FBX UVs: fbxcel_dom has limited UV extraction support
        // For now, use placeholder UVs - FBX files with textures should use GLTF/GLB instead
        // TODO: Implement proper UV extraction using fbxcel crate's layer element API
        uvs.resize(vertices.len(), [0.0, 0.0]);

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

    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("FBX")
        .to_string();
    Ok(MeshData {
        name,
        vertices,
        normals: vec![], // Será computado
        uvs,
        triangles,
        texture_path,
        material_path: None,
    })
}

fn load_obj_mesh(path: &Path) -> Result<MeshData, String> {
    let opt = tobj::LoadOptions {
        triangulate: true,
        single_index: true,
        ..Default::default()
    };
    let (models, materials_result) = tobj::load_obj(path, &opt).map_err(|e| e.to_string())?;
    let materials = materials_result.map_err(|e| e.to_string())?;

    // Prepend the directory of the obj file to material texture paths
    let obj_dir = path.parent().unwrap_or(Path::new(""));
    let mut materials = materials;
    for material in &mut materials {
        if let Some(ref mut tex_path) = material.diffuse_texture {
            let full_path = obj_dir.join(&*tex_path);
            *tex_path = normalize_path_string(&full_path.to_string_lossy());
        }
    }

    let mut vertices = Vec::new();
    let mut uvs = Vec::new();
    let mut triangles = Vec::new();
    let mut texture_path: Option<String> = None;

    for m in models {
        let base = vertices.len() as u32;
        let mesh = m.mesh;
        for p in mesh.positions.chunks_exact(3) {
            vertices.push(Vec3::new(p[0], p[1], p[2]));
        }
        // Load UVs if available
        if let Some(uv_data) = mesh.texcoords.get(..(mesh.positions.len() / 3 * 2)) {
            for uv in uv_data.chunks_exact(2) {
                uvs.push([uv[0], uv[1]]);
            }
        }
        for idx in mesh.indices.chunks_exact(3) {
            triangles.push([base + idx[0], base + idx[1], base + idx[2]]);
        }

        // Extract texture path from material
        if let Some(material_id) = mesh.material_id {
            if let Some(material) = materials.get(material_id) {
                if let Some(tex) = &material.diffuse_texture {
                    texture_path = Some(tex.clone());
                }
            }
        }
    }
    if vertices.is_empty() || triangles.is_empty() {
        return Err("OBJ sem vértices/triângulos".to_string());
    }
    // Pad UVs if needed
    while uvs.len() < vertices.len() {
        uvs.push([0.0, 0.0]);
    }
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("OBJ")
        .to_string();
    Ok(MeshData {
        name,
        vertices,
        normals: vec![], // Será computado
        uvs,
        triangles,
        texture_path,
        material_path: None,
    })
}

fn load_gltf_mesh(path: &Path) -> Result<MeshData, String> {
    let gltf = gltf::Gltf::open(path).map_err(|e| e.to_string())?;
    let buffers = load_gltf_buffers_mesh_only(path, &gltf)?;
    let mut vertices = Vec::new();
    let mut uvs = Vec::new();
    let mut triangles = Vec::new();
    let mut texture_path: Option<String> = None;

    let gltf_dir = path.parent().unwrap_or(Path::new(""));

    if let Some(scene) = gltf
        .document
        .default_scene()
        .or_else(|| gltf.document.scenes().next())
    {
        for node in scene.nodes() {
            append_gltf_node_meshes(
                node,
                Mat4::IDENTITY,
                &buffers,
                &mut vertices,
                &mut uvs,
                &mut triangles,
                &mut texture_path,
                gltf_dir,
            );
        }
    } else {
        for node in gltf.document.nodes() {
            append_gltf_node_meshes(
                node,
                Mat4::IDENTITY,
                &buffers,
                &mut vertices,
                &mut uvs,
                &mut triangles,
                &mut texture_path,
                gltf_dir,
            );
        }
    }

    if vertices.is_empty() || triangles.is_empty() {
        return Err("GLTF/GLB sem triângulos suportados".to_string());
    }
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("GLB")
        .to_string();
    Ok(MeshData {
        name,
        vertices,
        normals: vec![], // Será computado
        uvs,
        triangles,
        texture_path,
        material_path: None,
    })
}

fn append_gltf_node_meshes(
    node: gltf::Node<'_>,
    parent: Mat4,
    buffers: &[Vec<u8>],
    vertices: &mut Vec<Vec3>,
    uvs: &mut Vec<[f32; 2]>,
    triangles: &mut Vec<[u32; 3]>,
    texture_path: &mut Option<String>,
    gltf_dir: &Path,
) {
    let local = Mat4::from_cols_array_2d(&node.transform().matrix());
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

            // Extract texture path if not already found
            if texture_path.is_none() {
                let material = primitive.material();
                if let Some(info) = material.pbr_metallic_roughness().base_color_texture() {
                    let tex = info.texture();
                    match tex.source().source() {
                        gltf::image::Source::Uri { uri, .. } => {
                            let full_path = gltf_dir.join(uri);
                            *texture_path =
                                Some(normalize_path_string(&full_path.to_string_lossy()));
                        }
                        gltf::image::Source::View { view, mime_type } => {
                            let buffer_index = view.buffer().index();
                            if let Some(buffer_data) = buffers.get(buffer_index) {
                                let start = view.offset();
                                let end = start + view.length();
                                if end <= buffer_data.len() {
                                    let content = &buffer_data[start..end];
                                    let mut hasher =
                                        std::collections::hash_map::DefaultHasher::new();
                                    content.hash(&mut hasher);
                                    let h = hasher.finish();
                                    let ext = match mime_type {
                                        "image/jpeg" => "jpg",
                                        "image/png" => "png",
                                        _ => "bin",
                                    };
                                    let cache_dir =
                                        Path::new("Assets").join(".cache").join("textures");
                                    if let Ok(_) = std::fs::create_dir_all(&cache_dir) {
                                        let filename = format!("{:016x}.{}", h, ext);
                                        let file_path = cache_dir.join(filename);
                                        if !file_path.exists() {
                                            let _ = std::fs::write(&file_path, content);
                                        }
                                        if let Ok(abs) = std::fs::canonicalize(&file_path) {
                                            *texture_path =
                                                Some(normalize_path_string(&abs.to_string_lossy()));
                                        } else {
                                            // Fallback if canonicalize fails (e.g. file creation failed)
                                            *texture_path = Some(normalize_path_string(
                                                &file_path.to_string_lossy(),
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            let base = vertices.len() as u32;
            let positions_vec: Vec<_> = positions.collect();
            let positions_len = positions_vec.len();
            vertices.extend(
                positions_vec
                    .iter()
                    .map(|p| world.transform_point3(Vec3::new(p[0], p[1], p[2]))),
            );

            // Collect UVs
            if let Some(reader_uvs) = reader.read_tex_coords(0) {
                let uv_data: Vec<[f32; 2]> =
                    reader_uvs.into_f32().map(|uv| [uv[0], uv[1]]).collect();
                uvs.extend(uv_data);
            } else {
                // If no UVs, fill with zeros for each vertex
                for _ in 0..positions_len {
                    uvs.push([0.0, 0.0]);
                }
            }

            if let Some(indices) = reader.read_indices() {
                for i in indices.into_u32().collect::<Vec<u32>>().chunks_exact(3) {
                    triangles.push([base + i[0], base + i[1], base + i[2]]);
                }
            } else {
                // If no indices, generate a triangle fan
                let mut i = 0;
                while i + 2 < positions_len as u32 {
                    triangles.push([base, base + i + 1, base + i + 2]);
                    i += 1;
                }
            }
        }
    }

    // Process children
    for child in node.children() {
        append_gltf_node_meshes(
            child,
            world,
            buffers,
            vertices,
            uvs,
            triangles,
            texture_path,
            gltf_dir,
        );
    }
}

fn load_gltf_buffers_mesh_only(path: &Path, gltf: &gltf::Gltf) -> Result<Vec<Vec<u8>>, String> {
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
                    return Err("GLTF com data-uri não suportado no modo mesh-only".to_string());
                }
                let p = base_dir.join(uri);
                let bytes = fs::read(&p)
                    .map_err(|e| format!("falha ao ler buffer GLTF '{}': {e}", p.display()))?;
                out.push(bytes);
            }
        }
    }
    Ok(out)
}

fn normalize_mesh(mesh: &mut MeshData) {
    if mesh.vertices.is_empty() {
        return;
    }

    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for v in &mesh.vertices {
        min = min.min(*v);
        max = max.max(*v);
    }
    let center = (min + max) * 0.5;
    let extents = (max - min).max(Vec3::splat(1e-5));
    let longest = extents.x.max(extents.y).max(extents.z);
    let scale = if longest > 0.0 { 1.1 / longest } else { 1.0 };
    for v in &mut mesh.vertices {
        *v = (*v - center) * scale;
    }

    // Calcular normais suaves
    mesh.normals = vec![Vec3::ZERO; mesh.vertices.len()];
    for tri in &mesh.triangles {
        let i0 = tri[0] as usize;
        let i1 = tri[1] as usize;
        let i2 = tri[2] as usize;
        if i0 < mesh.vertices.len() && i1 < mesh.vertices.len() && i2 < mesh.vertices.len() {
            let p0 = mesh.vertices[i0];
            let p1 = mesh.vertices[i1];
            let p2 = mesh.vertices[i2];
            let n = (p1 - p0).cross(p2 - p0);
            mesh.normals[i0] += n;
            mesh.normals[i1] += n;
            mesh.normals[i2] += n;
        }
    }
    for n in &mut mesh.normals {
        *n = n.normalize_or_zero();
        if n.length_squared() < 0.01 {
            *n = Vec3::Y;
        }
    }
}

fn draw_view_orientation_gizmo(ui: &mut egui::Ui, rect: Rect, view: Mat4) -> Option<(f32, f32)> {
    let id = ui.id().with("viewport_view_orientation_gizmo");
    let resp = ui.interact(rect, id, Sense::click());
    let painter = ui.painter();
    let center = rect.center();
    let radius = rect.width().min(rect.height()) * 0.5;

    painter.circle_filled(
        center,
        radius,
        Color32::from_rgba_unmultiplied(28, 31, 36, if resp.hovered() { 230 } else { 205 }),
    );
    painter.circle_stroke(
        center,
        radius,
        Stroke::new(1.0, Color32::from_rgb(74, 82, 95)),
    );

    let axes = [
        (
            Vec3::X,
            Color32::from_rgb(228, 78, 88),
            0.0_f32,
            0.0_f32,
            Some("X"),
        ),
        (
            Vec3::NEG_X,
            Color32::from_rgb(124, 50, 57),
            std::f32::consts::PI,
            0.0_f32,
            None,
        ),
        (
            Vec3::Y,
            Color32::from_rgb(98, 206, 110),
            0.0_f32,
            1.45_f32,
            Some("Y"),
        ),
        (
            Vec3::NEG_Y,
            Color32::from_rgb(54, 110, 62),
            0.0_f32,
            -1.45_f32,
            None,
        ),
        (
            Vec3::Z,
            Color32::from_rgb(84, 153, 236),
            std::f32::consts::FRAC_PI_2,
            0.0_f32,
            Some("Z"),
        ),
        (
            Vec3::NEG_Z,
            Color32::from_rgb(52, 92, 138),
            -std::f32::consts::FRAC_PI_2,
            0.0_f32,
            None,
        ),
    ];

    let mut projected: Vec<(f32, Pos2, Color32, f32, f32, Option<&'static str>)> = axes
        .iter()
        .map(|(axis, color, yaw, pitch, label)| {
            let cam = view.transform_vector3(*axis);
            let pos = center + Vec2::new(cam.x, -cam.y) * (radius * 0.68);
            (cam.z, pos, *color, *yaw, *pitch, *label)
        })
        .collect();
    projected.sort_by(|a, b| a.0.total_cmp(&b.0));

    for (depth, pos, color, _yaw, _pitch, label) in projected {
        let thickness = if depth > 0.0 { 2.1 } else { 1.4 };
        let alpha = if depth > 0.0 { 255 } else { 155 };
        let draw_color = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha);

        painter.line_segment([center, pos], Stroke::new(thickness, draw_color));
        painter.circle_filled(pos, if depth > 0.0 { 4.2 } else { 3.2 }, draw_color);

        let hit_rect = Rect::from_center_size(pos, egui::vec2(14.0, 14.0));
        let hit_resp = ui.interact(
            hit_rect,
            id.with((pos.x as i32, pos.y as i32)),
            Sense::click(),
        );
        if hit_resp.hovered() {
            painter.circle_stroke(pos, 6.0, Stroke::new(1.0, Color32::WHITE));
        }
        if let Some(label) = label {
            painter.circle_filled(
                pos,
                if depth > 0.0 { 7.0 } else { 6.4 },
                Color32::from_rgba_unmultiplied(22, 24, 28, if depth > 0.0 { 220 } else { 185 }),
            );
            painter.circle_stroke(pos, 7.0, Stroke::new(1.0, draw_color));
            painter.text(
                pos,
                Align2::CENTER_CENTER,
                label,
                FontId::proportional(9.0),
                Color32::from_rgb(245, 245, 245),
            );
        }
        if hit_resp.clicked() {
            return Some((_yaw, _pitch));
        }
    }

    None
}

fn project_point(viewport: Rect, mvp: Mat4, point: Vec3) -> Option<Pos2> {
    let clip = mvp * point.extend(1.0);
    if clip.w.abs() <= 1e-6 {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    if ndc.z < -1.1 || ndc.z > 1.1 {
        return None;
    }
    let x = viewport.left() + (ndc.x * 0.5 + 0.5) * viewport.width();
    let y = viewport.top() + (1.0 - (ndc.y * 0.5 + 0.5)) * viewport.height();
    Some(egui::pos2(x, y))
}

#[allow(dead_code)]
fn draw_wire_mesh(ui: &mut egui::Ui, viewport: Rect, mvp: Mat4, mesh: &MeshData, selected: bool) {
    let projected: Vec<Option<Pos2>> = mesh
        .vertices
        .iter()
        .map(|p| project_point(viewport, mvp, *p))
        .collect();
    let stroke = if selected {
        Stroke::new(1.5, Color32::from_rgb(15, 232, 121))
    } else {
        Stroke::new(1.1, Color32::from_rgb(150, 150, 165))
    };

    let mut drawn = HashSet::<(u32, u32)>::new();
    let max_edges = 12_000usize;
    for tri in &mesh.triangles {
        let edges = [(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])];
        for (a, b) in edges {
            if drawn.len() >= max_edges {
                break;
            }
            let key = if a < b { (a, b) } else { (b, a) };
            if !drawn.insert(key) {
                continue;
            }
            let ai = a as usize;
            let bi = b as usize;
            if let (Some(pa), Some(pb)) = (
                projected.get(ai).and_then(|p| *p),
                projected.get(bi).and_then(|p| *p),
            ) {
                ui.painter().line_segment([pa, pb], stroke);
            }
        }
        if drawn.len() >= max_edges {
            break;
        }
    }
    ui.painter().text(
        egui::pos2(viewport.left() + 12.0, viewport.top() + 60.0),
        Align2::LEFT_TOP,
        format!("Mesh: {}", mesh.name),
        FontId::proportional(10.0),
        Color32::from_gray(180),
    );
}

fn draw_solid_mesh(
    ui: &mut egui::Ui,
    viewport: Rect,
    mvp: Mat4,
    mesh: &MeshData,
    texture_cache: &mut HashMap<String, TextureHandle>,
) {
    let max_triangles = 14_000usize;
    let light = Vec3::new(0.42, 0.78, 0.46).normalize();

    // Load texture from texture_path or from material_path
    let texture_path = mesh.texture_path.clone().or_else(|| {
        mesh.material_path
            .as_ref()
            .and_then(|mat_path| parse_material_texture_path(mat_path))
    });

    // Load texture if available
    let texture = texture_path.and_then(|path| {
        if !std::path::Path::new(&path).exists() {
            return None;
        }
        if let Some(cached) = texture_cache.get(&path) {
            return Some(cached.clone());
        }
        if let Ok(bytes) = std::fs::read(&path) {
            if let Ok(image) = image::load_from_memory(&bytes) {
                let rgba = image.to_rgba8();
                let size = [rgba.width() as usize, rgba.height() as usize];
                let color_image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
                let tex =
                    ui.ctx()
                        .load_texture(path.clone(), color_image, egui::TextureOptions::LINEAR);
                texture_cache.insert(path.clone(), tex.clone());
                return Some(tex);
            }
        }
        texture_cache.remove(&path);
        None
    });

    let has_texture = texture.is_some() && !mesh.uvs.is_empty();
    eprintln!(
        "[VIEWPORT] has_texture={}, texture.is_some()={}, uvs.len()={}",
        has_texture,
        texture.is_some(),
        mesh.uvs.len()
    );
    let has_normals = mesh.normals.len() == mesh.vertices.len();

    let mut tris: Vec<(f32, Pos2, Pos2, Pos2, Color32, [f32; 2], [f32; 2], [f32; 2])> = Vec::new();
    tris.reserve(mesh.triangles.len().min(max_triangles));

    for tri in mesh.triangles.iter().take(max_triangles) {
        let ia = tri[0] as usize;
        let ib = tri[1] as usize;
        let ic = tri[2] as usize;
        if ia >= mesh.vertices.len() || ib >= mesh.vertices.len() || ic >= mesh.vertices.len() {
            continue;
        }

        let a3 = mesh.vertices[ia];
        let b3 = mesh.vertices[ib];
        let c3 = mesh.vertices[ic];

        // Get normal
        let nrm = if has_normals {
            mesh.normals[ia]
        } else {
            let n = (b3 - a3).cross(c3 - a3);
            let n_len2 = n.length_squared();
            if n_len2 <= 1e-8 {
                Vec3::Y
            } else {
                n / n_len2.sqrt()
            }
        };

        let diff = nrm.dot(light).max(0.0);
        let amb = 0.28_f32;
        let lit = (amb + diff * 0.72).clamp(0.0, 1.0);

        let clip_a = mvp * a3.extend(1.0);
        let clip_b = mvp * b3.extend(1.0);
        let clip_c = mvp * c3.extend(1.0);
        if clip_a.w.abs() <= 1e-6 || clip_b.w.abs() <= 1e-6 || clip_c.w.abs() <= 1e-6 {
            continue;
        }
        let ndc_a = clip_a.truncate() / clip_a.w;
        let ndc_b = clip_b.truncate() / clip_b.w;
        let ndc_c = clip_c.truncate() / clip_c.w;
        if ndc_a.z < -1.2
            || ndc_a.z > 1.2
            || ndc_b.z < -1.2
            || ndc_b.z > 1.2
            || ndc_c.z < -1.2
            || ndc_c.z > 1.2
        {
            continue;
        }

        let pa = egui::pos2(
            viewport.left() + (ndc_a.x * 0.5 + 0.5) * viewport.width(),
            viewport.top() + (1.0 - (ndc_a.y * 0.5 + 0.5)) * viewport.height(),
        );
        let pb = egui::pos2(
            viewport.left() + (ndc_b.x * 0.5 + 0.5) * viewport.width(),
            viewport.top() + (1.0 - (ndc_b.y * 0.5 + 0.5)) * viewport.height(),
        );
        let pc = egui::pos2(
            viewport.left() + (ndc_c.x * 0.5 + 0.5) * viewport.width(),
            viewport.top() + (1.0 - (ndc_c.y * 0.5 + 0.5)) * viewport.height(),
        );

        let area2 = (pb.x - pa.x) * (pc.y - pa.y) - (pb.y - pa.y) * (pc.x - pa.x);
        if area2 <= 0.0 {
            continue;
        }

        let depth = (ndc_a.z + ndc_b.z + ndc_c.z) / 3.0;

        // Get UVs
        let uv_a = if ia < mesh.uvs.len() {
            mesh.uvs[ia]
        } else {
            [0.0, 0.0]
        };
        let uv_b = if ib < mesh.uvs.len() {
            mesh.uvs[ib]
        } else {
            [0.0, 0.0]
        };
        let uv_c = if ic < mesh.uvs.len() {
            mesh.uvs[ic]
        } else {
            [0.0, 0.0]
        };

        // Calculate color based on texture or solid color
        let base_color = if has_texture {
            Color32::WHITE // Texture will provide the color
        } else {
            let edge_boost = (1.0 - nrm.z.abs()).clamp(0.0, 1.0) * 0.12;
            let v = ((58.0 + (lit + edge_boost) * 156.0).min(255.0)) as u8;
            let b = ((v as f32) * 1.06).min(255.0) as u8;
            Color32::from_rgb(v, v, b)
        };

        tris.push((depth, pa, pb, pc, base_color, uv_a, uv_b, uv_c));
    }

    tris.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut solid = egui::epaint::Mesh::default();

    for (_, pa, pb, pc, color, uv_a, uv_b, uv_c) in tris {
        let base = solid.vertices.len() as u32;

        if has_texture {
            // Use textured vertices
            if let Some(ref tex) = texture {
                solid.vertices.push(egui::epaint::Vertex {
                    pos: pa,
                    uv: egui::pos2(uv_a[0], uv_a[1]),
                    color,
                });
                solid.vertices.push(egui::epaint::Vertex {
                    pos: pb,
                    uv: egui::pos2(uv_b[0], uv_b[1]),
                    color,
                });
                solid.vertices.push(egui::epaint::Vertex {
                    pos: pc,
                    uv: egui::pos2(uv_c[0], uv_c[1]),
                    color,
                });
                solid.indices.push(base);
                solid.indices.push(base + 1);
                solid.indices.push(base + 2);
                solid.texture_id = tex.id();
            }
        } else {
            // Use colored vertices (original behavior)
            solid.colored_vertex(pa, color);
            solid.colored_vertex(pb, color);
            solid.colored_vertex(pc, color);
            solid.add_triangle(base, base + 1, base + 2);
        }
    }

    if !solid.vertices.is_empty() {
        ui.painter().add(egui::Shape::mesh(solid));
    }
}

fn draw_mesh_silhouette(
    ui: &mut egui::Ui,
    viewport: Rect,
    mvp: Mat4,
    _model_view: Mat4,
    mesh: &MeshData,
) {
    if mesh.vertices.is_empty() {
        return;
    }
    let max_points = mesh.vertices.len().min(5000);
    let mut pts: Vec<Pos2> = mesh
        .vertices
        .iter()
        .take(max_points)
        .filter_map(|v| project_point(viewport, mvp, *v))
        .collect();
    if pts.len() < 3 {
        return;
    }
    pts.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));

    fn cross(o: Pos2, a: Pos2, b: Pos2) -> f32 {
        (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)
    }

    let mut lower: Vec<Pos2> = Vec::new();
    for p in &pts {
        while lower.len() >= 2 && cross(lower[lower.len() - 2], lower[lower.len() - 1], *p) <= 0.0 {
            lower.pop();
        }
        lower.push(*p);
    }

    let mut upper: Vec<Pos2> = Vec::new();
    for p in pts.iter().rev() {
        while upper.len() >= 2 && cross(upper[upper.len() - 2], upper[upper.len() - 1], *p) <= 0.0 {
            upper.pop();
        }
        upper.push(*p);
    }

    if !lower.is_empty() {
        lower.pop();
    }
    if !upper.is_empty() {
        upper.pop();
    }
    let mut hull = lower;
    hull.extend(upper);
    if hull.len() < 3 {
        return;
    }

    let glow = Stroke::new(3.0, Color32::from_rgba_unmultiplied(15, 232, 121, 70));
    let line = Stroke::new(1.7, Color32::from_rgb(15, 232, 121));
    for i in 0..hull.len() {
        let a = hull[i];
        let b = hull[(i + 1) % hull.len()];
        ui.painter().line_segment([a, b], glow);
        ui.painter().line_segment([a, b], line);
    }
}
