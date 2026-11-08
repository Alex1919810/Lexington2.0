use super::{app::Pet, clock::ClockAction};
use crate::backend::{
    codex_config::LocalCodexConfig,
    config::{CodexConfig, Config},
};
use eframe::egui::{self, vec2};
use std::time::{Duration, Instant};

#[derive(Default)]
struct SettingsActions {
    reload: bool,
    restart_codex: bool,
    no_approval: Option<bool>,
    clock_auto_changed: bool,
    clock_action: Option<ClockAction>,
    connection: Option<CodexConfig>,
    weather: Option<super::weather::WeatherAction>,
    role_card: Option<(String, crate::roleplay::RoleCard)>,
}

impl Pet {
    pub(super) fn settings_ui(&mut self, ctx: &egui::Context) {
        if !self.settings {
            self.role_editor = Default::default();
            self.connection_draft = None;
            self.connection_error.clear();
            self.settings_viewport_open = false;
            self.settings_drag = None;
            self.settings_pointer_down = false;
            self.settings_drag_active = false;
            return;
        }
        let id = egui::ViewportId::from_hash_of("lexington-settings");
        let size = vec2(464., 390.);
        let mut builder = egui::ViewportBuilder::default()
            .with_title("Lexington 设置")
            .with_inner_size(size)
            .with_resizable(false)
            .with_decorations(false)
            .with_transparent(false)
            .with_always_on_top()
            .with_window_type(egui::X11WindowType::Dock);
        if !self.settings_viewport_open {
            // Place near the pet, but fit within its monitor on first opening.
            let mut monitor = egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                ctx.input(|i| i.viewport().monitor_size)
                    .unwrap_or(vec2(1920., 1080.)),
            );
            #[cfg(any(target_os = "linux", windows))]
            if let Some(window) = &self.drag_window {
                let monitors = window.monitors(ctx.pixels_per_point());
                if let Some(found) = self
                    .window_position
                    .and_then(|p| {
                        monitors
                            .iter()
                            .find(|m| m.contains(p + ctx.screen_rect().size() / 2.))
                    })
                    .or_else(|| monitors.first())
                {
                    monitor = *found;
                }
            }
            let near = self.window_position.unwrap_or(monitor.min);
            let position = egui::pos2(
                near.x.clamp(
                    monitor.left(),
                    (monitor.right() - size.x).max(monitor.left()),
                ),
                near.y.clamp(
                    monitor.top(),
                    (monitor.bottom() - size.y).max(monitor.top()),
                ),
            );
            builder = builder.with_position(position);
            self.settings_started = Instant::now();
            self.settings_raised = false;
        }
        self.settings_viewport_open = true;
        let mut open = true;
        let mut actions = SettingsActions::default();
        ctx.show_viewport_immediate(id, builder, |settings, class| {
            if class == egui::ViewportClass::Embedded {
                if let Some(window) = egui::Window::new("设置")
                    .open(&mut open)
                    .order(egui::Order::Foreground)
                    .collapsible(false)
                    .resizable(false)
                    .fixed_size(vec2(440., 326.))
                    .default_pos(settings.screen_rect().center() - vec2(232., 190.))
                    .frame(
                        egui::Frame::window(&settings.style())
                            .fill(egui::Color32::from_rgb(247, 248, 250))
                            .inner_margin(12)
                            .corner_radius(10),
                    )
                    .show(settings, |ui| actions = self.settings_contents(ui))
                    && open
                {
                    self.input_regions.push(window.response.rect);
                }
            } else {
                if !self.settings_raised
                    && self.settings_started.elapsed() >= Duration::from_millis(800)
                {
                    settings.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
                        egui::WindowLevel::AlwaysOnTop,
                    ));
                    settings.send_viewport_cmd(egui::ViewportCommand::Focus);
                    self.settings_raised = true;
                }
                if settings.input(|i| i.viewport().close_requested()) {
                    open = false;
                }
                egui::CentralPanel::default()
                    .frame(
                        egui::Frame::central_panel(&settings.style())
                            .fill(egui::Color32::from_rgb(247, 248, 250))
                            .inner_margin(12),
                    )
                    .show(settings, |ui| {
                        // Register behind the controls so only unused body space
                        // starts a drag; buttons, editors and scrollbars win hit testing.
                        let background = ui.interact(
                            ui.max_rect(),
                            ui.id().with("settings-body-drag"),
                            egui::Sense::click_and_drag(),
                        );
                        let mut title_rect = egui::Rect::NOTHING;
                        let mut title_drag_started = false;
                        ui.horizontal(|ui| {
                            let (rect, response) = ui.allocate_exact_size(
                                vec2(ui.available_width() - 36., 28.),
                                egui::Sense::click_and_drag(),
                            );
                            ui.painter().text(
                                rect.left_center(),
                                egui::Align2::LEFT_CENTER,
                                "设置",
                                egui::FontId::proportional(16.),
                                ui.visuals().text_color(),
                            );
                            title_rect = rect;
                            title_drag_started = response.drag_started();
                            if ui.add_sized([28., 28.], egui::Button::new("×")).clicked() {
                                open = false;
                            }
                        });
                        ui.add_space(8.);
                        actions = self.settings_contents(ui);
                        let body_pressed = if settings.input(|i| i.pointer.primary_pressed()) {
                            // A background drag can share the press with a button
                            // (like a scroll area). Reject interactive children
                            // for the whole gesture, even after leaving the control.
                            let hits = settings.interaction_snapshot(|state| {
                                state.contains_pointer.iter().copied().collect::<Vec<_>>()
                            });
                            background.is_pointer_button_down_on()
                                && !hits
                                    .into_iter()
                                    .filter(|id| *id != background.id)
                                    .filter_map(|id| settings.read_response(id))
                                    .any(|response| {
                                        response.sense.senses_click()
                                            || response.sense.senses_drag()
                                    })
                        } else {
                            false
                        };
                        #[cfg(any(target_os = "linux", windows))]
                        if let Some(window) = &self.drag_window
                            && let Some((pointer, down)) =
                                window.screen_pointer(settings.pixels_per_point())
                        {
                            let was_active = self.settings_drag_active;
                            if !down {
                                self.settings_drag = None;
                                self.settings_drag_active = false;
                            } else if !self.settings_pointer_down {
                                self.settings_drag = window
                                    .window_at_pointer(
                                        "Lexington 设置",
                                        settings.pixels_per_point(),
                                    )
                                    .map(|(native, origin)| (pointer, origin, native));
                                self.settings_drag_active =
                                    self.settings_drag.is_some_and(|(_, origin, _)| {
                                        title_rect.contains(egui::Pos2::ZERO + (pointer - origin))
                                    });
                            }
                            // X11 polling may see the press one UI frame before
                            // egui receives it. Retain its native anchor while
                            // waiting for background hit testing to approve it.
                            if down && body_pressed {
                                self.settings_drag_active = self.settings_drag.is_some();
                            }
                            if self.settings_drag_active && !was_active {
                                settings.send_viewport_cmd(egui::ViewportCommand::Focus);
                            }
                            self.settings_pointer_down = down;
                            if self.settings_drag_active
                                && let Some((press, origin, native)) = self.settings_drag
                            {
                                let position = origin + (pointer - press);
                                // Use the same WM request as the pet. Mutter may
                                // ignore direct ConfigureWindow on managed Docks.
                                window.move_window(native, position, settings.pixels_per_point());
                            }
                        } else {
                            if settings.input(|i| i.pointer.primary_pressed()) {
                                self.settings_drag_active = body_pressed;
                            }
                            if title_drag_started
                                || (self.settings_drag_active && background.drag_started())
                            {
                                settings.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                            }
                        }
                        #[cfg(not(any(target_os = "linux", windows)))]
                        {
                            if settings.input(|i| i.pointer.primary_pressed()) {
                                self.settings_drag_active = body_pressed;
                            }
                            if title_drag_started
                                || (self.settings_drag_active && background.drag_started())
                            {
                                settings.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                            }
                        }
                    });
            }
        });
        self.settings = open;
        if !open {
            self.role_editor = Default::default();
            self.connection_draft = None;
            self.connection_error.clear();
            self.settings_viewport_open = false;
            self.settings_drag = None;
            self.settings_pointer_down = false;
            self.settings_drag_active = false;
            ctx.send_viewport_cmd_to(id, egui::ViewportCommand::Close);
        }
        if let Some(no_approval) = actions.no_approval {
            match Config::save_no_approval(&self.root, no_approval) {
                Ok(()) => {
                    self.config.codex.no_approval = no_approval;
                    self.config_error.clear();
                }
                Err(error) => self.config_error = error,
            }
        }
        if actions.clock_auto_changed {
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
        if let Some(action) = actions.clock_action {
            self.clock_action(action);
        }
        if let Some(connection) = actions.connection {
            match Config::save_connection(&self.root, &connection) {
                Ok(()) => {
                    self.reload_config();
                    self.connection_draft = Some(self.local_codex_config.draft(&self.config.codex));
                    self.connection_error.clear();
                }
                Err(error) => self.connection_error = error,
            }
        }
        if let Some(action) = actions.weather {
            self.weather_action(action, ctx);
        }
        if let Some((character, card)) = actions.role_card {
            self.save_role_card(&character, &card);
        }
        if actions.reload {
            self.refresh(ctx);
        }
        if actions.restart_codex {
            self.reload_config();
            self.start_codex(ctx);
        }
    }
    fn settings_contents(&mut self, ui: &mut egui::Ui) -> SettingsActions {
        let ctx = ui.ctx().clone();
        let mut restart_codex = false;
        let mut reload = false;
        let mut no_approval = self.config.codex.no_approval;
        let mut approval_changed = false;
        let mut clock_auto_changed = false;
        let mut clock_action = None;
        let mut connection = None;
        let mut weather = None;
        let mut role_card = None;
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
                    for (page, title) in [(0, "常规"), (1, "连接"), (2, "打卡"), (3, "闹钟"), (4, "天气"), (5, "角色卡")] {
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
                        .drag_to_scroll(false)
                        .max_height(326.)
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.add_space(2.);
                            ui.label(
                                egui::RichText::new(match self.settings_page { 1 => "连接", 2 => "打卡", 3 => "闹钟", 4 => "天气", 5 => "角色卡", _ => "常规" })
                                .size(20.)
                                .strong(),
                            );
                            ui.add_space(6.);
                            if self.settings_page == 5 {
                                role_card = self.role_card_ui(ui);
                            } else if self.settings_page == 4 {
                                weather = self.weather_settings_ui(ui);
                            } else if self.settings_page == 3 {
                                self.alarm_settings_ui(ui);
                            } else if self.settings_page == 2 {
                                (clock_auto_changed, clock_action) = self.clock_settings_ui(ui);
                            } else if self.settings_page == 1 {
                                if self.connection_draft.is_none() {
                                    if let Some(home) = &self.local_codex_home {
                                        match LocalCodexConfig::read(home) {
                                            Ok(local) => self.local_codex_config = local,
                                            Err(error) => self.connection_error = error,
                                        }
                                    }
                                    self.connection_draft = Some(self.local_codex_config.draft(&self.config.codex));
                                }
                                if ui.small_button("读取本机配置")
                                    .on_hover_text("读取本机 Codex 的模型、API 地址及 Key；点击保存后才应用自定义 API。")
                                    .clicked()
                                    && let Some(home) = &self.local_codex_home
                                {
                                    match LocalCodexConfig::read(home) {
                                        Ok(local) => {
                                            self.local_codex_config = local;
                                            // Explicitly importing prepares the fields without changing the active connection.
                                            let custom_api = self.connection_draft.as_ref()
                                                .is_some_and(|draft| draft.use_custom_api);
                                            let mut saved = self.config.codex.clone();
                                            saved.use_custom_api = false;
                                            saved.model.clear();
                                            let mut draft = self.local_codex_config.draft(&saved);
                                            draft.use_custom_api = custom_api;
                                            self.connection_draft = Some(draft);
                                            self.connection_error.clear();
                                        }
                                        Err(error) => self.connection_error = error,
                                    }
                                }
                                let draft = self.connection_draft
                                    .get_or_insert_with(|| self.config.codex.clone());
                                egui::Frame::group(ui.style())
                                    .fill(egui::Color32::WHITE)
                                    .inner_margin(12)
                                    .corner_radius(8)
                                    .show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.strong("模型与 API");
                                        if !draft.use_custom_api {
                                            ui.weak("沿用本机 Codex")
                                                .on_hover_text("显示 CODEX_HOME（默认 ~/.codex）中的用户配置；未设置模型时由 Codex 决定。保存时保留继承方式。");
                                        }
                                        ui.label("模型");
                                        ui.add(egui::TextEdit::singleline(&mut draft.model)
                                            .id(egui::Id::new("connection-model"))
                                            .desired_width(f32::INFINITY)
                                            .hint_text("留空沿用本机配置"));
                                        ui.checkbox(&mut draft.use_custom_api, "自定义 API")
                                            .on_hover_text("兼容 OpenAI Responses API；关闭时使用本机 Codex 服务。更改后点击保存。");
                                        ui.add_enabled_ui(draft.use_custom_api, |ui| {
                                            ui.label("API 地址");
                                            ui.add(egui::TextEdit::singleline(&mut draft.base_url)
                                                .id(egui::Id::new("connection-url"))
                                                .desired_width(f32::INFINITY)
                                                .hint_text("https://api.openai.com/v1"))
                                                .on_hover_text("填写基础地址（通常以 /v1 结尾），不要填写 /responses。");
                                        });
                                        let mut reveal_key = false;
                                        ui.horizontal(|ui| {
                                            ui.label("API Key");
                                            let reveal = ui.add(egui::Button::new("按住显示")
                                                .sense(egui::Sense::click_and_drag()))
                                                .on_hover_text("按住暂时显示密钥，松开或窗口失焦后恢复隐藏。");
                                            reveal_key = reveal.is_pointer_button_down_on()
                                                && ui.input(|i| i.focused && i.pointer.primary_down());
                                        });
                                        ui.add_enabled(draft.use_custom_api,
                                            egui::TextEdit::singleline(&mut draft.api_key)
                                                .id(egui::Id::new("connection-key"))
                                                .password(!reveal_key)
                                                .hint_text("未读取到 API Key")
                                                .desired_width(f32::INFINITY))
                                                .on_hover_text("开启自定义 API 时必须填写；保存后仅用于 Lexington。");
                                        ui.horizontal(|ui| {
                                            if ui.button("保存").clicked() {
                                                connection = Some(self.local_codex_config.to_save(draft, &self.config.codex));
                                            }
                                            if ui.button("取消修改").clicked() {
                                                *draft = self.local_codex_config.draft(&self.config.codex);
                                                self.connection_error.clear();
                                            }
                                        });
                                        if !self.connection_error.is_empty() {
                                            ui.colored_label(egui::Color32::from_rgb(180, 45, 45), &self.connection_error);
                                        }
                                    });
                                ui.add_space(4.);
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
        SettingsActions {
            reload,
            restart_codex,
            no_approval: approval_changed.then_some(no_approval),
            clock_auto_changed,
            clock_action,
            connection,
            weather,
            role_card,
        }
    }
}
