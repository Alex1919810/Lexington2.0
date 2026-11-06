use super::{assets::Picture, style};
use crate::{
    backend::{
        character::CharacterState,
        clock::{self, ClockState},
        codex::Client,
        config::Config,
        resources,
    },
    tools::{activity, paths, platform},
};
use eframe::egui::{self, Rect, Vec2, vec2};
use std::{
    collections::VecDeque,
    path::PathBuf,
    time::{Duration, Instant, SystemTime},
};

pub struct Pet {
    #[cfg(target_os = "linux")]
    pub(super) drag_window: Option<platform::DragWindow>,
    pub(super) root: PathBuf,
    pub(super) config: Config,
    pub(super) last_config_check: Instant,
    pub(super) config_stamp: Option<(SystemTime, u64)>,
    pub(super) config_error: String,
    pub(super) codex_restart_pending: bool,
    pub(super) library: CharacterState,
    pub(super) picture: Option<Picture>,
    pub(super) icons: Vec<egui::TextureHandle>,
    pub(super) dialog_bg: Option<Picture>,
    pub(super) input_bg: Option<Picture>,
    pub(super) input_bounds: Option<Rect>,
    pub(super) input_regions: Vec<Rect>,
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
    pub(super) activity_monitor: Option<activity::Monitor>,
    pub(super) activity_status: String,
    pub(super) clock_error: String,
    pub(super) clock_storage_ok: bool,
    pub(super) clock_edit: String,
    pub(super) clock_notice: bool,
    pub(super) last_clock_poll: Instant,
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
    pub(super) dev: bool,
    pub(super) window_position: Option<egui::Pos2>,
    #[cfg(target_os = "linux")]
    window_pixels_per_point: f32,
    pub(super) raised: bool,
    pub(super) hints_applied: bool,
    pub(super) about: bool,
    pub(super) settings: bool,
    pub(super) settings_page: u8,
}
impl Pet {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        platform::configure(cc);
        let mut app = Self::from_context(&cc.egui_ctx, true);
        if let Some(storage) = cc.storage {
            app.restore_state(storage);
        }
        #[cfg(target_os = "linux")]
        {
            app.drag_window = platform::DragWindow::new(cc);
        }
        match ClockState::load(paths::clock_state_file(app.dev), clock::now()) {
            Ok(state) => app.clock_state = state,
            Err(error) => {
                app.clock_error = error;
                app.clock_storage_ok = false;
            }
        }
        let ctx = cc.egui_ctx.clone();
        app.activity_monitor = Some(activity::Monitor::start(move || ctx.request_repaint()));
        app.start_codex(&cc.egui_ctx);
        app
    }
    pub(super) fn restore_state(&mut self, storage: &dyn eframe::Storage) {
        self.pinned = storage
            .get_string("sidebarAlwaysVisible")
            .is_none_or(|s| s != "false");
        self.window_position = storage
            .get_string(if self.dev {
                "devWindowPosition"
            } else {
                "windowPosition"
            })
            .and_then(|s| serde_json::from_str::<[f32; 2]>(&s).ok())
            .map(egui::Pos2::from)
            .filter(|position| position.is_finite());
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
            last_config_check: Instant::now(),
            config_stamp: None,
            config_error: String::new(),
            codex_restart_pending: false,
            library: CharacterState::default(),
            picture: None,
            icons: super::assets::icons(ctx),
            dialog_bg: None,
            input_bg: None,
            input_bounds: None,
            input_regions: Vec::new(),
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
            activity_monitor: None,
            activity_status: "等待系统活动".into(),
            clock_error: String::new(),
            clock_storage_ok: true,
            clock_edit: String::new(),
            clock_notice: false,
            last_clock_poll: Instant::now() - Duration::from_secs(2),
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
            dev: std::env::args().any(|arg| arg == "--dev"),
            window_position: None,
            #[cfg(target_os = "linux")]
            window_pixels_per_point: 1.,
            raised: false,
            hints_applied: false,
            about: false,
            settings: false,
            settings_page: 0,
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
        self.reload_config();
        self.library.refresh(&self.root);
        self.load_picture(ctx);
        self.dialog_bg = resources::background(&self.root, "dialog")
            .and_then(|path| Picture::load(ctx, &path).ok());
        self.input_bg = resources::background(&self.root, "input")
            .and_then(|path| Picture::load(ctx, &path).ok());
    }
    pub(super) fn reload_config(&mut self) {
        self.last_config_check = Instant::now();
        self.config_stamp = std::fs::metadata(self.root.join("config/config.json"))
            .ok()
            .and_then(|m| m.modified().ok().map(|time| (time, m.len())));
        match Config::read(&self.root) {
            Ok(config) => {
                if config.clock_in.auto_start != self.config.clock_in.auto_start
                    || config.clock_in.confirmation_seconds
                        != self.config.clock_in.confirmation_seconds
                {
                    self.clock_state.reset_candidate();
                }
                self.codex_restart_pending |= config.codex.executable
                    != self.config.codex.executable
                    || config.codex.working_directory != self.config.codex.working_directory;
                self.config = config;
                self.config_error.clear();
            }
            Err(error) => self.config_error = error,
        }
    }
    pub(super) fn context_menu(&mut self, ctx: &egui::Context, response: &egui::Response) {
        let mut menu_rect = None;
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
            // egui 0.31 returns no InnerResponse while a context menu stays open.
            // Capture its current contents and frame here for the native input mask.
            menu_rect = Some(ui.min_rect() + egui::Frame::menu(ui.style()).total_margin());
        });
        if response.context_menu_opened()
            && let Some(rect) = menu_rect
        {
            self.input_regions.push(rect);
        }
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
        let position = self.window_position;
        #[cfg(target_os = "linux")]
        let position = self
            .drag_window
            .as_ref()
            .and_then(|window| window.position(self.window_pixels_per_point))
            .or(position);
        if self.placed
            && let Some(position) = position.filter(|p| p.is_finite())
        {
            storage.set_string(
                if self.dev {
                    "devWindowPosition"
                } else {
                    "windowPosition"
                },
                serde_json::to_string(&[position.x, position.y]).expect("finite window position"),
            );
        }
    }
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.ui(ctx);
    }
}
impl Pet {
    pub(super) fn ui(&mut self, ctx: &egui::Context) {
        self.input_regions.clear();
        #[cfg(target_os = "linux")]
        {
            self.window_pixels_per_point = ctx.pixels_per_point();
        }
        ctx.request_repaint_after(Duration::from_millis(16));
        #[cfg(target_os = "linux")]
        if let Some(window) = &mut self.drag_window {
            if ctx.input(|i| i.pointer.any_pressed()) {
                window.focus();
            }
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
            let window_size = if self.dev {
                vec2(800., 600.)
            } else {
                vec2(500., 500.)
            };
            let mut monitors = vec![Rect::from_min_size(egui::Pos2::ZERO, size)];
            #[cfg(target_os = "linux")]
            if let Some(window) = &self.drag_window {
                let native_monitors = window.monitors(ctx.pixels_per_point());
                if !native_monitors.is_empty() {
                    monitors = native_monitors;
                }
            }
            // Use visible artwork, not the transparent window rectangle, when
            // checking whether a saved position is still reachable on any monitor.
            let content = if let Some(picture) = &self.picture {
                let image_size = picture.size() * (450. / picture.size().max_elem()).min(1.);
                let image_origin = (window_size - image_size) / 2.;
                picture
                    .opaque_regions
                    .iter()
                    .fold(Rect::NOTHING, |bounds, region| {
                        bounds.union(Rect::from_min_max(
                            (image_origin + region.min.to_vec2() * image_size).to_pos2(),
                            (image_origin + region.max.to_vec2() * image_size).to_pos2(),
                        ))
                    })
            } else {
                Rect::from_center_size((window_size / 2.).to_pos2(), vec2(120., 120.))
            };
            let position = self
                .window_position
                .filter(|position| {
                    position.is_finite()
                        && monitors.iter().any(|monitor| {
                            let visible = content.translate(position.to_vec2()).intersect(*monitor);
                            visible.width() >= 24. && visible.height() >= 24.
                        })
                })
                .unwrap_or_else(|| {
                    let monitor = monitors[0];
                    monitor.min
                        + if self.dev {
                            vec2(100., 100.)
                        } else {
                            vec2((monitor.width() - 520.).max(0.), 50.)
                        }
                });
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(window_size));
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(position));
            self.window_position = Some(position);
            self.placed = true;
        } else if self.placed {
            let position = ctx.input(|i| i.viewport().outer_rect.map(|rect| rect.min));
            #[cfg(target_os = "linux")]
            let position = self
                .drag_window
                .as_ref()
                .and_then(|window| window.position(ctx.pixels_per_point()))
                .or(position);
            if let Some(position) = position.filter(|p| p.is_finite()) {
                self.window_position = Some(position);
            }
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
        if self.last_config_check.elapsed() >= Duration::from_millis(500) {
            self.last_config_check = Instant::now();
            let stamp = std::fs::metadata(self.root.join("config/config.json"))
                .ok()
                .and_then(|m| m.modified().ok().map(|time| (time, m.len())));
            if stamp != self.config_stamp {
                self.reload_config();
            }
        }
        if self.codex_restart_pending && !self.busy {
            self.start_codex(ctx);
        }
        if !self.busy
            && !self.text.is_empty()
            && self.last_activity.elapsed().as_secs_f64() >= self.config.dialog.auto_close_delay
        {
            self.text.clear();
        }
        self.poll_clock();
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
        if let Some(window) = egui::Window::new("关于")
            .open(&mut self.about)
            .show(ctx, |ui| {
                ui.label("Lexington 2.0\n纯 Rust 桌面宠物\neframe / egui");
            })
            && self.about
        {
            self.input_regions.push(window.response.rect);
        }
        let mut restart_codex = false;
        let mut reload = false;
        let mut no_approval = self.config.codex.no_approval;
        let mut approval_changed = false;
        let mut clock_auto_changed = false;
        let mut clock_action = None;
        let mut settings_open = self.settings;
        if let Some(window) = egui::Window::new("设置")
            .open(&mut settings_open)
            .order(egui::Order::Foreground)
            .collapsible(false)
            .resizable(false)
            .fixed_size(vec2(440., 326.))
            .default_pos(screen.center() - vec2(232., 190.))
            .frame(
                egui::Frame::window(&ctx.style())
                    .fill(egui::Color32::from_rgb(247, 248, 250))
                    .inner_margin(12)
                    .corner_radius(10),
            )
            .show(ctx, |ui| {
                for (text_style, size) in [
                    (egui::TextStyle::Body, 14.),
                    (egui::TextStyle::Button, 14.),
                    (egui::TextStyle::Small, 12.),
                ] {
                    ui.style_mut()
                        .text_styles
                        .insert(text_style, egui::FontId::proportional(size));
                }
                ui.spacing_mut().item_spacing = vec2(10., 10.);
                ui.spacing_mut().button_padding = vec2(10., 6.);
                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(
                        vec2(76., 326.),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            for (page, title) in [(0, "常规"), (1, "连接"), (2, "打卡")] {
                                if ui
                                    .add_sized(
                                        [76., 34.],
                                        egui::Button::new(title)
                                            .selected(self.settings_page == page),
                                    )
                                    .clicked()
                                {
                                    self.settings_page = page;
                                }
                            }
                        },
                    );
                    ui.separator();
                    ui.allocate_ui_with_layout(
                        vec2(ui.available_width(), 326.),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            egui::ScrollArea::vertical()
                                .id_salt("settings-content")
                                .max_height(326.)
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.add_space(2.);
                                    ui.label(
                                        egui::RichText::new(match self.settings_page { 1 => "连接", 2 => "打卡", _ => "常规" })
                                        .size(20.)
                                        .strong(),
                                    );
                                    ui.add_space(6.);
                                    if self.settings_page == 2 {
                                        (clock_auto_changed, clock_action) = self.clock_settings_ui(ui);
                                    } else if self.settings_page == 1 {
                                        egui::Frame::group(ui.style())
                                            .fill(egui::Color32::WHITE)
                                            .inner_margin(12)
                                            .corner_radius(8)
                                            .show(ui, |ui| {
                                                ui.set_width(ui.available_width());
                                                ui.horizontal(|ui| {
                                                    ui.strong("Codex");
                                                    ui.with_layout(
                                                        egui::Layout::right_to_left(egui::Align::Center),
                                                        |ui| {
                                                            ui.add(
                                                                egui::Label::new(&self.codex_status)
                                                                    .truncate(),
                                                            )
                                                            .on_hover_text(&self.codex_status);
                                                        },
                                                    );
                                                });
                                                ui.separator();
                                                let directory = self
                                                    .config
                                                    .codex
                                                    .working_directory
                                                    .as_ref()
                                                    .map_or_else(|| self.root.clone(), |dir| self.root.join(dir));
                                                for (title, value) in [
                                                    ("程序", self.config.codex.executable.clone()),
                                                    ("工作目录", directory.display().to_string()),
                                                ] {
                                                    ui.weak(title);
                                                    ui.horizontal(|ui| {
                                                        if ui.small_button("复制").clicked() {
                                                            ctx.copy_text(value.clone());
                                                        }
                                                        ui.add(egui::Label::new(&value).truncate())
                                                            .on_hover_text(&value);
                                                    });
                                                }
                                            });
                                        if self.codex_restart_pending {
                                            ui.weak("任务结束后重新连接");
                                        }
                                        restart_codex = ui
                                            .add_enabled(!self.busy, egui::Button::new("重启会话"))
                                            .on_hover_text(if self.busy {
                                                "当前任务结束后可重启"
                                            } else {
                                                "重新连接 Codex，清空会话上下文"
                                            })
                                            .clicked();
                                    } else {
                                        egui::Frame::group(ui.style())
                                            .fill(egui::Color32::WHITE)
                                            .inner_margin(12)
                                            .corner_radius(8)
                                            .show(ui, |ui| {
                                                ui.set_width(ui.available_width());
                                                ui.strong("审批");
                                                approval_changed = ui
                                                    .checkbox(&mut no_approval, "无需审批")
                                                    .on_hover_text("下次发送生效；关闭时恢复本机审批策略。沙箱权限不变。")
                                                    .changed();
                                            });
                                        egui::Frame::group(ui.style())
                                            .fill(egui::Color32::WHITE)
                                            .inner_margin(12)
                                            .corner_radius(8)
                                            .show(ui, |ui| {
                                                ui.set_width(ui.available_width());
                                                ui.strong("资源与配置");
                                                let path = self.root.join("config/config.json").display().to_string();
                                                ui.horizontal(|ui| {
                                                    if ui.small_button("复制路径").clicked() {
                                                        ctx.copy_text(path.clone());
                                                    }
                                                    ui.add(egui::Label::new("config.json").sense(egui::Sense::hover()))
                                                        .on_hover_text(&path);
                                                });
                                                ui.separator();
                                                reload = ui.button("重新加载")
                                                    .on_hover_text("重新加载资源与配置；配置文件更改也会自动生效")
                                                    .clicked();
                                            });
                                    }
                                    if !self.config_error.is_empty() {
                                        ui.colored_label(
                                            egui::Color32::from_rgb(180, 45, 45),
                                            &self.config_error,
                                        );
                                    }
                                });
                        },
                    );
                });
            })
            && settings_open
        {
            self.input_regions.push(window.response.rect);
        }
        self.settings = settings_open;
        if approval_changed {
            match Config::save_no_approval(&self.root, no_approval) {
                Ok(()) => {
                    self.config.codex.no_approval = no_approval;
                    self.config_error.clear();
                }
                Err(error) => self.config_error = error,
            }
        }
        if clock_auto_changed {
            let enabled = !self.config.clock_in.auto_start;
            match Config::save_auto_clock(&self.root, enabled) {
                Ok(()) => {
                    self.config.clock_in.auto_start = enabled;
                    self.clock_state.reset_candidate();
                    self.clock_error.clear();
                }
                Err(error) => self.clock_error = error,
            }
        }
        if let Some(action) = clock_action {
            self.clock_action(action);
        }
        if reload {
            self.refresh(ctx);
        }
        if restart_codex {
            self.reload_config();
            self.start_codex(ctx);
        }
        self.codex_prompt_ui(ctx);
        self.clock_notice_ui(ctx);
        #[cfg(target_os = "linux")]
        if let Some(window) = &mut self.drag_window {
            window.set_input_regions(&self.input_regions, screen, ctx.pixels_per_point());
        }
    }
}
