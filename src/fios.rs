use crate::EngineLanguage;
use eframe::egui;
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
    Constant,
    Add,
    Multiply,
    Clamp,
    Deadzone,
    Invert,
    Smooth,
    OutputMove,
}

impl FiosNodeKind {
    fn id(self) -> &'static str {
        match self {
            Self::InputAxis => "input_axis",
            Self::Constant => "constant",
            Self::Add => "add",
            Self::Multiply => "multiply",
            Self::Clamp => "clamp",
            Self::Deadzone => "deadzone",
            Self::Invert => "invert",
            Self::Smooth => "smooth",
            Self::OutputMove => "output_move",
        }
    }

    fn from_id(id: &str) -> Option<Self> {
        Some(match id {
            "input_axis" => Self::InputAxis,
            "constant" => Self::Constant,
            "add" => Self::Add,
            "multiply" => Self::Multiply,
            "clamp" => Self::Clamp,
            "deadzone" => Self::Deadzone,
            "invert" => Self::Invert,
            "smooth" => Self::Smooth,
            "output_move" => Self::OutputMove,
            _ => return None,
        })
    }

    fn input_count(self) -> usize {
        match self {
            Self::InputAxis => 0,
            Self::Constant => 0,
            Self::Add => 2,
            Self::Multiply => 2,
            Self::Clamp => 1,
            Self::Deadzone => 1,
            Self::Invert => 1,
            Self::Smooth => 1,
            Self::OutputMove => 2,
        }
    }

    fn output_count(self) -> usize {
        match self {
            Self::InputAxis => 2,
            Self::Constant => 1,
            Self::Add => 1,
            Self::Multiply => 1,
            Self::Clamp => 1,
            Self::Deadzone => 1,
            Self::Invert => 1,
            Self::Smooth => 1,
            Self::OutputMove => 0,
        }
    }

    fn input_name(self, idx: usize) -> &'static str {
        match (self, idx) {
            (Self::Add, 0) | (Self::Multiply, 0) => "A",
            (Self::Add, 1) | (Self::Multiply, 1) => "B",
            (Self::Clamp, 0) | (Self::Deadzone, 0) | (Self::Invert, 0) | (Self::Smooth, 0) => "In",
            (Self::OutputMove, 0) => "X",
            (Self::OutputMove, 1) => "Y",
            _ => "",
        }
    }

    fn output_name(self, idx: usize) -> &'static str {
        match (self, idx) {
            (Self::InputAxis, 0) => "X",
            (Self::InputAxis, 1) => "Y",
            (Self::Constant, 0)
            | (Self::Add, 0)
            | (Self::Multiply, 0)
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

pub struct FiosState {
    bindings: [egui::Key; ACTION_COUNT],
    pressed: [bool; ACTION_COUNT],
    just_pressed: [bool; ACTION_COUNT],
    capture_index: Option<usize>,
    status: Option<String>,
    tab: FiosTab,
    nodes: Vec<FiosNode>,
    links: Vec<FiosLink>,
    next_node_id: u32,
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
    last_axis: [f32; 2],
}

impl FiosState {
    fn default_node_name(kind: FiosNodeKind) -> &'static str {
        match kind {
            FiosNodeKind::InputAxis => "Input Axis",
            FiosNodeKind::Constant => "Constant",
            FiosNodeKind::Add => "Add",
            FiosNodeKind::Multiply => "Multiply",
            FiosNodeKind::Clamp => "Clamp",
            FiosNodeKind::Deadzone => "Deadzone",
            FiosNodeKind::Invert => "Invert",
            FiosNodeKind::Smooth => "Smooth",
            FiosNodeKind::OutputMove => "Output Move",
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
        let mut out = Self {
            bindings: Self::default_bindings(),
            pressed: [false; ACTION_COUNT],
            just_pressed: [false; ACTION_COUNT],
            capture_index: None,
            status: None,
            tab: FiosTab::Controls,
            nodes: Vec::new(),
            links: Vec::new(),
            next_node_id: 1,
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
            last_axis: [0.0, 0.0],
        };
        out.load_from_disk();
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

    fn config_path() -> PathBuf {
        PathBuf::from(".dengine_fios_controls.cfg")
    }

    fn graph_path() -> PathBuf {
        PathBuf::from(".dengine_fios_graph.cfg")
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
            let Some(key) = Self::key_from_string(key_name) else {
                continue;
            };
            if let Some(idx) = FiosAction::ALL.iter().position(|a| a.id() == action_id.trim()) {
                self.bindings[idx] = key;
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
        fs::write(Self::graph_path(), out).map_err(|e| e.to_string())
    }

    fn load_graph_from_disk(&mut self) -> bool {
        let Ok(raw) = fs::read_to_string(Self::graph_path()) else {
            return false;
        };
        let mut parsed_nodes = Vec::<FiosNode>::new();
        let mut parsed_links = Vec::<FiosLink>::new();
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
                _ => {}
            }
        }
        if parsed_nodes.is_empty() {
            return false;
        }
        self.nodes = parsed_nodes;
        self.links = parsed_links;
        self.next_node_id = next_node_id.max(
            self.nodes
                .iter()
                .map(|n| n.id)
                .max()
                .unwrap_or(0)
                .saturating_add(1)
                .max(1),
        );
        self.selected_node = None;
        self.selected_nodes.clear();
        self.rename_node = None;
        self.rename_buffer.clear();
        self.smooth_state.clear();
        true
    }

    pub fn update_input(&mut self, ctx: &egui::Context) {
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
        self.last_axis = self.evaluate_graph_axis(base);
    }

    fn raw_movement_axis(&self) -> [f32; 2] {
        let x = (self.pressed[3] as i32 - self.pressed[2] as i32) as f32;
        let y = (self.pressed[0] as i32 - self.pressed[1] as i32) as f32;
        [x, y]
    }

    pub fn movement_axis(&self) -> [f32; 2] {
        self.last_axis
    }

    fn node_index_by_id(&self, id: u32) -> Option<usize> {
        self.nodes.iter().position(|n| n.id == id)
    }

    fn evaluate_graph_axis(&mut self, base_axis: [f32; 2]) -> [f32; 2] {
        let output = self.nodes.iter().find(|n| n.kind == FiosNodeKind::OutputMove).map(|n| n.id);
        let Some(out_id) = output else {
            return base_axis;
        };
        let mut cache = HashMap::<(u32, u8), f32>::new();
        let mut stack = HashSet::<(u32, u8)>::new();
        let nodes = &self.nodes;
        let links = &self.links;
        let smooth = &mut self.smooth_state;
        let x = Self::eval_input_of_node(nodes, links, smooth, out_id, 0, base_axis[0], base_axis, &mut cache, &mut stack);
        let y = Self::eval_input_of_node(nodes, links, smooth, out_id, 1, base_axis[1], base_axis, &mut cache, &mut stack);
        [x.clamp(-1000.0, 1000.0), y.clamp(-1000.0, 1000.0)]
    }

    fn node_index_by_id_in(nodes: &[FiosNode], id: u32) -> Option<usize> {
        nodes.iter().position(|n| n.id == id)
    }

    fn eval_input_of_node(
        nodes: &[FiosNode],
        links: &[FiosLink],
        smooth_state: &mut HashMap<(u32, u8), f32>,
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
                FiosNodeKind::Constant => node.value,
                FiosNodeKind::Add => {
                    let a = Self::eval_input_of_node(nodes, links, smooth_state, node_id, 0, 0.0, base_axis, cache, stack);
                    let b = Self::eval_input_of_node(nodes, links, smooth_state, node_id, 1, 0.0, base_axis, cache, stack);
                    a + b
                }
                FiosNodeKind::Multiply => {
                    let a = Self::eval_input_of_node(nodes, links, smooth_state, node_id, 0, 0.0, base_axis, cache, stack);
                    let b = Self::eval_input_of_node(nodes, links, smooth_state, node_id, 1, 0.0, base_axis, cache, stack);
                    a * b
                }
                FiosNodeKind::Clamp => {
                    let v = Self::eval_input_of_node(nodes, links, smooth_state, node_id, 0, 0.0, base_axis, cache, stack);
                    v.clamp(node.param_a.min(node.param_b), node.param_a.max(node.param_b))
                }
                FiosNodeKind::Deadzone => {
                    let v = Self::eval_input_of_node(nodes, links, smooth_state, node_id, 0, 0.0, base_axis, cache, stack);
                    let t = node.param_a.abs().clamp(0.0, 1.0);
                    if v.abs() < t { 0.0 } else { v }
                }
                FiosNodeKind::Invert => {
                    -Self::eval_input_of_node(nodes, links, smooth_state, node_id, 0, 0.0, base_axis, cache, stack)
                }
                FiosNodeKind::Smooth => {
                    let target = Self::eval_input_of_node(nodes, links, smooth_state, node_id, 0, 0.0, base_axis, cache, stack);
                    let alpha = node.param_a.clamp(0.0, 1.0);
                    let prev = *smooth_state.get(&key).unwrap_or(&target);
                    let v = prev + (target - prev) * alpha;
                    smooth_state.insert(key, v);
                    v
                }
                FiosNodeKind::OutputMove => 0.0,
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
            FiosNodeKind::Constant => egui::vec2(170.0, 88.0),
            FiosNodeKind::Add | FiosNodeKind::Multiply => egui::vec2(170.0, 84.0),
            FiosNodeKind::Clamp | FiosNodeKind::Deadzone | FiosNodeKind::Invert | FiosNodeKind::Smooth => egui::vec2(180.0, 94.0),
            FiosNodeKind::OutputMove => egui::vec2(190.0, 88.0),
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
        self.drag_from_output = None;
        self.rename_node = None;
        self.rename_buffer.clear();
        self.selected_nodes.clear();
        self.selected_node = None;
        self.smooth_state.clear();
        true
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
            const_txt,
            add_txt,
            mul_txt,
            clamp_txt,
            deadzone_txt,
            invert_txt,
            smooth_txt,
            output_move_txt,
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
                "Constante",
                "Somar",
                "Multiplicar",
                "Limitar",
                "Zona Morta",
                "Inverter",
                "Suavizar",
                "Saida Mover",
                "Selecionado(s)",
                "Nenhum",
                "Renomear",
                "Aplicar Nome",
                "Shift: multi-selecao | Arraste no vazio: caixa | Arraste do output e solte em qualquer lugar para auto-conectar | Alt + botao direito: cortar fio",
                "Add Bloco",
                "Acoes",
                "Atalhos",
                "Excluir Selecionado",
            ),
            EngineLanguage::En => (
                "Input Axis",
                "Constant",
                "Add",
                "Multiply",
                "Clamp",
                "Deadzone",
                "Invert",
                "Smooth",
                "Output Move",
                "Selected",
                "None",
                "Rename",
                "Apply Name",
                "Shift: multi-select | Drag empty: marquee | Drag from output and release anywhere to auto-connect | Alt + right mouse: cut wire",
                "Add Block",
                "Actions",
                "Shortcuts",
                "Delete Selected",
            ),
            EngineLanguage::Es => (
                "Entrada Eje",
                "Constante",
                "Sumar",
                "Multiplicar",
                "Limitar",
                "Zona Muerta",
                "Invertir",
                "Suavizar",
                "Salida Mover",
                "Seleccionado(s)",
                "Ninguno",
                "Renombrar",
                "Aplicar Nombre",
                "Shift: multi-seleccion | Arrastrar vacio: caja | Arrastrar desde salida y soltar en cualquier lugar para auto-conectar | Alt + boton derecho: cortar cable",
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
                        if ui.button(const_txt).clicked() {
                            self.add_node(FiosNodeKind::Constant);
                            ui.close();
                        }
                        if ui.button(add_txt).clicked() {
                            self.add_node(FiosNodeKind::Add);
                            ui.close();
                        }
                        if ui.button(mul_txt).clicked() {
                            self.add_node(FiosNodeKind::Multiply);
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
        let shift = ui.ctx().input(|i| i.modifiers.shift);
        let alt = ui.ctx().input(|i| i.modifiers.alt);
        let primary_pressed = ui.ctx().input(|i| i.pointer.primary_pressed());
        let primary_down = ui.ctx().input(|i| i.pointer.primary_down());
        let primary_released = ui.ctx().input(|i| i.pointer.primary_released());
        let secondary_pressed = ui.ctx().input(|i| i.pointer.secondary_pressed());
        let secondary_down = ui.ctx().input(|i| i.pointer.secondary_down());
        let secondary_released = ui.ctx().input(|i| i.pointer.secondary_released());
        let hovered_node = pointer_pos.and_then(|p| {
            rect_by_id
                .iter()
                .find_map(|(id, r)| if r.contains(p) { Some(*id) } else { None })
        });
        if canvas_resp.clicked() && hovered_node.is_none() && !shift {
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
                if !shift {
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

        let mut link_curves: Vec<(usize, Vec<egui::Pos2>)> = Vec::new();
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

        if alt && secondary_pressed {
            self.cut_points.clear();
            if let Some(p) = pointer_pos {
                self.cut_points.push(p);
            }
        }
        if alt && secondary_down && !self.cut_points.is_empty() {
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
        let mut pending_group_drag_delta: Option<egui::Vec2> = None;
        for node in &mut self.nodes {
            let rect = egui::Rect::from_min_size(canvas_rect.min + node.pos, Self::node_size(node.kind));
            let id = ui.id().with(("fios_node_drag", node.id));
            let drag_resp = ui.interact(rect, id, egui::Sense::click_and_drag());
            if drag_resp.clicked() {
                if shift {
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
                let resp = ui.interact(r, ui.id().with(("fios_out_port", node.id, i)), egui::Sense::click());
                if resp.clicked() {
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
            graph_dirty = false;
        }

        if let Some((from_node, from_port)) = self.drag_from_output {
            if let Some(fi) = self.node_index_by_id(from_node) {
                if let Some(from_rect) = rect_by_id.get(&from_node) {
                    let from = Self::output_port_pos(*from_rect, self.nodes[fi].kind, from_port as usize);
                    let mouse = ui.ctx().input(|i| i.pointer.hover_pos()).unwrap_or(from + egui::vec2(80.0, 0.0));
                    if ui.ctx().input(|i| i.pointer.primary_down()) {
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
                }
            }
            if !ui.ctx().input(|i| i.pointer.primary_down()) {
                let release_pos = ui.ctx().input(|i| i.pointer.hover_pos()).or_else(|| self.wire_drag_path.last().copied());
                if let Some(release_pos) = release_pos {
                    let mut best: Option<(u32, u8, f32)> = None;
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
                    if let Some((to_node, to_port, _)) = best {
                        self.create_link(from_node, from_port, to_node, to_port);
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
        if let Some(status) = &self.status {
            ui.label(status);
        }
        let [mx, my] = self.last_axis;
        ui.label(format!("Move Axis: X={mx:.2} Y={my:.2}"));
        ui.separator();

        egui::Grid::new("fios_bind_grid")
            .num_columns(3)
            .spacing([10.0, 8.0])
            .striped(true)
            .show(ui, |ui| {
                ui.strong(match lang {
                    EngineLanguage::Pt => "Acao",
                    EngineLanguage::En => "Action",
                    EngineLanguage::Es => "Accion",
                });
                ui.strong("Tecla");
                ui.strong("Estado");
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
                    if ui.add_sized([130.0, 24.0], egui::Button::new(key_text)).clicked() {
                        self.capture_index = Some(i);
                        self.status = Some(match lang {
                            EngineLanguage::Pt => "Aguardando tecla...",
                            EngineLanguage::En => "Waiting for key...",
                            EngineLanguage::Es => "Esperando tecla...",
                        }.to_string());
                    }
                    let state = if self.pressed[i] { "ON" } else { "OFF" };
                    ui.colored_label(if self.pressed[i] { egui::Color32::from_rgb(15, 232, 121) } else { egui::Color32::from_gray(150) }, state);
                    ui.end_row();
                }
            });

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui.button(match lang {
                EngineLanguage::Pt => "Restaurar Padrao",
                EngineLanguage::En => "Restore Defaults",
                EngineLanguage::Es => "Restaurar Pred.",
            }).clicked() {
                self.bindings = Self::default_bindings();
                self.status = match self.save_to_disk() {
                    Ok(()) => Some(match lang {
                        EngineLanguage::Pt => "Padrao restaurado",
                        EngineLanguage::En => "Defaults restored",
                        EngineLanguage::Es => "Pred. restaurado",
                    }.to_string()),
                    Err(err) => Some(format!("Falha ao salvar: {err}")),
                };
            }
            if ui.button(match lang {
                EngineLanguage::Pt => "Salvar",
                EngineLanguage::En => "Save",
                EngineLanguage::Es => "Guardar",
            }).clicked() {
                self.status = match self.save_to_disk() {
                    Ok(()) => Some(match lang {
                        EngineLanguage::Pt => "Controles salvos",
                        EngineLanguage::En => "Controls saved",
                        EngineLanguage::Es => "Controles guardados",
                    }.to_string()),
                    Err(err) => Some(format!("Falha ao salvar: {err}")),
                };
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
