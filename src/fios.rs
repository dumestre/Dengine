use crate::EngineLanguage;
use eframe::egui;
use mlua::{Function, Lua, MultiValue, RegistryKey, Table, Value};
use rfd::FileDialog;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

const ACTION_COUNT: usize = 8;

#[derive(Clone, Copy, PartialEq, Eq)]
enum FiosTab {
    Controls,
    Graph,
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

pub struct FiosState {
    controls_enabled: bool,
    bindings: [egui::Key; ACTION_COUNT],
    pressed: [bool; ACTION_COUNT],
    just_pressed: [bool; ACTION_COUNT],
    capture_index: Option<usize>,
    status: Option<String>,
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
}

impl FiosState {
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

    pub fn new() -> Self {
        let lua_runtime = Lua::new();
        let mut out = Self {
            controls_enabled: true,
            bindings: Self::default_bindings(),
            pressed: [false; ACTION_COUNT],
            just_pressed: [false; ACTION_COUNT],
            capture_index: None,
            status: None,
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
            if let Some(idx) = FiosAction::ALL.iter().position(|a| a.id() == action_id.trim()) {
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

    fn save_lua_script_to_disk(&self) -> Result<(), String> {
        fs::write(Self::lua_script_path(), &self.lua_script).map_err(|e| e.to_string())
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
            let Some(k) = parts.next() else { continue; };
            let Some(v) = parts.next() else { continue; };
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
                    let Ok(id) = seg[0].parse::<u32>() else { continue; };
                    let Some(kind) = FiosNodeKind::from_id(seg[1]) else { continue; };
                    let Ok(x) = seg[2].parse::<f32>() else { continue; };
                    let Ok(y) = seg[3].parse::<f32>() else { continue; };
                    let Ok(value) = seg[4].parse::<f32>() else { continue; };
                    let Ok(param_a) = seg[5].parse::<f32>() else { continue; };
                    let Ok(param_b) = seg[6].parse::<f32>() else { continue; };
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
                    let Ok(from_node) = seg[0].parse::<u32>() else { continue; };
                    let Ok(from_port) = seg[1].parse::<u8>() else { continue; };
                    let Ok(to_node) = seg[2].parse::<u32>() else { continue; };
                    let Ok(to_port) = seg[3].parse::<u8>() else { continue; };
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
                    let Ok(id) = seg[0].parse::<u32>() else { continue; };
                    let name = Self::decode_field(seg[1]);
                    let Ok(r) = seg[2].parse::<u8>() else { continue; };
                    let Ok(g) = seg[3].parse::<u8>() else { continue; };
                    let Ok(b) = seg[4].parse::<u8>() else { continue; };
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
        if !self.controls_enabled {
            self.pressed = [false; ACTION_COUNT];
            self.just_pressed = [false; ACTION_COUNT];
            self.last_axis = [0.0, 0.0];
            self.last_look = [0.0, 0.0];
            self.last_action = 0.0;
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
                if let egui::Event::Key { key, pressed: true, .. } = ev {
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
                FiosNodeKind::InputAxis => if output_port == 0 { base_axis[0] } else { base_axis[1] },
                FiosNodeKind::InputAction => {
                    let action_idx = node.param_a.round().clamp(0.0, (ACTION_COUNT.saturating_sub(1)) as f32) as usize;
                    let mode_just = node.param_b.round() >= 1.0;
                    let active = if mode_just { just_pressed[action_idx] } else { pressed[action_idx] };
                    if active { 1.0 } else { 0.0 }
                }
                FiosNodeKind::Constant => node.value,
                FiosNodeKind::Add => {
                    let a = Self::eval_input_of_node(nodes, links, smooth_state, pressed, just_pressed, node_id, 0, 0.0, base_axis, cache, stack);
                    let b = Self::eval_input_of_node(nodes, links, smooth_state, pressed, just_pressed, node_id, 1, 0.0, base_axis, cache, stack);
                    a + b
                }
                FiosNodeKind::Subtract => {
                    let a = Self::eval_input_of_node(nodes, links, smooth_state, pressed, just_pressed, node_id, 0, 0.0, base_axis, cache, stack);
                    let b = Self::eval_input_of_node(nodes, links, smooth_state, pressed, just_pressed, node_id, 1, 0.0, base_axis, cache, stack);
                    a - b
                }
                FiosNodeKind::Multiply => {
                    let a = Self::eval_input_of_node(nodes, links, smooth_state, pressed, just_pressed, node_id, 0, 0.0, base_axis, cache, stack);
                    let b = Self::eval_input_of_node(nodes, links, smooth_state, pressed, just_pressed, node_id, 1, 0.0, base_axis, cache, stack);
                    a * b
                }
                FiosNodeKind::Divide => {
                    let a = Self::eval_input_of_node(nodes, links, smooth_state, pressed, just_pressed, node_id, 0, 0.0, base_axis, cache, stack);
                    let b = Self::eval_input_of_node(nodes, links, smooth_state, pressed, just_pressed, node_id, 1, 1.0, base_axis, cache, stack);
                    if b.abs() < 1e-5 { 0.0 } else { a / b }
                }
                FiosNodeKind::Max => {
                    let a = Self::eval_input_of_node(nodes, links, smooth_state, pressed, just_pressed, node_id, 0, 0.0, base_axis, cache, stack);
                    let b = Self::eval_input_of_node(nodes, links, smooth_state, pressed, just_pressed, node_id, 1, 0.0, base_axis, cache, stack);
                    a.max(b)
                }
                FiosNodeKind::Min => {
                    let a = Self::eval_input_of_node(nodes, links, smooth_state, pressed, just_pressed, node_id, 0, 0.0, base_axis, cache, stack);
                    let b = Self::eval_input_of_node(nodes, links, smooth_state, pressed, just_pressed, node_id, 1, 0.0, base_axis, cache, stack);
                    a.min(b)
                }
                FiosNodeKind::Gate => {
                    let v = Self::eval_input_of_node(nodes, links, smooth_state, pressed, just_pressed, node_id, 0, 0.0, base_axis, cache, stack);
                    let g = Self::eval_input_of_node(nodes, links, smooth_state, pressed, just_pressed, node_id, 1, 0.0, base_axis, cache, stack);
                    if g > 0.0 { v } else { 0.0 }
                }
                FiosNodeKind::Abs => {
                    Self::eval_input_of_node(nodes, links, smooth_state, pressed, just_pressed, node_id, 0, 0.0, base_axis, cache, stack).abs()
                }
                FiosNodeKind::Sign => {
                    Self::eval_input_of_node(nodes, links, smooth_state, pressed, just_pressed, node_id, 0, 0.0, base_axis, cache, stack).signum()
                }
                FiosNodeKind::Clamp => {
                    let v = Self::eval_input_of_node(nodes, links, smooth_state, pressed, just_pressed, node_id, 0, 0.0, base_axis, cache, stack);
                    v.clamp(node.param_a.min(node.param_b), node.param_a.max(node.param_b))
                }
                FiosNodeKind::Deadzone => {
                    let v = Self::eval_input_of_node(nodes, links, smooth_state, pressed, just_pressed, node_id, 0, 0.0, base_axis, cache, stack);
                    let t = node.param_a.abs().clamp(0.0, 1.0);
                    if v.abs() < t { 0.0 } else { v }
                }
                FiosNodeKind::Invert => {
                    -Self::eval_input_of_node(nodes, links, smooth_state, pressed, just_pressed, node_id, 0, 0.0, base_axis, cache, stack)
                }
                FiosNodeKind::Smooth => {
                    let target = Self::eval_input_of_node(nodes, links, smooth_state, pressed, just_pressed, node_id, 0, 0.0, base_axis, cache, stack);
                    let alpha = node.param_a.clamp(0.0, 1.0);
                    let prev = *smooth_state.get(&key).unwrap_or(&target);
                    let v = prev + (target - prev) * alpha;
                    smooth_state.insert(key, v);
                    v
                }
                FiosNodeKind::OutputMove | FiosNodeKind::OutputLook | FiosNodeKind::OutputAction => 0.0,
            }
        } else {
            0.0
        };

        stack.remove(&key);
        cache.insert(key, out);
        out
    }

    fn create_link(&mut self, from_node: u32, from_port: u8, to_node: u32, to_port: u8) {
        self.links.retain(|l| !(l.to_node == to_node && l.to_port == to_port));
        self.links.push(FiosLink { from_node, from_port, to_node, to_port });
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
            FiosNodeKind::OutputAction => egui::vec2(170.0, 74.0),
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

    fn remove_selected_nodes(&mut self) -> bool {
        if self.selected_nodes.is_empty() {
            if let Some(id) = self.selected_node {
                self.selected_nodes.insert(id);
            }
        }
        if self.selected_nodes.is_empty() {
            return false;
        }
        self.nodes
            .retain(|n| !self.selected_nodes.contains(&n.id));
        self.links.retain(|l| {
            !self.selected_nodes.contains(&l.from_node)
                && !self.selected_nodes.contains(&l.to_node)
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
            selected_txt,
            none_txt,
            rename_txt,
            apply_name_txt,
            hint_txt,
            add_block_txt,
            actions_txt,
            shortcuts_txt,
            del_txt,
        ) = match lang {
            EngineLanguage::Pt => (
                "Entrada Eixo",
                "Entrada Acao",
                "Constante",
                "Somar",
                "Subtrair",
                "Multiplicar",
                "Dividir",
                "Maximo",
                "Minimo",
                "Portao",
                "Absoluto",
                "Sinal",
                "Limitar",
                "Zona Morta",
                "Inverter",
                "Suavizar",
                "Saida Mover",
                "Saida Olhar",
                "Saida Acao",
                "Selecionado(s)",
                "Nenhum",
                "Renomear",
                "Aplicar Nome",
                "Ctrl: multi-selecao | Arraste no vazio: caixa | Clique perto do output e arraste: prever/ligar fio | Alt + botao direito no output: ligar fio | Botao direito + arrastar: cortar fio",
                "Add Bloco",
                "Acoes",
                "Atalhos",
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
                "Selected",
                "None",
                "Rename",
                "Apply Name",
                "Ctrl: multi-select | Drag empty: marquee | Drag near output: predict/connect wire | Alt + right mouse on output: connect wire | Right mouse + drag: cut wire",
                "Add Block",
                "Actions",
                "Shortcuts",
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
                "Seleccionado(s)",
                "Ninguno",
                "Renombrar",
                "Aplicar Nombre",
                "Ctrl: multi-seleccion | Arrastrar vacio: caja | Arrastrar cerca de salida: predecir/conectar cable | Alt + boton derecho en salida: conectar cable | Boton derecho + arrastrar: cortar cable",
                "Agregar Bloque",
                "Acciones",
                "Atajos",
                "Eliminar Seleccionado",
            ),
        };

        ui.group(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(add_block_txt).strong().color(egui::Color32::from_gray(220)));
                ui.separator();
                ui.menu_button(
                    egui::RichText::new(add_block_txt).strong(),
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
                    egui::RichText::new(format!("{actions_txt}  |  {selected_txt}: {selected_text}"))
                        .strong()
                        .color(egui::Color32::from_gray(220)),
                );
                if ui
                    .add_sized(
                        egui::vec2(140.0, 26.0),
                        egui::Button::new(del_txt).fill(egui::Color32::from_rgb(96, 50, 50)),
                    )
                    .clicked()
                    && self.remove_selected_nodes()
                {
                    graph_dirty = true;
                }
                if ui.add_sized(egui::vec2(120.0, 26.0), egui::Button::new(rename_txt)).clicked() {
                    if let Some(id) = self.selected_nodes.iter().next().copied() {
                        self.rename_node = Some(id);
                        if let Some(i) = self.node_index_by_id(id) {
                            self.rename_buffer = self.nodes[i].display_name.clone();
                        }
                    }
                }
                if self.rename_node.is_some() {
                    ui.add_sized([190.0, 26.0], egui::TextEdit::singleline(&mut self.rename_buffer));
                    if ui.add_sized(egui::vec2(130.0, 26.0), egui::Button::new(apply_name_txt)).clicked() {
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
            ui.label(
                egui::RichText::new(format!("{shortcuts_txt}: {hint_txt}"))
                    .size(11.0)
                    .color(egui::Color32::from_gray(160)),
            );
        });
        ui.add_space(6.0);

        let canvas_size = ui.available_size();
        let (canvas_rect, canvas_resp) = ui.allocate_exact_size(canvas_size, egui::Sense::click_and_drag());
        let painter = ui.painter_at(canvas_rect);
        painter.rect_filled(canvas_rect, 6.0, egui::Color32::from_rgb(21, 22, 24));
        painter.rect_stroke(
            canvas_rect,
            6.0,
            egui::Stroke::new(1.0, egui::Color32::from_rgb(64, 66, 72)),
            egui::StrokeKind::Outside,
        );

        let grid = 24.0;
        let mut x = canvas_rect.left();
        while x < canvas_rect.right() {
            painter.line_segment(
                [egui::pos2(x, canvas_rect.top()), egui::pos2(x, canvas_rect.bottom())],
                egui::Stroke::new(0.5, egui::Color32::from_rgb(30, 31, 36)),
            );
            x += grid;
        }
        let mut y = canvas_rect.top();
        while y < canvas_rect.bottom() {
            painter.line_segment(
                [egui::pos2(canvas_rect.left(), y), egui::pos2(canvas_rect.right(), y)],
                egui::Stroke::new(0.5, egui::Color32::from_rgb(30, 31, 36)),
            );
            y += grid;
        }

        let mut rect_by_id = HashMap::<u32, egui::Rect>::new();
        for node in &self.nodes {
            let rect = egui::Rect::from_min_size(canvas_rect.min + node.pos, Self::node_size(node.kind));
            rect_by_id.insert(node.id, rect);
        }

        let pointer_pos = ui.ctx().input(|i| i.pointer.interact_pos());
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
                    let Some(rect) = rect_by_id.get(&node.id) else { continue; };
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
        if canvas_resp.clicked() && hovered_node.is_none() && !ctrl {
            self.selected_nodes.clear();
            self.selected_node = None;
        }
        if primary_pressed && hovered_node.is_none() {
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
                });
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
        for g in &self.groups {
            let mut min = egui::pos2(f32::INFINITY, f32::INFINITY);
            let mut max = egui::pos2(f32::NEG_INFINITY, f32::NEG_INFINITY);
            let mut count = 0usize;
            for id in &g.nodes {
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
            let fill = egui::Color32::from_rgba_unmultiplied(g.color.r(), g.color.g(), g.color.b(), 26);
            painter.rect_filled(gr, 8.0, fill);
            painter.rect_stroke(
                gr,
                8.0,
                egui::Stroke::new(1.2, g.color),
                egui::StrokeKind::Outside,
            );
            painter.text(
                gr.left_top() + egui::vec2(8.0, 6.0),
                egui::Align2::LEFT_TOP,
                &g.name,
                egui::FontId::proportional(11.0),
                g.color,
            );
        }
        for (link_idx, link) in self.links.iter().enumerate() {
            let Some(fi) = self.node_index_by_id(link.from_node) else { continue; };
            let Some(ti) = self.node_index_by_id(link.to_node) else { continue; };
            let Some(fr) = rect_by_id.get(&link.from_node) else { continue; };
            let Some(tr) = rect_by_id.get(&link.to_node) else { continue; };
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
            painter.add(egui::Shape::line(pts.clone(), egui::Stroke::new(2.0, egui::Color32::from_rgb(15, 232, 121))));
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
                    let Some(rect) = rect_by_id.get(&node.id) else { continue; };
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
        if secondary_pressed && !started_alt_wire_drag && self.drag_from_output.is_none() {
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
        let mut next_drag_from_output = self.drag_from_output;
        if next_drag_from_output.is_none() {
            if let Some((from_node, from_port, from_pos)) = auto_start_wire {
                next_drag_from_output = Some((from_node, from_port));
                self.wire_drag_path.clear();
                self.wire_drag_path.push(from_pos);
            }
        }
        let mut pending_group_drag_delta: Option<egui::Vec2> = None;
        for node in &mut self.nodes {
            let rect = egui::Rect::from_min_size(canvas_rect.min + node.pos, Self::node_size(node.kind));
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
            if drag_resp.dragged() {
                if self.selected_nodes.contains(&node.id) && self.selected_nodes.len() > 1 {
                    pending_group_drag_delta = Some(ui.ctx().input(|i| i.pointer.delta()));
                } else {
                    node.pos += ui.ctx().input(|i| i.pointer.delta());
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
                let val_rect = egui::Rect::from_min_size(rect.left_top() + egui::vec2(8.0, 32.0), egui::vec2(rect.width() - 16.0, 24.0));
                ui.scope_builder(egui::UiBuilder::new().max_rect(val_rect), |ui| {
                    ui.horizontal(|ui| {
                        ui.label("V");
                        if ui.add(egui::DragValue::new(&mut node.value).speed(0.05).range(-1000.0..=1000.0)).changed() {
                            graph_dirty = true;
                        }
                    });
                });
            }
            if node.kind == FiosNodeKind::InputAction {
                let r1 = egui::Rect::from_min_size(rect.left_top() + egui::vec2(8.0, 32.0), egui::vec2(rect.width() - 16.0, 24.0));
                let r2 = egui::Rect::from_min_size(rect.left_top() + egui::vec2(8.0, 58.0), egui::vec2(rect.width() - 16.0, 24.0));
                ui.scope_builder(egui::UiBuilder::new().max_rect(r1), |ui| {
                    let mut selected_idx = node
                        .param_a
                        .round()
                        .clamp(0.0, (ACTION_COUNT.saturating_sub(1)) as f32) as usize;
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
                    let mode_txt = if mode_just {
                        "JustPressed"
                    } else {
                        "Pressed"
                    };
                    if ui.checkbox(&mut mode_just, mode_txt).changed() {
                        node.param_b = if mode_just { 1.0 } else { 0.0 };
                        graph_dirty = true;
                    }
                });
            }
            if node.kind == FiosNodeKind::Clamp {
                let r1 = egui::Rect::from_min_size(rect.left_top() + egui::vec2(8.0, 32.0), egui::vec2(rect.width() - 16.0, 22.0));
                let r2 = egui::Rect::from_min_size(rect.left_top() + egui::vec2(8.0, 56.0), egui::vec2(rect.width() - 16.0, 22.0));
                ui.scope_builder(egui::UiBuilder::new().max_rect(r1), |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Min");
                        if ui.add(egui::DragValue::new(&mut node.param_a).speed(0.05).range(-1000.0..=1000.0)).changed() {
                            graph_dirty = true;
                        }
                    });
                });
                ui.scope_builder(egui::UiBuilder::new().max_rect(r2), |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Max");
                        if ui.add(egui::DragValue::new(&mut node.param_b).speed(0.05).range(-1000.0..=1000.0)).changed() {
                            graph_dirty = true;
                        }
                    });
                });
            }
            if node.kind == FiosNodeKind::Deadzone {
                let r1 = egui::Rect::from_min_size(rect.left_top() + egui::vec2(8.0, 34.0), egui::vec2(rect.width() - 16.0, 24.0));
                ui.scope_builder(egui::UiBuilder::new().max_rect(r1), |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Dz");
                        if ui.add(egui::DragValue::new(&mut node.param_a).speed(0.01).range(0.0..=1.0)).changed() {
                            graph_dirty = true;
                        }
                    });
                });
            }
            if node.kind == FiosNodeKind::Smooth {
                let r1 = egui::Rect::from_min_size(rect.left_top() + egui::vec2(8.0, 34.0), egui::vec2(rect.width() - 16.0, 24.0));
                ui.scope_builder(egui::UiBuilder::new().max_rect(r1), |ui| {
                    ui.horizontal(|ui| {
                        ui.label("A");
                        if ui.add(egui::DragValue::new(&mut node.param_a).speed(0.01).range(0.0..=1.0)).changed() {
                            graph_dirty = true;
                        }
                    });
                });
            }

            if node.kind == FiosNodeKind::OutputMove {
                painter.text(rect.left_top() + egui::vec2(8.0, 36.0), egui::Align2::LEFT_TOP, format!("X: {:.2}  Y: {:.2}", self.last_axis[0], self.last_axis[1]), egui::FontId::monospace(11.0), egui::Color32::from_gray(190));
            }
            if node.kind == FiosNodeKind::OutputLook {
                painter.text(rect.left_top() + egui::vec2(8.0, 36.0), egui::Align2::LEFT_TOP, format!("Yaw: {:.2}  Pitch: {:.2}", self.last_look[0], self.last_look[1]), egui::FontId::monospace(11.0), egui::Color32::from_gray(190));
            }
            if node.kind == FiosNodeKind::OutputAction {
                painter.text(rect.left_top() + egui::vec2(8.0, 32.0), egui::Align2::LEFT_TOP, format!("A: {:.2}", self.last_action), egui::FontId::monospace(11.0), egui::Color32::from_gray(190));
            }

            for i in 0..node.kind.input_count() {
                let p = Self::input_port_pos(rect, node.kind, i);
                painter.circle_filled(p, 4.0, egui::Color32::from_rgb(205, 120, 120));
                painter.text(p + egui::vec2(8.0, -6.0), egui::Align2::LEFT_TOP, node.kind.input_name(i), egui::FontId::proportional(10.0), egui::Color32::from_gray(170));
                let r = egui::Rect::from_center_size(p, egui::vec2(24.0, 24.0));
                let resp = ui.interact(r, ui.id().with(("fios_in_port", node.id, i)), egui::Sense::click());
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
                painter.text(p + egui::vec2(-8.0, -6.0), egui::Align2::RIGHT_TOP, node.kind.output_name(i), egui::FontId::proportional(10.0), egui::Color32::from_gray(170));
                let r = egui::Rect::from_center_size(p, egui::vec2(24.0, 24.0));
                let resp = ui.interact(r, ui.id().with(("fios_out_port", node.id, i)), egui::Sense::click_and_drag());
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
                    let from = Self::output_port_pos(*from_rect, self.nodes[fi].kind, from_port as usize);
                    let mouse = ui.ctx().input(|i| i.pointer.hover_pos()).unwrap_or(from + egui::vec2(80.0, 0.0));
                    let mut predicted_input: Option<(u32, u8, f32, egui::Pos2)> = None;
                    if let Some(target_node) = hovered_node {
                        if let Some(ti) = self.node_index_by_id(target_node) {
                            let target_kind = self.nodes[ti].kind;
                            if target_kind.input_count() > 0 {
                                if let Some(target_rect) = rect_by_id.get(&target_node) {
                                    for input_idx in 0..target_kind.input_count() {
                                        let p = Self::input_port_pos(*target_rect, target_kind, input_idx);
                                        let d2 = (p - mouse).length_sq();
                                        match predicted_input {
                                            Some((_, _, bd2, _)) if d2 >= bd2 => {}
                                            _ => {
                                                predicted_input = Some((target_node, input_idx as u8, d2, p));
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
                            let Some(rect) = rect_by_id.get(&node.id) else { continue; };
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
                        painter.line_segment([from, mouse], egui::Stroke::new(2.0, egui::Color32::from_rgb(15, 232, 121)));
                    }
                    if let Some((_, _, _, predicted_pos)) = predicted_input {
                        painter.circle_stroke(
                            predicted_pos,
                            7.0,
                            egui::Stroke::new(1.5, egui::Color32::from_rgb(15, 232, 121)),
                        );
                        painter.line_segment(
                            [mouse, predicted_pos],
                            egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(15, 232, 121, 130)),
                        );
                    }
                }
            }
            let connect_drag_down = ui.ctx().input(|i| {
                i.pointer.primary_down() || (i.modifiers.alt && i.pointer.secondary_down())
            });
            if !connect_drag_down {
                let release_pos = ui.ctx().input(|i| i.pointer.hover_pos()).or_else(|| self.wire_drag_path.last().copied());
                if let Some(release_pos) = release_pos {
                    let mut best: Option<(u32, u8, f32)> = None;
                    if let Some(target_node) = hovered_node {
                        if let Some(ti) = self.node_index_by_id(target_node) {
                            let target_kind = self.nodes[ti].kind;
                            if target_kind.input_count() > 0 {
                                if let Some(target_rect) = rect_by_id.get(&target_node) {
                                    for input_idx in 0..target_kind.input_count() {
                                        let p = Self::input_port_pos(*target_rect, target_kind, input_idx);
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
                            let Some(rect) = rect_by_id.get(&node.id) else { continue; };
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
            painter.rect_filled(r, 0.0, egui::Color32::from_rgba_unmultiplied(86, 148, 255, 24));
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

        let delete_pressed = ui.ctx().input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace));
        if delete_pressed && self.remove_selected_nodes() {
            graph_dirty = true;
        }
        if graph_dirty {
            let _ = self.save_graph_to_disk();
        }
    }

    fn draw_controls_tab(&mut self, ui: &mut egui::Ui, lang: EngineLanguage) {
        let card_fill = egui::Color32::from_rgb(29, 32, 34);
        let card_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(60, 66, 70));
        let accent = egui::Color32::from_rgb(15, 232, 121);
        let warn = egui::Color32::from_rgb(220, 130, 80);
        let section_title = |ui: &mut egui::Ui, text: &str| {
            ui.label(egui::RichText::new(text).strong().color(egui::Color32::from_gray(225)));
            ui.add_space(4.0);
        };

        let controls_title = match lang {
            EngineLanguage::Pt => "Controles",
            EngineLanguage::En => "Controls",
            EngineLanguage::Es => "Controles",
        };
        let runtime_title = match lang {
            EngineLanguage::Pt => "Runtime",
            EngineLanguage::En => "Runtime",
            EngineLanguage::Es => "Runtime",
        };
        let action_header = match lang {
            EngineLanguage::Pt => "Acao",
            EngineLanguage::En => "Action",
            EngineLanguage::Es => "Accion",
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
        let controls_enabled_txt = match lang {
            EngineLanguage::Pt => "Ativar Controles",
            EngineLanguage::En => "Enable Controls",
            EngineLanguage::Es => "Activar Controles",
        };
        let lua_toggle_txt = match lang {
            EngineLanguage::Pt => "Ativar Lua",
            EngineLanguage::En => "Enable Lua",
            EngineLanguage::Es => "Activar Lua",
        };
        let save_txt = match lang {
            EngineLanguage::Pt => "Salvar",
            EngineLanguage::En => "Save",
            EngineLanguage::Es => "Guardar",
        };
        let restore_txt = match lang {
            EngineLanguage::Pt => "Restaurar Padrao",
            EngineLanguage::En => "Restore Defaults",
            EngineLanguage::Es => "Restaurar Pred.",
        };
        let lua_tools_title = match lang {
            EngineLanguage::Pt => "Ferramentas Lua",
            EngineLanguage::En => "Lua Tools",
            EngineLanguage::Es => "Herramientas Lua",
        };

        egui::Frame::new()
            .fill(card_fill)
            .stroke(card_stroke)
            .corner_radius(8)
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                section_title(ui, runtime_title);
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .checkbox(&mut self.controls_enabled, controls_enabled_txt)
                        .changed()
                    {
                        let _ = self.save_to_disk();
                    }
                    if ui.checkbox(&mut self.lua_enabled, lua_toggle_txt).changed() {
                        let _ = self.save_to_disk();
                    }

                    let controls_state = if self.controls_enabled {
                        match lang {
                            EngineLanguage::Pt => "Controles ON",
                            EngineLanguage::En => "Controls ON",
                            EngineLanguage::Es => "Controles ON",
                        }
                    } else {
                        match lang {
                            EngineLanguage::Pt => "Controles OFF",
                            EngineLanguage::En => "Controls OFF",
                            EngineLanguage::Es => "Controles OFF",
                        }
                    };
                    ui.label(
                        egui::RichText::new(controls_state).color(if self.controls_enabled {
                            accent
                        } else {
                            warn
                        }),
                    );
                });
                let lua_status_txt = self.lua_status.clone().unwrap_or_else(|| match lang {
                    EngineLanguage::Pt => "Lua inativo".to_string(),
                    EngineLanguage::En => "Lua inactive".to_string(),
                    EngineLanguage::Es => "Lua inactivo".to_string(),
                });
                let [mx, my] = self.last_axis;
                let [lx, ly] = self.last_look;
                let a = self.last_action;
                ui.add_space(2.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!("Lua: {lua_status_txt}"));
                    ui.separator();
                    ui.label(format!("Move Axis: X={mx:.2} Y={my:.2}"));
                    ui.separator();
                    ui.label(format!("Look: Yaw={lx:.2} Pitch={ly:.2}"));
                    ui.separator();
                    ui.label(format!("Action={a:.2}"));
                    if let Some(status) = &self.status {
                        ui.separator();
                        ui.label(egui::RichText::new(status).color(egui::Color32::from_gray(190)));
                    }
                });
            });

        ui.add_space(8.0);
        egui::Frame::new()
            .fill(card_fill)
            .stroke(card_stroke)
            .corner_radius(8)
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                section_title(ui, controls_title);
                egui::Grid::new("fios_bind_grid")
                    .num_columns(3)
                    .spacing([10.0, 8.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.strong(action_header);
                        ui.strong(key_header);
                        ui.strong(state_header);
                        ui.end_row();

                        for (i, action) in FiosAction::ALL.iter().enumerate() {
                            ui.label(action.label(lang));
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
                            if ui
                                .add_sized([140.0, 24.0], egui::Button::new(key_text))
                                .clicked()
                            {
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
                            let state = if self.pressed[i] { "ON" } else { "OFF" };
                            ui.colored_label(
                                if self.pressed[i] {
                                    accent
                                } else {
                                    egui::Color32::from_gray(150)
                                },
                                state,
                            );
                            ui.end_row();
                        }
                    });

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(restore_txt).clicked() {
                        self.bindings = Self::default_bindings();
                        self.status = match self.save_to_disk() {
                            Ok(()) => Some(
                                match lang {
                                    EngineLanguage::Pt => "Padrao restaurado",
                                    EngineLanguage::En => "Defaults restored",
                                    EngineLanguage::Es => "Pred. restaurado",
                                }
                                .to_string(),
                            ),
                            Err(err) => Some(format!("Falha ao salvar: {err}")),
                        };
                    }
                    if ui.button(save_txt).clicked() {
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

        ui.add_space(8.0);
        egui::Frame::new()
            .fill(card_fill)
            .stroke(card_stroke)
            .corner_radius(8)
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                section_title(ui, lua_tools_title);
                ui.horizontal_wrapped(|ui| {
                    if ui.button(match lang {
                        EngineLanguage::Pt => "Salvar Script Lua",
                        EngineLanguage::En => "Save Lua Script",
                        EngineLanguage::Es => "Guardar Script Lua",
                    }).clicked() {
                        self.lua_dirty = true;
                        self.lua_fn_key = None;
                        self.lua_status = match self.save_lua_script_to_disk() {
                            Ok(()) => Some(match lang {
                                EngineLanguage::Pt => "Script Lua salvo",
                                EngineLanguage::En => "Lua script saved",
                                EngineLanguage::Es => "Script Lua guardado",
                            }.to_string()),
                            Err(err) => Some(format!("Lua save failed: {err}")),
                        };
                    }
                    if ui.button(match lang {
                        EngineLanguage::Pt => "Criar Arquivo .lua",
                        EngineLanguage::En => "Create .lua File",
                        EngineLanguage::Es => "Crear Archivo .lua",
                    }).clicked() {
                        if let Some(path) = FileDialog::new()
                            .add_filter("Lua", &["lua"])
                            .set_file_name("fios_controller.lua")
                            .save_file()
                        {
                            self.lua_status = match fs::write(&path, &self.lua_script) {
                                Ok(()) => Some(format!("Lua file created: {}", path.display())),
                                Err(err) => Some(format!("Lua file create failed: {err}")),
                            };
                        }
                    }
                    if ui.button(match lang {
                        EngineLanguage::Pt => "Abrir Arquivo .lua",
                        EngineLanguage::En => "Open .lua File",
                        EngineLanguage::Es => "Abrir Archivo .lua",
                    }).clicked() {
                        if let Some(path) = FileDialog::new().add_filter("Lua", &["lua"]).pick_file() {
                            self.lua_status = match fs::read_to_string(&path) {
                                Ok(raw) => {
                                    self.lua_script = raw;
                                    self.lua_dirty = true;
                                    self.lua_fn_key = None;
                                    Some(format!("Lua file loaded: {}", path.display()))
                                }
                                Err(err) => Some(format!("Lua file open failed: {err}")),
                            };
                        }
                    }
                });
                ui.add_space(4.0);
                ui.label(match lang {
                    EngineLanguage::Pt => "Script recebe (x, y, dt) e deve retornar {x=..., y=...} ou x, y",
                    EngineLanguage::En => "Script receives (x, y, dt) and should return {x=..., y=...} or x, y",
                    EngineLanguage::Es => "El script recibe (x, y, dt) y debe devolver {x=..., y=...} o x, y",
                });
                if ui
                    .add_sized(
                        [ui.available_width(), 150.0],
                        egui::TextEdit::multiline(&mut self.lua_script)
                            .font(egui::TextStyle::Monospace),
                    )
                    .changed()
                {
                    self.lua_dirty = true;
                    self.lua_fn_key = None;
                }
            });
    }

    pub fn draw_window(&mut self, ctx: &egui::Context, open: &mut bool, lang: EngineLanguage) {
        if !*open {
            return;
        }
        let viewport_id = egui::ViewportId::from_hash_of("dengine_fios_viewport");
        let mut close_req = false;
        ctx.show_viewport_immediate(
            viewport_id,
            egui::ViewportBuilder::default()
                .with_title("Fios")
                .with_inner_size([860.0, 560.0])
                .with_min_inner_size([680.0, 420.0])
                .with_resizable(true)
                .with_decorations(true),
            |ctx, _class| {
                if ctx.input(|i| i.viewport().close_requested()) {
                    close_req = true;
                    return;
                }
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        let controls_txt = match lang {
                            EngineLanguage::Pt => "Controles",
                            EngineLanguage::En => "Controls",
                            EngineLanguage::Es => "Controles",
                        };
                        let graph_txt = match lang {
                            EngineLanguage::Pt => "Grafo",
                            EngineLanguage::En => "Graph",
                            EngineLanguage::Es => "Grafo",
                        };
                        let c = self.tab == FiosTab::Controls;
                        let g = self.tab == FiosTab::Graph;
                        if ui.add(egui::Button::new(controls_txt).fill(if c { egui::Color32::from_rgb(58, 84, 64) } else { egui::Color32::from_rgb(52, 52, 52) })).clicked() {
                            self.tab = FiosTab::Controls;
                        }
                        if ui.add(egui::Button::new(graph_txt).fill(if g { egui::Color32::from_rgb(58, 84, 64) } else { egui::Color32::from_rgb(52, 52, 52) })).clicked() {
                            self.tab = FiosTab::Graph;
                        }
                    });
                    ui.separator();
                    match self.tab {
                        FiosTab::Controls => self.draw_controls_tab(ui, lang),
                        FiosTab::Graph => self.draw_graph(ui, lang),
                    }
                });
            },
        );
        if close_req {
            *open = false;
            ctx.send_viewport_cmd_to(viewport_id, egui::ViewportCommand::Close);
        }
    }
}
