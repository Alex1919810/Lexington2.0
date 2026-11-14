use super::{app::Wife, clock::ClockAction};
use crate::backend::config::Config;
use eframe::egui::{self, vec2};
use std::time::{Duration, Instant};

#[derive(Default)]
struct SettingsActions {
    reload: bool,
    restart_codex: bool,
    no_approval: Option<bool>,
    clock_auto_changed: bool,
    clock_action: Option<ClockAction>,
    development_mode: Option<bool>,
    weather: Option<super::weather::WeatherAction>,
    role_card: Option<(String, crate::roleplay::RoleCard)>,
    connection: Option<super::connection::ConnectionAction>,
}

impl Wife {
    pub(super) fn settings_ui(&mut self, ctx: &egui::Context) {
        if !self.settings {
            self.role_editor = Default::default();
            self.connection_editor = Default::default();
            self.settings_viewport_open = false;
            self.settings_drag = None;
            self.settings_pointer_down = false;
            self.settings_drag_active = false;
            return;
        }
        let id = egui::ViewportId::from_hash_of("lexington-settings");
        let size = vec2(720., 560.);
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
                    .fixed_size(vec2(688., 494.))
                    .default_pos(settings.screen_rect().center() - size / 2.)
                    .frame(
                        egui::Frame::window(&settings.style())
                            .fill(super::style::PANEL_BG)
                            .inner_margin(16)
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
                            .fill(super::style::PANEL_BG)
                            .inner_margin(16),
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
                                "Lexington  /  设置",
                                egui::FontId::proportional(16.),
                                super::style::INK,
                            );
                            title_rect = rect;
                            title_drag_started = response.drag_started();
                            if ui.add_sized([28., 28.], egui::Button::new("×")).clicked() {
                                open = false;
                            }
                        });
                        ui.add_space(16.);
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
            self.connection_editor = Default::default();
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
        if let Some(enabled) = actions.development_mode {
            match Config::save_development_mode(&self.root, enabled) {
                Ok(()) => self.reload_config(),
                Err(error) => self.config_error = error,
            }
        }
        if let Some(action) = actions.weather {
            self.weather_action(action, ctx);
        }
        if let Some(action) = actions.connection {
            self.apply_connection_action(action);
        }
        if let Some((character, card)) = actions.role_card {
            self.save_role_card(&character, &card);
        }
        self.apply_profile_action();
        self.apply_new_character_action(ctx);
        self.apply_vector_action();
        if actions.reload {
            self.refresh(ctx);
        }
        if actions.restart_codex {
            self.reload_config();
            self.new_development_task(ctx);
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
        let mut development_mode = self.config.codex.development_mode;
        let mut development_mode_changed = false;
        let mut weather = None;
        let mut role_card = None;
        let mut connection = None;
        super::style::panel_style(ui);
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                vec2(126., ui.available_height()),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.visuals_mut().widgets.inactive.weak_bg_fill = egui::Color32::TRANSPARENT;
                    for (page, title) in [
                        (0, "常规"),
                        (1, "连接"),
                        (2, "打卡"),
                        (3, "闹钟"),
                        (4, "天气"),
                        (5, "角色卡"),
                    ] {
                        if ui
                            .add_sized(
                                [126., 40.],
                                egui::Button::new(egui::RichText::new(title).color(
                                    if self.settings_page == page {
                                        super::style::ACCENT
                                    } else {
                                        super::style::MUTED
                                    },
                                ))
                                .selected(self.settings_page == page)
                                .stroke(egui::Stroke::NONE),
                            )
                            .clicked()
                        {
                            self.settings_page = page;
                        }
                    }
                    ui.add_space(14.);
                    ui.weak("LEXINGTON");
                    ui.small("日常相伴 · 随心设定");
                },
            );
            ui.separator();
            ui.allocate_ui_with_layout(
                vec2(ui.available_width(), ui.available_height()),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt(("settings-content", self.settings_page))
                        .drag_to_scroll(false)
                        .max_height(ui.available_height())
                        .min_scrolled_height(0.)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            let (title, subtitle) = match self.settings_page {
                                1 => ("连接", "查看连接状态，管理当前会话与开发模式"),
                                2 => ("打卡", "记录每一天的开始，安排自己的节奏"),
                                3 => ("闹钟", "为重要时刻留一份提醒"),
                                4 => ("天气", "选择城市，让日常问候多一点关心"),
                                5 => ("角色卡", "编辑角色设定，定制你们的相处方式"),
                                _ => ("常规", "让 Lexington 更贴合你的使用习惯"),
                            };
                            ui.heading(title);
                            ui.weak(subtitle);
                            ui.add_space(6.);
                            let content_frame = if self.settings_page == 0 {
                                egui::Frame::new()
                            } else {
                                super::style::card()
                            };
                            content_frame.show(ui, |ui| {
                                ui.set_min_width(ui.available_width());
                                if self.settings_page == 5 {
                                    role_card = self.role_card_ui(ui);
                                } else if self.settings_page == 4 {
                                    weather = self.weather_settings_ui(ui);
                                } else if self.settings_page == 3 {
                                    self.alarm_settings_ui(ui);
                                } else if self.settings_page == 2 {
                                    (clock_auto_changed, clock_action) = self.clock_settings_ui(ui);
                                } else if self.settings_page == 1 {
                                    connection = self.connection_settings_ui(ui);
                                    ui.separator();
                                    development_mode_changed = ui.checkbox(&mut development_mode, "development 模式").changed();
                                    ui.small("开启后注入代码开发指令。切换将在当前任务结束后新建会话。");
                                    if ui.button("打开 development").clicked() {
                                        self.development.open = true;
                                    }
                                    ui.add_space(4.);
                                    super::style::card()
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
                                            let directory = self.development.book.active.clone().unwrap_or_else(||self.root.clone());
                                            for (title, value) in [
                                                ("程序", std::env::var("LEXINGTON_CODEX_EXECUTABLE").unwrap_or_else(|_|"codex（继承环境）".into())),
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
                                    super::style::card()
                                        .show(ui, |ui| {
                                            ui.set_width(ui.available_width());
                                            ui.strong("操作审批");
                                            ui.weak("选择发送消息时的审批方式");
                                            approval_changed = ui
                                                .checkbox(&mut no_approval, "无需审批")
                                                .on_hover_text("下次发送生效；关闭时恢复本机审批策略。沙箱权限不变。")
                                                .changed();
                                        });
                                    super::style::card()
                                        .show(ui, |ui| {
                                            ui.set_width(ui.available_width());
                                            ui.strong("资源与配置");
                                            ui.weak("管理本地资源与个性化设置");
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
                            });
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
            development_mode: development_mode_changed.then_some(development_mode),
            weather,
            role_card,
            connection,
        }
    }
}
