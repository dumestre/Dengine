use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum TerminalCliModel {
    Qwen,
    Gemini,
    Codex,
}

impl TerminalCliModel {
    fn label(self) -> &'static str {
        match self {
            TerminalCliModel::Qwen => "Qwen CLI",
            TerminalCliModel::Gemini => "Gemini CLI",
            TerminalCliModel::Codex => "Codex CLI",
        }
    }

    fn exe_name(self) -> &'static str {
        match self {
            TerminalCliModel::Qwen => "qwen",
            TerminalCliModel::Gemini => "gemini",
            TerminalCliModel::Codex => "codex",
        }
    }

    fn npm_package(self) -> &'static str {
        match self {
            TerminalCliModel::Qwen => "@qwen-code/qwen-code",
            TerminalCliModel::Gemini => "@google/gemini-cli",
            TerminalCliModel::Codex => "@openai/codex",
        }
    }
}

struct TerminalProvisionResult {
    ok: bool,
    message: String,
    model: Option<TerminalCliModel>,
}

struct EmbeddedTerminalSession {
    child: Box<dyn portable_pty::Child + Send>,
    master: Box<dyn portable_pty::MasterPty + Send>,
    writer: Box<dyn Write + Send>,
}

pub(crate) struct TerminAiState {
    pub(crate) terminal_enabled: bool,
    terminal_selected_model: Option<TerminalCliModel>,
    terminal_status: Option<String>,
    terminal_busy: bool,
    terminal_job_rx: Option<Receiver<TerminalProvisionResult>>,
    terminal_output_rx: Option<Receiver<Vec<u8>>>,
    terminal_output: String,
    terminal_transcript: String,
    terminal_parser: Option<Parser>,
    terminal_cols: u16,
    terminal_rows: u16,
    terminal_input: String,
    terminal_session: Option<EmbeddedTerminalSession>,
}

impl TerminAiState {
    pub(crate) fn new() -> Self {
        Self {
            terminal_enabled: false,
            terminal_selected_model: None,
            terminal_status: None,
            terminal_busy: false,
            terminal_job_rx: None,
            terminal_output_rx: None,
            terminal_output: String::new(),
            terminal_transcript: String::new(),
            terminal_parser: None,
            terminal_cols: 120,
            terminal_rows: 34,
            terminal_input: String::new(),
            terminal_session: None,
        }
    }
}

impl EditorApp {
    pub(crate) fn poll_terminal_job(&mut self) {
        let Some(rx) = self.terminai.terminal_job_rx.take() else {
            return;
        };
        match rx.try_recv() {
            Ok(result) => {
                self.terminai.terminal_busy = false;
                self.terminai.terminal_status = Some(result.message);
                if !result.ok {
                    self.terminai.terminal_selected_model = None;
                } else if let Some(model) = result.model {
                    if let Err(err) = self.start_embedded_cli_session(model) {
                        self.terminai.terminal_status = Some(err);
                    }
                }
            }
            Err(TryRecvError::Empty) => {
                self.terminai.terminal_job_rx = Some(rx);
            }
            Err(TryRecvError::Disconnected) => {
                self.terminai.terminal_busy = false;
                self.terminai.terminal_status =
                    Some("Falha ao iniciar tarefa do terminal".to_string());
                self.terminai.terminal_selected_model = None;
            }
        }
    }

    fn poll_terminal_output(&mut self) {
        let Some(rx) = self.terminai.terminal_output_rx.take() else {
            return;
        };
        let mut keep_rx = true;
        loop {
            match rx.try_recv() {
                Ok(chunk) => {
                    self.terminai
                        .terminal_transcript
                        .push_str(&String::from_utf8_lossy(&chunk));
                    if self.terminai.terminal_transcript.len() > 800_000 {
                        let cut = self
                            .terminai
                            .terminal_transcript
                            .len()
                            .saturating_sub(700_000);
                        self.terminai.terminal_transcript.drain(..cut);
                    }
                    if chunk.windows(4).any(|w| w == b"\x1b[6n")
                        || chunk.windows(5).any(|w| w == b"\x1b[?6n")
                    {
                        if let Some(session) = self.terminai.terminal_session.as_mut() {
                            let _ = session.writer.write_all(b"\x1b[1;1R");
                            let _ = session.writer.flush();
                        }
                    }
                    if let Some(parser) = self.terminai.terminal_parser.as_mut() {
                        parser.process(&chunk);
                        self.terminai.terminal_output = parser.screen().contents().to_string();
                    } else {
                        self.terminai
                            .terminal_output
                            .push_str(&String::from_utf8_lossy(&chunk));
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    keep_rx = false;
                    self.terminai.terminal_status =
                        Some("Sessão de terminal finalizada".to_string());
                    break;
                }
            }
        }
        if keep_rx {
            self.terminai.terminal_output_rx = Some(rx);
        }
    }

    fn is_cli_installed(exe: &str) -> bool {
        #[cfg(target_os = "windows")]
        {
            Command::new("where")
                .arg(exe)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        }
        #[cfg(not(target_os = "windows"))]
        {
            Command::new("which")
                .arg(exe)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        }
    }

    fn node_tooling_ready() -> Result<(), String> {
        let has_node = Self::is_cli_installed("node");
        let has_npm = Self::is_cli_installed("npm");
        if has_node && has_npm {
            return Ok(());
        }
        Self::try_install_node_tooling()
    }

    fn try_install_node_tooling() -> Result<(), String> {
        #[cfg(target_os = "windows")]
        {
            let winget_ok = Command::new("winget")
                .args([
                    "install",
                    "-e",
                    "--id",
                    "OpenJS.NodeJS.LTS",
                    "--accept-package-agreements",
                    "--accept-source-agreements",
                ])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
            if winget_ok && Self::is_cli_installed("node") && Self::is_cli_installed("npm") {
                return Ok(());
            }

            let choco_ok = Command::new("choco")
                .args(["install", "nodejs-lts", "-y"])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
            if choco_ok && Self::is_cli_installed("node") && Self::is_cli_installed("npm") {
                return Ok(());
            }

            Err("Node.js/npm não encontrados e falhou a instalação automática. Instale Node LTS e reabra a engine.".to_string())
        }
        #[cfg(target_os = "macos")]
        {
            let brew_ok = Command::new("brew")
                .args(["install", "node"])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
            if brew_ok && Self::is_cli_installed("node") && Self::is_cli_installed("npm") {
                return Ok(());
            }
            Err("Node.js/npm não encontrados e falhou a instalação automática via Homebrew. Instale Node LTS para continuar.".to_string())
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            let apt_ok = if Self::is_cli_installed("apt-get") {
                Command::new("sh")
                    .args([
                        "-lc",
                        "sudo apt-get update && sudo apt-get install -y nodejs npm",
                    ])
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false)
            } else {
                false
            };
            if apt_ok && Self::is_cli_installed("node") && Self::is_cli_installed("npm") {
                return Ok(());
            }

            let dnf_ok = if Self::is_cli_installed("dnf") {
                Command::new("sh")
                    .args(["-lc", "sudo dnf install -y nodejs npm"])
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false)
            } else {
                false
            };
            if dnf_ok && Self::is_cli_installed("node") && Self::is_cli_installed("npm") {
                return Ok(());
            }

            let pacman_ok = if Self::is_cli_installed("pacman") {
                Command::new("sh")
                    .args(["-lc", "sudo pacman -S --noconfirm nodejs npm"])
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false)
            } else {
                false
            };
            if pacman_ok && Self::is_cli_installed("node") && Self::is_cli_installed("npm") {
                return Ok(());
            }

            Err("Node.js/npm não encontrados e falhou a instalação automática (apt/dnf/pacman). Instale Node LTS para continuar.".to_string())
        }
    }

    fn install_cli_npm(model: TerminalCliModel) -> Result<(), String> {
        let pkg = model.npm_package();
        #[cfg(target_os = "windows")]
        let output = Command::new("cmd")
            .args(["/C", &format!("npm install -g {pkg}")])
            .output()
            .map_err(|e| format!("erro ao executar npm: {e}"))?;
        #[cfg(not(target_os = "windows"))]
        let output = Command::new("sh")
            .args(["-lc", &format!("npm install -g {pkg}")])
            .output()
            .map_err(|e| format!("erro ao executar npm: {e}"))?;

        if output.status.success() {
            Ok(())
        } else {
            let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
            if err.is_empty() {
                Err("falha ao instalar CLI via npm".to_string())
            } else {
                Err(err)
            }
        }
    }

    fn terminal_working_dir(&self) -> PathBuf {
        if let Some(project_file) = &self.current_project {
            let normalized = Self::resolve_project_file_path(project_file, false);
            let parent = normalized
                .parent()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."));
            if parent.join("Assets").is_dir() {
                return parent;
            }
            let stem = normalized
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Projeto");
            let candidate = parent.join(stem);
            if candidate.is_dir() && candidate.join("Assets").is_dir() {
                return candidate;
            }
            return parent;
        }
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    }

    fn terminal_working_dir_for_spawn(&self) -> PathBuf {
        let p = self.terminal_working_dir();
        #[cfg(target_os = "windows")]
        {
            let s = p.to_string_lossy().to_string();
            if s.starts_with(r"\\?\") {
                return PathBuf::from(s.trim_start_matches(r"\\?\"));
            }
        }
        p
    }

    fn shell_escape_path_for_cd(path: &Path, windows: bool) -> String {
        let mut raw = path.to_string_lossy().to_string();
        if windows && raw.starts_with(r"\\?\") {
            raw = raw.trim_start_matches(r"\\?\").to_string();
        }
        if windows {
            format!("\"{}\"", raw.replace('"', "\"\""))
        } else {
            format!("\"{}\"", raw.replace('"', "\\\""))
        }
    }

    fn resize_embedded_terminal(&mut self, cols: u16, rows: u16) {
        if cols == 0 || rows == 0 {
            return;
        }
        if self.terminai.terminal_cols == cols && self.terminai.terminal_rows == rows {
            return;
        }
        self.terminai.terminal_cols = cols;
        self.terminai.terminal_rows = rows;
        if let Some(parser) = self.terminai.terminal_parser.as_mut() {
            parser.set_size(rows, cols);
        }
        if let Some(session) = self.terminai.terminal_session.as_mut() {
            let _ = session.master.resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            });
        }
    }

    fn terminal_color_256(idx: u8) -> egui::Color32 {
        const BASE16: [(u8, u8, u8); 16] = [
            (0, 0, 0),
            (205, 49, 49),
            (13, 188, 121),
            (229, 229, 16),
            (36, 114, 200),
            (188, 63, 188),
            (17, 168, 205),
            (229, 229, 229),
            (102, 102, 102),
            (241, 76, 76),
            (35, 209, 139),
            (245, 245, 67),
            (59, 142, 234),
            (214, 112, 214),
            (41, 184, 219),
            (255, 255, 255),
        ];
        if idx < 16 {
            let (r, g, b) = BASE16[idx as usize];
            return egui::Color32::from_rgb(r, g, b);
        }
        if idx < 232 {
            let v = idx - 16;
            let r = v / 36;
            let g = (v % 36) / 6;
            let b = v % 6;
            let comp = |n: u8| if n == 0 { 0 } else { 55 + n * 40 };
            return egui::Color32::from_rgb(comp(r), comp(g), comp(b));
        }
        let gray = 8 + (idx - 232) * 10;
        egui::Color32::from_rgb(gray, gray, gray)
    }

    fn terminal_color(color: vt100::Color) -> Option<egui::Color32> {
        match color {
            vt100::Color::Default => None,
            vt100::Color::Idx(i) => Some(Self::terminal_color_256(i)),
            vt100::Color::Rgb(r, g, b) => Some(egui::Color32::from_rgb(r, g, b)),
        }
    }

    fn build_terminal_layout_job(&self) -> LayoutJob {
        let mut job = LayoutJob::default();
        let default_fg = egui::Color32::from_rgb(220, 220, 220);
        let default_bg = egui::Color32::TRANSPARENT;
        let base_font = egui::FontId::monospace(13.0);

        let Some(parser) = &self.terminai.terminal_parser else {
            let mut fmt = egui::TextFormat::default();
            fmt.font_id = base_font;
            fmt.color = default_fg;
            fmt.background = default_bg;
            job.append(&self.terminai.terminal_output, 0.0, fmt);
            return job;
        };

        let screen = parser.screen();
        let (rows, cols) = screen.size();
        let (cursor_row, cursor_col) = screen.cursor_position();
        let cursor_visible = !screen.hide_cursor();

        for row in 0..rows {
            for col in 0..cols {
                let mut text = String::from(" ");
                let mut fmt = egui::TextFormat::default();
                fmt.font_id = base_font.clone();
                fmt.color = default_fg;
                fmt.background = default_bg;

                if let Some(cell) = screen.cell(row, col) {
                    let c = cell.contents();
                    if !c.is_empty() {
                        text = c;
                    }
                    if let Some(fg) = Self::terminal_color(cell.fgcolor()) {
                        fmt.color = fg;
                    }
                    if let Some(bg) = Self::terminal_color(cell.bgcolor()) {
                        fmt.background = bg;
                    }
                    if cell.bold() {
                        fmt.font_id = egui::FontId::monospace(14.0);
                    }
                    if cell.italic() {
                        fmt.italics = true;
                    }
                    if cell.underline() {
                        fmt.underline = egui::Stroke::new(1.0, fmt.color);
                    }
                    if cell.inverse() {
                        std::mem::swap(&mut fmt.color, &mut fmt.background);
                        if fmt.background == egui::Color32::TRANSPARENT {
                            fmt.background = default_fg;
                        }
                    }
                }

                if cursor_visible && row == cursor_row && col == cursor_col {
                    fmt.background = egui::Color32::from_rgb(210, 210, 210);
                    fmt.color = egui::Color32::from_rgb(15, 15, 15);
                }

                job.append(&text, 0.0, fmt);
            }
            if row + 1 < rows {
                job.append(
                    "\n",
                    0.0,
                    egui::TextFormat {
                        font_id: base_font.clone(),
                        color: default_fg,
                        background: default_bg,
                        ..Default::default()
                    },
                );
            }
        }
        job
    }

    fn start_embedded_cli_session(&mut self, model: TerminalCliModel) -> Result<(), String> {
        self.stop_embedded_terminal_session();
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: self.terminai.terminal_rows,
                cols: self.terminai.terminal_cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("falha ao abrir PTY: {e}"))?;

        let mut cmd = {
            #[cfg(target_os = "windows")]
            {
                let comspec = std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string());
                let mut c = CommandBuilder::new(comspec);
                c.arg("/K");
                c
            }
            #[cfg(not(target_os = "windows"))]
            {
                let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
                let mut c = CommandBuilder::new(shell);
                c.arg("-i");
                c
            }
        };
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        cmd.env("FORCE_COLOR", "1");
        cmd.env("CLICOLOR", "1");
        cmd.env("CLICOLOR_FORCE", "1");
        cmd.cwd(self.terminal_working_dir_for_spawn());
        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("falha ao iniciar sessão PTY: {e}"))?;
        drop(pair.slave);

        let master = pair.master;
        let mut reader = master
            .try_clone_reader()
            .map_err(|e| format!("falha ao clonar leitor PTY: {e}"))?;
        let mut writer = master
            .take_writer()
            .map_err(|e| format!("falha ao abrir writer PTY: {e}"))?;
        let (tx, rx) = mpsc::channel::<Vec<u8>>();
        std::thread::spawn(move || {
            let mut buf = [0_u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        // Primeiro entra no diretorio raiz do projeto, depois executa o CLI.
        let project_dir = self.terminal_working_dir();
        let cd_cmd = {
            #[cfg(target_os = "windows")]
            {
                format!(
                    "cd /d {}",
                    Self::shell_escape_path_for_cd(&project_dir, true)
                )
            }
            #[cfg(not(target_os = "windows"))]
            {
                format!("cd {}", Self::shell_escape_path_for_cd(&project_dir, false))
            }
        };
        let mut cd_line = cd_cmd;
        #[cfg(target_os = "windows")]
        cd_line.push_str("\r\n");
        #[cfg(not(target_os = "windows"))]
        cd_line.push('\n');
        let _ = writer.write_all(cd_line.as_bytes());
        let _ = writer.flush();

        let cli_cmd = {
            #[cfg(target_os = "windows")]
            {
                let exe = model.exe_name();
                let cmd_shim = format!("{exe}.cmd");
                if Self::is_cli_installed(&cmd_shim) {
                    cmd_shim
                } else {
                    exe.to_string()
                }
            }
            #[cfg(not(target_os = "windows"))]
            {
                model.exe_name().to_string()
            }
        };
        let mut line = cli_cmd;
        #[cfg(target_os = "windows")]
        line.push_str("\r\n");
        #[cfg(not(target_os = "windows"))]
        line.push('\n');
        let _ = writer.write_all(line.as_bytes());
        let _ = writer.flush();

        self.terminai.terminal_output.clear();
        self.terminai.terminal_transcript.clear();
        self.terminai.terminal_input.clear();
        self.terminai.terminal_parser = Some(Parser::new(
            self.terminai.terminal_rows,
            self.terminai.terminal_cols,
            10_000,
        ));
        self.terminai.terminal_output_rx = Some(rx);
        self.terminai.terminal_session = Some(EmbeddedTerminalSession {
            child,
            master,
            writer,
        });
        let mut wd = self.terminal_working_dir().to_string_lossy().to_string();
        if wd.starts_with(r"\\?\") {
            wd = wd.trim_start_matches(r"\\?\").to_string();
        }
        self.terminai.terminal_status =
            Some(format!("{} iniciado no TerminAI em {}", model.label(), wd));
        Ok(())
    }

    fn stop_embedded_terminal_session(&mut self) {
        if let Some(mut session) = self.terminai.terminal_session.take() {
            let _ = session.child.kill();
            let _ = session.child.wait();
        }
        self.terminai.terminal_output_rx = None;
        self.terminai.terminal_parser = None;
    }

    fn start_terminal_provision(&mut self, model: TerminalCliModel) {
        if self.terminai.terminal_busy {
            return;
        }
        if self.current_project.is_none() {
            self.terminai.terminal_status =
                Some("Abra um projeto (.deng) antes de iniciar o TerminAI".to_string());
            self.terminai.terminal_selected_model = None;
            return;
        }
        self.terminai.terminal_busy = true;
        self.terminai.terminal_status =
            Some(format!("Verificando e preparando {}...", model.label()));
        let (tx, rx) = mpsc::channel::<TerminalProvisionResult>();
        self.terminai.terminal_job_rx = Some(rx);
        std::thread::spawn(move || {
            if let Err(err) = Self::node_tooling_ready() {
                let _ = tx.send(TerminalProvisionResult {
                    ok: false,
                    message: err,
                    model: None,
                });
                return;
            }
            let exe = model.exe_name();
            if !Self::is_cli_installed(exe) {
                let install = Self::install_cli_npm(model);
                if let Err(err) = install {
                    let _ = tx.send(TerminalProvisionResult {
                        ok: false,
                        message: format!("Falha ao instalar {}: {}", model.label(), err),
                        model: None,
                    });
                    return;
                }
                if !Self::is_cli_installed(exe) {
                    let _ = tx.send(TerminalProvisionResult {
                        ok: false,
                        message: format!(
                            "{} instalado, mas comando não foi encontrado no PATH",
                            model.label()
                        ),
                        model: None,
                    });
                    return;
                }
            }
            let _ = tx.send(TerminalProvisionResult {
                ok: true,
                message: format!("{} pronto para iniciar no TerminAI", model.label()),
                model: Some(model),
            });
        });
    }

    pub(crate) fn draw_terminal_window(&mut self, ctx: &egui::Context) {
        self.poll_terminal_job();
        if !self.terminai.terminal_enabled {
            return;
        }
        let viewport_id = egui::ViewportId::from_hash_of("dengine_terminal_viewport");
        let mut close_terminal = false;
        ctx.show_viewport_immediate(
            viewport_id,
            egui::ViewportBuilder::default()
                .with_title("TerminAI")
                .with_inner_size([520.0, 280.0])
                .with_min_inner_size([420.0, 220.0])
                .with_resizable(true)
                .with_decorations(true),
            |ctx, _class| {
                if ctx.input(|i| i.viewport().close_requested()) {
                    close_terminal = true;
                    return;
                }
                egui::CentralPanel::default().show(ctx, |ui| {
                    if self.terminai.terminal_busy || self.terminai.terminal_session.is_some() {
                        ctx.request_repaint();
                        ctx.request_repaint_after(std::time::Duration::from_millis(16));
                    }
                    self.poll_terminal_output();
                    ui.label("Escolha um modelo para abrir no terminal:");
                    ui.add_space(8.0);

                    let button_w = ((ui.available_width() - 16.0) / 3.0).max(96.0);
                    ui.horizontal(|ui| {
                        for model in [
                            TerminalCliModel::Qwen,
                            TerminalCliModel::Gemini,
                            TerminalCliModel::Codex,
                        ] {
                            let selected = self.terminai.terminal_selected_model == Some(model);
                            let button = egui::Button::new(model.label())
                                .fill(if selected {
                                    egui::Color32::from_rgb(58, 84, 64)
                                } else {
                                    egui::Color32::from_rgb(52, 52, 52)
                                })
                                .stroke(egui::Stroke::new(
                                    1.0,
                                    if selected {
                                        egui::Color32::from_rgb(15, 232, 121)
                                    } else {
                                        egui::Color32::from_gray(80)
                                    },
                                ));
                            if ui
                                .add_enabled(
                                    !self.terminai.terminal_busy,
                                    button.min_size(egui::vec2(button_w, 34.0)),
                                )
                                .clicked()
                            {
                                self.terminai.terminal_selected_model = Some(model);
                                self.start_terminal_provision(model);
                            }
                        }
                    });

                    if self.terminai.terminal_busy {
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::Spinner::new()
                                    .size(14.0)
                                    .color(egui::Color32::from_rgb(15, 232, 121)),
                            );
                            ui.label("Preparando terminal...");
                        });
                    }
                    if let Some(status) = &self.terminai.terminal_status {
                        ui.add_space(6.0);
                        ui.label(status);
                    }
                    ui.separator();
                    ui.label("Terminal virtual:");
                    let term_id = ui.make_persistent_id("terminai_terminal_surface");
                    let term_h = ui.available_height().max(120.0);
                    let frame = egui::Frame::new()
                        .fill(egui::Color32::from_rgb(14, 14, 14))
                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(60)))
                        .inner_margin(egui::Margin::same(6));
                    let frame_resp = ui
                        .allocate_ui_with_layout(
                            egui::vec2(ui.available_width(), term_h),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                frame
                                    .show(ui, |ui| {
                                        let max = ui.available_size();
                                        let cols = (max.x / 8.2).floor().max(40.0) as u16;
                                        let rows = (max.y / 16.0).floor().max(10.0) as u16;
                                        self.resize_embedded_terminal(cols, rows);
                                        let layout_job = self.build_terminal_layout_job();
                                        egui::ScrollArea::both()
                                            .id_salt("terminai_output_scroll")
                                            .stick_to_bottom(true)
                                            .show(ui, |ui| {
                                                ui.add(
                                                    egui::Label::new(layout_job).selectable(true),
                                                );
                                            });
                                    })
                                    .response
                            },
                        )
                        .inner;
                    let term_resp = ui.interact(frame_resp.rect, term_id, egui::Sense::click());
                    if self.terminai.terminal_enabled {
                        ui.memory_mut(|m| m.request_focus(term_id));
                    } else if term_resp.clicked() {
                        ui.memory_mut(|m| m.request_focus(term_id));
                    }
                    let terminal_has_focus = self.terminai.terminal_enabled;
                    if terminal_has_focus {
                        ui.painter().rect_stroke(
                            frame_resp.rect,
                            3.0,
                            egui::Stroke::new(1.0, egui::Color32::from_rgb(15, 232, 121)),
                            egui::StrokeKind::Outside,
                        );
                    }

                    if let Some(session) = self.terminai.terminal_session.as_mut() {
                        if terminal_has_focus {
                            let events = ctx.input(|i| i.events.clone());
                            for ev in events {
                                match ev {
                                    egui::Event::Text(t) => {
                                        let _ = session.writer.write_all(t.as_bytes());
                                    }
                                    egui::Event::Paste(t) => {
                                        let _ = session.writer.write_all(t.as_bytes());
                                    }
                                    egui::Event::Key {
                                        key,
                                        pressed: true,
                                        modifiers,
                                        ..
                                    } => {
                                        let seq: Option<&'static [u8]> = match key {
                                            egui::Key::Enter => Some(b"\r"),
                                            egui::Key::Tab => Some(b"\t"),
                                            egui::Key::Backspace => Some(&[0x08]),
                                            egui::Key::Delete => Some(b"\x1b[3~"),
                                            egui::Key::Home => Some(b"\x1b[H"),
                                            egui::Key::End => Some(b"\x1b[F"),
                                            egui::Key::ArrowUp => Some(b"\x1b[A"),
                                            egui::Key::ArrowDown => Some(b"\x1b[B"),
                                            egui::Key::ArrowRight => Some(b"\x1b[C"),
                                            egui::Key::ArrowLeft => Some(b"\x1b[D"),
                                            _ => None,
                                        };
                                        if let Some(s) = seq {
                                            let _ = session.writer.write_all(s);
                                        } else if modifiers.ctrl && key == egui::Key::C {
                                            let _ = session.writer.write_all(&[0x03]);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        let _ = session.writer.flush();
                    }

                    egui::CollapsingHeader::new("Log completo do terminal")
                        .default_open(false)
                        .show(ui, |ui| {
                            egui::ScrollArea::vertical()
                                .id_salt("terminai_full_log_scroll")
                                .max_height(96.0)
                                .stick_to_bottom(true)
                                .show(ui, |ui| {
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(&self.terminai.terminal_transcript)
                                                .monospace()
                                                .size(12.0),
                                        )
                                        .selectable(true),
                                    );
                                });
                        });
                });
            },
        );
        if close_terminal {
            self.stop_embedded_terminal_session();
            self.terminai.terminal_enabled = false;
            ctx.send_viewport_cmd_to(viewport_id, egui::ViewportCommand::Close);
        }
    }
}
