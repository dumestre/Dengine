use crate::EngineLanguage;
use eframe::egui::{self, UiKind};
use mlua::{Function, Lua, MultiValue, RegistryKey, Table, Value};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

mod modules;
use modules::{
    AvailableModule, ModuleCategory, ModuleChainItem, ModuleControl, friendly_module_name,
    group_modules_by_category, parse_available_module,
};

const ACTION_COUNT: usize = 8;

#[derive(Clone, Copy, PartialEq, Eq)]
enum FiosTab {
    Controls,
    Graph,
    Controller,
    Animator,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FiosControlMode {
    Movement,
    Animation,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FiosAction {
    Forward,
    Backward,
    Left,
    Right,
    Jump,
    Interact,
    Action1,
    Action2,
}

impl FiosAction {
    const ALL: [Self; ACTION_COUNT] = [
        Self::Forward,
        Self::Backward,
        Self::Left,
        Self::Right,
        Self::Jump,
        Self::Interact,
        Self::Action1,
        Self::Action2,
    ];

    fn id(self) -> &'static str {
        match self {
            Self::Forward => "forward",
            Self::Backward => "backward",
            Self::Left => "left",
            Self::Right => "right",
            Self::Jump => "jump",
            Self::Interact => "interact",
            Self::Action1 => "action_1",
            Self::Action2 => "action_2",
        }
    }

    fn index(self) -> usize {
        match self {
            Self::Forward => 0,
            Self::Backward => 1,
            Self::Left => 2,
            Self::Right => 3,
            Self::Jump => 4,
            Self::Interact => 5,
            Self::Action1 => 6,
            Self::Action2 => 7,
        }
    }

    fn label(self, lang: EngineLanguage) -> &'static str {
        match (lang, self) {
            (EngineLanguage::Pt, Self::Forward) => "Mover Frente",
            (EngineLanguage::Pt, Self::Backward) => "Mover Tras",
            (EngineLanguage::Pt, Self::Left) => "Mover Esquerda",
            (EngineLanguage::Pt, Self::Right) => "Mover Direita",
            (EngineLanguage::Pt, Self::Jump) => "Pular",
            (EngineLanguage::Pt, Self::Interact) => "Interagir",
            (EngineLanguage::Pt, Self::Action1) => "Acao 1",
            (EngineLanguage::Pt, Self::Action2) => "Acao 2",
            (EngineLanguage::En, Self::Forward) => "Move Forward",
            (EngineLanguage::En, Self::Backward) => "Move Backward",
            (EngineLanguage::En, Self::Left) => "Move Left",
            (EngineLanguage::En, Self::Right) => "Move Right",
            (EngineLanguage::En, Self::Jump) => "Jump",
            (EngineLanguage::En, Self::Interact) => "Interact",
            (EngineLanguage::En, Self::Action1) => "Action 1",
            (EngineLanguage::En, Self::Action2) => "Action 2",
            (EngineLanguage::Es, Self::Forward) => "Mover Adelante",
            (EngineLanguage::Es, Self::Backward) => "Mover Atras",
            (EngineLanguage::Es, Self::Left) => "Mover Izquierda",
            (EngineLanguage::Es, Self::Right) => "Mover Derecha",
            (EngineLanguage::Es, Self::Jump) => "Saltar",
            (EngineLanguage::Es, Self::Interact) => "Interactuar",
            (EngineLanguage::Es, Self::Action1) => "Accion 1",
            (EngineLanguage::Es, Self::Action2) => "Accion 2",
        }
    }

    fn label_for_mode(self, lang: EngineLanguage, mode: FiosControlMode) -> &'static str {
        match mode {
            FiosControlMode::Movement => self.label(lang),
            FiosControlMode::Animation => match (lang, self) {
                (EngineLanguage::Pt, Self::Forward) => "Animacao Anterior",
                (EngineLanguage::Pt, Self::Backward) => "Proxima Animacao",
                (EngineLanguage::Pt, Self::Left) => "Frame -",
                (EngineLanguage::Pt, Self::Right) => "Frame +",
                (EngineLanguage::Pt, Self::Jump) => "Play/Pause",
                (EngineLanguage::Pt, Self::Interact) => "Stop",
                (EngineLanguage::Pt, Self::Action1) => "Blend +",
                (EngineLanguage::Pt, Self::Action2) => "Blend -",
                (EngineLanguage::En, Self::Forward) => "Previous Anim",
                (EngineLanguage::En, Self::Backward) => "Next Anim",
                (EngineLanguage::En, Self::Left) => "Frame -",
                (EngineLanguage::En, Self::Right) => "Frame +",
                (EngineLanguage::En, Self::Jump) => "Play/Pause",
                (EngineLanguage::En, Self::Interact) => "Stop",
                (EngineLanguage::En, Self::Action1) => "Blend +",
                (EngineLanguage::En, Self::Action2) => "Blend -",
                (EngineLanguage::Es, Self::Forward) => "Animacion Anterior",
                (EngineLanguage::Es, Self::Backward) => "Siguiente Animacion",
                (EngineLanguage::Es, Self::Left) => "Frame -",
                (EngineLanguage::Es, Self::Right) => "Frame +",
                (EngineLanguage::Es, Self::Jump) => "Play/Pause",
                (EngineLanguage::Es, Self::Interact) => "Stop",
                (EngineLanguage::Es, Self::Action1) => "Blend +",
                (EngineLanguage::Es, Self::Action2) => "Blend -",
            },
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FiosNodeKind {
    InputAxis,
    InputAction,
    Constant,
    Add,
    Subtract,
    Multiply,
    Divide,
    Max,
    Min,
    Gate,
    Abs,
    Sign,
    Clamp,
    Deadzone,
    Invert,
    Smooth,
    OutputMove,
    OutputLook,
    OutputAction,
    OutputAnimCommand,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FiosAnimationCommand {
    PlayPause,
    Next,
    Prev,
}

impl FiosNodeKind {
    fn id(self) -> &'static str {
        match self {
            Self::InputAxis => "input_axis",
            Self::InputAction => "input_action",
            Self::Constant => "constant",
            Self::Add => "add",
            Self::Subtract => "subtract",
            Self::Multiply => "multiply",
            Self::Divide => "divide",
            Self::Max => "max",
            Self::Min => "min",
            Self::Gate => "gate",
            Self::Abs => "abs",
            Self::Sign => "sign",
            Self::Clamp => "clamp",
            Self::Deadzone => "deadzone",
            Self::Invert => "invert",
            Self::Smooth => "smooth",
            Self::OutputMove => "output_move",
            Self::OutputLook => "output_look",
            Self::OutputAction => "output_action",
            Self::OutputAnimCommand => "output_anim_cmd",
        }
    }

    fn from_id(id: &str) -> Option<Self> {
        Some(match id {
            "input_axis" => Self::InputAxis,
            "input_action" => Self::InputAction,
            "constant" => Self::Constant,
            "add" => Self::Add,
            "subtract" => Self::Subtract,
            "multiply" => Self::Multiply,
            "divide" => Self::Divide,
            "max" => Self::Max,
            "min" => Self::Min,
            "gate" => Self::Gate,
            "abs" => Self::Abs,
            "sign" => Self::Sign,
            "clamp" => Self::Clamp,
            "deadzone" => Self::Deadzone,
            "invert" => Self::Invert,
            "smooth" => Self::Smooth,
            "output_move" => Self::OutputMove,
            "output_look" => Self::OutputLook,
            "output_action" => Self::OutputAction,
            "output_anim_cmd" => Self::OutputAnimCommand,
            _ => return None,
        })
    }

    fn input_count(self) -> usize {
        match self {
            Self::InputAxis => 0,
            Self::InputAction => 0,
            Self::Constant => 0,
            Self::Add => 2,
            Self::Subtract => 2,
            Self::Multiply => 2,
            Self::Divide => 2,
            Self::Max => 2,
            Self::Min => 2,
            Self::Gate => 2,
            Self::Abs => 1,
            Self::Sign => 1,
            Self::Clamp => 1,
            Self::Deadzone => 1,
            Self::Invert => 1,
            Self::Smooth => 1,
            Self::OutputMove => 2,
            Self::OutputLook => 2,
            Self::OutputAction => 1,
            Self::OutputAnimCommand => 1,
        }
    }

    fn output_count(self) -> usize {
        match self {
            Self::InputAxis => 2,
            Self::InputAction => 1,
            Self::Constant => 1,
            Self::Add => 1,
            Self::Subtract => 1,
            Self::Multiply => 1,
            Self::Divide => 1,
            Self::Max => 1,
            Self::Min => 1,
            Self::Gate => 1,
            Self::Abs => 1,
            Self::Sign => 1,
            Self::Clamp => 1,
            Self::Deadzone => 1,
            Self::Invert => 1,
            Self::Smooth => 1,
            Self::OutputMove => 0,
            Self::OutputLook => 0,
            Self::OutputAction => 0,
            Self::OutputAnimCommand => 0,
        }
    }

    fn input_name(self, idx: usize) -> &'static str {
        match (self, idx) {
            (Self::Add, 0)
            | (Self::Subtract, 0)
            | (Self::Multiply, 0)
            | (Self::Divide, 0)
            | (Self::Max, 0)
            | (Self::Min, 0) => "A",
            (Self::Add, 1)
            | (Self::Subtract, 1)
            | (Self::Multiply, 1)
            | (Self::Divide, 1)
            | (Self::Max, 1)
            | (Self::Min, 1) => "B",
            (Self::Gate, 0) => "V",
            (Self::Gate, 1) => "Gate",
            (Self::Clamp, 0) | (Self::Deadzone, 0) | (Self::Invert, 0) | (Self::Smooth, 0) => "In",
            (Self::Abs, 0) | (Self::Sign, 0) => "In",
            (Self::OutputMove, 0) => "X",
            (Self::OutputMove, 1) => "Y",
            (Self::OutputLook, 0) => "Yaw",
            (Self::OutputLook, 1) => "Pitch",
            (Self::OutputAction, 0) => "A",
            (Self::OutputAnimCommand, 0) => "Cmd",
            _ => "",
        }
    }

    fn output_name(self, idx: usize) -> &'static str {
        match (self, idx) {
            (Self::InputAxis, 0) => "X",
            (Self::InputAxis, 1) => "Y",
            (Self::InputAction, 0)
            | (Self::Constant, 0)
            | (Self::Add, 0)
            | (Self::Subtract, 0)
            | (Self::Multiply, 0)
            | (Self::Divide, 0)
            | (Self::Max, 0)
            | (Self::Min, 0)
            | (Self::Gate, 0)
            | (Self::Abs, 0)
            | (Self::Sign, 0)
            | (Self::Clamp, 0)
            | (Self::Deadzone, 0)
            | (Self::Invert, 0)
            | (Self::Smooth, 0) => "Out",
            _ => "",
        }
    }
}

#[derive(Clone)]
struct FiosNode {
    id: u32,
    kind: FiosNodeKind,
    display_name: String,
    pos: egui::Vec2,
    value: f32,
    param_a: f32,
    param_b: f32,
}

#[derive(Clone, Copy)]
struct FiosLink {
    from_node: u32,
    from_port: u8,
    to_node: u32,
    to_port: u8,
}

#[derive(Clone)]
struct FiosGroup {
    id: u32,
    name: String,
    color: egui::Color32,
    nodes: HashSet<u32>,
}

#[derive(Clone)]
struct AnimControllerNode {
    id: u32,
    name: String,
    clip_ref: String,
    pos: egui::Pos2,
    speed: f32,
}

#[derive(Clone, Copy)]
struct AnimControllerLink {
    from: u32,
    to: u32,
    blend_time: f32,
    transition_type: TransitionType,
}

#[derive(Clone, Copy, PartialEq)]
enum TransitionType {
    Immediate,
    CrossFade,
    Freeze,
}

pub struct FiosState {
    controls_enabled: bool,
    bindings: [egui::Key; ACTION_COUNT],
    pressed: [bool; ACTION_COUNT],
    just_pressed: [bool; ACTION_COUNT],
    capture_index: Option<usize>,
    status: Option<String>,
    add_icon_texture: Option<egui::TextureHandle>,
    module_add_texture: Option<egui::TextureHandle>,
    available_modules: Vec<ModuleCategory>,
    module_chain: Vec<ModuleChainItem>,
    next_module_id: u32,
    control_modes: Vec<FiosControlMode>,
    active_control_mode: FiosControlMode,
    tab: FiosTab,
    nodes: Vec<FiosNode>,
    links: Vec<FiosLink>,
    groups: Vec<FiosGroup>,
    next_node_id: u32,
    next_group_id: u32,
    drag_from_output: Option<(u32, u8)>,
    wire_drag_path: Vec<egui::Pos2>,
    selected_node: Option<u32>,
    selected_nodes: HashSet<u32>,
    rename_node: Option<u32>,
    rename_buffer: String,
    marquee_start: Option<egui::Pos2>,
    marquee_end: Option<egui::Pos2>,
    cut_points: Vec<egui::Pos2>,
    graph_zoom: f32,
    graph_pan: egui::Vec2,
    smooth_state: HashMap<(u32, u8), f32>,
    lua_enabled: bool,
    lua_script: String,
    lua_status: Option<String>,
    lua_runtime: Lua,
    lua_fn_key: Option<RegistryKey>,
    lua_dirty: bool,
    last_axis: [f32; 2],
    last_look: [f32; 2],
    last_action: f32,
    last_anim_cmd_signal: f32,
    prev_anim_cmd_bucket: i8,
    pending_anim_cmd: Option<FiosAnimationCommand>,
    anim_nodes: Vec<AnimControllerNode>,
    anim_links: Vec<AnimControllerLink>,
    anim_next_node_id: u32,
    anim_drag_clip: Option<String>,
    anim_connect_from: Option<u32>,
    anim_tab_status: Option<String>,
    anim_selected_nodes: HashSet<u32>,
    anim_selected_link: Option<usize>,
    anim_clip_cache: Vec<String>,
    anim_clip_cache_dirty: bool,
    anim_clip_cache_next_scan: f64,
    embedded_panel_rect: Option<egui::Rect>,
    anim_is_playing: bool,
    anim_current_time: f64,
    anim_total_duration: f64,
    anim_is_recording: bool,
    _anim_selected_track: Option<usize>,
}

impl FiosState {
    fn control_mode_label(mode: FiosControlMode, lang: EngineLanguage) -> &'static str {
        match (lang, mode) {
            (EngineLanguage::Pt, FiosControlMode::Movement) => "Movimento",
            (EngineLanguage::Pt, FiosControlMode::Animation) => "Animacao",
            (EngineLanguage::En, FiosControlMode::Movement) => "Movement",
            (EngineLanguage::En, FiosControlMode::Animation) => "Animation",
            (EngineLanguage::Es, FiosControlMode::Movement) => "Movimiento",
            (EngineLanguage::Es, FiosControlMode::Animation) => "Animacion",
        }
    }

    pub fn set_available_modules(&mut self, modules: Vec<String>) {
        let defs = modules.into_iter().map(parse_available_module).collect();
        self.available_modules = group_modules_by_category(defs);
    }

    pub fn set_animation_clips(&mut self, clips: Vec<String>) {
        self.anim_clip_cache = clips;
        self.anim_clip_cache_dirty = false;
    }

    pub fn set_animator_tab(&mut self) {
        self.tab = FiosTab::Animator;
    }

    fn instantiate_module_from_asset(&mut self, asset: &str) -> Option<u32> {
        let key = asset.to_ascii_lowercase();
        match key.as_str() {
            "movimento_basico.animodule" => self.add_module_move_basic(),
            "movimento_avancado.animodule" => self.add_module_move_advanced(),
            "camera_fps.animodule" => self.add_module_look_basic(),
            "camera_3p.animodule" => self.add_module_look_advanced(),
            "acao_principal.animodule" => self.add_module_action_basic(FiosAction::Action1.index()),
            "acao_pulo.animodule" => self.add_module_action_basic(FiosAction::Jump.index()),
            "controlador_animacao.animodule" => self.add_module_animation_controls(),
            "mapa_teclas.animodule" => self.add_module_key_map(),
            _ => None,
        }
    }

    fn push_module_from_asset(&mut self, asset: &str, group_id: Option<u32>) {
        let id = self.next_module_id;
        self.next_module_id = self.next_module_id.saturating_add(1);
        let name = friendly_module_name(asset);
        let (description, extra_info) = match self.available_module_by_asset(asset) {
            Some(module) => (module.description.clone(), module.extra_info.clone()),
            None => (None, Vec::new()),
        };
        let controls = self.module_controls_for_group(group_id);
        self.module_chain.push(ModuleChainItem {
            id,
            name,
            asset: asset.to_string(),
            enabled: true,
            group_id,
            description,
            controls,
            extra_info,
        });
    }

    fn available_module_by_asset(&self, asset: &str) -> Option<&AvailableModule> {
        self.available_modules.iter().find_map(|category| {
            category
                .modules
                .iter()
                .find(|module| module.asset.eq_ignore_ascii_case(asset))
        })
    }

    fn module_controls_for_group(&self, group_id: Option<u32>) -> Vec<ModuleControl> {
        if let Some(group_id) = group_id {
            if let Some(group) = self.groups.iter().find(|g| g.id == group_id) {
                let mut controls: Vec<ModuleControl> = group
                    .nodes
                    .iter()
                    .filter_map(|node_id| self.node_index_by_id(*node_id))
                    .map(|idx| {
                        let node = &self.nodes[idx];
                        ModuleControl {
                            node_id: node.id,
                            name: node.display_name.clone(),
                            value: node.value,
                            param_a: node.param_a,
                            param_b: node.param_b,
                        }
                    })
                    .collect();
                controls.sort_by(|a, b| a.name.cmp(&b.name));
                controls
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        }
    }

    #[allow(dead_code)]
    fn apply_module_controls(&mut self, module_idx: usize) {
        if module_idx >= self.module_chain.len() {
            return;
        }
        let controls = self.module_chain[module_idx].controls.clone();
        for control in controls {
            if let Some(node_idx) = self.node_index_by_id(control.node_id) {
                let node = &mut self.nodes[node_idx];
                node.value = control.value;
                node.param_a = control.param_a;
                node.param_b = control.param_b;
            }
        }
    }

    fn add_module_key_map(&mut self) -> Option<u32> {
        let mut ids = Vec::new();
        let columns = 4;
        let spacing = egui::vec2(180.0, 140.0);
        for (i, action) in FiosAction::ALL.iter().enumerate() {
            let col = (i % columns) as f32;
            let row = (i / columns) as f32;
            let pos_input = egui::vec2(60.0 + col * spacing.x, 120.0 + row * spacing.y);
            let pos_output = pos_input + egui::vec2(360.0, 0.0);
            let input_id = self.add_node_custom(
                FiosNodeKind::InputAction,
                pos_input,
                0.0,
                action.index() as f32,
                1.0,
            );
            let output_id =
                self.add_node_custom(FiosNodeKind::OutputAction, pos_output, 0.0, 0.0, 0.0);
            self.create_link(input_id, 0, output_id, 0);
            ids.push(input_id);
            ids.push(output_id);
        }
        let group = self.create_module_group(
            "Módulo Mapa de Teclas",
            egui::Color32::from_rgb(122, 88, 152),
            ids,
        );
        let _ = self.save_graph_to_disk();
        group
    }

    fn render_module_card_details(
        &mut self,
        ui: &mut egui::Ui,
        module_idx: usize,
        lang: EngineLanguage,
        bindings: &[egui::Key; ACTION_COUNT],
    ) {
        let asset = {
            let module = &self.module_chain[module_idx];
            module.asset.to_ascii_lowercase()
        };
        let control_count = {
            let module = &self.module_chain[module_idx];
            module.controls.len()
        };
        let details_label = match lang {
            EngineLanguage::Pt => format!("▸ Controles ({control_count})"),
            EngineLanguage::En => format!("▸ Controls ({control_count})"),
            EngineLanguage::Es => format!("▸ Controles ({control_count})"),
        };
        let header_id = ui.id().with(("mod_details_collapse", module_idx));
        egui::CollapsingHeader::new(
            egui::RichText::new(details_label)
                .size(11.0)
                .color(egui::Color32::from_gray(185)),
        )
        .id_salt(header_id)
        .default_open(false)
        .show(ui, |ui| {
            ui.add_space(4.0);
            match asset.as_str() {
                "mapa_teclas.animodule" => self.render_module_key_map(ui, lang, bindings),
                _ => self.render_module_controls(ui, module_idx, lang),
            }
        });
    }

    fn render_module_extra_info(ui: &mut egui::Ui, module: &ModuleChainItem) {
        if module.extra_info.is_empty() {
            return;
        }
        for (key, value) in &module.extra_info {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("{key}:"))
                        .size(10.0)
                        .color(egui::Color32::from_gray(130)),
                );
                ui.label(
                    egui::RichText::new(value)
                        .size(10.0)
                        .color(egui::Color32::from_gray(165)),
                );
            });
        }
    }

    fn render_module_controls(
        &mut self,
        ui: &mut egui::Ui,
        module_idx: usize,
        lang: EngineLanguage,
    ) {
        let module = &mut self.module_chain[module_idx];
        if module.controls.is_empty() {
            let empty_txt = match lang {
                EngineLanguage::Pt => "Nenhum controle disponível para este módulo",
                EngineLanguage::En => "No editable controls for this module",
                EngineLanguage::Es => "Ningún control editable para este módulo",
            };
            ui.label(
                egui::RichText::new(empty_txt)
                    .small()
                    .color(egui::Color32::from_gray(150)),
            );
            return;
        }
        let label_txt = match lang {
            EngineLanguage::Pt => "Controle",
            EngineLanguage::En => "Control",
            EngineLanguage::Es => "Control",
        };
        let value_txt = match lang {
            EngineLanguage::Pt => "Valor",
            EngineLanguage::En => "Value",
            EngineLanguage::Es => "Valor",
        };
        let param_a_txt = match lang {
            EngineLanguage::Pt => "Parâmetro A",
            EngineLanguage::En => "Param A",
            EngineLanguage::Es => "Parámetro A",
        };
        let param_b_txt = match lang {
            EngineLanguage::Pt => "Parâmetro B",
            EngineLanguage::En => "Param B",
            EngineLanguage::Es => "Parámetro B",
        };
        let grid_id = format!("module_controls_grid_{}", module.id);
        egui::Frame::new()
            .fill(egui::Color32::from_rgb(22, 24, 28))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(70, 76, 90)))
            .corner_radius(8.0)
            .inner_margin(egui::Margin::symmetric(10, 10))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("module_controls_scroll")
                    .max_height(160.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(10.0, 4.0);
                        egui::Grid::new(grid_id)
                            .striped(true)
                            .spacing((12.0, 6.0))
                            .show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new(label_txt)
                                        .strong()
                                        .color(egui::Color32::from_gray(220)),
                                );
                                ui.label(
                                    egui::RichText::new(value_txt)
                                        .strong()
                                        .color(egui::Color32::from_gray(220)),
                                );
                                ui.label(
                                    egui::RichText::new(param_a_txt)
                                        .strong()
                                        .color(egui::Color32::from_gray(220)),
                                );
                                ui.label(
                                    egui::RichText::new(param_b_txt)
                                        .strong()
                                        .color(egui::Color32::from_gray(220)),
                                );
                                ui.end_row();
                                for control in &mut module.controls {
                                    ui.label(
                                        egui::RichText::new(&control.name)
                                            .small()
                                            .color(egui::Color32::from_gray(210)),
                                    );
                                    ui.add(
                                        egui::DragValue::new(&mut control.value)
                                            .speed(0.05)
                                            .range(-10.0..=10.0)
                                            .fixed_decimals(2)
                                            .max_decimals(3)
                                            .min_decimals(1),
                                    );
                                    ui.add(
                                        egui::DragValue::new(&mut control.param_a)
                                            .speed(0.05)
                                            .range(-10.0..=10.0)
                                            .fixed_decimals(2)
                                            .max_decimals(3)
                                            .min_decimals(1),
                                    );
                                    ui.add(
                                        egui::DragValue::new(&mut control.param_b)
                                            .speed(0.05)
                                            .range(-10.0..=10.0)
                                            .fixed_decimals(2)
                                            .max_decimals(3)
                                            .min_decimals(1),
                                    );
                                    ui.end_row();
                                }
                            });
                    });
            });
    }

    fn render_module_key_map(
        &mut self,
        ui: &mut egui::Ui,
        lang: EngineLanguage,
        bindings: &[egui::Key; ACTION_COUNT],
    ) {
        let key_map_title = match lang {
            EngineLanguage::Pt => "Mapa de teclas",
            EngineLanguage::En => "Key map",
            EngineLanguage::Es => "Mapa de teclas",
        };
        let action_col = match lang {
            EngineLanguage::Pt => "Ação",
            EngineLanguage::En => "Action",
            EngineLanguage::Es => "Acción",
        };
        let key_col = match lang {
            EngineLanguage::Pt => "Tecla",
            EngineLanguage::En => "Key",
            EngineLanguage::Es => "Tecla",
        };
        egui::Frame::new()
            .fill(egui::Color32::from_rgb(22, 24, 28))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(70, 76, 90)))
            .corner_radius(8.0)
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new(key_map_title)
                        .strong()
                        .color(egui::Color32::from_gray(210)),
                );
                ui.add_space(4.0);

                let instructions = match lang {
                    EngineLanguage::Pt => "Clique no botão da tecla para editar diretamente:",
                    EngineLanguage::En => "Click the key button to edit directly:",
                    EngineLanguage::Es => {
                        "Haga clic en el botón de la tecla para editar directamente:"
                    }
                };
                ui.label(
                    egui::RichText::new(instructions)
                        .small()
                        .color(egui::Color32::from_rgb(200, 200, 200)),
                );
                ui.add_space(6.0);

                let grid_id = ui.id().with("module_key_map_grid");
                egui::Grid::new(grid_id)
                    .striped(true)
                    .spacing((12.0, 6.0))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(action_col)
                                .small()
                                .strong()
                                .color(egui::Color32::from_gray(210)),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                egui::RichText::new(key_col)
                                    .small()
                                    .strong()
                                    .color(egui::Color32::from_gray(210)),
                            )
                        });
                        ui.end_row();

                        for (i, action) in FiosAction::ALL.iter().enumerate() {
                            ui.label(
                                egui::RichText::new(action.label(lang))
                                    .small()
                                    .color(egui::Color32::from_gray(200)),
                            );

                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let key_name = Self::key_display_name(bindings[i]);
                                    let button_text = key_name;

                                    let button_response = ui.add_sized(
                                        [120.0, 20.0],
                                        egui::Button::new(button_text)
                                            .stroke(egui::Stroke::new(
                                                1.0,
                                                egui::Color32::from_rgb(100, 150, 200),
                                            ))
                                            .fill(egui::Color32::from_rgba_unmultiplied(
                                                50, 75, 110, 180,
                                            )),
                                    );

                                    if button_response.clicked() {
                                        self.capture_index = Some(i);
                                        self.status = match lang {
                                            EngineLanguage::Pt => {
                                                Some("Aguardando tecla...".to_string())
                                            }
                                            EngineLanguage::En => {
                                                Some("Waiting for key...".to_string())
                                            }
                                            EngineLanguage::Es => {
                                                Some("Esperando tecla...".to_string())
                                            }
                                        };
                                    }
                                },
                            );
                            ui.end_row();
                        }
                    });
            });
        ui.add_space(6.0);
    }

    fn key_display_name(key: egui::Key) -> String {
        let mut out = format!("{key:?}");
        if let Some(stripped) = out.strip_prefix("Key::") {
            out = stripped.to_string();
        }
        out.replace("::", " ")
    }

    fn module_menu_content(&mut self, ui: &mut egui::Ui, lang: EngineLanguage) -> bool {
        if self.available_modules.is_empty() {
            let no_modules_txt = match lang {
                EngineLanguage::Pt => "Nenhum módulo disponível",
                EngineLanguage::En => "No modules available",
                EngineLanguage::Es => "No hay módulos disponibles",
            };
            ui.label(no_modules_txt);
            return false;
        }
        let mut selected_asset: Option<String> = None;
        for category in &self.available_modules {
            if category.modules.is_empty() {
                continue;
            }
            ui.menu_button(
                egui::RichText::new(&category.name)
                    .size(11.0)
                    .strong()
                    .color(egui::Color32::from_gray(200)),
                |ui| {
                    ui.set_min_width(208.0);
                    for module in &category.modules {
                        let btn = ui.add(
                            egui::Button::new(
                                egui::RichText::new(&module.display_name)
                                    .strong()
                                    .size(12.0),
                            )
                            .frame(false)
                            .fill(egui::Color32::from_rgba_unmultiplied(80, 80, 90, 200)),
                        );
                        if btn.clicked() {
                            selected_asset = Some(module.asset.clone());
                            ui.close_kind(UiKind::Menu);
                            return;
                        }
                        if let Some(desc) = module.description.as_ref() {
                            ui.label(
                                egui::RichText::new(desc)
                                    .small()
                                    .color(egui::Color32::from_gray(150)),
                            );
                        }
                        ui.add_space(4.0);
                    }
                },
            );
            if selected_asset.is_some() {
                break;
            }
        }
        if let Some(asset) = selected_asset {
            let group_id = self.instantiate_module_from_asset(&asset);
            self.push_module_from_asset(&asset, group_id);
            true
        } else {
            false
        }
    }

    fn module_add_button(&mut self, ui: &mut egui::Ui, label: &str) -> egui::Response {
        if self.module_add_texture.is_none() {
            self.module_add_texture =
                Self::load_png_texture(ui.ctx(), "src/assets/icons/addmodulo.png");
        }
        let accent = egui::Color32::from_rgb(15, 232, 121);
        let button = if let Some(texture) = &self.module_add_texture {
            let icon = egui::Image::new(texture).fit_to_exact_size(egui::vec2(14.0, 14.0));
            egui::Button::image_and_text(
                icon,
                egui::RichText::new(label).size(11.5).strong().color(accent),
            )
        } else {
            egui::Button::new(egui::RichText::new(label).size(11.5).strong().color(accent))
        }
        .fill(egui::Color32::from_rgba_unmultiplied(15, 232, 121, 18))
        .stroke(egui::Stroke::new(
            1.0,
            egui::Color32::from_rgba_unmultiplied(15, 232, 121, 60),
        ))
        .corner_radius(16.0)
        .min_size(egui::vec2(130.0, 30.0));
        ui.add(button)
    }

    fn load_png_texture(ctx: &egui::Context, path: &str) -> Option<egui::TextureHandle> {
        let bytes = fs::read(path).ok()?;
        let rgba = image::load_from_memory(&bytes).ok()?.to_rgba8();
        let size = [rgba.width() as usize, rgba.height() as usize];
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
        Some(ctx.load_texture(path.to_owned(), color_image, egui::TextureOptions::LINEAR))
    }

    fn default_node_name(kind: FiosNodeKind) -> &'static str {
        match kind {
            FiosNodeKind::InputAxis => "Input Axis",
            FiosNodeKind::InputAction => "Input Action",
            FiosNodeKind::Constant => "Constant",
            FiosNodeKind::Add => "Add",
            FiosNodeKind::Subtract => "Subtract",
            FiosNodeKind::Multiply => "Multiply",
            FiosNodeKind::Divide => "Divide",
            FiosNodeKind::Max => "Max",
            FiosNodeKind::Min => "Min",
            FiosNodeKind::Gate => "Gate",
            FiosNodeKind::Abs => "Abs",
            FiosNodeKind::Sign => "Sign",
            FiosNodeKind::Clamp => "Clamp",
            FiosNodeKind::Deadzone => "Deadzone",
            FiosNodeKind::Invert => "Invert",
            FiosNodeKind::Smooth => "Smooth",
            FiosNodeKind::OutputMove => "Output Move",
            FiosNodeKind::OutputLook => "Output Look",
            FiosNodeKind::OutputAction => "Output Action",
            FiosNodeKind::OutputAnimCommand => "Output Anim Cmd",
        }
    }

    fn encode_field(raw: &str) -> String {
        raw.replace('%', "%25")
            .replace('|', "%7C")
            .replace('\n', "%0A")
    }

    fn decode_field(raw: &str) -> String {
        raw.replace("%0A", "\n")
            .replace("%7C", "|")
            .replace("%25", "%")
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

    pub fn clear_embedded_rect(&mut self) {
        self.embedded_panel_rect = None;
    }

    pub fn contains_point(&self, p: egui::Pos2) -> bool {
        self.embedded_panel_rect.is_some_and(|r| r.contains(p))
    }

    pub fn panel_rect(&self) -> Option<egui::Rect> {
        self.embedded_panel_rect
    }

    pub fn on_asset_dropped(&mut self, asset_name: &str, path: Option<&Path>) -> bool {
        self.anim_clip_cache_dirty = true;
        let lower = asset_name.to_ascii_lowercase();
        if lower.ends_with(".fbx") {
            let path = path
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("Assets").join("Meshes").join(asset_name));
            let clips = fs::read(&path)
                .ok()
                .map(|bytes| Self::parse_fbx_animation_names(&String::from_utf8_lossy(&bytes)))
                .unwrap_or_default();
            let file_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(asset_name)
                .to_string();
            self.tab = FiosTab::Controller;
            if clips.is_empty() {
                self.add_anim_controller_node(file_name.clone(), egui::pos2(320.0, 120.0));
                self.anim_tab_status =
                    Some("FBX sem clipes detectados; estado base criado".to_string());
                return true;
            }
            let start = egui::pos2(320.0, 120.0);
            for (i, clip) in clips.iter().enumerate() {
                let col = (i % 3) as f32;
                let row = (i / 3) as f32;
                let pos = start + egui::vec2(col * 196.0, row * 76.0);
                self.add_anim_controller_node(format!("{file_name}::{clip}"), pos);
            }
            self.anim_tab_status = Some(format!(
                "{} estados criados a partir de {}",
                clips.len(),
                file_name
            ));
            return true;
        }
        if lower.ends_with(".anim") || asset_name.contains("::") {
            self.tab = FiosTab::Controller;
            self.add_anim_controller_node(asset_name.to_string(), egui::pos2(320.0, 120.0));
            self.anim_tab_status = Some("Estado criado por arrastar e soltar".to_string());
            return true;
        }
        false
    }

    fn available_animation_clips() -> Vec<String> {
        let mut out = Vec::<String>::new();
        let mesh_dir = PathBuf::from("Assets").join("Meshes");
        if let Ok(entries) = fs::read_dir(&mesh_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let is_fbx = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.eq_ignore_ascii_case("fbx"))
                    .unwrap_or(false);
                if !is_fbx {
                    continue;
                }
                let file_name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("mesh.fbx")
                    .to_string();
                if let Ok(bytes) = fs::read(&path) {
                    let raw = String::from_utf8_lossy(&bytes);
                    for clip in Self::parse_fbx_animation_names(&raw) {
                        out.push(format!("{file_name}::{clip}"));
                    }
                }
            }
        }
        let anim_dir = PathBuf::from("Assets").join("Animations");
        if let Ok(entries) = fs::read_dir(anim_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let is_anim = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.eq_ignore_ascii_case("anim"))
                    .unwrap_or(false);
                if is_anim {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        out.push(name.to_string());
                    }
                }
            }
        }
        out.sort_by_key(|s| s.to_ascii_lowercase());
        out.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        out
    }

    fn refresh_anim_clip_cache(&mut self, ctx: &egui::Context, force: bool) {
        let now = ctx.input(|i| i.time);
        if !force && !self.anim_clip_cache_dirty && now < self.anim_clip_cache_next_scan {
            return;
        }
        let mut out = Self::available_animation_clips();
        let module_dir = PathBuf::from("Assets").join("Animations").join("Modules");
        if let Ok(entries) = fs::read_dir(module_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let is_module = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.eq_ignore_ascii_case("animodule"))
                    .unwrap_or(false);
                if !is_module {
                    continue;
                }
                if let Ok(text) = fs::read_to_string(path) {
                    for line in text.lines() {
                        if let Some(v) = line.strip_prefix("clip=") {
                            let clip = v.trim();
                            if !clip.is_empty() {
                                out.push(clip.to_string());
                            }
                        }
                    }
                }
            }
        }
        out.sort_by_key(|s| s.to_ascii_lowercase());
        out.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        self.anim_clip_cache = out;
        self.anim_clip_cache_dirty = false;
        self.anim_clip_cache_next_scan = now + 1.5;
    }

    fn ensure_clip_in_cache(&mut self, clip_ref: &str) {
        if clip_ref.trim().is_empty() {
            return;
        }
        if !self
            .anim_clip_cache
            .iter()
            .any(|c| c.eq_ignore_ascii_case(clip_ref))
        {
            self.anim_clip_cache.push(clip_ref.to_string());
            self.anim_clip_cache.sort_by_key(|s| s.to_ascii_lowercase());
            self.anim_clip_cache
                .dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        }
    }

    fn find_clip_by_keywords(clips: &[String], keys: &[&str]) -> Option<String> {
        clips
            .iter()
            .find(|c| {
                let low = c.to_ascii_lowercase();
                keys.iter().any(|k| low.contains(k))
            })
            .cloned()
    }

    fn seed_animation_controller_defaults(&mut self) -> usize {
        let clips = self.anim_clip_cache.clone();
        if clips.is_empty() {
            return 0;
        }

        let mut targets: Vec<String> = Vec::new();
        if let Some(idle) = Self::find_clip_by_keywords(&clips, &["::idle", " idle", "idle"]) {
            targets.push(idle);
        }
        if let Some(walk) = Self::find_clip_by_keywords(&clips, &["::walk", " walk", "walk"]) {
            targets.push(walk);
        }
        if let Some(run) = Self::find_clip_by_keywords(&clips, &["::run", " run", "sprint"]) {
            targets.push(run);
        }
        if let Some(jump) = Self::find_clip_by_keywords(&clips, &["::jump", " jump", "leap"]) {
            targets.push(jump);
        }
        if targets.is_empty() {
            targets.push(clips[0].clone());
        }
        targets.dedup_by(|a, b| a.eq_ignore_ascii_case(b));

        let base = egui::pos2(36.0, 40.0);
        let mut created_ids = Vec::new();
        for (i, clip) in targets.iter().enumerate() {
            if self
                .anim_nodes
                .iter()
                .any(|n| n.clip_ref.eq_ignore_ascii_case(clip))
            {
                continue;
            }
            let pos = base + egui::vec2((i as f32 % 2.0) * 210.0, (i as f32 / 2.0).floor() * 74.0);
            created_ids.push(self.add_anim_controller_node(clip.clone(), pos));
        }

        for w in created_ids.windows(2) {
            let from = w[0];
            let to = w[1];
            if !self.anim_links.iter().any(|l| l.from == from && l.to == to) {
                self.anim_links.push(AnimControllerLink {
                    from,
                    to,
                    blend_time: 0.3,
                    transition_type: TransitionType::CrossFade,
                });
            }
        }
        created_ids.len()
    }

    fn add_anim_controller_node(&mut self, clip_ref: String, pos: egui::Pos2) -> u32 {
        let id = self.anim_next_node_id;
        self.anim_next_node_id = self.anim_next_node_id.saturating_add(1).max(1);
        let name = clip_ref.split("::").last().unwrap_or("State").to_string();
        self.anim_nodes.push(AnimControllerNode {
            id,
            name,
            clip_ref,
            pos,
            speed: 1.0,
        });
        let clip = self
            .anim_nodes
            .last()
            .map(|n| n.clip_ref.clone())
            .unwrap_or_default();
        self.ensure_clip_in_cache(&clip);
        id
    }

    pub fn new() -> Self {
        let lua_runtime = Lua::new();
        let mut out = Self {
            controls_enabled: true,
            bindings: Self::default_bindings(),
            pressed: [false; ACTION_COUNT],
            just_pressed: [false; ACTION_COUNT],
            capture_index: None,
            status: None,
            add_icon_texture: None,
            module_add_texture: None,
            available_modules: Vec::new(),
            module_chain: Vec::new(),
            next_module_id: 1,
            control_modes: vec![FiosControlMode::Movement],
            active_control_mode: FiosControlMode::Movement,
            tab: FiosTab::Controls,
            nodes: Vec::new(),
            links: Vec::new(),
            groups: Vec::new(),
            next_node_id: 1,
            next_group_id: 1,
            drag_from_output: None,
            wire_drag_path: Vec::new(),
            selected_node: None,
            selected_nodes: HashSet::new(),
            rename_node: None,
            rename_buffer: String::new(),
            marquee_start: None,
            marquee_end: None,
            cut_points: Vec::new(),
            graph_zoom: 1.0,
            graph_pan: egui::vec2(0.0, 0.0),
            smooth_state: HashMap::new(),
            lua_enabled: false,
            lua_script: "return { x = x, y = y }".to_string(),
            lua_status: None,
            lua_runtime,
            lua_fn_key: None,
            lua_dirty: true,
            last_axis: [0.0, 0.0],
            last_look: [0.0, 0.0],
            last_action: 0.0,
            last_anim_cmd_signal: 0.0,
            prev_anim_cmd_bucket: 0,
            pending_anim_cmd: None,
            anim_nodes: Vec::new(),
            anim_links: Vec::new(),
            anim_next_node_id: 1,
            anim_drag_clip: None,
            anim_connect_from: None,
            anim_tab_status: None,
            anim_selected_nodes: HashSet::new(),
            anim_selected_link: None,
            anim_clip_cache: Vec::new(),
            anim_clip_cache_dirty: true,
            anim_clip_cache_next_scan: 0.0,
            embedded_panel_rect: None,
            anim_is_playing: false,
            anim_current_time: 0.0,
            anim_total_duration: 5.0,
            anim_is_recording: false,
            _anim_selected_track: None,
        };
        out.load_from_disk();
        out.load_lua_script_from_disk();
        if !out.load_graph_from_disk() {
            out.init_default_graph();
            let _ = out.save_graph_to_disk();
        }
        out
    }

    fn init_default_graph(&mut self) {
        if !self.nodes.is_empty() {
            return;
        }
        let input_id = self.alloc_node_id();
        let output_id = self.alloc_node_id();
        self.nodes.push(FiosNode {
            id: input_id,
            kind: FiosNodeKind::InputAxis,
            display_name: Self::default_node_name(FiosNodeKind::InputAxis).to_string(),
            pos: egui::vec2(40.0, 80.0),
            value: 0.0,
            param_a: 0.0,
            param_b: 0.0,
        });
        self.nodes.push(FiosNode {
            id: output_id,
            kind: FiosNodeKind::OutputMove,
            display_name: Self::default_node_name(FiosNodeKind::OutputMove).to_string(),
            pos: egui::vec2(360.0, 90.0),
            value: 0.0,
            param_a: 0.0,
            param_b: 0.0,
        });
        self.links.push(FiosLink {
            from_node: input_id,
            from_port: 0,
            to_node: output_id,
            to_port: 0,
        });
        self.links.push(FiosLink {
            from_node: input_id,
            from_port: 1,
            to_node: output_id,
            to_port: 1,
        });
    }

    fn alloc_node_id(&mut self) -> u32 {
        let id = self.next_node_id.max(1);
        self.next_node_id = id.wrapping_add(1).max(1);
        id
    }

    fn alloc_group_id(&mut self) -> u32 {
        let id = self.next_group_id.max(1);
        self.next_group_id = id.wrapping_add(1).max(1);
        id
    }

    fn config_path() -> PathBuf {
        PathBuf::from(".dengine_fios_controls.cfg")
    }

    fn graph_path() -> PathBuf {
        PathBuf::from(".dengine_fios_graph.cfg")
    }

    fn lua_script_path() -> PathBuf {
        PathBuf::from(".dengine_fios.lua")
    }

    fn default_bindings() -> [egui::Key; ACTION_COUNT] {
        [
            egui::Key::W,
            egui::Key::S,
            egui::Key::A,
            egui::Key::D,
            egui::Key::Space,
            egui::Key::E,
            egui::Key::Q,
            egui::Key::R,
        ]
    }

    fn key_to_string(key: egui::Key) -> &'static str {
        match key {
            egui::Key::ArrowDown => "ArrowDown",
            egui::Key::ArrowLeft => "ArrowLeft",
            egui::Key::ArrowRight => "ArrowRight",
            egui::Key::ArrowUp => "ArrowUp",
            egui::Key::Escape => "Escape",
            egui::Key::Tab => "Tab",
            egui::Key::Backspace => "Backspace",
            egui::Key::Enter => "Enter",
            egui::Key::Space => "Space",
            egui::Key::Insert => "Insert",
            egui::Key::Delete => "Delete",
            egui::Key::Home => "Home",
            egui::Key::End => "End",
            egui::Key::PageUp => "PageUp",
            egui::Key::PageDown => "PageDown",
            egui::Key::Num0 => "Num0",
            egui::Key::Num1 => "Num1",
            egui::Key::Num2 => "Num2",
            egui::Key::Num3 => "Num3",
            egui::Key::Num4 => "Num4",
            egui::Key::Num5 => "Num5",
            egui::Key::Num6 => "Num6",
            egui::Key::Num7 => "Num7",
            egui::Key::Num8 => "Num8",
            egui::Key::Num9 => "Num9",
            egui::Key::A => "A",
            egui::Key::B => "B",
            egui::Key::C => "C",
            egui::Key::D => "D",
            egui::Key::E => "E",
            egui::Key::F => "F",
            egui::Key::G => "G",
            egui::Key::H => "H",
            egui::Key::I => "I",
            egui::Key::J => "J",
            egui::Key::K => "K",
            egui::Key::L => "L",
            egui::Key::M => "M",
            egui::Key::N => "N",
            egui::Key::O => "O",
            egui::Key::P => "P",
            egui::Key::Q => "Q",
            egui::Key::R => "R",
            egui::Key::S => "S",
            egui::Key::T => "T",
            egui::Key::U => "U",
            egui::Key::V => "V",
            egui::Key::W => "W",
            egui::Key::X => "X",
            egui::Key::Y => "Y",
            egui::Key::Z => "Z",
            _ => "Unknown",
        }
    }

    fn key_from_string(s: &str) -> Option<egui::Key> {
        Some(match s.trim() {
            "ArrowDown" => egui::Key::ArrowDown,
            "ArrowLeft" => egui::Key::ArrowLeft,
            "ArrowRight" => egui::Key::ArrowRight,
            "ArrowUp" => egui::Key::ArrowUp,
            "Escape" => egui::Key::Escape,
            "Tab" => egui::Key::Tab,
            "Backspace" => egui::Key::Backspace,
            "Enter" => egui::Key::Enter,
            "Space" => egui::Key::Space,
            "Insert" => egui::Key::Insert,
            "Delete" => egui::Key::Delete,
            "Home" => egui::Key::Home,
            "End" => egui::Key::End,
            "PageUp" => egui::Key::PageUp,
            "PageDown" => egui::Key::PageDown,
            "Num0" => egui::Key::Num0,
            "Num1" => egui::Key::Num1,
            "Num2" => egui::Key::Num2,
            "Num3" => egui::Key::Num3,
            "Num4" => egui::Key::Num4,
            "Num5" => egui::Key::Num5,
            "Num6" => egui::Key::Num6,
            "Num7" => egui::Key::Num7,
            "Num8" => egui::Key::Num8,
            "Num9" => egui::Key::Num9,
            "A" => egui::Key::A,
            "B" => egui::Key::B,
            "C" => egui::Key::C,
            "D" => egui::Key::D,
            "E" => egui::Key::E,
            "F" => egui::Key::F,
            "G" => egui::Key::G,
            "H" => egui::Key::H,
            "I" => egui::Key::I,
            "J" => egui::Key::J,
            "K" => egui::Key::K,
            "L" => egui::Key::L,
            "M" => egui::Key::M,
            "N" => egui::Key::N,
            "O" => egui::Key::O,
            "P" => egui::Key::P,
            "Q" => egui::Key::Q,
            "R" => egui::Key::R,
            "S" => egui::Key::S,
            "T" => egui::Key::T,
            "U" => egui::Key::U,
            "V" => egui::Key::V,
            "W" => egui::Key::W,
            "X" => egui::Key::X,
            "Y" => egui::Key::Y,
            "Z" => egui::Key::Z,
            _ => return None,
        })
    }

    fn save_to_disk(&self) -> Result<(), String> {
        let mut out = String::new();
        for (i, action) in FiosAction::ALL.iter().enumerate() {
            out.push_str(action.id());
            out.push('=');
            out.push_str(Self::key_to_string(self.bindings[i]));
            out.push('\n');
        }
        out.push_str("lua_enabled=");
        out.push_str(if self.lua_enabled { "1" } else { "0" });
        out.push('\n');
        out.push_str("controls_enabled=");
        out.push_str(if self.controls_enabled { "1" } else { "0" });
        out.push('\n');
        fs::write(Self::config_path(), out).map_err(|e| e.to_string())
    }

    fn load_from_disk(&mut self) {
        let Ok(raw) = fs::read_to_string(Self::config_path()) else {
            return;
        };
        for line in raw.lines() {
            let mut parts = line.splitn(2, '=');
            let Some(action_id) = parts.next() else {
                continue;
            };
            let Some(key_name) = parts.next() else {
                continue;
            };
            if action_id.trim() == "lua_enabled" {
                self.lua_enabled = matches!(key_name.trim(), "1" | "true" | "on" | "yes");
                continue;
            }
            if action_id.trim() == "controls_enabled" {
                self.controls_enabled = matches!(key_name.trim(), "1" | "true" | "on" | "yes");
                continue;
            }
            let Some(key) = Self::key_from_string(key_name) else {
                continue;
            };
            if let Some(idx) = FiosAction::ALL
                .iter()
                .position(|a| a.id() == action_id.trim())
            {
                self.bindings[idx] = key;
            }
        }
    }

    fn load_lua_script_from_disk(&mut self) {
        if let Ok(raw) = fs::read_to_string(Self::lua_script_path()) {
            if !raw.trim().is_empty() {
                self.lua_script = raw;
                self.lua_dirty = true;
            }
        }
    }

    fn save_graph_to_disk(&self) -> Result<(), String> {
        let mut out = String::new();
        out.push_str("version=1\n");
        out.push_str(&format!("next_node_id={}\n", self.next_node_id));
        for n in &self.nodes {
            out.push_str(&format!(
                "node={}|{}|{}|{}|{}|{}|{}|{}\n",
                n.id,
                n.kind.id(),
                n.pos.x,
                n.pos.y,
                n.value,
                n.param_a,
                n.param_b,
                Self::encode_field(&n.display_name)
            ));
        }
        for l in &self.links {
            out.push_str(&format!(
                "link={}|{}|{}|{}\n",
                l.from_node, l.from_port, l.to_node, l.to_port
            ));
        }
        for g in &self.groups {
            let mut ids: Vec<u32> = g.nodes.iter().copied().collect();
            ids.sort_unstable();
            let ids_csv = ids
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(",");
            out.push_str(&format!(
                "group={}|{}|{}|{}|{}|{}\n",
                g.id,
                Self::encode_field(&g.name),
                g.color.r(),
                g.color.g(),
                g.color.b(),
                ids_csv
            ));
        }
        fs::write(Self::graph_path(), out).map_err(|e| e.to_string())
    }

    fn load_graph_from_disk(&mut self) -> bool {
        let Ok(raw) = fs::read_to_string(Self::graph_path()) else {
            return false;
        };
        let mut parsed_nodes = Vec::<FiosNode>::new();
        let mut parsed_links = Vec::<FiosLink>::new();
        let mut parsed_groups = Vec::<FiosGroup>::new();
        let mut next_node_id = 1_u32;
        for line in raw.lines() {
            let mut parts = line.splitn(2, '=');
            let Some(k) = parts.next() else {
                continue;
            };
            let Some(v) = parts.next() else {
                continue;
            };
            match k.trim() {
                "next_node_id" => {
                    if let Ok(n) = v.trim().parse::<u32>() {
                        next_node_id = n.max(1);
                    }
                }
                "node" => {
                    let seg: Vec<&str> = v.split('|').collect();
                    if seg.len() < 7 {
                        continue;
                    }
                    let Ok(id) = seg[0].parse::<u32>() else {
                        continue;
                    };
                    let Some(kind) = FiosNodeKind::from_id(seg[1]) else {
                        continue;
                    };
                    let Ok(x) = seg[2].parse::<f32>() else {
                        continue;
                    };
                    let Ok(y) = seg[3].parse::<f32>() else {
                        continue;
                    };
                    let Ok(value) = seg[4].parse::<f32>() else {
                        continue;
                    };
                    let Ok(param_a) = seg[5].parse::<f32>() else {
                        continue;
                    };
                    let Ok(param_b) = seg[6].parse::<f32>() else {
                        continue;
                    };
                    let display_name = if seg.len() >= 8 {
                        Self::decode_field(seg[7])
                    } else {
                        Self::default_node_name(kind).to_string()
                    };
                    parsed_nodes.push(FiosNode {
                        id,
                        kind,
                        display_name,
                        pos: egui::vec2(x, y),
                        value,
                        param_a,
                        param_b,
                    });
                }
                "link" => {
                    let seg: Vec<&str> = v.split('|').collect();
                    if seg.len() < 4 {
                        continue;
                    }
                    let Ok(from_node) = seg[0].parse::<u32>() else {
                        continue;
                    };
                    let Ok(from_port) = seg[1].parse::<u8>() else {
                        continue;
                    };
                    let Ok(to_node) = seg[2].parse::<u32>() else {
                        continue;
                    };
                    let Ok(to_port) = seg[3].parse::<u8>() else {
                        continue;
                    };
                    parsed_links.push(FiosLink {
                        from_node,
                        from_port,
                        to_node,
                        to_port,
                    });
                }
                "group" => {
                    let seg: Vec<&str> = v.split('|').collect();
                    if seg.len() < 6 {
                        continue;
                    }
                    let Ok(id) = seg[0].parse::<u32>() else {
                        continue;
                    };
                    let name = Self::decode_field(seg[1]);
                    let Ok(r) = seg[2].parse::<u8>() else {
                        continue;
                    };
                    let Ok(g) = seg[3].parse::<u8>() else {
                        continue;
                    };
                    let Ok(b) = seg[4].parse::<u8>() else {
                        continue;
                    };
                    let mut ids = HashSet::new();
                    for part in seg[5].split(',') {
                        if let Ok(v) = part.parse::<u32>() {
                            ids.insert(v);
                        }
                    }
                    parsed_groups.push(FiosGroup {
                        id,
                        name,
                        color: egui::Color32::from_rgb(r, g, b),
                        nodes: ids,
                    });
                }
                _ => {}
            }
        }
        if parsed_nodes.is_empty() {
            return false;
        }
        self.nodes = parsed_nodes;
        self.links = parsed_links;
        self.groups = parsed_groups;
        self.groups.retain(|g| !g.nodes.is_empty());
        self.next_node_id = next_node_id.max(
            self.nodes
                .iter()
                .map(|n| n.id)
                .max()
                .unwrap_or(0)
                .saturating_add(1)
                .max(1),
        );
        self.next_group_id = self
            .groups
            .iter()
            .map(|g| g.id)
            .max()
            .unwrap_or(0)
            .saturating_add(1)
            .max(1);
        self.selected_node = None;
        self.selected_nodes.clear();
        self.rename_node = None;
        self.rename_buffer.clear();
        self.smooth_state.clear();
        true
    }

    pub fn update_input(&mut self, ctx: &egui::Context) {
        self.controls_enabled = true;
        if !self.controls_enabled {
            self.pressed = [false; ACTION_COUNT];
            self.just_pressed = [false; ACTION_COUNT];
            self.last_axis = [0.0, 0.0];
            self.last_look = [0.0, 0.0];
            self.last_action = 0.0;
            self.last_anim_cmd_signal = 0.0;
            self.prev_anim_cmd_bucket = 0;
            self.pending_anim_cmd = None;
            return;
        }
        for i in 0..ACTION_COUNT {
            let down = ctx.input(|inp| inp.key_down(self.bindings[i]));
            self.just_pressed[i] = down && !self.pressed[i];
            self.pressed[i] = down;
        }

        if let Some(idx) = self.capture_index {
            let events = ctx.input(|i| i.events.clone());
            for ev in events {
                if let egui::Event::Key {
                    key, pressed: true, ..
                } = ev
                {
                    if key == egui::Key::Escape {
                        self.capture_index = None;
                        self.status = Some("Captura cancelada".to_string());
                        break;
                    }
                    self.bindings[idx] = key;
                    self.capture_index = None;
                    self.status = match self.save_to_disk() {
                        Ok(()) => Some(format!("Bind atualizado: {}", Self::key_to_string(key))),
                        Err(err) => Some(format!("Falha ao salvar bind: {err}")),
                    };
                    break;
                }
            }
        }

        let base = self.raw_movement_axis();
        let graph_axis = self.evaluate_graph_axis(base);
        self.last_look = self.evaluate_graph_look();
        self.last_action = self.evaluate_graph_action();
        self.last_anim_cmd_signal = self.evaluate_graph_anim_command_signal();
        let bucket = Self::anim_bucket(self.last_anim_cmd_signal);
        if self.prev_anim_cmd_bucket == 0 && bucket != 0 {
            self.pending_anim_cmd = Some(match bucket {
                2 => FiosAnimationCommand::PlayPause,
                1 => FiosAnimationCommand::Next,
                -1 => FiosAnimationCommand::Prev,
                _ => FiosAnimationCommand::Next,
            });
        }
        self.prev_anim_cmd_bucket = bucket;
        if self.lua_enabled {
            let dt = ctx.input(|i| i.stable_dt).max(1.0 / 240.0);
            self.last_axis = self.eval_lua_axis(graph_axis, dt);
        } else {
            self.last_axis = graph_axis;
        }
    }

    fn raw_movement_axis(&self) -> [f32; 2] {
        let x = (self.pressed[3] as i32 - self.pressed[2] as i32) as f32;
        let y = (self.pressed[0] as i32 - self.pressed[1] as i32) as f32;
        [x, y]
    }

    pub fn movement_axis(&self) -> [f32; 2] {
        self.last_axis
    }

    pub fn look_axis(&self) -> [f32; 2] {
        self.last_look
    }

    pub fn action_signal(&self) -> f32 {
        self.last_action
    }

    pub fn take_animation_command(&mut self) -> Option<FiosAnimationCommand> {
        self.pending_anim_cmd.take()
    }

    fn anim_bucket(v: f32) -> i8 {
        if v >= 1.5 {
            2
        } else if v >= 0.5 {
            1
        } else if v <= -0.5 {
            -1
        } else {
            0
        }
    }

    fn ensure_lua_compiled(&mut self) -> Result<(), String> {
        if !self.lua_dirty && self.lua_fn_key.is_some() {
            return Ok(());
        }
        self.lua_fn_key = None;
        let wrapped = format!("return function(x, y, dt)\n{}\nend", self.lua_script);
        let func: Function = self
            .lua_runtime
            .load(&wrapped)
            .eval()
            .map_err(|e| format!("Lua compile error: {e}"))?;
        let key = self
            .lua_runtime
            .create_registry_value(func)
            .map_err(|e| format!("Lua registry error: {e}"))?;
        self.lua_fn_key = Some(key);
        self.lua_dirty = false;
        Ok(())
    }

    fn eval_lua_axis(&mut self, axis: [f32; 2], dt: f32) -> [f32; 2] {
        if let Err(err) = self.ensure_lua_compiled() {
            self.lua_status = Some(err);
            return axis;
        }
        let Some(key) = &self.lua_fn_key else {
            return axis;
        };
        let func: Function = match self.lua_runtime.registry_value(key) {
            Ok(f) => f,
            Err(e) => {
                self.lua_status = Some(format!("Lua function load failed: {e}"));
                return axis;
            }
        };
        let values: MultiValue = match func.call((axis[0], axis[1], dt)) {
            Ok(v) => v,
            Err(e) => {
                self.lua_status = Some(format!("Lua runtime error: {e}"));
                return axis;
            }
        };
        self.lua_status = Some("Lua OK".to_string());
        if values.len() >= 2 {
            let x = match &values[0] {
                Value::Integer(v) => *v as f32,
                Value::Number(v) => *v as f32,
                _ => axis[0],
            };
            let y = match &values[1] {
                Value::Integer(v) => *v as f32,
                Value::Number(v) => *v as f32,
                _ => axis[1],
            };
            return [x.clamp(-1000.0, 1000.0), y.clamp(-1000.0, 1000.0)];
        }
        if values.len() == 1 {
            if let Value::Table(t) = &values[0] {
                let x = Self::lua_table_f32(t, "x", 1).unwrap_or(axis[0]);
                let y = Self::lua_table_f32(t, "y", 2).unwrap_or(axis[1]);
                return [x.clamp(-1000.0, 1000.0), y.clamp(-1000.0, 1000.0)];
            }
        }
        axis
    }

    fn lua_table_f32(table: &Table, key_name: &str, key_index: i64) -> Option<f32> {
        if let Ok(v) = table.get::<f32>(key_name) {
            return Some(v);
        }
        if let Ok(v) = table.get::<f32>(key_index) {
            return Some(v);
        }
        None
    }

    fn node_index_by_id(&self, id: u32) -> Option<usize> {
        self.nodes.iter().position(|n| n.id == id)
    }

    fn evaluate_graph_axis(&mut self, base_axis: [f32; 2]) -> [f32; 2] {
        let Some(out_id) = self
            .nodes
            .iter()
            .find(|n| n.kind == FiosNodeKind::OutputMove)
            .map(|n| n.id)
        else {
            return base_axis;
        };
        let mut cache = HashMap::<(u32, u8), f32>::new();
        let mut stack = HashSet::<(u32, u8)>::new();
        let nodes = &self.nodes;
        let links = &self.links;
        let smooth = &mut self.smooth_state;
        let x = Self::eval_input_of_node(
            nodes,
            links,
            smooth,
            &self.pressed,
            &self.just_pressed,
            out_id,
            0,
            base_axis[0],
            base_axis,
            &mut cache,
            &mut stack,
        );
        let y = Self::eval_input_of_node(
            nodes,
            links,
            smooth,
            &self.pressed,
            &self.just_pressed,
            out_id,
            1,
            base_axis[1],
            base_axis,
            &mut cache,
            &mut stack,
        );
        [x.clamp(-1000.0, 1000.0), y.clamp(-1000.0, 1000.0)]
    }

    fn evaluate_graph_look(&mut self) -> [f32; 2] {
        let Some(out_id) = self
            .nodes
            .iter()
            .find(|n| n.kind == FiosNodeKind::OutputLook)
            .map(|n| n.id)
        else {
            return [0.0, 0.0];
        };
        let mut cache = HashMap::<(u32, u8), f32>::new();
        let mut stack = HashSet::<(u32, u8)>::new();
        let nodes = &self.nodes;
        let links = &self.links;
        let smooth = &mut self.smooth_state;
        let yaw = Self::eval_input_of_node(
            nodes,
            links,
            smooth,
            &self.pressed,
            &self.just_pressed,
            out_id,
            0,
            0.0,
            [0.0, 0.0],
            &mut cache,
            &mut stack,
        );
        let pitch = Self::eval_input_of_node(
            nodes,
            links,
            smooth,
            &self.pressed,
            &self.just_pressed,
            out_id,
            1,
            0.0,
            [0.0, 0.0],
            &mut cache,
            &mut stack,
        );
        [yaw.clamp(-1000.0, 1000.0), pitch.clamp(-1000.0, 1000.0)]
    }

    fn evaluate_graph_action(&mut self) -> f32 {
        let Some(out_id) = self
            .nodes
            .iter()
            .find(|n| n.kind == FiosNodeKind::OutputAction)
            .map(|n| n.id)
        else {
            return 0.0;
        };
        let mut cache = HashMap::<(u32, u8), f32>::new();
        let mut stack = HashSet::<(u32, u8)>::new();
        let nodes = &self.nodes;
        let links = &self.links;
        let smooth = &mut self.smooth_state;
        Self::eval_input_of_node(
            nodes,
            links,
            smooth,
            &self.pressed,
            &self.just_pressed,
            out_id,
            0,
            0.0,
            [0.0, 0.0],
            &mut cache,
            &mut stack,
        )
        .clamp(-1000.0, 1000.0)
    }

    fn evaluate_graph_anim_command_signal(&mut self) -> f32 {
        let Some(out_id) = self
            .nodes
            .iter()
            .find(|n| n.kind == FiosNodeKind::OutputAnimCommand)
            .map(|n| n.id)
        else {
            return 0.0;
        };
        let mut cache = HashMap::<(u32, u8), f32>::new();
        let mut stack = HashSet::<(u32, u8)>::new();
        let nodes = &self.nodes;
        let links = &self.links;
        let smooth = &mut self.smooth_state;
        Self::eval_input_of_node(
            nodes,
            links,
            smooth,
            &self.pressed,
            &self.just_pressed,
            out_id,
            0,
            0.0,
            [0.0, 0.0],
            &mut cache,
            &mut stack,
        )
        .clamp(-1000.0, 1000.0)
    }

    fn node_index_by_id_in(nodes: &[FiosNode], id: u32) -> Option<usize> {
        nodes.iter().position(|n| n.id == id)
    }

    fn eval_input_of_node(
        nodes: &[FiosNode],
        links: &[FiosLink],
        smooth_state: &mut HashMap<(u32, u8), f32>,
        pressed: &[bool; ACTION_COUNT],
        just_pressed: &[bool; ACTION_COUNT],
        node_id: u32,
        input_port: u8,
        default: f32,
        base_axis: [f32; 2],
        cache: &mut HashMap<(u32, u8), f32>,
        stack: &mut HashSet<(u32, u8)>,
    ) -> f32 {
        for link in links.iter().rev() {
            if link.to_node == node_id && link.to_port == input_port {
                return Self::eval_output_of_node(
                    nodes,
                    links,
                    smooth_state,
                    pressed,
                    just_pressed,
                    link.from_node,
                    link.from_port,
                    base_axis,
                    cache,
                    stack,
                );
            }
        }
        default
    }

    fn eval_output_of_node(
        nodes: &[FiosNode],
        links: &[FiosLink],
        smooth_state: &mut HashMap<(u32, u8), f32>,
        pressed: &[bool; ACTION_COUNT],
        just_pressed: &[bool; ACTION_COUNT],
        node_id: u32,
        output_port: u8,
        base_axis: [f32; 2],
        cache: &mut HashMap<(u32, u8), f32>,
        stack: &mut HashSet<(u32, u8)>,
    ) -> f32 {
        let key = (node_id, output_port);
        if let Some(v) = cache.get(&key) {
            return *v;
        }
        if stack.contains(&key) {
            return 0.0;
        }
        stack.insert(key);

        let out = if let Some(idx) = Self::node_index_by_id_in(nodes, node_id) {
            let node = &nodes[idx];
            match node.kind {
                FiosNodeKind::InputAxis => {
                    if output_port == 0 {
                        base_axis[0]
                    } else {
                        base_axis[1]
                    }
                }
                FiosNodeKind::InputAction => {
                    let action_idx = node
                        .param_a
                        .round()
                        .clamp(0.0, (ACTION_COUNT.saturating_sub(1)) as f32)
                        as usize;
                    let mode_just = node.param_b.round() >= 1.0;
                    let active = if mode_just {
                        just_pressed[action_idx]
                    } else {
                        pressed[action_idx]
                    };
                    if active { 1.0 } else { 0.0 }
                }
                FiosNodeKind::Constant => node.value,
                FiosNodeKind::Add => {
                    let a = Self::eval_input_of_node(
                        nodes,
                        links,
                        smooth_state,
                        pressed,
                        just_pressed,
                        node_id,
                        0,
                        0.0,
                        base_axis,
                        cache,
                        stack,
                    );
                    let b = Self::eval_input_of_node(
                        nodes,
                        links,
                        smooth_state,
                        pressed,
                        just_pressed,
                        node_id,
                        1,
                        0.0,
                        base_axis,
                        cache,
                        stack,
                    );
                    a + b
                }
                FiosNodeKind::Subtract => {
                    let a = Self::eval_input_of_node(
                        nodes,
                        links,
                        smooth_state,
                        pressed,
                        just_pressed,
                        node_id,
                        0,
                        0.0,
                        base_axis,
                        cache,
                        stack,
                    );
                    let b = Self::eval_input_of_node(
                        nodes,
                        links,
                        smooth_state,
                        pressed,
                        just_pressed,
                        node_id,
                        1,
                        0.0,
                        base_axis,
                        cache,
                        stack,
                    );
                    a - b
                }
                FiosNodeKind::Multiply => {
                    let a = Self::eval_input_of_node(
                        nodes,
                        links,
                        smooth_state,
                        pressed,
                        just_pressed,
                        node_id,
                        0,
                        0.0,
                        base_axis,
                        cache,
                        stack,
                    );
                    let b = Self::eval_input_of_node(
                        nodes,
                        links,
                        smooth_state,
                        pressed,
                        just_pressed,
                        node_id,
                        1,
                        0.0,
                        base_axis,
                        cache,
                        stack,
                    );
                    a * b
                }
                FiosNodeKind::Divide => {
                    let a = Self::eval_input_of_node(
                        nodes,
                        links,
                        smooth_state,
                        pressed,
                        just_pressed,
                        node_id,
                        0,
                        0.0,
                        base_axis,
                        cache,
                        stack,
                    );
                    let b = Self::eval_input_of_node(
                        nodes,
                        links,
                        smooth_state,
                        pressed,
                        just_pressed,
                        node_id,
                        1,
                        1.0,
                        base_axis,
                        cache,
                        stack,
                    );
                    if b.abs() < 1e-5 { 0.0 } else { a / b }
                }
                FiosNodeKind::Max => {
                    let a = Self::eval_input_of_node(
                        nodes,
                        links,
                        smooth_state,
                        pressed,
                        just_pressed,
                        node_id,
                        0,
                        0.0,
                        base_axis,
                        cache,
                        stack,
                    );
                    let b = Self::eval_input_of_node(
                        nodes,
                        links,
                        smooth_state,
                        pressed,
                        just_pressed,
                        node_id,
                        1,
                        0.0,
                        base_axis,
                        cache,
                        stack,
                    );
                    a.max(b)
                }
                FiosNodeKind::Min => {
                    let a = Self::eval_input_of_node(
                        nodes,
                        links,
                        smooth_state,
                        pressed,
                        just_pressed,
                        node_id,
                        0,
                        0.0,
                        base_axis,
                        cache,
                        stack,
                    );
                    let b = Self::eval_input_of_node(
                        nodes,
                        links,
                        smooth_state,
                        pressed,
                        just_pressed,
                        node_id,
                        1,
                        0.0,
                        base_axis,
                        cache,
                        stack,
                    );
                    a.min(b)
                }
                FiosNodeKind::Gate => {
                    let v = Self::eval_input_of_node(
                        nodes,
                        links,
                        smooth_state,
                        pressed,
                        just_pressed,
                        node_id,
                        0,
                        0.0,
                        base_axis,
                        cache,
                        stack,
                    );
                    let g = Self::eval_input_of_node(
                        nodes,
                        links,
                        smooth_state,
                        pressed,
                        just_pressed,
                        node_id,
                        1,
                        0.0,
                        base_axis,
                        cache,
                        stack,
                    );
                    if g > 0.0 { v } else { 0.0 }
                }
                FiosNodeKind::Abs => Self::eval_input_of_node(
                    nodes,
                    links,
                    smooth_state,
                    pressed,
                    just_pressed,
                    node_id,
                    0,
                    0.0,
                    base_axis,
                    cache,
                    stack,
                )
                .abs(),
                FiosNodeKind::Sign => Self::eval_input_of_node(
                    nodes,
                    links,
                    smooth_state,
                    pressed,
                    just_pressed,
                    node_id,
                    0,
                    0.0,
                    base_axis,
                    cache,
                    stack,
                )
                .signum(),
                FiosNodeKind::Clamp => {
                    let v = Self::eval_input_of_node(
                        nodes,
                        links,
                        smooth_state,
                        pressed,
                        just_pressed,
                        node_id,
                        0,
                        0.0,
                        base_axis,
                        cache,
                        stack,
                    );
                    v.clamp(
                        node.param_a.min(node.param_b),
                        node.param_a.max(node.param_b),
                    )
                }
                FiosNodeKind::Deadzone => {
                    let v = Self::eval_input_of_node(
                        nodes,
                        links,
                        smooth_state,
                        pressed,
                        just_pressed,
                        node_id,
                        0,
                        0.0,
                        base_axis,
                        cache,
                        stack,
                    );
                    let t = node.param_a.abs().clamp(0.0, 1.0);
                    if v.abs() < t { 0.0 } else { v }
                }
                FiosNodeKind::Invert => -Self::eval_input_of_node(
                    nodes,
                    links,
                    smooth_state,
                    pressed,
                    just_pressed,
                    node_id,
                    0,
                    0.0,
                    base_axis,
                    cache,
                    stack,
                ),
                FiosNodeKind::Smooth => {
                    let target = Self::eval_input_of_node(
                        nodes,
                        links,
                        smooth_state,
                        pressed,
                        just_pressed,
                        node_id,
                        0,
                        0.0,
                        base_axis,
                        cache,
                        stack,
                    );
                    let alpha = node.param_a.clamp(0.0, 1.0);
                    let prev = *smooth_state.get(&key).unwrap_or(&target);
                    let v = prev + (target - prev) * alpha;
                    smooth_state.insert(key, v);
                    v
                }
                FiosNodeKind::OutputMove
                | FiosNodeKind::OutputLook
                | FiosNodeKind::OutputAction
                | FiosNodeKind::OutputAnimCommand => 0.0,
            }
        } else {
            0.0
        };

        stack.remove(&key);
        cache.insert(key, out);
        out
    }

    fn create_link(&mut self, from_node: u32, from_port: u8, to_node: u32, to_port: u8) {
        self.links
            .retain(|l| !(l.to_node == to_node && l.to_port == to_port));
        self.links.push(FiosLink {
            from_node,
            from_port,
            to_node,
            to_port,
        });
        let _ = self.save_graph_to_disk();
    }

    fn node_size(kind: FiosNodeKind) -> egui::Vec2 {
        match kind {
            FiosNodeKind::InputAxis => egui::vec2(170.0, 74.0),
            FiosNodeKind::InputAction => egui::vec2(190.0, 96.0),
            FiosNodeKind::Constant => egui::vec2(170.0, 88.0),
            FiosNodeKind::Add
            | FiosNodeKind::Subtract
            | FiosNodeKind::Multiply
            | FiosNodeKind::Divide
            | FiosNodeKind::Max
            | FiosNodeKind::Min
            | FiosNodeKind::Gate => egui::vec2(170.0, 84.0),
            FiosNodeKind::Abs
            | FiosNodeKind::Sign
            | FiosNodeKind::Clamp
            | FiosNodeKind::Deadzone
            | FiosNodeKind::Invert
            | FiosNodeKind::Smooth => egui::vec2(180.0, 94.0),
            FiosNodeKind::OutputMove | FiosNodeKind::OutputLook => egui::vec2(190.0, 88.0),
            FiosNodeKind::OutputAction | FiosNodeKind::OutputAnimCommand => egui::vec2(170.0, 74.0),
        }
    }

    fn input_port_pos(rect: egui::Rect, kind: FiosNodeKind, idx: usize) -> egui::Pos2 {
        let n = kind.input_count().max(1) as f32;
        let y = rect.top() + 32.0 + ((idx as f32 + 0.5) * ((rect.height() - 36.0) / n));
        egui::pos2(rect.left() + 4.0, y)
    }

    fn output_port_pos(rect: egui::Rect, kind: FiosNodeKind, idx: usize) -> egui::Pos2 {
        let n = kind.output_count().max(1) as f32;
        let y = rect.top() + 32.0 + ((idx as f32 + 0.5) * ((rect.height() - 36.0) / n));
        egui::pos2(rect.right() - 4.0, y)
    }

    fn add_node(&mut self, kind: FiosNodeKind) {
        let id = self.alloc_node_id();
        let slot = (id % 6) as f32;
        let (value, param_a, param_b) = match kind {
            FiosNodeKind::InputAction => (0.0, 0.0, 0.0),
            FiosNodeKind::Constant => (1.0, 0.0, 0.0),
            FiosNodeKind::Clamp => (0.0, -1.0, 1.0),
            FiosNodeKind::Deadzone => (0.0, 0.15, 0.0),
            FiosNodeKind::Smooth => (0.0, 0.2, 0.0),
            _ => (0.0, 0.0, 0.0),
        };
        self.nodes.push(FiosNode {
            id,
            kind,
            display_name: Self::default_node_name(kind).to_string(),
            pos: egui::vec2(40.0 + slot * 26.0, 70.0 + slot * 18.0),
            value,
            param_a,
            param_b,
        });
        self.selected_node = Some(id);
        self.selected_nodes.clear();
        self.selected_nodes.insert(id);
        let _ = self.save_graph_to_disk();
    }

    fn add_node_custom(
        &mut self,
        kind: FiosNodeKind,
        pos: egui::Vec2,
        value: f32,
        param_a: f32,
        param_b: f32,
    ) -> u32 {
        let id = self.alloc_node_id();
        self.nodes.push(FiosNode {
            id,
            kind,
            display_name: Self::default_node_name(kind).to_string(),
            pos,
            value,
            param_a,
            param_b,
        });
        id
    }

    fn first_node_id_of_kind(&self, kind: FiosNodeKind) -> Option<u32> {
        self.nodes.iter().find(|n| n.kind == kind).map(|n| n.id)
    }

    fn add_module_move_basic(&mut self) -> Option<u32> {
        let input_id = self
            .first_node_id_of_kind(FiosNodeKind::InputAxis)
            .unwrap_or_else(|| {
                self.add_node_custom(
                    FiosNodeKind::InputAxis,
                    egui::vec2(60.0, 120.0),
                    0.0,
                    0.0,
                    0.0,
                )
            });
        let output_id = self
            .first_node_id_of_kind(FiosNodeKind::OutputMove)
            .unwrap_or_else(|| {
                self.add_node_custom(
                    FiosNodeKind::OutputMove,
                    egui::vec2(420.0, 120.0),
                    0.0,
                    0.0,
                    0.0,
                )
            });
        self.create_link(input_id, 0, output_id, 0);
        self.create_link(input_id, 1, output_id, 1);
        let group = self.create_module_group(
            "Módulo Movimento Básico",
            egui::Color32::from_rgb(72, 132, 102),
            vec![input_id, output_id],
        );
        let _ = self.save_graph_to_disk();
        group
    }

    fn add_module_look_basic(&mut self) -> Option<u32> {
        let input_id = self
            .first_node_id_of_kind(FiosNodeKind::InputAxis)
            .unwrap_or_else(|| {
                self.add_node_custom(
                    FiosNodeKind::InputAxis,
                    egui::vec2(60.0, 240.0),
                    0.0,
                    0.0,
                    0.0,
                )
            });
        let output_id = self
            .first_node_id_of_kind(FiosNodeKind::OutputLook)
            .unwrap_or_else(|| {
                self.add_node_custom(
                    FiosNodeKind::OutputLook,
                    egui::vec2(420.0, 240.0),
                    0.0,
                    0.0,
                    0.0,
                )
            });
        self.create_link(input_id, 0, output_id, 0);
        self.create_link(input_id, 1, output_id, 1);
        let group = self.create_module_group(
            "Módulo Look Básico",
            egui::Color32::from_rgb(72, 108, 132),
            vec![input_id, output_id],
        );
        let _ = self.save_graph_to_disk();
        group
    }

    fn add_module_action_basic(&mut self, action_idx: usize) -> Option<u32> {
        let idx = action_idx.min(ACTION_COUNT.saturating_sub(1));
        let input_id = self.add_node_custom(
            FiosNodeKind::InputAction,
            egui::vec2(60.0, 360.0),
            0.0,
            idx as f32,
            0.0,
        );
        let output_id = self
            .first_node_id_of_kind(FiosNodeKind::OutputAction)
            .unwrap_or_else(|| {
                self.add_node_custom(
                    FiosNodeKind::OutputAction,
                    egui::vec2(420.0, 360.0),
                    0.0,
                    0.0,
                    0.0,
                )
            });
        self.create_link(input_id, 0, output_id, 0);
        let group = self.create_module_group(
            "Módulo Ação Básica",
            egui::Color32::from_rgb(158, 102, 62),
            vec![input_id, output_id],
        );
        let _ = self.save_graph_to_disk();
        group
    }

    fn add_module_move_advanced(&mut self) -> Option<u32> {
        let input_id = self
            .first_node_id_of_kind(FiosNodeKind::InputAxis)
            .unwrap_or_else(|| {
                self.add_node_custom(
                    FiosNodeKind::InputAxis,
                    egui::vec2(40.0, 110.0),
                    0.0,
                    0.0,
                    0.0,
                )
            });
        let output_id = self
            .first_node_id_of_kind(FiosNodeKind::OutputMove)
            .unwrap_or_else(|| {
                self.add_node_custom(
                    FiosNodeKind::OutputMove,
                    egui::vec2(700.0, 130.0),
                    0.0,
                    0.0,
                    0.0,
                )
            });

        let dz_x = self.add_node_custom(
            FiosNodeKind::Deadzone,
            egui::vec2(190.0, 70.0),
            0.0,
            0.15,
            0.0,
        );
        let dz_y = self.add_node_custom(
            FiosNodeKind::Deadzone,
            egui::vec2(190.0, 200.0),
            0.0,
            0.15,
            0.0,
        );
        let sm_x = self.add_node_custom(
            FiosNodeKind::Smooth,
            egui::vec2(340.0, 70.0),
            0.0,
            0.25,
            0.0,
        );
        let sm_y = self.add_node_custom(
            FiosNodeKind::Smooth,
            egui::vec2(340.0, 200.0),
            0.0,
            0.25,
            0.0,
        );
        let kx = self.add_node_custom(
            FiosNodeKind::Constant,
            egui::vec2(480.0, 30.0),
            1.0,
            0.0,
            0.0,
        );
        let ky = self.add_node_custom(
            FiosNodeKind::Constant,
            egui::vec2(480.0, 160.0),
            1.0,
            0.0,
            0.0,
        );
        let mx = self.add_node_custom(
            FiosNodeKind::Multiply,
            egui::vec2(560.0, 70.0),
            0.0,
            0.0,
            0.0,
        );
        let my = self.add_node_custom(
            FiosNodeKind::Multiply,
            egui::vec2(560.0, 200.0),
            0.0,
            0.0,
            0.0,
        );

        self.create_link(input_id, 0, dz_x, 0);
        self.create_link(input_id, 1, dz_y, 0);
        self.create_link(dz_x, 0, sm_x, 0);
        self.create_link(dz_y, 0, sm_y, 0);
        self.create_link(sm_x, 0, mx, 0);
        self.create_link(kx, 0, mx, 1);
        self.create_link(sm_y, 0, my, 0);
        self.create_link(ky, 0, my, 1);
        self.create_link(mx, 0, output_id, 0);
        self.create_link(my, 0, output_id, 1);
        let group = self.create_module_group(
            "Módulo Movimento Avançado",
            egui::Color32::from_rgb(72, 132, 102),
            vec![input_id, output_id, dz_x, dz_y, sm_x, sm_y, kx, ky, mx, my],
        );
        let _ = self.save_graph_to_disk();
        group
    }

    fn add_module_look_advanced(&mut self) -> Option<u32> {
        let input_id = self
            .first_node_id_of_kind(FiosNodeKind::InputAxis)
            .unwrap_or_else(|| {
                self.add_node_custom(
                    FiosNodeKind::InputAxis,
                    egui::vec2(40.0, 360.0),
                    0.0,
                    0.0,
                    0.0,
                )
            });
        let output_id = self
            .first_node_id_of_kind(FiosNodeKind::OutputLook)
            .unwrap_or_else(|| {
                self.add_node_custom(
                    FiosNodeKind::OutputLook,
                    egui::vec2(700.0, 380.0),
                    0.0,
                    0.0,
                    0.0,
                )
            });

        let dz_yaw = self.add_node_custom(
            FiosNodeKind::Deadzone,
            egui::vec2(190.0, 320.0),
            0.0,
            0.08,
            0.0,
        );
        let dz_pitch = self.add_node_custom(
            FiosNodeKind::Deadzone,
            egui::vec2(190.0, 450.0),
            0.0,
            0.08,
            0.0,
        );
        let sm_yaw = self.add_node_custom(
            FiosNodeKind::Smooth,
            egui::vec2(340.0, 320.0),
            0.0,
            0.18,
            0.0,
        );
        let sm_pitch = self.add_node_custom(
            FiosNodeKind::Smooth,
            egui::vec2(340.0, 450.0),
            0.0,
            0.18,
            0.0,
        );
        let kyaw = self.add_node_custom(
            FiosNodeKind::Constant,
            egui::vec2(480.0, 280.0),
            1.0,
            0.0,
            0.0,
        );
        let kpitch = self.add_node_custom(
            FiosNodeKind::Constant,
            egui::vec2(480.0, 410.0),
            1.0,
            0.0,
            0.0,
        );
        let myaw = self.add_node_custom(
            FiosNodeKind::Multiply,
            egui::vec2(560.0, 320.0),
            0.0,
            0.0,
            0.0,
        );
        let mpitch = self.add_node_custom(
            FiosNodeKind::Multiply,
            egui::vec2(560.0, 450.0),
            0.0,
            0.0,
            0.0,
        );

        self.create_link(input_id, 0, dz_yaw, 0);
        self.create_link(input_id, 1, dz_pitch, 0);
        self.create_link(dz_yaw, 0, sm_yaw, 0);
        self.create_link(dz_pitch, 0, sm_pitch, 0);
        self.create_link(sm_yaw, 0, myaw, 0);
        self.create_link(kyaw, 0, myaw, 1);
        self.create_link(sm_pitch, 0, mpitch, 0);
        self.create_link(kpitch, 0, mpitch, 1);
        self.create_link(myaw, 0, output_id, 0);
        self.create_link(mpitch, 0, output_id, 1);
        let group = self.create_module_group(
            "Módulo Look Avançado",
            egui::Color32::from_rgb(72, 108, 132),
            vec![
                input_id, output_id, dz_yaw, dz_pitch, sm_yaw, sm_pitch, kyaw, kpitch, myaw, mpitch,
            ],
        );
        let _ = self.save_graph_to_disk();
        group
    }

    fn add_module_animation_controls(&mut self) -> Option<u32> {
        let out_id = self
            .first_node_id_of_kind(FiosNodeKind::OutputAnimCommand)
            .unwrap_or_else(|| {
                self.add_node_custom(
                    FiosNodeKind::OutputAnimCommand,
                    egui::vec2(760.0, 340.0),
                    0.0,
                    0.0,
                    0.0,
                )
            });

        let in_play = self.add_node_custom(
            FiosNodeKind::InputAction,
            egui::vec2(120.0, 260.0),
            0.0,
            FiosAction::Jump.index() as f32,
            1.0,
        );
        let in_next = self.add_node_custom(
            FiosNodeKind::InputAction,
            egui::vec2(120.0, 340.0),
            0.0,
            FiosAction::Action1.index() as f32,
            1.0,
        );
        let in_prev = self.add_node_custom(
            FiosNodeKind::InputAction,
            egui::vec2(120.0, 420.0),
            0.0,
            FiosAction::Action2.index() as f32,
            1.0,
        );
        let c_play = self.add_node_custom(
            FiosNodeKind::Constant,
            egui::vec2(300.0, 220.0),
            2.0,
            0.0,
            0.0,
        );
        let c_next = self.add_node_custom(
            FiosNodeKind::Constant,
            egui::vec2(300.0, 300.0),
            1.0,
            0.0,
            0.0,
        );
        let c_prev = self.add_node_custom(
            FiosNodeKind::Constant,
            egui::vec2(300.0, 380.0),
            -1.0,
            0.0,
            0.0,
        );
        let m_play = self.add_node_custom(
            FiosNodeKind::Multiply,
            egui::vec2(430.0, 260.0),
            0.0,
            0.0,
            0.0,
        );
        let m_next = self.add_node_custom(
            FiosNodeKind::Multiply,
            egui::vec2(430.0, 340.0),
            0.0,
            0.0,
            0.0,
        );
        let m_prev = self.add_node_custom(
            FiosNodeKind::Multiply,
            egui::vec2(430.0, 420.0),
            0.0,
            0.0,
            0.0,
        );
        let add_1 =
            self.add_node_custom(FiosNodeKind::Add, egui::vec2(590.0, 320.0), 0.0, 0.0, 0.0);
        let add_2 =
            self.add_node_custom(FiosNodeKind::Add, egui::vec2(680.0, 360.0), 0.0, 0.0, 0.0);

        self.create_link(in_play, 0, m_play, 0);
        self.create_link(c_play, 0, m_play, 1);
        self.create_link(in_next, 0, m_next, 0);
        self.create_link(c_next, 0, m_next, 1);
        self.create_link(in_prev, 0, m_prev, 0);
        self.create_link(c_prev, 0, m_prev, 1);
        self.create_link(m_next, 0, add_1, 0);
        self.create_link(m_prev, 0, add_1, 1);
        self.create_link(add_1, 0, add_2, 0);
        self.create_link(m_play, 0, add_2, 1);
        self.create_link(add_2, 0, out_id, 0);
        let group = self.create_module_group(
            "Módulo Animação",
            egui::Color32::from_rgb(122, 88, 152),
            vec![
                out_id, in_play, in_next, in_prev, c_play, c_next, c_prev, m_play, m_next, m_prev,
                add_1, add_2,
            ],
        );
        let _ = self.save_graph_to_disk();
        group
    }

    fn create_module_group(
        &mut self,
        name: &str,
        color: egui::Color32,
        node_ids: Vec<u32>,
    ) -> Option<u32> {
        let nodes: HashSet<u32> = node_ids.into_iter().collect();
        if nodes.is_empty() {
            return None;
        }
        let id = self.alloc_group_id();
        self.groups.push(FiosGroup {
            id,
            name: format!("{name} {id}"),
            color,
            nodes: nodes.clone(),
        });
        Some(id)
    }

    fn remove_module_group(&mut self, group_id: u32) {
        let mut nodes_to_remove: Option<HashSet<u32>> = None;
        self.groups.retain(|group| {
            if group.id == group_id {
                nodes_to_remove = Some(group.nodes.clone());
                false
            } else {
                true
            }
        });
        if let Some(nodes) = nodes_to_remove {
            self.nodes.retain(|node| !nodes.contains(&node.id));
            self.links
                .retain(|link| !nodes.contains(&link.from_node) && !nodes.contains(&link.to_node));
            let _ = self.save_graph_to_disk();
        }
    }

    fn remove_selected_nodes(&mut self) -> bool {
        if self.selected_nodes.is_empty() {
            if let Some(id) = self.selected_node {
                self.selected_nodes.insert(id);
            }
        }
        if self.selected_nodes.is_empty() {
            return false;
        }
        self.nodes.retain(|n| !self.selected_nodes.contains(&n.id));
        self.links.retain(|l| {
            !self.selected_nodes.contains(&l.from_node) && !self.selected_nodes.contains(&l.to_node)
        });
        for g in &mut self.groups {
            for id in &self.selected_nodes {
                g.nodes.remove(id);
            }
        }
        self.groups.retain(|g| !g.nodes.is_empty());
        self.drag_from_output = None;
        self.rename_node = None;
        self.rename_buffer.clear();
        self.selected_nodes.clear();
        self.selected_node = None;
        self.smooth_state.clear();
        true
    }

    fn group_selected_nodes(&mut self) -> bool {
        if self.selected_nodes.len() < 2 {
            return false;
        }
        let id = self.alloc_group_id();
        let name = format!("Grupo {}", id);
        self.groups.push(FiosGroup {
            id,
            name,
            color: egui::Color32::from_rgb(72, 108, 132),
            nodes: self.selected_nodes.clone(),
        });
        true
    }

    fn recolor_selected_groups(&mut self, color: egui::Color32) -> bool {
        if self.selected_nodes.is_empty() {
            return false;
        }
        let mut changed = false;
        for g in &mut self.groups {
            if self.selected_nodes.iter().any(|id| g.nodes.contains(id)) {
                g.color = color;
                changed = true;
            }
        }
        changed
    }

    fn seg_intersects(a: egui::Pos2, b: egui::Pos2, c: egui::Pos2, d: egui::Pos2) -> bool {
        fn orient(a: egui::Pos2, b: egui::Pos2, c: egui::Pos2) -> f32 {
            (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
        }
        fn between(a: f32, b: f32, x: f32) -> bool {
            x >= a.min(b) - 1e-5 && x <= a.max(b) + 1e-5
        }
        let o1 = orient(a, b, c);
        let o2 = orient(a, b, d);
        let o3 = orient(c, d, a);
        let o4 = orient(c, d, b);
        if o1.abs() < 1e-5 && between(a.x, b.x, c.x) && between(a.y, b.y, c.y) {
            return true;
        }
        if o2.abs() < 1e-5 && between(a.x, b.x, d.x) && between(a.y, b.y, d.y) {
            return true;
        }
        if o3.abs() < 1e-5 && between(c.x, d.x, a.x) && between(c.y, d.y, a.y) {
            return true;
        }
        if o4.abs() < 1e-5 && between(c.x, d.x, b.x) && between(c.y, d.y, b.y) {
            return true;
        }
        (o1 > 0.0) != (o2 > 0.0) && (o3 > 0.0) != (o4 > 0.0)
    }

    fn draw_graph(&mut self, ui: &mut egui::Ui, lang: EngineLanguage) {
        let mut graph_dirty = false;
        let (
            input_axis_txt,
            input_action_txt,
            const_txt,
            add_txt,
            sub_txt,
            mul_txt,
            div_txt,
            max_txt,
            min_txt,
            gate_txt,
            abs_txt,
            sign_txt,
            clamp_txt,
            deadzone_txt,
            invert_txt,
            smooth_txt,
            output_move_txt,
            output_look_txt,
            output_action_txt,
            output_anim_cmd_txt,
            selected_txt,
            none_txt,
            rename_txt,
            apply_name_txt,
            add_block_txt,
            modules_txt,
            module_move_txt,
            module_move_adv_txt,
            module_look_txt,
            module_look_adv_txt,
            module_action1_txt,
            module_jump_txt,
            actions_txt,
            del_txt,
        ) = match lang {
            EngineLanguage::Pt => (
                "Entrada Eixo",
                "Entrada Ação",
                "Constante",
                "Somar",
                "Subtrair",
                "Multiplicar",
                "Dividir",
                "Máximo",
                "Mínimo",
                "Portão",
                "Absoluto",
                "Sinal",
                "Limitar",
                "Zona Morta",
                "Inverter",
                "Suavizar",
                "Saída Mover",
                "Saída Olhar",
                "Saída Ação",
                "Saída Cmd Anim",
                "Selecionado(s)",
                "Nenhum",
                "Renomear",
                "Aplicar Nome",
                "Add Bloco",
                "Módulos",
                "Locomoção Básica",
                "Locomoção Avançada",
                "Look Básico",
                "Look Avançado",
                "Ação 1 Básica",
                "Pulo Básico",
                "Ações",
                "Excluir Selecionado",
            ),
            EngineLanguage::En => (
                "Input Axis",
                "Input Action",
                "Constant",
                "Add",
                "Subtract",
                "Multiply",
                "Divide",
                "Max",
                "Min",
                "Gate",
                "Abs",
                "Sign",
                "Clamp",
                "Deadzone",
                "Invert",
                "Smooth",
                "Output Move",
                "Output Look",
                "Output Action",
                "Output Anim Cmd",
                "Selected",
                "None",
                "Rename",
                "Apply Name",
                "Add Block",
                "Modules",
                "Basic Locomotion",
                "Advanced Locomotion",
                "Basic Look",
                "Advanced Look",
                "Basic Action 1",
                "Basic Jump",
                "Actions",
                "Delete Selected",
            ),
            EngineLanguage::Es => (
                "Entrada Eje",
                "Entrada Accion",
                "Constante",
                "Sumar",
                "Restar",
                "Multiplicar",
                "Dividir",
                "Maximo",
                "Minimo",
                "Compuerta",
                "Absoluto",
                "Signo",
                "Limitar",
                "Zona Muerta",
                "Invertir",
                "Suavizar",
                "Salida Mover",
                "Salida Mirar",
                "Salida Accion",
                "Salida Cmd Anim",
                "Seleccionado(s)",
                "Ninguno",
                "Renombrar",
                "Aplicar Nombre",
                "Agregar Bloque",
                "Modulos",
                "Locomocion Basica",
                "Locomocion Avanzada",
                "Look Basico",
                "Look Avanzado",
                "Accion 1 Basica",
                "Salto Basico",
                "Acciones",
                "Eliminar Seleccionado",
            ),
        };

        ui.group(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    egui::RichText::new(add_block_txt)
                        .strong()
                        .color(egui::Color32::from_gray(220)),
                );
                ui.separator();
                ui.menu_button(
                    egui::RichText::new(add_block_txt)
                        .strong()
                        .color(egui::Color32::from_rgb(20, 24, 20))
                        .background_color(egui::Color32::from_rgb(15, 232, 121)),
                    |ui| {
                        if ui.button(input_axis_txt).clicked() {
                            self.add_node(FiosNodeKind::InputAxis);
                            ui.close();
                        }
                        if ui.button(input_action_txt).clicked() {
                            self.add_node(FiosNodeKind::InputAction);
                            ui.close();
                        }
                        if ui.button(const_txt).clicked() {
                            self.add_node(FiosNodeKind::Constant);
                            ui.close();
                        }
                        if ui.button(add_txt).clicked() {
                            self.add_node(FiosNodeKind::Add);
                            ui.close();
                        }
                        if ui.button(sub_txt).clicked() {
                            self.add_node(FiosNodeKind::Subtract);
                            ui.close();
                        }
                        if ui.button(mul_txt).clicked() {
                            self.add_node(FiosNodeKind::Multiply);
                            ui.close();
                        }
                        if ui.button(div_txt).clicked() {
                            self.add_node(FiosNodeKind::Divide);
                            ui.close();
                        }
                        if ui.button(max_txt).clicked() {
                            self.add_node(FiosNodeKind::Max);
                            ui.close();
                        }
                        if ui.button(min_txt).clicked() {
                            self.add_node(FiosNodeKind::Min);
                            ui.close();
                        }
                        if ui.button(gate_txt).clicked() {
                            self.add_node(FiosNodeKind::Gate);
                            ui.close();
                        }
                        if ui.button(abs_txt).clicked() {
                            self.add_node(FiosNodeKind::Abs);
                            ui.close();
                        }
                        if ui.button(sign_txt).clicked() {
                            self.add_node(FiosNodeKind::Sign);
                            ui.close();
                        }
                        if ui.button(clamp_txt).clicked() {
                            self.add_node(FiosNodeKind::Clamp);
                            ui.close();
                        }
                        if ui.button(deadzone_txt).clicked() {
                            self.add_node(FiosNodeKind::Deadzone);
                            ui.close();
                        }
                        if ui.button(invert_txt).clicked() {
                            self.add_node(FiosNodeKind::Invert);
                            ui.close();
                        }
                        if ui.button(smooth_txt).clicked() {
                            self.add_node(FiosNodeKind::Smooth);
                            ui.close();
                        }
                        if ui.button(output_move_txt).clicked() {
                            self.add_node(FiosNodeKind::OutputMove);
                            ui.close();
                        }
                        if ui.button(output_look_txt).clicked() {
                            self.add_node(FiosNodeKind::OutputLook);
                            ui.close();
                        }
                        if ui.button(output_action_txt).clicked() {
                            self.add_node(FiosNodeKind::OutputAction);
                            ui.close();
                        }
                        if ui.button(output_anim_cmd_txt).clicked() {
                            self.add_node(FiosNodeKind::OutputAnimCommand);
                            ui.close();
                        }
                    },
                );
                ui.separator();
                ui.menu_button(
                    egui::RichText::new(modules_txt)
                        .strong()
                        .color(egui::Color32::from_rgb(238, 232, 255))
                        .background_color(egui::Color32::from_rgb(108, 76, 156)),
                    |ui| {
                        if self.module_menu_content(ui, lang) {
                            ui.close_kind(UiKind::Menu);
                        }
                    },
                );
            });
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                let selected_count = self.selected_nodes.len();
                let selected_text = if selected_count == 0 {
                    none_txt.to_string()
                } else {
                    selected_count.to_string()
                };
                ui.label(
                    egui::RichText::new(format!(
                        "{actions_txt}  |  {selected_txt}: {selected_text}"
                    ))
                    .strong()
                    .color(egui::Color32::from_gray(220)),
                );
                if ui
                    .add_sized(
                        egui::vec2(140.0, 26.0),
                        egui::Button::new(del_txt).fill(egui::Color32::from_rgb(128, 72, 78)),
                    )
                    .clicked()
                    && self.remove_selected_nodes()
                {
                    graph_dirty = true;
                }
                if ui
                    .add_sized(egui::vec2(120.0, 26.0), egui::Button::new(rename_txt))
                    .clicked()
                {
                    if let Some(id) = self.selected_nodes.iter().next().copied() {
                        self.rename_node = Some(id);
                        if let Some(i) = self.node_index_by_id(id) {
                            self.rename_buffer = self.nodes[i].display_name.clone();
                        }
                    }
                }
                if self.rename_node.is_some() {
                    ui.add_sized(
                        [190.0, 26.0],
                        egui::TextEdit::singleline(&mut self.rename_buffer),
                    );
                    if ui
                        .add_sized(egui::vec2(130.0, 26.0), egui::Button::new(apply_name_txt))
                        .clicked()
                    {
                        if let Some(id) = self.rename_node {
                            if let Some(i) = self.node_index_by_id(id) {
                                let nm = self.rename_buffer.trim();
                                if !nm.is_empty() {
                                    self.nodes[i].display_name = nm.to_string();
                                    graph_dirty = true;
                                }
                            }
                        }
                        self.rename_node = None;
                        self.rename_buffer.clear();
                    }
                }
            });
            ui.add_space(2.0);
        });
        ui.add_space(6.0);

        let canvas_size = ui.available_size();
        let (canvas_rect, canvas_resp) =
            ui.allocate_exact_size(canvas_size, egui::Sense::click_and_drag());
        let painter = ui.painter_at(canvas_rect);
        painter.rect_filled(canvas_rect, 6.0, egui::Color32::from_rgb(21, 22, 24));
        painter.rect_stroke(
            canvas_rect,
            6.0,
            egui::Stroke::new(1.0, egui::Color32::from_rgb(64, 66, 72)),
            egui::StrokeKind::Outside,
        );

        let pointer_pos = ui.ctx().input(|i| i.pointer.interact_pos());
        let pointer_inside_canvas = pointer_pos
            .map(|p| canvas_rect.contains(p))
            .unwrap_or(false);
        if canvas_resp.hovered() || pointer_inside_canvas {
            let scroll = ui.ctx().input(|i| i.raw_scroll_delta);
            let ctrl_zoom = ui.ctx().input(|i| i.modifiers.ctrl);
            if ctrl_zoom && scroll.y.abs() > 0.0 {
                let old_zoom = self.graph_zoom;
                let zoom_mul = (1.0 + scroll.y * 0.0008).clamp(0.94, 1.06);
                self.graph_zoom = (self.graph_zoom * zoom_mul).clamp(0.35, 2.8);
                if let Some(mouse) = pointer_pos {
                    let world = (mouse - canvas_rect.min - self.graph_pan) / old_zoom.max(0.0001);
                    self.graph_pan = mouse - canvas_rect.min - world * self.graph_zoom;
                }
            } else if scroll.length_sq() > 0.0 {
                // Touchpad 2-fingers / mouse wheel scroll moves the canvas.
                self.graph_pan += egui::vec2(scroll.x, scroll.y);
            }
            if ui.ctx().input(|i| i.pointer.middle_down()) {
                self.graph_pan += ui.ctx().input(|i| i.pointer.delta());
            }
            let kb_zoom_in = ui.ctx().input(|i| {
                i.modifiers.ctrl
                    && (i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals))
            });
            let kb_zoom_out = ui
                .ctx()
                .input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Minus));
            let kb_zoom_reset = ui
                .ctx()
                .input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Num0));
            if kb_zoom_in {
                self.graph_zoom = (self.graph_zoom * 1.12).clamp(0.35, 2.8);
            }
            if kb_zoom_out {
                self.graph_zoom = (self.graph_zoom / 1.12).clamp(0.35, 2.8);
            }
            if kb_zoom_reset {
                self.graph_zoom = 1.0;
                self.graph_pan = egui::vec2(0.0, 0.0);
            }
        }

        let grid = 24.0 * self.graph_zoom.max(0.35);
        let grid_off_x = ((self.graph_pan.x % grid) + grid) % grid;
        let grid_off_y = ((self.graph_pan.y % grid) + grid) % grid;
        let mut x = canvas_rect.left() + grid_off_x;
        while x < canvas_rect.right() {
            painter.line_segment(
                [
                    egui::pos2(x, canvas_rect.top()),
                    egui::pos2(x, canvas_rect.bottom()),
                ],
                egui::Stroke::new(0.5, egui::Color32::from_rgb(30, 31, 36)),
            );
            x += grid;
        }
        let mut y = canvas_rect.top() + grid_off_y;
        while y < canvas_rect.bottom() {
            painter.line_segment(
                [
                    egui::pos2(canvas_rect.left(), y),
                    egui::pos2(canvas_rect.right(), y),
                ],
                egui::Stroke::new(0.5, egui::Color32::from_rgb(30, 31, 36)),
            );
            y += grid;
        }

        let graph_origin = canvas_rect.min + self.graph_pan;
        let mut rect_by_id = HashMap::<u32, egui::Rect>::new();
        for node in &self.nodes {
            let rect = egui::Rect::from_min_size(
                graph_origin + node.pos * self.graph_zoom,
                Self::node_size(node.kind) * self.graph_zoom,
            );
            rect_by_id.insert(node.id, rect);
        }

        let ctrl = ui.ctx().input(|i| i.modifiers.ctrl);
        let alt = ui.ctx().input(|i| i.modifiers.alt);
        let primary_pressed = ui.ctx().input(|i| i.pointer.primary_pressed());
        let primary_down = ui.ctx().input(|i| i.pointer.primary_down());
        let primary_released = ui.ctx().input(|i| i.pointer.primary_released());
        let secondary_pressed = ui.ctx().input(|i| i.pointer.secondary_pressed());
        let secondary_down = ui.ctx().input(|i| i.pointer.secondary_down());
        let secondary_released = ui.ctx().input(|i| i.pointer.secondary_released());
        let mut auto_start_wire: Option<(u32, u8, egui::Pos2)> = None;
        if self.drag_from_output.is_none() && primary_pressed {
            if let Some(mouse) = pointer_pos {
                let mut best_out: Option<(u32, u8, f32, egui::Pos2)> = None;
                for node in &self.nodes {
                    if node.kind.output_count() == 0 {
                        continue;
                    }
                    let Some(rect) = rect_by_id.get(&node.id) else {
                        continue;
                    };
                    for out_idx in 0..node.kind.output_count() {
                        let p = Self::output_port_pos(*rect, node.kind, out_idx);
                        let d2 = (p - mouse).length_sq();
                        match best_out {
                            Some((_, _, best_d2, _)) if d2 >= best_d2 => {}
                            _ => {
                                best_out = Some((node.id, out_idx as u8, d2, p));
                            }
                        }
                    }
                }
                if let Some((from_node, from_port, d2, from_pos)) = best_out {
                    // Permite iniciar o fio sem acertar exatamente a bolinha de output.
                    if d2 <= 34.0_f32.powi(2) {
                        auto_start_wire = Some((from_node, from_port, from_pos));
                    }
                }
            }
        }
        let hovered_node = pointer_pos.and_then(|p| {
            rect_by_id
                .iter()
                .find_map(|(id, r)| if r.contains(p) { Some(*id) } else { None })
        });
        let hovered_group_early = pointer_pos.and_then(|p| {
            for g in &self.groups {
                let mut min = egui::pos2(f32::INFINITY, f32::INFINITY);
                let mut max = egui::pos2(f32::NEG_INFINITY, f32::NEG_INFINITY);
                let mut any = false;
                for id in &g.nodes {
                    let Some(r) = rect_by_id.get(id) else {
                        continue;
                    };
                    any = true;
                    min.x = min.x.min(r.left() - 12.0);
                    min.y = min.y.min(r.top() - 22.0);
                    max.x = max.x.max(r.right() + 12.0);
                    max.y = max.y.max(r.bottom() + 12.0);
                }
                if any && egui::Rect::from_min_max(min, max).contains(p) {
                    return Some(g.id);
                }
            }
            None
        });
        if canvas_resp.clicked() && hovered_node.is_none() && hovered_group_early.is_none() && !ctrl
        {
            self.selected_nodes.clear();
            self.selected_node = None;
        }
        if primary_pressed && hovered_node.is_none() && hovered_group_early.is_none() {
            self.marquee_start = pointer_pos;
            self.marquee_end = pointer_pos;
        }
        if primary_down && self.marquee_start.is_some() {
            self.marquee_end = pointer_pos;
        }
        if primary_released {
            if let (Some(a), Some(b)) = (self.marquee_start.take(), self.marquee_end.take()) {
                let mrect = egui::Rect::from_two_pos(a, b);
                if !ctrl {
                    self.selected_nodes.clear();
                }
                for (id, r) in &rect_by_id {
                    if mrect.intersects(*r) {
                        self.selected_nodes.insert(*id);
                    }
                }
                self.selected_node = self.selected_nodes.iter().next().copied();
            }
        }
        let mut do_group = false;
        let mut quick_color: Option<egui::Color32> = None;
        canvas_resp.context_menu(|ui| {
            let add_block_menu_txt = match lang {
                EngineLanguage::Pt => "Add Bloco",
                EngineLanguage::En => "Add Block",
                EngineLanguage::Es => "Agregar Bloque",
            };
            let input_txt = match lang {
                EngineLanguage::Pt => "Entradas",
                EngineLanguage::En => "Inputs",
                EngineLanguage::Es => "Entradas",
            };
            let math_txt = match lang {
                EngineLanguage::Pt => "Matematica",
                EngineLanguage::En => "Math",
                EngineLanguage::Es => "Matematica",
            };
            let out_txt = match lang {
                EngineLanguage::Pt => "Saida",
                EngineLanguage::En => "Output",
                EngineLanguage::Es => "Salida",
            };
            let group_txt = match lang {
                EngineLanguage::Pt => "Agrupar Selecionados",
                EngineLanguage::En => "Group Selected",
                EngineLanguage::Es => "Agrupar Seleccionados",
            };
            let color_txt = match lang {
                EngineLanguage::Pt => "Cor Rapida do Grupo",
                EngineLanguage::En => "Quick Group Color",
                EngineLanguage::Es => "Color Rapido del Grupo",
            };
            ui.menu_button(add_block_menu_txt, |ui| {
                ui.menu_button(input_txt, |ui| {
                    if ui.button(input_axis_txt).clicked() {
                        self.add_node(FiosNodeKind::InputAxis);
                        ui.close();
                    }
                    if ui.button(input_action_txt).clicked() {
                        self.add_node(FiosNodeKind::InputAction);
                        ui.close();
                    }
                    if ui.button(const_txt).clicked() {
                        self.add_node(FiosNodeKind::Constant);
                        ui.close();
                    }
                });
                ui.menu_button(math_txt, |ui| {
                    if ui.button(add_txt).clicked() {
                        self.add_node(FiosNodeKind::Add);
                        ui.close();
                    }
                    if ui.button(sub_txt).clicked() {
                        self.add_node(FiosNodeKind::Subtract);
                        ui.close();
                    }
                    if ui.button(mul_txt).clicked() {
                        self.add_node(FiosNodeKind::Multiply);
                        ui.close();
                    }
                    if ui.button(div_txt).clicked() {
                        self.add_node(FiosNodeKind::Divide);
                        ui.close();
                    }
                    if ui.button(max_txt).clicked() {
                        self.add_node(FiosNodeKind::Max);
                        ui.close();
                    }
                    if ui.button(min_txt).clicked() {
                        self.add_node(FiosNodeKind::Min);
                        ui.close();
                    }
                    if ui.button(gate_txt).clicked() {
                        self.add_node(FiosNodeKind::Gate);
                        ui.close();
                    }
                    if ui.button(abs_txt).clicked() {
                        self.add_node(FiosNodeKind::Abs);
                        ui.close();
                    }
                    if ui.button(sign_txt).clicked() {
                        self.add_node(FiosNodeKind::Sign);
                        ui.close();
                    }
                    if ui.button(clamp_txt).clicked() {
                        self.add_node(FiosNodeKind::Clamp);
                        ui.close();
                    }
                    if ui.button(deadzone_txt).clicked() {
                        self.add_node(FiosNodeKind::Deadzone);
                        ui.close();
                    }
                    if ui.button(invert_txt).clicked() {
                        self.add_node(FiosNodeKind::Invert);
                        ui.close();
                    }
                    if ui.button(smooth_txt).clicked() {
                        self.add_node(FiosNodeKind::Smooth);
                        ui.close();
                    }
                });
                ui.menu_button(out_txt, |ui| {
                    if ui.button(output_move_txt).clicked() {
                        self.add_node(FiosNodeKind::OutputMove);
                        ui.close();
                    }
                    if ui.button(output_look_txt).clicked() {
                        self.add_node(FiosNodeKind::OutputLook);
                        ui.close();
                    }
                    if ui.button(output_action_txt).clicked() {
                        self.add_node(FiosNodeKind::OutputAction);
                        ui.close();
                    }
                    if ui.button(output_anim_cmd_txt).clicked() {
                        self.add_node(FiosNodeKind::OutputAnimCommand);
                        ui.close();
                    }
                });
            });
            ui.menu_button(modules_txt, |ui| {
                if ui.button(module_move_txt).clicked() {
                    self.add_module_move_basic();
                    ui.close();
                }
                if ui.button(module_move_adv_txt).clicked() {
                    self.add_module_move_advanced();
                    ui.close();
                }
                if ui.button(module_look_txt).clicked() {
                    self.add_module_look_basic();
                    ui.close();
                }
                if ui.button(module_look_adv_txt).clicked() {
                    self.add_module_look_advanced();
                    ui.close();
                }
                if ui.button(module_action1_txt).clicked() {
                    self.add_module_action_basic(FiosAction::Action1.index());
                    ui.close();
                }
                if ui.button(module_jump_txt).clicked() {
                    self.add_module_action_basic(FiosAction::Jump.index());
                    ui.close();
                }
            });
            if ui.button(group_txt).clicked() {
                do_group = true;
                ui.close();
            }
            ui.menu_button(color_txt, |ui| {
                let mut color_button = |label: &str, c: egui::Color32, ui: &mut egui::Ui| {
                    if ui
                        .add(
                            egui::Button::new(label)
                                .fill(c)
                                .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(30))),
                        )
                        .clicked()
                    {
                        quick_color = Some(c);
                        ui.close();
                    }
                };
                color_button("Azul", egui::Color32::from_rgb(72, 108, 132), ui);
                color_button("Verde", egui::Color32::from_rgb(72, 132, 102), ui);
                color_button("Laranja", egui::Color32::from_rgb(158, 102, 62), ui);
                color_button("Roxo", egui::Color32::from_rgb(122, 88, 152), ui);
                color_button("Cinza", egui::Color32::from_rgb(95, 95, 102), ui);
            });
        });
        if do_group && self.group_selected_nodes() {
            graph_dirty = true;
        }
        if let Some(c) = quick_color {
            if self.recolor_selected_groups(c) {
                graph_dirty = true;
            }
        }

        let mut link_curves: Vec<(usize, Vec<egui::Pos2>)> = Vec::new();
        let mut group_rects: Vec<(u32, egui::Rect)> = Vec::new();
        let mut pending_group_drag_delta: Option<egui::Vec2> = None;
        let mut pending_group_select: Option<u32> = None;
        for gi in 0..self.groups.len() {
            let (group_id, group_name, group_color, group_nodes): (
                u32,
                String,
                egui::Color32,
                Vec<u32>,
            ) = {
                let g = &self.groups[gi];
                (
                    g.id,
                    g.name.clone(),
                    g.color,
                    g.nodes.iter().copied().collect(),
                )
            };
            let mut min = egui::pos2(f32::INFINITY, f32::INFINITY);
            let mut max = egui::pos2(f32::NEG_INFINITY, f32::NEG_INFINITY);
            let mut count = 0usize;
            for id in &group_nodes {
                if let Some(r) = rect_by_id.get(id) {
                    min.x = min.x.min(r.min.x);
                    min.y = min.y.min(r.min.y);
                    max.x = max.x.max(r.max.x);
                    max.y = max.y.max(r.max.y);
                    count += 1;
                }
            }
            if count == 0 {
                continue;
            }
            let gr = egui::Rect::from_min_max(min, max).expand2(egui::vec2(12.0, 18.0));
            group_rects.push((group_id, gr));
            let g_resp = ui.interact(
                gr,
                ui.id().with(("fios_group_drag", group_id)),
                egui::Sense::click_and_drag(),
            );
            if g_resp.clicked() {
                pending_group_select = Some(group_id);
            }
            if g_resp.dragged() {
                pending_group_drag_delta =
                    Some(ui.ctx().input(|i| i.pointer.delta()) / self.graph_zoom.max(0.0001));
            }
            let fill = egui::Color32::from_rgba_unmultiplied(
                group_color.r(),
                group_color.g(),
                group_color.b(),
                26,
            );
            painter.rect_filled(gr, 8.0, fill);
            painter.rect_stroke(
                gr,
                8.0,
                egui::Stroke::new(1.2, group_color),
                egui::StrokeKind::Outside,
            );
            painter.text(
                gr.left_top() + egui::vec2(8.0, 6.0),
                egui::Align2::LEFT_TOP,
                &group_name,
                egui::FontId::proportional(11.0),
                group_color,
            );
            let color_btn_rect = egui::Rect::from_center_size(
                egui::pos2(gr.right() - 12.0, gr.top() + 12.0),
                egui::vec2(16.0, 16.0),
            );
            let color_resp = ui.interact(
                color_btn_rect,
                ui.id().with(("fios_group_color_btn", group_id)),
                egui::Sense::click(),
            );
            painter.circle_filled(color_btn_rect.center(), 5.0, group_color);
            painter.circle_stroke(
                color_btn_rect.center(),
                5.0,
                egui::Stroke::new(1.0, egui::Color32::from_gray(25)),
            );
            egui::Popup::menu(&color_resp)
                .id(ui.id().with(("fios_group_color_popup", group_id)))
                .width(170.0)
                .show(|ui| {
                    let mut c = self.groups[gi].color;
                    if ui.color_edit_button_srgba(&mut c).changed() {
                        self.groups[gi].color = c;
                        graph_dirty = true;
                    }
                });
        }
        if let Some(group_id) = pending_group_select {
            if let Some(group) = self.groups.iter().find(|g| g.id == group_id) {
                if !ctrl {
                    self.selected_nodes.clear();
                }
                for id in &group.nodes {
                    self.selected_nodes.insert(*id);
                }
                self.selected_node = self.selected_nodes.iter().next().copied();
            }
        }
        let hovered_group = pointer_pos.and_then(|p| {
            group_rects
                .iter()
                .find_map(|(gid, r)| if r.contains(p) { Some(*gid) } else { None })
        });
        for (link_idx, link) in self.links.iter().enumerate() {
            let Some(fi) = self.node_index_by_id(link.from_node) else {
                continue;
            };
            let Some(ti) = self.node_index_by_id(link.to_node) else {
                continue;
            };
            let Some(fr) = rect_by_id.get(&link.from_node) else {
                continue;
            };
            let Some(tr) = rect_by_id.get(&link.to_node) else {
                continue;
            };
            let from = Self::output_port_pos(*fr, self.nodes[fi].kind, link.from_port as usize);
            let to = Self::input_port_pos(*tr, self.nodes[ti].kind, link.to_port as usize);
            let c1 = egui::pos2(from.x + 50.0, from.y);
            let c2 = egui::pos2(to.x - 50.0, to.y);
            let mut pts = Vec::with_capacity(20);
            for i in 0..20 {
                let t = i as f32 / 19.0;
                let omt = 1.0 - t;
                let p = from.to_vec2() * (omt * omt * omt)
                    + c1.to_vec2() * (3.0 * omt * omt * t)
                    + c2.to_vec2() * (3.0 * omt * t * t)
                    + to.to_vec2() * (t * t * t);
                pts.push(egui::pos2(p.x, p.y));
            }
            painter.add(egui::Shape::line(
                pts.clone(),
                egui::Stroke::new(2.0, egui::Color32::from_rgb(15, 232, 121)),
            ));
            link_curves.push((link_idx, pts));
        }

        let mut started_alt_wire_drag = false;
        if alt && secondary_pressed && self.drag_from_output.is_none() {
            if let Some(mouse) = pointer_pos {
                let mut best_out: Option<(u32, u8, f32, egui::Pos2)> = None;
                for node in &self.nodes {
                    if node.kind.output_count() == 0 {
                        continue;
                    }
                    let Some(rect) = rect_by_id.get(&node.id) else {
                        continue;
                    };
                    for out_idx in 0..node.kind.output_count() {
                        let p = Self::output_port_pos(*rect, node.kind, out_idx);
                        let d2 = (p - mouse).length_sq();
                        match best_out {
                            Some((_, _, best_d2, _)) if d2 >= best_d2 => {}
                            _ => {
                                best_out = Some((node.id, out_idx as u8, d2, p));
                            }
                        }
                    }
                }
                if let Some((from_node, from_port, d2, from_pos)) = best_out {
                    if d2 <= 16.0_f32.powi(2) {
                        self.drag_from_output = Some((from_node, from_port));
                        self.wire_drag_path.clear();
                        self.wire_drag_path.push(from_pos);
                        started_alt_wire_drag = true;
                    }
                }
            }
        }
        if secondary_pressed
            && !started_alt_wire_drag
            && self.drag_from_output.is_none()
            && hovered_node.is_none()
            && hovered_group.is_none()
        {
            self.cut_points.clear();
            if let Some(p) = pointer_pos {
                self.cut_points.push(p);
            }
        }
        if secondary_down && !self.cut_points.is_empty() && self.drag_from_output.is_none() {
            if let Some(p) = pointer_pos {
                let should_push = self
                    .cut_points
                    .last()
                    .map(|lp| (*lp - p).length_sq() > 9.0)
                    .unwrap_or(true);
                if should_push {
                    self.cut_points.push(p);
                }
            }
        }
        if secondary_released && self.cut_points.len() > 1 {
            let mut remove = HashSet::<usize>::new();
            for (link_idx, curve) in &link_curves {
                'hit: for seg in curve.windows(2) {
                    for cut in self.cut_points.windows(2) {
                        if Self::seg_intersects(seg[0], seg[1], cut[0], cut[1]) {
                            remove.insert(*link_idx);
                            break 'hit;
                        }
                    }
                }
            }
            if !remove.is_empty() {
                let old = self.links.clone();
                self.links.clear();
                for (idx, link) in old.into_iter().enumerate() {
                    if !remove.contains(&idx) {
                        self.links.push(link);
                    }
                }
                graph_dirty = true;
            }
            self.cut_points.clear();
        } else if secondary_released && self.cut_points.len() <= 1 && !started_alt_wire_drag {
            self.cut_points.clear();
        }
        if !self.cut_points.is_empty() {
            painter.add(egui::Shape::line(
                self.cut_points.clone(),
                egui::Stroke::new(2.0, egui::Color32::from_rgb(245, 100, 100)),
            ));
        }

        let mut pending_new_link: Option<(u32, u8, u32, u8)> = None;
        let mut pending_remove_links: Vec<(u32, u8)> = Vec::new();
        let mut pending_context_rename_node: Option<u32> = None;
        let mut pending_context_delete_node: Option<u32> = None;
        let mut next_drag_from_output = self.drag_from_output;
        if next_drag_from_output.is_none() {
            if let Some((from_node, from_port, from_pos)) = auto_start_wire {
                next_drag_from_output = Some((from_node, from_port));
                self.wire_drag_path.clear();
                self.wire_drag_path.push(from_pos);
            }
        }
        for node in &mut self.nodes {
            let rect = egui::Rect::from_min_size(
                graph_origin + node.pos * self.graph_zoom,
                Self::node_size(node.kind) * self.graph_zoom,
            );
            let id = ui.id().with(("fios_node_drag", node.id));
            let drag_resp = ui.interact(rect, id, egui::Sense::click_and_drag());
            if drag_resp.clicked() {
                if ctrl {
                    if self.selected_nodes.contains(&node.id) {
                        self.selected_nodes.remove(&node.id);
                    } else {
                        self.selected_nodes.insert(node.id);
                    }
                    self.selected_node = self.selected_nodes.iter().next().copied();
                } else {
                    self.selected_node = Some(node.id);
                    self.selected_nodes.clear();
                    self.selected_nodes.insert(node.id);
                }
            }
            if drag_resp.double_clicked() {
                self.rename_node = Some(node.id);
                self.rename_buffer = node.display_name.clone();
            }
            drag_resp.context_menu(|ui| {
                if ui.button(rename_txt).clicked() {
                    pending_context_rename_node = Some(node.id);
                    ui.close();
                }
                if ui.button(del_txt).clicked() {
                    pending_context_delete_node = Some(node.id);
                    ui.close();
                }
            });
            if drag_resp.dragged() {
                if self.selected_nodes.contains(&node.id) && self.selected_nodes.len() > 1 {
                    pending_group_drag_delta =
                        Some(ui.ctx().input(|i| i.pointer.delta()) / self.graph_zoom.max(0.0001));
                } else {
                    node.pos += ui.ctx().input(|i| i.pointer.delta()) / self.graph_zoom.max(0.0001);
                    ui.ctx().request_repaint();
                }
            }
            if drag_resp.drag_stopped() {
                graph_dirty = true;
            }
            let is_selected = self.selected_nodes.contains(&node.id);
            painter.rect_filled(rect, 6.0, egui::Color32::from_rgb(37, 37, 40));
            painter.rect_stroke(
                rect,
                6.0,
                egui::Stroke::new(
                    if is_selected { 2.0 } else { 1.0 },
                    if is_selected {
                        egui::Color32::from_rgb(15, 232, 121)
                    } else {
                        egui::Color32::from_rgb(78, 78, 86)
                    },
                ),
                egui::StrokeKind::Outside,
            );
            painter.text(
                rect.left_top() + egui::vec2(8.0, 8.0),
                egui::Align2::LEFT_TOP,
                &node.display_name,
                egui::FontId::proportional(12.0),
                egui::Color32::from_gray(220),
            );

            if node.kind == FiosNodeKind::Constant {
                let val_rect = egui::Rect::from_min_size(
                    rect.left_top() + egui::vec2(8.0, 32.0),
                    egui::vec2(rect.width() - 16.0, 24.0),
                );
                ui.scope_builder(egui::UiBuilder::new().max_rect(val_rect), |ui| {
                    ui.horizontal(|ui| {
                        ui.label("V");
                        if ui
                            .add(
                                egui::DragValue::new(&mut node.value)
                                    .speed(0.05)
                                    .range(-1000.0..=1000.0),
                            )
                            .changed()
                        {
                            graph_dirty = true;
                        }
                    });
                });
            }
            if node.kind == FiosNodeKind::InputAction {
                let r1 = egui::Rect::from_min_size(
                    rect.left_top() + egui::vec2(8.0, 32.0),
                    egui::vec2(rect.width() - 16.0, 24.0),
                );
                let r2 = egui::Rect::from_min_size(
                    rect.left_top() + egui::vec2(8.0, 58.0),
                    egui::vec2(rect.width() - 16.0, 24.0),
                );
                ui.scope_builder(egui::UiBuilder::new().max_rect(r1), |ui| {
                    let mut selected_idx = node
                        .param_a
                        .round()
                        .clamp(0.0, (ACTION_COUNT.saturating_sub(1)) as f32)
                        as usize;
                    egui::ComboBox::from_id_salt(ui.id().with(("fios_action_idx", node.id)))
                        .selected_text(FiosAction::ALL[selected_idx].label(lang))
                        .show_ui(ui, |ui| {
                            for (idx, action) in FiosAction::ALL.iter().enumerate() {
                                if ui
                                    .selectable_label(selected_idx == idx, action.label(lang))
                                    .clicked()
                                {
                                    selected_idx = idx;
                                    node.param_a = idx as f32;
                                    graph_dirty = true;
                                }
                            }
                        });
                });
                ui.scope_builder(egui::UiBuilder::new().max_rect(r2), |ui| {
                    let mut mode_just = node.param_b.round() >= 1.0;
                    let mode_txt = if mode_just { "JustPressed" } else { "Pressed" };
                    if ui.checkbox(&mut mode_just, mode_txt).changed() {
                        node.param_b = if mode_just { 1.0 } else { 0.0 };
                        graph_dirty = true;
                    }
                });
            }
            if node.kind == FiosNodeKind::Clamp {
                let r1 = egui::Rect::from_min_size(
                    rect.left_top() + egui::vec2(8.0, 32.0),
                    egui::vec2(rect.width() - 16.0, 22.0),
                );
                let r2 = egui::Rect::from_min_size(
                    rect.left_top() + egui::vec2(8.0, 56.0),
                    egui::vec2(rect.width() - 16.0, 22.0),
                );
                ui.scope_builder(egui::UiBuilder::new().max_rect(r1), |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Min");
                        if ui
                            .add(
                                egui::DragValue::new(&mut node.param_a)
                                    .speed(0.05)
                                    .range(-1000.0..=1000.0),
                            )
                            .changed()
                        {
                            graph_dirty = true;
                        }
                    });
                });
                ui.scope_builder(egui::UiBuilder::new().max_rect(r2), |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Max");
                        if ui
                            .add(
                                egui::DragValue::new(&mut node.param_b)
                                    .speed(0.05)
                                    .range(-1000.0..=1000.0),
                            )
                            .changed()
                        {
                            graph_dirty = true;
                        }
                    });
                });
            }
            if node.kind == FiosNodeKind::Deadzone {
                let r1 = egui::Rect::from_min_size(
                    rect.left_top() + egui::vec2(8.0, 34.0),
                    egui::vec2(rect.width() - 16.0, 24.0),
                );
                ui.scope_builder(egui::UiBuilder::new().max_rect(r1), |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Dz");
                        if ui
                            .add(
                                egui::DragValue::new(&mut node.param_a)
                                    .speed(0.01)
                                    .range(0.0..=1.0),
                            )
                            .changed()
                        {
                            graph_dirty = true;
                        }
                    });
                });
            }
            if node.kind == FiosNodeKind::Smooth {
                let r1 = egui::Rect::from_min_size(
                    rect.left_top() + egui::vec2(8.0, 34.0),
                    egui::vec2(rect.width() - 16.0, 24.0),
                );
                ui.scope_builder(egui::UiBuilder::new().max_rect(r1), |ui| {
                    ui.horizontal(|ui| {
                        ui.label("A");
                        if ui
                            .add(
                                egui::DragValue::new(&mut node.param_a)
                                    .speed(0.01)
                                    .range(0.0..=1.0),
                            )
                            .changed()
                        {
                            graph_dirty = true;
                        }
                    });
                });
            }

            if node.kind == FiosNodeKind::OutputMove {
                painter.text(
                    rect.left_top() + egui::vec2(8.0, 36.0),
                    egui::Align2::LEFT_TOP,
                    format!("X: {:.2}  Y: {:.2}", self.last_axis[0], self.last_axis[1]),
                    egui::FontId::monospace(11.0),
                    egui::Color32::from_gray(190),
                );
            }
            if node.kind == FiosNodeKind::OutputLook {
                painter.text(
                    rect.left_top() + egui::vec2(8.0, 36.0),
                    egui::Align2::LEFT_TOP,
                    format!(
                        "Yaw: {:.2}  Pitch: {:.2}",
                        self.last_look[0], self.last_look[1]
                    ),
                    egui::FontId::monospace(11.0),
                    egui::Color32::from_gray(190),
                );
            }
            if node.kind == FiosNodeKind::OutputAction {
                painter.text(
                    rect.left_top() + egui::vec2(8.0, 32.0),
                    egui::Align2::LEFT_TOP,
                    format!("A: {:.2}", self.last_action),
                    egui::FontId::monospace(11.0),
                    egui::Color32::from_gray(190),
                );
            }
            if node.kind == FiosNodeKind::OutputAnimCommand {
                painter.text(
                    rect.left_top() + egui::vec2(8.0, 32.0),
                    egui::Align2::LEFT_TOP,
                    format!("Cmd: {:.2}", self.last_anim_cmd_signal),
                    egui::FontId::monospace(11.0),
                    egui::Color32::from_gray(190),
                );
            }

            for i in 0..node.kind.input_count() {
                let p = Self::input_port_pos(rect, node.kind, i);
                painter.circle_filled(p, 4.0, egui::Color32::from_rgb(205, 120, 120));
                painter.text(
                    p + egui::vec2(8.0, -6.0),
                    egui::Align2::LEFT_TOP,
                    node.kind.input_name(i),
                    egui::FontId::proportional(10.0),
                    egui::Color32::from_gray(170),
                );
                let r = egui::Rect::from_center_size(p, egui::vec2(24.0, 24.0));
                let resp = ui.interact(
                    r,
                    ui.id().with(("fios_in_port", node.id, i)),
                    egui::Sense::click(),
                );
                if resp.clicked() {
                    if let Some((from_n, from_p)) = next_drag_from_output.take() {
                        pending_new_link = Some((from_n, from_p, node.id, i as u8));
                        self.wire_drag_path.clear();
                    }
                }
                if resp.secondary_clicked() {
                    pending_remove_links.push((node.id, i as u8));
                }
            }
            for i in 0..node.kind.output_count() {
                let p = Self::output_port_pos(rect, node.kind, i);
                painter.circle_filled(p, 4.0, egui::Color32::from_rgb(120, 180, 230));
                painter.text(
                    p + egui::vec2(-8.0, -6.0),
                    egui::Align2::RIGHT_TOP,
                    node.kind.output_name(i),
                    egui::FontId::proportional(10.0),
                    egui::Color32::from_gray(170),
                );
                let r = egui::Rect::from_center_size(p, egui::vec2(24.0, 24.0));
                let resp = ui.interact(
                    r,
                    ui.id().with(("fios_out_port", node.id, i)),
                    egui::Sense::click_and_drag(),
                );
                if resp.drag_started() || resp.clicked() {
                    next_drag_from_output = Some((node.id, i as u8));
                    self.wire_drag_path.clear();
                    self.wire_drag_path.push(p);
                }
            }
        }
        if let Some(delta) = pending_group_drag_delta {
            if delta.length_sq() > 0.0 {
                for n in &mut self.nodes {
                    if self.selected_nodes.contains(&n.id) {
                        n.pos += delta;
                    }
                }
                graph_dirty = true;
            }
        }
        self.drag_from_output = next_drag_from_output;
        if let Some(id) = pending_context_rename_node {
            self.rename_node = Some(id);
            if let Some(i) = self.node_index_by_id(id) {
                self.rename_buffer = self.nodes[i].display_name.clone();
            }
        }
        if let Some(id) = pending_context_delete_node {
            self.selected_nodes.clear();
            self.selected_nodes.insert(id);
            self.selected_node = Some(id);
            if self.remove_selected_nodes() {
                graph_dirty = true;
            }
        }
        for (to_node, to_port) in pending_remove_links {
            self.links
                .retain(|l| !(l.to_node == to_node && l.to_port == to_port));
            graph_dirty = true;
        }
        if let Some((from_n, from_p, to_n, to_p)) = pending_new_link {
            self.create_link(from_n, from_p, to_n, to_p);
            self.wire_drag_path.clear();
            graph_dirty = true;
        }

        if let Some((from_node, from_port)) = self.drag_from_output {
            if let Some(fi) = self.node_index_by_id(from_node) {
                if let Some(from_rect) = rect_by_id.get(&from_node) {
                    let from =
                        Self::output_port_pos(*from_rect, self.nodes[fi].kind, from_port as usize);
                    let mouse = ui
                        .ctx()
                        .input(|i| i.pointer.hover_pos())
                        .unwrap_or(from + egui::vec2(80.0, 0.0));
                    let mut predicted_input: Option<(u32, u8, f32, egui::Pos2)> = None;
                    if let Some(target_node) = hovered_node {
                        if let Some(ti) = self.node_index_by_id(target_node) {
                            let target_kind = self.nodes[ti].kind;
                            if target_kind.input_count() > 0 {
                                if let Some(target_rect) = rect_by_id.get(&target_node) {
                                    for input_idx in 0..target_kind.input_count() {
                                        let p = Self::input_port_pos(
                                            *target_rect,
                                            target_kind,
                                            input_idx,
                                        );
                                        let d2 = (p - mouse).length_sq();
                                        match predicted_input {
                                            Some((_, _, bd2, _)) if d2 >= bd2 => {}
                                            _ => {
                                                predicted_input =
                                                    Some((target_node, input_idx as u8, d2, p));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if predicted_input.is_none() {
                        for node in &self.nodes {
                            if node.kind.input_count() == 0 {
                                continue;
                            }
                            let Some(rect) = rect_by_id.get(&node.id) else {
                                continue;
                            };
                            for input_idx in 0..node.kind.input_count() {
                                let p = Self::input_port_pos(*rect, node.kind, input_idx);
                                let d2 = (p - mouse).length_sq();
                                match predicted_input {
                                    Some((_, _, bd2, _)) if d2 >= bd2 => {}
                                    _ => {
                                        predicted_input = Some((node.id, input_idx as u8, d2, p));
                                    }
                                }
                            }
                        }
                    }
                    let connect_drag_down = ui.ctx().input(|i| {
                        i.pointer.primary_down() || (i.modifiers.alt && i.pointer.secondary_down())
                    });
                    if connect_drag_down {
                        if self.wire_drag_path.is_empty() {
                            self.wire_drag_path.push(from);
                        }
                        let should_push = self
                            .wire_drag_path
                            .last()
                            .map(|lp| (*lp - mouse).length_sq() > 16.0)
                            .unwrap_or(true);
                        if should_push {
                            self.wire_drag_path.push(mouse);
                        }
                    }
                    if self.wire_drag_path.len() > 1 {
                        painter.add(egui::Shape::line(
                            self.wire_drag_path.clone(),
                            egui::Stroke::new(2.0, egui::Color32::from_rgb(15, 232, 121)),
                        ));
                    } else {
                        painter.line_segment(
                            [from, mouse],
                            egui::Stroke::new(2.0, egui::Color32::from_rgb(15, 232, 121)),
                        );
                    }
                    if let Some((_, _, _, predicted_pos)) = predicted_input {
                        painter.circle_stroke(
                            predicted_pos,
                            7.0,
                            egui::Stroke::new(1.5, egui::Color32::from_rgb(15, 232, 121)),
                        );
                        painter.line_segment(
                            [mouse, predicted_pos],
                            egui::Stroke::new(
                                1.0,
                                egui::Color32::from_rgba_unmultiplied(15, 232, 121, 130),
                            ),
                        );
                    }
                }
            }
            let connect_drag_down = ui.ctx().input(|i| {
                i.pointer.primary_down() || (i.modifiers.alt && i.pointer.secondary_down())
            });
            if !connect_drag_down {
                let release_pos = ui
                    .ctx()
                    .input(|i| i.pointer.hover_pos())
                    .or_else(|| self.wire_drag_path.last().copied());
                if let Some(release_pos) = release_pos {
                    let mut best: Option<(u32, u8, f32)> = None;
                    if let Some(target_node) = hovered_node {
                        if let Some(ti) = self.node_index_by_id(target_node) {
                            let target_kind = self.nodes[ti].kind;
                            if target_kind.input_count() > 0 {
                                if let Some(target_rect) = rect_by_id.get(&target_node) {
                                    for input_idx in 0..target_kind.input_count() {
                                        let p = Self::input_port_pos(
                                            *target_rect,
                                            target_kind,
                                            input_idx,
                                        );
                                        let d2 = (p - release_pos).length_sq();
                                        match best {
                                            Some((_, _, bd2)) if d2 >= bd2 => {}
                                            _ => {
                                                best = Some((target_node, input_idx as u8, d2));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if best.is_none() {
                        for node in &self.nodes {
                            if node.kind.input_count() == 0 {
                                continue;
                            }
                            let Some(rect) = rect_by_id.get(&node.id) else {
                                continue;
                            };
                            for input_idx in 0..node.kind.input_count() {
                                let p = Self::input_port_pos(*rect, node.kind, input_idx);
                                let d2 = (p - release_pos).length_sq();
                                match best {
                                    Some((_, _, bd2)) if d2 >= bd2 => {}
                                    _ => {
                                        best = Some((node.id, input_idx as u8, d2));
                                    }
                                }
                            }
                        }
                    }
                    if let Some((to_node, to_port, _)) = best {
                        self.create_link(from_node, from_port, to_node, to_port);
                        graph_dirty = true;
                    }
                }
                self.wire_drag_path.clear();
                self.drag_from_output = None;
            }
        }

        if let (Some(a), Some(b)) = (self.marquee_start, self.marquee_end) {
            let r = egui::Rect::from_two_pos(a, b);
            painter.rect_filled(
                r,
                0.0,
                egui::Color32::from_rgba_unmultiplied(86, 148, 255, 24),
            );
            painter.rect_stroke(
                r,
                0.0,
                egui::Stroke::new(1.0, egui::Color32::from_rgb(86, 148, 255)),
                egui::StrokeKind::Outside,
            );
        }

        if ui.ctx().input(|i| i.key_pressed(egui::Key::F2)) {
            if let Some(id) = self.selected_nodes.iter().next().copied() {
                self.rename_node = Some(id);
                if let Some(i) = self.node_index_by_id(id) {
                    self.rename_buffer = self.nodes[i].display_name.clone();
                }
            }
        }

        let delete_pressed = ui
            .ctx()
            .input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace));
        if delete_pressed && self.remove_selected_nodes() {
            graph_dirty = true;
        }
        if graph_dirty {
            let _ = self.save_graph_to_disk();
        }
    }

    fn draw_controls_tab(&mut self, ui: &mut egui::Ui, lang: EngineLanguage) {
        let accent = egui::Color32::from_rgb(15, 232, 121);
        let surface_0 = egui::Color32::from_rgb(22, 24, 28);
        let surface_1 = egui::Color32::from_rgb(30, 33, 37);
        let surface_2 = egui::Color32::from_rgb(38, 42, 48);
        let border = egui::Color32::from_rgb(52, 58, 66);
        let text_primary = egui::Color32::from_gray(235);
        let text_secondary = egui::Color32::from_gray(170);
        let text_muted = egui::Color32::from_gray(120);

        let enabled_txt = match lang {
            EngineLanguage::Pt => "Ativo",
            EngineLanguage::En => "Enabled",
            EngineLanguage::Es => "Activo",
        };
        let add_module_txt = match lang {
            EngineLanguage::Pt => "+ Adicionar módulo",
            EngineLanguage::En => "+ Add Module",
            EngineLanguage::Es => "+ Agregar módulo",
        };
        let modules_section_txt = match lang {
            EngineLanguage::Pt => "Módulos Ativos",
            EngineLanguage::En => "Active Modules",
            EngineLanguage::Es => "Módulos Activos",
        };
        let modes_section_txt = match lang {
            EngineLanguage::Pt => "Modos de Controle",
            EngineLanguage::En => "Control Modes",
            EngineLanguage::Es => "Modos de Control",
        };
        let keys_section_txt = match lang {
            EngineLanguage::Pt => "Mapa de Teclas",
            EngineLanguage::En => "Key Map",
            EngineLanguage::Es => "Mapa de Teclas",
        };
        let action_header = match lang {
            EngineLanguage::Pt => "Ação",
            EngineLanguage::En => "Action",
            EngineLanguage::Es => "Acción",
        };
        let key_header = match lang {
            EngineLanguage::Pt => "Tecla",
            EngineLanguage::En => "Key",
            EngineLanguage::Es => "Tecla",
        };
        let state_header = match lang {
            EngineLanguage::Pt => "Estado",
            EngineLanguage::En => "State",
            EngineLanguage::Es => "Estado",
        };
        let save_txt = match lang {
            EngineLanguage::Pt => "Salvar",
            EngineLanguage::En => "Save",
            EngineLanguage::Es => "Guardar",
        };
        let restore_txt = match lang {
            EngineLanguage::Pt => "Restaurar Padrão",
            EngineLanguage::En => "Restore Defaults",
            EngineLanguage::Es => "Restaurar Pred.",
        };

        let bindings = self.bindings;

        // ─── Status banner ───
        if let Some(status) = &self.status {
            egui::Frame::new()
                .fill(egui::Color32::from_rgba_unmultiplied(15, 232, 121, 12))
                .corner_radius(6.0)
                .inner_margin(egui::Margin::symmetric(10, 5))
                .show(ui, |ui| {
                    ui.label(egui::RichText::new(status).size(11.0).color(accent));
                });
            ui.add_space(6.0);
        }

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

                // ═══════════════════════════════════════════
                // SEÇÃO 1: Módulos Ativos
                // ═══════════════════════════════════════════
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(modules_section_txt)
                            .size(13.0)
                            .strong()
                            .color(text_primary),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(format!("({})", self.module_chain.len()))
                            .size(11.0)
                            .color(text_muted),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.checkbox(&mut self.controls_enabled, enabled_txt);
                        ui.add_space(8.0);
                        let resp = self.module_add_button(ui, add_module_txt);
                        egui::Popup::menu(&resp).show(|ui| {
                            if self.module_menu_content(ui, lang) {
                                ui.close_kind(UiKind::Menu);
                            }
                        });
                    });
                });

                ui.add_space(8.0);

                if self.module_chain.is_empty() {
                    // Empty state
                    egui::Frame::new()
                        .fill(surface_0)
                        .stroke(egui::Stroke::new(1.0, border))
                        .corner_radius(10.0)
                        .inner_margin(egui::Margin::symmetric(20, 28))
                        .show(ui, |ui| {
                            ui.vertical_centered(|ui| {
                                ui.label(egui::RichText::new("🔌").size(28.0));
                                ui.add_space(8.0);
                                let empty_txt = match lang {
                                    EngineLanguage::Pt => "Nenhum módulo adicionado",
                                    EngineLanguage::En => "No modules added",
                                    EngineLanguage::Es => "Ningún módulo agregado",
                                };
                                ui.label(
                                    egui::RichText::new(empty_txt)
                                        .size(12.0)
                                        .color(text_secondary),
                                );
                                ui.add_space(6.0);
                                let hint_txt = match lang {
                                    EngineLanguage::Pt => {
                                        "Clique em \"+ Adicionar módulo\" para começar"
                                    }
                                    EngineLanguage::En => "Click \"+ Add Module\" to get started",
                                    EngineLanguage::Es => {
                                        "Haga clic en \"+ Agregar módulo\" para comenzar"
                                    }
                                };
                                ui.label(
                                    egui::RichText::new(hint_txt).size(10.5).color(text_muted),
                                );
                                ui.add_space(12.0);
                                let resp = self.module_add_button(ui, add_module_txt);
                                egui::Popup::menu(&resp).show(|ui| {
                                    if self.module_menu_content(ui, lang) {
                                        ui.close_kind(UiKind::Menu);
                                    }
                                });
                            });
                        });
                } else {
                    let mut modules_to_remove = Vec::new();
                    for module_idx in 0..self.module_chain.len() {
                        let is_enabled = self.module_chain[module_idx].enabled;
                        let dot_color = if is_enabled {
                            accent
                        } else {
                            egui::Color32::from_gray(70)
                        };
                        let card_bg = if is_enabled { surface_1 } else { surface_0 };
                        let card_border = if is_enabled {
                            egui::Color32::from_rgba_unmultiplied(15, 232, 121, 35)
                        } else {
                            border
                        };

                        egui::Frame::new()
                            .fill(card_bg)
                            .stroke(egui::Stroke::new(1.0, card_border))
                            .corner_radius(8.0)
                            .inner_margin(egui::Margin::symmetric(12, 10))
                            .show(ui, |ui| {
                                ui.vertical(|ui| {
                                    // Card header: dot + name + controls
                                    ui.horizontal(|ui| {
                                        // Status dot
                                        let (dot_rect, _) = ui.allocate_exact_size(
                                            egui::vec2(8.0, 8.0),
                                            egui::Sense::hover(),
                                        );
                                        ui.painter().circle_filled(
                                            dot_rect.center(),
                                            3.5,
                                            dot_color,
                                        );

                                        let module = &mut self.module_chain[module_idx];
                                        ui.label(
                                            egui::RichText::new(&module.name)
                                                .strong()
                                                .size(12.5)
                                                .color(text_primary),
                                        );

                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                // Close button
                                                let close_resp = ui.add(
                                                    egui::Button::new(
                                                        egui::RichText::new("✕")
                                                            .size(10.0)
                                                            .color(egui::Color32::from_gray(100)),
                                                    )
                                                    .frame(false)
                                                    .min_size(egui::vec2(20.0, 20.0))
                                                    .fill(egui::Color32::TRANSPARENT),
                                                );
                                                if close_resp.clicked() {
                                                    modules_to_remove
                                                        .push((module.id, module.group_id));
                                                    return;
                                                }
                                                if close_resp.hovered() {
                                                    ui.painter().circle_filled(
                                                        close_resp.rect.center(),
                                                        10.0,
                                                        egui::Color32::from_rgba_unmultiplied(
                                                            200, 60, 60, 40,
                                                        ),
                                                    );
                                                }

                                                // Enable checkbox
                                                let checkbox = ui.checkbox(&mut module.enabled, "");
                                                checkbox.on_hover_text(match lang {
                                                    EngineLanguage::Pt => "Ativar módulo",
                                                    EngineLanguage::En => "Enable module",
                                                    EngineLanguage::Es => "Activar módulo",
                                                });
                                            },
                                        );
                                    });

                                    // Description + asset info (compact)
                                    {
                                        let module = &self.module_chain[module_idx];
                                        if let Some(desc) = module.description.as_ref() {
                                            ui.add_space(3.0);
                                            ui.label(
                                                egui::RichText::new(desc)
                                                    .size(10.5)
                                                    .color(text_secondary),
                                            );
                                        }
                                        ui.add_space(2.0);
                                        ui.label(
                                            egui::RichText::new(&module.asset)
                                                .size(9.5)
                                                .color(text_muted),
                                        );
                                        Self::render_module_extra_info(ui, module);
                                    }

                                    // Collapsible controls
                                    ui.add_space(4.0);
                                    self.render_module_card_details(
                                        ui, module_idx, lang, &bindings,
                                    );
                                });
                            });
                        ui.add_space(6.0);
                    }
                    for (id, group_id) in modules_to_remove {
                        if let Some(group_id) = group_id {
                            self.remove_module_group(group_id);
                        }
                        if let Some(pos) = self.module_chain.iter().position(|m| m.id == id) {
                            self.module_chain.remove(pos);
                        }
                    }
                }

                ui.add_space(16.0);

                // ═══════════════════════════════════════════
                // SEÇÃO 2: Modos de Controle (Pills)
                // ═══════════════════════════════════════════
                ui.label(
                    egui::RichText::new(modes_section_txt)
                        .size(13.0)
                        .strong()
                        .color(text_primary),
                );
                ui.add_space(6.0);

                let mut to_remove: Option<FiosControlMode> = None;
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                    for mode in self.control_modes.clone() {
                        let selected = self.active_control_mode == mode;
                        let pill_fill = if selected {
                            egui::Color32::from_rgba_unmultiplied(15, 232, 121, 30)
                        } else {
                            surface_2
                        };
                        let pill_stroke = if selected {
                            egui::Stroke::new(1.0, accent)
                        } else {
                            egui::Stroke::new(1.0, border)
                        };
                        let label_color = if selected { accent } else { text_secondary };

                        let label_txt = Self::control_mode_label(mode, lang);
                        let btn_label = if self.control_modes.len() > 1 {
                            format!("{label_txt}  ✕")
                        } else {
                            label_txt.to_string()
                        };

                        let pill = egui::Button::new(
                            egui::RichText::new(&btn_label)
                                .size(11.5)
                                .color(label_color),
                        )
                        .fill(pill_fill)
                        .stroke(pill_stroke)
                        .corner_radius(14.0)
                        .min_size(egui::vec2(0.0, 28.0));

                        let resp = ui.add(pill);
                        if resp.clicked() {
                            self.active_control_mode = mode;
                        }
                        if resp.secondary_clicked() && self.control_modes.len() > 1 {
                            to_remove = Some(mode);
                        }
                    }

                    // Add mode button
                    let add_btn = egui::Button::new(
                        egui::RichText::new("+").size(13.0).color(text_secondary),
                    )
                    .fill(surface_2)
                    .stroke(egui::Stroke::new(1.0, border))
                    .corner_radius(14.0)
                    .min_size(egui::vec2(28.0, 28.0));

                    let add_resp = ui.add(add_btn);
                    egui::Popup::menu(&add_resp)
                        .id(ui.id().with("fios_add_control_mode_popup"))
                        .show(|ui| {
                            if !self.control_modes.contains(&FiosControlMode::Animation)
                                && ui
                                    .button(Self::control_mode_label(
                                        FiosControlMode::Animation,
                                        lang,
                                    ))
                                    .clicked()
                            {
                                self.control_modes.push(FiosControlMode::Animation);
                                self.active_control_mode = FiosControlMode::Animation;
                                ui.close();
                            }
                            if !self.control_modes.contains(&FiosControlMode::Movement)
                                && ui
                                    .button(Self::control_mode_label(
                                        FiosControlMode::Movement,
                                        lang,
                                    ))
                                    .clicked()
                            {
                                self.control_modes.push(FiosControlMode::Movement);
                                self.active_control_mode = FiosControlMode::Movement;
                                ui.close();
                            }
                            if ui.button("Controlador de animação").clicked() {
                                self.refresh_anim_clip_cache(ui.ctx(), true);
                                let created = self.seed_animation_controller_defaults();
                                self.tab = FiosTab::Controller;
                                self.anim_tab_status = Some(if created > 0 {
                                    format!("Controlador criado com {created} estado(s) base")
                                } else {
                                    "Nenhum clipe encontrado para criar controlador base"
                                        .to_string()
                                });
                                if !self.control_modes.contains(&FiosControlMode::Animation) {
                                    self.control_modes.push(FiosControlMode::Animation);
                                }
                                self.active_control_mode = FiosControlMode::Animation;
                                ui.close();
                            }
                        });
                });
                if let Some(mode) = to_remove {
                    self.control_modes.retain(|m| *m != mode);
                    if self.control_modes.is_empty() {
                        self.control_modes.push(FiosControlMode::Movement);
                    }
                    if self.active_control_mode == mode {
                        self.active_control_mode = self.control_modes[0];
                    }
                }

                ui.add_space(16.0);

                // ═══════════════════════════════════════════
                // SEÇÃO 3: Mapa de Teclas
                // ═══════════════════════════════════════════
                ui.label(
                    egui::RichText::new(keys_section_txt)
                        .size(13.0)
                        .strong()
                        .color(text_primary),
                );
                ui.add_space(6.0);

                egui::Frame::new()
                    .fill(surface_0)
                    .stroke(egui::Stroke::new(1.0, border))
                    .corner_radius(8.0)
                    .inner_margin(egui::Margin::symmetric(10, 8))
                    .show(ui, |ui| {
                        egui::Grid::new("fios_bind_grid")
                            .num_columns(3)
                            .spacing([8.0, 5.0])
                            .striped(true)
                            .show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new(action_header)
                                        .size(10.5)
                                        .strong()
                                        .color(text_secondary),
                                );
                                ui.label(
                                    egui::RichText::new(key_header)
                                        .size(10.5)
                                        .strong()
                                        .color(text_secondary),
                                );
                                ui.label(
                                    egui::RichText::new(state_header)
                                        .size(10.5)
                                        .strong()
                                        .color(text_secondary),
                                );
                                ui.end_row();

                                for (i, action) in FiosAction::ALL.iter().enumerate() {
                                    ui.label(
                                        egui::RichText::new(
                                            action.label_for_mode(lang, self.active_control_mode),
                                        )
                                        .size(11.0)
                                        .color(text_primary),
                                    );

                                    let capture = self.capture_index == Some(i);
                                    let key_text = if capture {
                                        match lang {
                                            EngineLanguage::Pt => "Pressione...",
                                            EngineLanguage::En => "Press key...",
                                            EngineLanguage::Es => "Presione...",
                                        }
                                    } else {
                                        Self::key_to_string(self.bindings[i])
                                    };

                                    let key_btn = egui::Button::new(
                                        egui::RichText::new(key_text).size(10.5).color(
                                            if capture {
                                                accent
                                            } else {
                                                egui::Color32::from_gray(200)
                                            },
                                        ),
                                    )
                                    .fill(if capture {
                                        egui::Color32::from_rgba_unmultiplied(15, 232, 121, 18)
                                    } else {
                                        surface_2
                                    })
                                    .stroke(egui::Stroke::new(
                                        1.0,
                                        if capture { accent } else { border },
                                    ))
                                    .corner_radius(5.0);

                                    if ui.add_sized([110.0, 22.0], key_btn).clicked() {
                                        self.capture_index = Some(i);
                                        self.status = Some(
                                            match lang {
                                                EngineLanguage::Pt => "Aguardando tecla...",
                                                EngineLanguage::En => "Waiting for key...",
                                                EngineLanguage::Es => "Esperando tecla...",
                                            }
                                            .to_string(),
                                        );
                                    }

                                    let is_on = self.pressed[i];
                                    let state_txt = if is_on { "●" } else { "○" };
                                    ui.label(egui::RichText::new(state_txt).size(12.0).color(
                                        if is_on {
                                            accent
                                        } else {
                                            egui::Color32::from_gray(70)
                                        },
                                    ));
                                    ui.end_row();
                                }
                            });

                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            let restore_btn = egui::Button::new(
                                egui::RichText::new(restore_txt)
                                    .size(10.5)
                                    .color(text_secondary),
                            )
                            .fill(surface_2)
                            .stroke(egui::Stroke::new(1.0, border))
                            .corner_radius(6.0);
                            if ui.add(restore_btn).clicked() {
                                self.bindings = Self::default_bindings();
                                self.status = match self.save_to_disk() {
                                    Ok(()) => Some(
                                        match lang {
                                            EngineLanguage::Pt => "Padrão restaurado",
                                            EngineLanguage::En => "Defaults restored",
                                            EngineLanguage::Es => "Pred. restaurado",
                                        }
                                        .to_string(),
                                    ),
                                    Err(err) => Some(format!("Falha ao salvar: {err}")),
                                };
                            }

                            let save_btn = egui::Button::new(
                                egui::RichText::new(save_txt).size(10.5).color(accent),
                            )
                            .fill(egui::Color32::from_rgba_unmultiplied(15, 232, 121, 18))
                            .stroke(egui::Stroke::new(
                                1.0,
                                egui::Color32::from_rgba_unmultiplied(15, 232, 121, 60),
                            ))
                            .corner_radius(6.0);
                            if ui.add(save_btn).clicked() {
                                self.status = match self.save_to_disk() {
                                    Ok(()) => Some(
                                        match lang {
                                            EngineLanguage::Pt => "Controles salvos",
                                            EngineLanguage::En => "Controls saved",
                                            EngineLanguage::Es => "Controles guardados",
                                        }
                                        .to_string(),
                                    ),
                                    Err(err) => Some(format!("Falha ao salvar: {err}")),
                                };
                            }
                        });
                    });

                ui.add_space(12.0);
            });
    }

    fn draw_animator_tab(&mut self, ui: &mut egui::Ui, lang: EngineLanguage) {
        if self.anim_is_playing {
            self.anim_current_time += ui.ctx().input(|i| i.stable_dt as f64).max(1.0 / 60.0);
            if self.anim_current_time >= self.anim_total_duration {
                self.anim_current_time = 0.0;
            }
        }

        egui::Frame::default()
            .fill(egui::Color32::from_rgb(32, 32, 36))
            .show(ui, |ui| {
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    let icon_size = egui::vec2(32.0, 32.0);
                    
                    let play_icon = egui::RichText::new("▶").size(16.0).color(egui::Color32::WHITE);
                    let play_btn = egui::Button::new(play_icon)
                        .fill(if self.anim_is_playing { egui::Color32::from_rgb(60, 100, 60) } else { egui::Color32::from_rgb(40, 80, 50) })
                        .frame(true)
                        .min_size(icon_size);
                    if ui.add(play_btn).clicked() {
                        self.anim_is_playing = !self.anim_is_playing;
                        self.anim_tab_status = Some(if self.anim_is_playing { "Playing".to_string() } else { "Paused".to_string() });
                    }

                    ui.add_space(4.0);

                    let stop_icon = egui::RichText::new("◼").size(14.0).color(egui::Color32::WHITE);
                    let stop_btn = egui::Button::new(stop_icon)
                        .fill(egui::Color32::from_rgb(90, 50, 50))
                        .frame(true)
                        .min_size(icon_size);
                    if ui.add(stop_btn).clicked() {
                        self.anim_is_playing = false;
                        self.anim_current_time = 0.0;
                        self.anim_tab_status = Some("Stopped".to_string());
                    }

                    ui.add_space(12.0);

                    let record_icon = egui::RichText::new("●").size(12.0).color(if self.anim_is_recording { egui::Color32::RED } else { egui::Color32::from_gray(150) });
                    let record_btn = egui::Button::new(record_icon)
                        .fill(if self.anim_is_recording { egui::Color32::from_rgb(100, 30, 30) } else { egui::Color32::from_rgb(60, 40, 40) })
                        .frame(true)
                        .min_size(icon_size);
                    if ui.add(record_btn).clicked() {
                        self.anim_is_recording = !self.anim_is_recording;
                        self.anim_tab_status = Some(if self.anim_is_recording { "Recording" } else { "Recording stopped" }.to_string());
                    }

                    ui.add_space(16.0);

                    let skip_start_icon = egui::RichText::new("⏮").size(14.0);
                    let skip_start_btn = egui::Button::new(skip_start_icon)
                        .fill(egui::Color32::from_rgb(45, 45, 55))
                        .frame(true)
                        .min_size(icon_size);
                    if ui.add(skip_start_btn).clicked() {
                        self.anim_current_time = 0.0;
                    }

                    ui.add_space(4.0);

                    let prev_key_icon = egui::RichText::new("⏪").size(14.0);
                    let prev_key_btn = egui::Button::new(prev_key_icon)
                        .fill(egui::Color32::from_rgb(45, 45, 55))
                        .frame(true)
                        .min_size(icon_size);
                    if ui.add(prev_key_btn).clicked() {
                        self.anim_current_time = (self.anim_current_time - 0.1).max(0.0);
                    }

                    ui.add_space(4.0);

                    let next_key_icon = egui::RichText::new("⏩").size(14.0);
                    let next_key_btn = egui::Button::new(next_key_icon)
                        .fill(egui::Color32::from_rgb(45, 45, 55))
                        .frame(true)
                        .min_size(icon_size);
                    if ui.add(next_key_btn).clicked() {
                        self.anim_current_time = (self.anim_current_time + 0.1).min(self.anim_total_duration);
                    }

                    ui.add_space(4.0);

                    let skip_end_icon = egui::RichText::new("⏭").size(14.0);
                    let skip_end_btn = egui::Button::new(skip_end_icon)
                        .fill(egui::Color32::from_rgb(45, 45, 55))
                        .frame(true)
                        .min_size(icon_size);
                    if ui.add(skip_end_btn).clicked() {
                        self.anim_current_time = self.anim_total_duration;
                    }

                    ui.add_space(16.0);

                    let keyframe_icon = egui::RichText::new("◆").size(12.0).color(egui::Color32::from_rgb(100, 180, 255));
                    let keyframe_btn = egui::Button::new(keyframe_icon)
                        .fill(egui::Color32::from_rgb(50, 70, 100))
                        .frame(true)
                        .min_size(icon_size);
                    if ui.add(keyframe_btn).clicked() {
                        self.anim_tab_status = Some(format!("Keyframe added at {:.2}s", self.anim_current_time));
                    }

                    ui.add_space(4.0);

                    let delete_key_icon = egui::RichText::new("◇").size(12.0).color(egui::Color32::from_gray(150));
                    let delete_key_btn = egui::Button::new(delete_key_icon)
                        .fill(egui::Color32::from_rgb(80, 50, 50))
                        .frame(true)
                        .min_size(icon_size);
                    if ui.add(delete_key_btn).clicked() {
                        self.anim_tab_status = Some("Delete Keyframe".to_string());
                    }

                    ui.add_space(16.0);

                    let loop_icon = egui::RichText::new("🔁").size(14.0);
                    let loop_btn = egui::Button::new(loop_icon)
                        .fill(egui::Color32::from_rgb(45, 45, 55))
                        .frame(true)
                        .min_size(icon_size);
                    if ui.add(loop_btn).clicked() {
                        self.anim_tab_status = Some("Loop toggled".to_string());
                    }
                });

                ui.add_space(12.0);

                ui.separator();

                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Time:").size(12.0).color(egui::Color32::from_gray(180)));
                    ui.add_space(4.0);
                    
                    let current_str = format!("{:.2}", self.anim_current_time);
                    let duration_str = format!("{:.2}", self.anim_total_duration);
                    ui.label(egui::RichText::new(format!("{}/{}", current_str, duration_str)).size(13.0).strong().color(egui::Color32::from_rgb(255, 200, 100)));

                    ui.add_space(20.0);

                    ui.label(egui::RichText::new("Duration:").size(12.0).color(egui::Color32::from_gray(180)));
                    ui.add_space(4.0);
                    
                    let mut duration = self.anim_total_duration;
                    ui.add(egui::DragValue::new(&mut duration).range(0.1..=60.0).speed(0.1));
                    self.anim_total_duration = duration;

                    ui.add_space(20.0);

                    let fps = 30;
                    ui.label(egui::RichText::new(format!("FPS: {}", fps)).size(12.0).color(egui::Color32::from_gray(150)));
                });

                ui.add_space(12.0);

                let timeline_rect = ui.available_rect_before_wrap();
                let timeline_height = 140.0;
                let track_area = egui::Rect::from_min_size(
                    egui::pos2(timeline_rect.left(), timeline_rect.top()),
                    egui::vec2(timeline_rect.width(), timeline_height),
                );

                let painter = ui.painter();
                painter.rect_filled(track_area, 4.0, egui::Color32::from_rgb(28, 28, 32));
                painter.rect_stroke(track_area, 4.0, egui::Stroke::new(1.0, egui::Color32::from_rgb(60, 60, 70)), egui::StrokeKind::Outside);

                let time_to_x = |t: f64| -> f32 {
                    track_area.left() + (t / self.anim_total_duration * track_area.width() as f64) as f32
                };

                let tick_count = (self.anim_total_duration / 0.5).ceil() as usize;
                for i in 0..=tick_count {
                    let t = i as f64 * 0.5;
                    let x = time_to_x(t);
                    let is_major = i % 2 == 0;
                    let tick_height = if is_major { 16.0 } else { 8.0 };
                    
                    painter.line_segment(
                        [
                            egui::pos2(x, track_area.bottom() - tick_height),
                            egui::pos2(x, track_area.bottom()),
                        ],
                        egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(100, 100, 120, 150)),
                    );
                    
                    if is_major {
                        painter.text(
                            egui::pos2(x, track_area.bottom() - tick_height - 14.0),
                            egui::Align2::CENTER_TOP,
                            format!("{:.1}s", t),
                            egui::FontId::proportional(10.0),
                            egui::Color32::from_gray(140),
                        );
                    }
                }

                for i in 0..6 {
                    let track_y = track_area.top() + 8.0 + (i as f32) * 22.0;
                    painter.line_segment(
                        [
                            egui::pos2(track_area.left(), track_y),
                            egui::pos2(track_area.right(), track_y),
                        ],
                        egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(70, 70, 90, 80)),
                    );
                }

                let track_labels = ["X", "Y", "Z", "RX", "RY", "RZ"];
                for (i, label) in track_labels.iter().enumerate().take(6) {
                    let track_y = track_area.top() + 10.0 + (i as f32) * 22.0;
                    painter.text(
                        egui::pos2(track_area.left() + 4.0, track_y),
                        egui::Align2::LEFT_TOP,
                        *label,
                        egui::FontId::proportional(10.0),
                        egui::Color32::from_gray(160),
                    );
                }

                let playhead_x = time_to_x(self.anim_current_time);
                painter.line_segment(
                    [
                        egui::pos2(playhead_x, track_area.top()),
                        egui::pos2(playhead_x, track_area.bottom()),
                    ],
                    egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 100, 80)),
                );
                painter.circle_filled(egui::pos2(playhead_x, track_area.top() + 6.0), 5.0, egui::Color32::from_rgb(255, 100, 80));

                let keyframe_times = [0.5, 1.2, 2.0, 3.5, 4.0];
                for &kf_time in &keyframe_times {
                    let kf_x = time_to_x(kf_time);
                    for track in 0..3 {
                        let track_y = track_area.top() + 12.0 + (track as f32) * 22.0;
                        painter.circle_filled(egui::pos2(kf_x, track_y), 4.0, egui::Color32::from_rgb(100, 200, 255));
                    }
                }

                ui.add_space(timeline_height + 12.0);

                ui.separator();

                ui.add_space(8.0);

                egui::Grid::new("anim_properties_grid")
                    .num_columns(2)
                    .spacing([8.0, 6.0])
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("Position").size(12.0).strong().color(egui::Color32::from_rgb(180, 180, 220)));
                        ui.horizontal(|ui| {
                            let mut px = 0.0f32;
                            let mut py = 0.0f32;
                            let mut pz = 0.0f32;
                            ui.add(egui::DragValue::new(&mut px).prefix("X:").speed(0.01));
                            ui.add(egui::DragValue::new(&mut py).prefix("Y:").speed(0.01));
                            ui.add(egui::DragValue::new(&mut pz).prefix("Z:").speed(0.01));
                        });
                        ui.end_row();

                        ui.label(egui::RichText::new("Rotation").size(12.0).strong().color(egui::Color32::from_rgb(220, 180, 180)));
                        ui.horizontal(|ui| {
                            let mut rx = 0.0f32;
                            let mut ry = 0.0f32;
                            let mut rz = 0.0f32;
                            ui.add(egui::DragValue::new(&mut rx).prefix("X:").speed(0.5));
                            ui.add(egui::DragValue::new(&mut ry).prefix("Y:").speed(0.5));
                            ui.add(egui::DragValue::new(&mut rz).prefix("Z:").speed(0.5));
                        });
                        ui.end_row();

                        ui.label(egui::RichText::new("Scale").size(12.0).strong().color(egui::Color32::from_rgb(180, 220, 180)));
                        ui.horizontal(|ui| {
                            let mut sx = 1.0f32;
                            let mut sy = 1.0f32;
                            let mut sz = 1.0f32;
                            ui.add(egui::DragValue::new(&mut sx).prefix("X:").speed(0.01));
                            ui.add(egui::DragValue::new(&mut sy).prefix("Y:").speed(0.01));
                            ui.add(egui::DragValue::new(&mut sz).prefix("Z:").speed(0.01));
                        });
                        ui.end_row();
                    });

                ui.add_space(12.0);

                ui.separator();

                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    let help_txt = match lang {
                        EngineLanguage::Pt => "Barra de Espaço: Play/Pause | K: Add Keyframe | L: Loop | Home/End: Ir para início/fim",
                        EngineLanguage::En => "Space: Play/Pause | K: Add Keyframe | L: Loop | Home/End: Go to start/end",
                        EngineLanguage::Es => "Espacio: Play/Pause | K: Add Keyframe | L: Loop | Inicio/Fin: Ir al inicio/final",
                    };
                    ui.label(egui::RichText::new(help_txt).size(10.0).color(egui::Color32::from_gray(120)));
                });
            });

        if let Some(status) = &self.anim_tab_status {
            ui.add_space(8.0);
            egui::Frame::default()
                .fill(egui::Color32::from_rgba_unmultiplied(40, 40, 50, 200))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(status)
                            .size(11.0)
                            .color(egui::Color32::from_gray(200)),
                    );
                });
        }
    }

    fn draw_controller_tab(&mut self, ui: &mut egui::Ui, lang: EngineLanguage) {
        self.refresh_anim_clip_cache(ui.ctx(), false);
        let clips_txt = match lang {
            EngineLanguage::Pt => "Clipes",
            EngineLanguage::En => "Clips",
            EngineLanguage::Es => "Clips",
        };
        let help_txt = match lang {
            EngineLanguage::Pt => {
                "Arraste clipes para o canvas. Clique saída e depois entrada para ligar estados."
            }
            EngineLanguage::En => {
                "Drag clips to canvas. Click output then input to connect states."
            }
            EngineLanguage::Es => {
                "Arrastra clips al canvas. Haz clic en salida y luego entrada para conectar estados."
            }
        };

        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(help_txt)
                    .size(11.0)
                    .color(egui::Color32::from_gray(185)),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Atualizar").clicked() {
                    self.anim_clip_cache_dirty = true;
                    self.refresh_anim_clip_cache(ui.ctx(), true);
                }
                if ui.button("Limpar").clicked() {
                    self.anim_nodes.clear();
                    self.anim_links.clear();
                    self.anim_connect_from = None;
                    self.anim_tab_status = Some("Canvas limpo".to_string());
                }
            });
        });
        ui.add_space(6.0);

        let area = ui.available_rect_before_wrap();
        if area.width() < 300.0 || area.height() < 120.0 {
            return;
        }

        let left_w = 220.0_f32.min((area.width() * 0.22).max(180.0));
        let right_w = 200.0_f32.min((area.width() * 0.2).max(160.0));
        let canvas_rect = egui::Rect::from_min_size(
            egui::pos2(area.left() + left_w + 6.0, area.top()),
            egui::vec2(area.width() - left_w - right_w - 12.0, area.height()),
        );
        let left_rect =
            egui::Rect::from_min_max(area.min, egui::pos2(area.left() + left_w, area.bottom()));
        let right_rect =
            egui::Rect::from_min_max(egui::pos2(canvas_rect.right() + 6.0, area.top()), area.max);

        let canvas_painter = ui.painter().with_clip_rect(canvas_rect);

        ui.painter()
            .rect_filled(left_rect, 6.0, egui::Color32::from_rgb(24, 26, 30));
        ui.painter().rect_stroke(
            left_rect,
            6.0,
            egui::Stroke::new(1.0, egui::Color32::from_rgb(58, 64, 72)),
            egui::StrokeKind::Outside,
        );

        ui.painter()
            .rect_filled(right_rect, 6.0, egui::Color32::from_rgb(24, 26, 30));
        ui.painter().rect_stroke(
            right_rect,
            6.0,
            egui::Stroke::new(1.0, egui::Color32::from_rgb(58, 64, 72)),
            egui::StrokeKind::Outside,
        );

        canvas_painter.rect_filled(canvas_rect, 6.0, egui::Color32::from_rgb(19, 21, 25));
        canvas_painter.rect_stroke(
            canvas_rect,
            6.0,
            egui::Stroke::new(1.0, egui::Color32::from_rgb(56, 70, 94)),
            egui::StrokeKind::Outside,
        );

        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(left_rect.shrink(8.0))
                .layout(egui::Layout::top_down(egui::Align::Min)),
            |ui| {
                ui.label(egui::RichText::new(clips_txt).strong().size(12.0));
                ui.add_space(4.0);
                if self.anim_clip_cache.is_empty() {
                    ui.label(
                        egui::RichText::new("Sem clipes detectados")
                            .size(11.0)
                            .color(egui::Color32::from_gray(170)),
                    );
                    ui.add_space(4.0);
                    if ui.button("Atualizar").clicked() {
                        self.anim_clip_cache_dirty = true;
                        self.refresh_anim_clip_cache(ui.ctx(), true);
                    }
                }
                let clip_list = self.anim_clip_cache.clone();
                egui::ScrollArea::vertical()
                    .id_salt("clip_library_scroll")
                    .max_height(150.0)
                    .show(ui, |ui| {
                        for clip in &clip_list {
                            ui.push_id(clip.as_str(), |ui| {
                                let resp = ui
                                    .add_sized(
                                        [ui.available_width(), 22.0],
                                        egui::Button::new(
                                            egui::RichText::new(
                                                clip.split("::").last().unwrap_or(clip.as_str()),
                                            )
                                            .size(10.5),
                                        )
                                        .fill(egui::Color32::from_rgb(36, 40, 46))
                                        .stroke(
                                            egui::Stroke::new(
                                                1.0,
                                                egui::Color32::from_rgb(52, 58, 66),
                                            ),
                                        ),
                                    )
                                    .on_hover_text(clip.as_str());
                                if resp.drag_started() {
                                    self.anim_drag_clip = Some(clip.clone());
                                }
                                if resp.double_clicked() {
                                    let pos = egui::pos2(
                                        20.0 + (self.anim_nodes.len() as f32 * 30.0)
                                            % (canvas_rect.width() - 180.0),
                                        20.0 + (self.anim_nodes.len() as f32 * 20.0)
                                            % (canvas_rect.height() - 60.0),
                                    );
                                    self.add_anim_controller_node(clip.clone(), pos);
                                    self.anim_tab_status = Some("Estado criado".to_string());
                                }
                            });
                        }
                    });
            },
        );

        let grid_step = 28.0;
        let mut gx = canvas_rect.left();
        while gx <= canvas_rect.right() {
            canvas_painter.line_segment(
                [
                    egui::pos2(gx, canvas_rect.top()),
                    egui::pos2(gx, canvas_rect.bottom()),
                ],
                egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(76, 98, 132, 18)),
            );
            gx += grid_step;
        }
        let mut gy = canvas_rect.top();
        while gy <= canvas_rect.bottom() {
            canvas_painter.line_segment(
                [
                    egui::pos2(canvas_rect.left(), gy),
                    egui::pos2(canvas_rect.right(), gy),
                ],
                egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(76, 98, 132, 18)),
            );
            gy += grid_step;
        }

        for (link_idx, link) in self.anim_links.iter().enumerate() {
            let from = self.anim_nodes.iter().find(|n| n.id == link.from);
            let to = self.anim_nodes.iter().find(|n| n.id == link.to);
            if let (Some(a), Some(b)) = (from, to) {
                let p0 = canvas_rect.min + a.pos.to_vec2() + egui::vec2(170.0, 24.0);
                let p1 = canvas_rect.min + b.pos.to_vec2() + egui::vec2(0.0, 24.0);

                let is_selected = self.anim_selected_link == Some(link_idx);
                let link_color = if is_selected {
                    egui::Color32::from_rgb(15, 232, 121)
                } else {
                    egui::Color32::from_rgb(110, 182, 232)
                };

                canvas_painter.line_segment([p0, p1], egui::Stroke::new(2.5, link_color));

                let mid = egui::pos2((p0.x + p1.x) / 2.0, (p0.y + p1.y) / 2.0);
                let hitbox = egui::Rect::from_center_size(mid, egui::vec2(20.0, 20.0));
                let link_hit = ui.interact(
                    hitbox,
                    ui.id().with(("anim_link", link_idx)),
                    egui::Sense::click(),
                );
                if link_hit.clicked() {
                    self.anim_selected_nodes.clear();
                    self.anim_selected_link = Some(link_idx);
                }
            }
        }

        for i in 0..self.anim_nodes.len() {
            let id = self.anim_nodes[i].id;
            ui.push_id(id, |ui| {
                let mut local = self.anim_nodes[i].pos;
                local.x = local.x.clamp(4.0, (canvas_rect.width() - 174.0).max(4.0));
                local.y = local.y.clamp(4.0, (canvas_rect.height() - 52.0).max(4.0));
                self.anim_nodes[i].pos = local;

                let rect = egui::Rect::from_min_size(
                    canvas_rect.min + local.to_vec2(),
                    egui::vec2(170.0, 48.0),
                );
                canvas_painter.rect_filled(rect, 5.0, egui::Color32::from_rgb(35, 45, 58));
                canvas_painter.rect_stroke(
                    rect,
                    5.0,
                    egui::Stroke::new(1.0, egui::Color32::from_rgb(80, 124, 174)),
                    egui::StrokeKind::Outside,
                );
                canvas_painter.text(
                    rect.left_top() + egui::vec2(8.0, 7.0),
                    egui::Align2::LEFT_TOP,
                    &self.anim_nodes[i].name,
                    egui::FontId::proportional(11.0),
                    egui::Color32::from_gray(232),
                );
                canvas_painter.text(
                    rect.left_bottom() + egui::vec2(8.0, -7.0),
                    egui::Align2::LEFT_BOTTOM,
                    &self.anim_nodes[i].clip_ref,
                    egui::FontId::proportional(9.0),
                    egui::Color32::from_gray(186),
                );

                let body = ui.interact(
                    rect,
                    ui.id().with(("anim_node_body", id)),
                    egui::Sense::click_and_drag(),
                );
                if body.dragged() {
                    self.anim_nodes[i].pos += body.drag_delta();
                }
                if body.clicked() {
                    if ui.input(|i| i.modifiers.shift) {
                        if self.anim_selected_nodes.contains(&id) {
                            self.anim_selected_nodes.remove(&id);
                        } else {
                            self.anim_selected_nodes.insert(id);
                        }
                    } else {
                        self.anim_selected_nodes.clear();
                        self.anim_selected_nodes.insert(id);
                        self.anim_selected_link = None;
                    }
                }

                let in_p = rect.left_center();
                let out_p = rect.right_center();
                canvas_painter.circle_filled(in_p, 4.0, egui::Color32::from_rgb(220, 116, 116));
                canvas_painter.circle_filled(out_p, 4.0, egui::Color32::from_rgb(112, 194, 238));
                let in_r = ui.interact(
                    egui::Rect::from_center_size(in_p, egui::vec2(12.0, 12.0)),
                    ui.id().with(("anim_node_in", id)),
                    egui::Sense::click(),
                );
                let out_r = ui.interact(
                    egui::Rect::from_center_size(out_p, egui::vec2(12.0, 12.0)),
                    ui.id().with(("anim_node_out", id)),
                    egui::Sense::click(),
                );
                if out_r.clicked() {
                    self.anim_connect_from = Some(id);
                }
                if in_r.clicked() {
                    if let Some(from) = self.anim_connect_from.take() {
                        if from != id
                            && !self.anim_links.iter().any(|l| l.from == from && l.to == id)
                        {
                            self.anim_links.push(AnimControllerLink {
                                from,
                                to: id,
                                blend_time: 0.3,
                                transition_type: TransitionType::CrossFade,
                            });
                        }
                    }
                }
            });
        }

        let pointer_pos = ui.ctx().input(|i| i.pointer.hover_pos());
        let mouse_down = ui.ctx().input(|i| i.pointer.primary_down());
        if let Some(clip) = self.anim_drag_clip.clone() {
            ui.ctx().request_repaint();
            if let Some(p) = pointer_pos {
                let drag_rect =
                    egui::Rect::from_center_size(p + egui::vec2(8.0, 8.0), egui::vec2(180.0, 22.0));
                canvas_painter.rect_filled(
                    drag_rect,
                    4.0,
                    egui::Color32::from_rgba_unmultiplied(48, 66, 90, 220),
                );
                canvas_painter.text(
                    drag_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    &clip,
                    egui::FontId::proportional(10.0),
                    egui::Color32::from_gray(235),
                );
            }
            if !mouse_down {
                if let Some(p) = pointer_pos {
                    if canvas_rect.contains(p) {
                        let mut assigned = false;
                        for node in &mut self.anim_nodes {
                            let node_rect = egui::Rect::from_min_size(
                                canvas_rect.min + node.pos.to_vec2(),
                                egui::vec2(170.0, 48.0),
                            );
                            if node_rect.contains(p) {
                                node.clip_ref = clip.clone();
                                node.name = clip.split("::").last().unwrap_or("State").to_string();
                                assigned = true;
                                self.anim_tab_status =
                                    Some("Clipe atribuído ao estado".to_string());
                                break;
                            }
                        }
                        if !assigned {
                            let local = p - canvas_rect.min.to_vec2() - egui::vec2(85.0, 24.0);
                            self.add_anim_controller_node(clip.clone(), local);
                            self.anim_tab_status = Some("Estado criado".to_string());
                        }
                        self.ensure_clip_in_cache(&clip);
                    }
                }
                self.anim_drag_clip = None;
            }
        }

        if let Some(msg) = &self.anim_tab_status {
            canvas_painter.text(
                canvas_rect.left_bottom() + egui::vec2(8.0, -6.0),
                egui::Align2::LEFT_BOTTOM,
                msg,
                egui::FontId::proportional(10.0),
                egui::Color32::from_gray(172),
            );
        }

        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(right_rect.shrink(10.0))
                .layout(egui::Layout::top_down(egui::Align::Min)),
            |ui| {
                let props_txt = match lang {
                    EngineLanguage::Pt => "Propriedades",
                    EngineLanguage::En => "Properties",
                    EngineLanguage::Es => "Propiedades",
                };
                ui.label(egui::RichText::new(props_txt).strong().size(12.0));
                ui.add_space(8.0);

                let selected_count = self.anim_selected_nodes.len();
                let selected_link = self.anim_selected_link;

                if selected_count == 0 && selected_link.is_none() {
                    ui.label(
                        egui::RichText::new("Selecione um nó ou conexão")
                            .size(11.0)
                            .color(egui::Color32::from_gray(120)),
                    );
                } else if selected_count == 1 {
                    if let Some(&node_id) = self.anim_selected_nodes.iter().next() {
                        if let Some(node_idx) = self.anim_nodes.iter().position(|n| n.id == node_id)
                        {
                            let node = &mut self.anim_nodes[node_idx];

                            let name_txt = match lang {
                                EngineLanguage::Pt => "Nome",
                                EngineLanguage::En => "Name",
                                EngineLanguage::Es => "Nombre",
                            };
                            ui.label(
                                egui::RichText::new(name_txt)
                                    .size(10.0)
                                    .color(egui::Color32::from_gray(170)),
                            );
                            ui.text_edit_singleline(&mut node.name);

                            ui.add_space(6.0);
                            let clip_txt = match lang {
                                EngineLanguage::Pt => "Clipe",
                                EngineLanguage::En => "Clip",
                                EngineLanguage::Es => "Clip",
                            };
                            ui.label(
                                egui::RichText::new(clip_txt)
                                    .size(10.0)
                                    .color(egui::Color32::from_gray(170)),
                            );
                            ui.text_edit_singleline(&mut node.clip_ref);

                            ui.add_space(6.0);
                            let speed_txt = match lang {
                                EngineLanguage::Pt => "Velocidade",
                                EngineLanguage::En => "Speed",
                                EngineLanguage::Es => "Velocidad",
                            };
                            ui.label(
                                egui::RichText::new(speed_txt)
                                    .size(10.0)
                                    .color(egui::Color32::from_gray(170)),
                            );
                            ui.add(egui::Slider::new(&mut node.speed, 0.1..=3.0).text("Speed"));

                            ui.add_space(10.0);
                            let delete_txt = match lang {
                                EngineLanguage::Pt => "Deletar",
                                EngineLanguage::En => "Delete",
                                EngineLanguage::Es => "Eliminar",
                            };
                            if ui.button(delete_txt).clicked() {
                                self.anim_links
                                    .retain(|l| l.from != node_id && l.to != node_id);
                                self.anim_nodes.retain(|n| n.id != node_id);
                                self.anim_selected_nodes.clear();
                                self.anim_tab_status = Some("Estado deletado".to_string());
                            }
                        }
                    }
                } else if selected_count > 1 {
                    ui.label(
                        egui::RichText::new(format!("{} nós selecionados", selected_count))
                            .size(11.0)
                            .color(egui::Color32::from_gray(170)),
                    );
                } else if let Some(link_idx) = selected_link {
                    if let Some(_link) = self.anim_links.get(link_idx) {
                        let trans_txt = match lang {
                            EngineLanguage::Pt => "Tipo de Transição",
                            EngineLanguage::En => "Transition Type",
                            EngineLanguage::Es => "Tipo de Transición",
                        };
                        ui.label(
                            egui::RichText::new(trans_txt)
                                .size(10.0)
                                .color(egui::Color32::from_gray(170)),
                        );

                        let link_mut = &mut self.anim_links[link_idx];
                        let trans_options = ["CrossFade", "Immediate", "Freeze"];
                        let current_idx = match link_mut.transition_type {
                            TransitionType::CrossFade => 0,
                            TransitionType::Immediate => 1,
                            TransitionType::Freeze => 2,
                        };

                        egui::ComboBox::from_id_salt("trans_type_combo")
                            .selected_text(trans_options[current_idx])
                            .show_ui(ui, |ui| {
                                for (i, opt) in trans_options.iter().enumerate() {
                                    if ui.selectable_label(current_idx == i, *opt).clicked() {
                                        link_mut.transition_type = match i {
                                            0 => TransitionType::CrossFade,
                                            1 => TransitionType::Immediate,
                                            _ => TransitionType::Freeze,
                                        };
                                    }
                                }
                            });

                        ui.add_space(6.0);
                        let blend_txt = match lang {
                            EngineLanguage::Pt => "Tempo de Blend",
                            EngineLanguage::En => "Blend Time",
                            EngineLanguage::Es => "Tiempo de Blend",
                        };
                        ui.label(
                            egui::RichText::new(blend_txt)
                                .size(10.0)
                                .color(egui::Color32::from_gray(170)),
                        );
                        ui.add(egui::Slider::new(&mut link_mut.blend_time, 0.0..=2.0).text("s"));

                        ui.add_space(10.0);
                        if ui.button("Remover Conexão").clicked() {
                            self.anim_links.remove(link_idx);
                            self.anim_selected_link = None;
                            self.anim_tab_status = Some("Conexão removida".to_string());
                        }
                    }
                }
            },
        );
    }

    fn draw_tabs_content(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, lang: EngineLanguage) {
        if self.add_icon_texture.is_none() {
            self.add_icon_texture = Self::load_png_texture(ctx, "src/assets/icons/add.png");
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let controls_txt = match lang {
                EngineLanguage::Pt => "Módulos",
                EngineLanguage::En => "Módulos",
                EngineLanguage::Es => "Módulos",
            };
            let graph_txt = match lang {
                EngineLanguage::Pt => "Fios",
                EngineLanguage::En => "Fios",
                EngineLanguage::Es => "Fios",
            };
            let creator_txt = match lang {
                EngineLanguage::Pt => "Controlador de animação",
                EngineLanguage::En => "Animation Controller",
                EngineLanguage::Es => "Controlador de animación",
            };
            let animator_txt = match lang {
                EngineLanguage::Pt => "Animador",
                EngineLanguage::En => "Animator",
                EngineLanguage::Es => "Animador",
            };
            let c = self.tab == FiosTab::Controls;
            let g = self.tab == FiosTab::Graph;
            let k = self.tab == FiosTab::Controller;
            let a = self.tab == FiosTab::Animator;
            if ui
                .add(egui::Button::new(controls_txt).fill(if c {
                    egui::Color32::from_rgb(58, 84, 64)
                } else {
                    egui::Color32::from_rgb(52, 52, 52)
                }))
                .clicked()
            {
                self.tab = FiosTab::Controls;
            }
            if ui
                .add(egui::Button::new(graph_txt).fill(if g {
                    egui::Color32::from_rgb(108, 76, 156)
                } else {
                    egui::Color32::from_rgb(52, 52, 52)
                }))
                .clicked()
            {
                self.tab = FiosTab::Graph;
            }
            if ui
                .add(egui::Button::new(creator_txt).fill(if k {
                    egui::Color32::from_rgb(76, 96, 156)
                } else {
                    egui::Color32::from_rgb(52, 52, 52)
                }))
                .clicked()
            {
                self.tab = FiosTab::Controller;
            }
            if ui
                .add(egui::Button::new(animator_txt).fill(if a {
                    egui::Color32::from_rgb(156, 96, 76)
                } else {
                    egui::Color32::from_rgb(52, 52, 52)
                }))
                .clicked()
            {
                self.tab = FiosTab::Animator;
            }
        });
        ui.add_space(4.0);
        ui.separator();
        ui.add_space(8.0);
        match self.tab {
            FiosTab::Controls => self.draw_controls_tab(ui, lang),
            FiosTab::Graph => self.draw_graph(ui, lang),
            FiosTab::Controller => self.draw_controller_tab(ui, lang),
            FiosTab::Animator => self.draw_animator_tab(ui, lang),
        }
    }

    pub fn draw_embedded(
        &mut self,
        ctx: &egui::Context,
        left_reserved: f32,
        right_reserved: f32,
        bottom_reserved: f32,
        lang: EngineLanguage,
    ) {
        ctx.output_mut(|o| o.cursor_icon = egui::CursorIcon::Default);
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(28, 28, 30))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(48, 48, 52))),
            )
            .show(ctx, |ui| {
                let content = ui.max_rect();
                let panel_rect = egui::Rect::from_min_max(
                    egui::pos2(content.left() + left_reserved, content.top()),
                    egui::pos2(
                        content.right() - right_reserved,
                        content.bottom() - bottom_reserved,
                    ),
                );
                if panel_rect.width() < 80.0 || panel_rect.height() < 80.0 {
                    self.embedded_panel_rect = None;
                    return;
                }
                self.embedded_panel_rect = Some(panel_rect);
                ui.painter()
                    .rect_filled(panel_rect, 0.0, egui::Color32::from_rgb(22, 22, 24));
                ui.painter().rect_stroke(
                    panel_rect,
                    0.0,
                    egui::Stroke::new(1.0, egui::Color32::from_rgb(58, 58, 62)),
                    egui::StrokeKind::Outside,
                );
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(panel_rect.shrink2(egui::vec2(10.0, 8.0)))
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                    |ui| self.draw_tabs_content(ui, ctx, lang),
                );
            });
    }
}
