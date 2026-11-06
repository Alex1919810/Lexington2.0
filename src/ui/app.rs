use super::{assets::Picture, style};
use crate::{
    backend::{
        character::CharacterState, clock::ClockState, codex::Client, config::Config, resources,
    },
    tools::{paths, platform},
};
use eframe::egui::{self, Rect, Vec2, pos2, vec2};
use std::{
    collections::VecDeque,
    path::PathBuf,
    time::{Duration, Instant},
};

pub struct Pet {
    #[cfg(target_os = "linux")]
    pub(super) drag_window: Option<platform::DragWindow>,
    pub(super) root: PathBuf,
    pub(super) config: Config,
    pub(super) library: CharacterState,
    pub(super) picture: Option<Picture>,
    pub(super) icons: Vec<egui::TextureHandle>,
    pub(super) dialog_bg: Option<Picture>,
    pub(super) input_bg: Option<Picture>,
    pub(super) input_bounds: Option<Rect>,
    pub(super) dialog_bounds: Option<Rect>,
    pub(super) pinned: bool,
    pub(super) text: String,
    pub(super) typed_at: Instant,
    pub(super) full_text: bool,
    pub(super) dialog_clicks: u8,
    pub(super) pet_pressed: Option<Instant>,
    pub(super) drag_sent: bool,
    pub(super) last_activity: Instant,
    pub(super) clock_state: ClockState,
    pub(super) input: String,
    pub(super) error: String,
    pub(super) codex: Option<Client>,
    pub(super) codex_status: String,
    pub(super) busy: bool,
    pub(super) codex_messages: Vec<(String, String)>,
    pub(super) codex_prompts: VecDeque<super::codex::PendingPrompt>,
    pub(super) capture: Option<PathBuf>,
    pub(super) capture_requested: bool,
    pub(super) capture_delay: Duration,
    pub(super) started: Instant,
    pub(super) placed: bool,
    pub(super) raised: bool,
    pub(super) hints_applied: bool,
    pub(super) about: bool,
    pub(super) settings: bool,
}
impl Pet {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        platform::configure(cc);
        let pinned = cc
            .storage
            .and_then(|s| s.get_string("sidebarAlwaysVisible"))
            .is_none_or(|s| s != "false");
        let mut app = Self::from_context(&cc.egui_ctx, pinned);
        #[cfg(target_os = "linux")]
        {
            app.drag_window = platform::DragWindow::new(cc);
        }
        app.start_codex(&cc.egui_ctx);
        app
    }
    pub(super) fn from_context(ctx: &egui::Context, pinned: bool) -> Self {
        Self::from_root(ctx, pinned, paths::resource_root())
    }
    pub(super) fn from_root(ctx: &egui::Context, pinned: bool, root: PathBuf) -> Self {
        style::install(ctx);
        let config = Config::load(&root);
        let mut app = Self {
            #[cfg(target_os = "linux")]
            drag_window: None,
            root,
            config,
            library: CharacterState::default(),
            picture: None,
            icons: super::assets::icons(ctx),
            dialog_bg: None,
            input_bg: None,
            input_bounds: None,
            dialog_bounds: None,
            pinned,
            text: String::new(),
            typed_at: Instant::now(),
            full_text: false,
            dialog_clicks: 0,
            pet_pressed: None,
            drag_sent: false,
            last_activity: Instant::now(),
            clock_state: ClockState::default(),
            input: String::new(),
            error: String::new(),
            codex: None,
            codex_status: "未启动".into(),
            busy: false,
            codex_messages: vec![],
            codex_prompts: VecDeque::new(),
            capture: std::env::args()
                .skip_while(|a| a != "--screenshot")
                .nth(1)
                .map(PathBuf::from),
            capture_requested: false,
            capture_delay: Duration::from_secs_f64(
                std::env::args()
                    .skip_while(|arg| arg != "--screenshot-delay")
                    .nth(1)
                    .and_then(|s| s.parse::<f64>().ok())
                    .filter(|s| s.is_finite() && *s >= 0.)
                    .unwrap_or(4.),
            ),
            started: Instant::now(),
            placed: false,
            raised: false,
            hints_applied: false,
            about: false,
            settings: false,
        };
        app.refresh(ctx);
        app
    }
    pub(super) fn load_picture(&mut self, ctx: &egui::Context) {
        self.picture = self.library.selected_path().and_then(|path| {
            Picture::load(ctx, path)
                .map_err(|e| eprintln!("加载立绘失败: {e}"))
                .ok()
        });
    }
    pub(super) fn refresh(&mut self, ctx: &egui::Context) {
        self.config = Config::load(&self.root);
        self.library.refresh(&self.root);
        self.load_picture(ctx);
        self.dialog_bg = resources::background(&self.root, "dialog")
            .and_then(|path| Picture::load(ctx, &path).ok());
        self.input_bg = resources::background(&self.root, "input")
            .and_then(|path| Picture::load(ctx, &path).ok());
    }
    pub(super) fn context_menu(&mut self, ctx: &egui::Context, response: &egui::Response) {
        response.context_menu(|ui| {
            if ui.button("设置").clicked() {
                self.settings = true;
                ui.close_menu();
            }
            ui.separator();
            if ui.button("关于").clicked() {
                self.about = true;
                ui.close_menu();
            }
            ui.separator();
            if ui.button("关闭").clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
    }
}
impl eframe::App for Pet {
    fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
        [0.; 4]
    }
    fn persist_egui_memory(&self) -> bool {
        false
    }
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string("window", String::new());
        storage.set_string("sidebarAlwaysVisible", self.pinned.to_string());
    }
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.ui(ctx);
    }
}
impl Pet {
    pub(super) fn ui(&mut self, ctx: &egui::Context) {
        ctx.request_repaint_after(Duration::from_millis(16));
        #[cfg(target_os = "linux")]
        if let Some(window) = &mut self.drag_window {
            self.drag_sent |= window.update();
        }
        if !self.raised && self.started.elapsed() > Duration::from_millis(300) {
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
                egui::WindowLevel::AlwaysOnTop,
            ));
            self.raised = true;
        }
        if !self.hints_applied && self.started.elapsed() > Duration::from_millis(600) {
            #[cfg(target_os = "linux")]
            if let Some(window) = &self.drag_window {
                window.configure();
            }
            self.hints_applied = true;
        }
        if !self.placed
            && let Some(size) = ctx.input(|i| i.viewport().monitor_size)
        {
            let dev = std::env::args().any(|a| a == "--dev");
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(if dev {
                vec2(800., 600.)
            } else {
                vec2(500., 500.)
            }));
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(if dev {
                pos2(100., 100.)
            } else {
                pos2((size.x - 520.).max(0.), 50.)
            }));
            self.placed = true;
        }
        if self.capture.is_some()
            && !self.capture_requested
            && self.started.elapsed() > self.capture_delay
        {
            self.capture_requested = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        let screenshot = ctx.input(|i| {
            i.events.iter().find_map(|event| {
                if let egui::Event::Screenshot { image, .. } = event {
                    Some(image.clone())
                } else {
                    None
                }
            })
        });
        if let (Some(path), Some(image)) = (&self.capture, screenshot) {
            let bytes: Vec<u8> = image
                .pixels
                .iter()
                .flat_map(|p| p.to_srgba_unmultiplied())
                .collect();
            if let Err(error) = image::save_buffer(
                path,
                &bytes,
                image.width() as u32,
                image.height() as u32,
                image::ColorType::Rgba8,
            ) {
                eprintln!("截图失败: {error}");
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if ctx.input(|i| i.pointer.delta() != Vec2::ZERO) {
            self.last_activity = Instant::now();
        }
        self.poll_codex();
        if !self.busy
            && !self.text.is_empty()
            && self.last_activity.elapsed().as_secs_f64() >= self.config.dialog.auto_close_delay
        {
            self.text.clear();
        }
        if let Some(message) = self.clock_state.poll(&self.config.clock_in) {
            self.say(message);
        }
        let screen = ctx.screen_rect();
        let (size, transform, clicked) = self.pet_ui(ctx, screen);
        let center = screen.center();
        let bubble = self.bubble(ctx, center).map(|rect| transform * rect);
        self.dialog_bounds = bubble;
        self.input_ui(ctx, screen, center, size.y);
        let sidebar_handled = self.sidebar(ctx, screen);
        if !clicked
            && !sidebar_handled
            && ctx.input(|i| i.pointer.primary_clicked())
            && let Some(rect) = bubble
        {
            let outside = ctx.input(|i| {
                i.pointer.interact_pos().is_some_and(|p| {
                    !rect.contains(p) && (transform.inverse() * p).distance(center) > size.y * 0.3
                })
            });
            if outside {
                self.text.clear();
            }
        }
        egui::Window::new("关于")
            .open(&mut self.about)
            .show(ctx, |ui| {
                ui.label("Lexington 2.0\n纯 Rust 桌面宠物\neframe / egui");
            });
        let mut restart_codex = false;
        let reload = egui::Window::new("设置")
            .open(&mut self.settings)
            .show(ctx, |ui| {
                ui.label("配置文件：");
                ui.label(self.root.join("config/config.json").display().to_string());
                ui.separator();
                ui.label(format!("Codex：{}", self.codex_status));
                ui.label(format!("程序：{}", self.config.codex.executable));
                let directory = self
                    .config
                    .codex
                    .working_directory
                    .as_ref()
                    .map_or_else(|| self.root.clone(), |dir| self.root.join(dir));
                ui.label(format!("工作目录：{}", directory.display()));
                ui.label("修改 codex 工作目录或程序路径后，请重启会话。");
                restart_codex = ui.button("重启 Codex 会话").clicked();
                ui.button("重新加载资源与配置").clicked()
            })
            .is_some_and(|response| response.inner == Some(true));
        if reload {
            self.refresh(ctx);
        }
        if restart_codex {
            self.config = Config::load(&self.root);
            self.start_codex(ctx);
        }
        self.codex_prompt_ui(ctx);
    }
}
