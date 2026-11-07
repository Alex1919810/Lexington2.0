use super::app::Pet;
use crate::{
    backend::{
        alarm::AlarmState,
        clock::{self, local_time},
    },
    tools::paths,
};
use eframe::egui::{self, vec2};
use std::time::{Duration, Instant};

pub(super) struct AlarmUi {
    pub state: AlarmState,
    pub error: String,
    available: bool,
    editing: Option<u64>,
    time: String,
    message: String,
    daily: bool,
    last_poll: Instant,
    popup_open: bool,
    popup_started: Instant,
    popup_raised: bool,
}
impl Default for AlarmUi {
    fn default() -> Self {
        Self {
            state: AlarmState::default(),
            error: String::new(),
            available: true,
            editing: None,
            time: "09:00".into(),
            message: String::new(),
            daily: false,
            last_poll: Instant::now() - Duration::from_secs(2),
            popup_open: false,
            popup_started: Instant::now(),
            popup_raised: false,
        }
    }
}
impl Pet {
    pub(super) fn load_alarms(&mut self) {
        match AlarmState::load(paths::alarm_state_file(self.dev)) {
            Ok(state) => self.alarm.state = state,
            Err(error) => {
                self.alarm.error = error;
                self.alarm.available = false;
            }
        }
    }
    pub(super) fn alarm_settings_ui(&mut self, ui: &mut egui::Ui) {
        let mut result = None;
        ui.add_enabled_ui(self.alarm.available, |ui| {
            ui.horizontal(|ui| {
                ui.label("时间");
                ui.add(
                    egui::TextEdit::singleline(&mut self.alarm.time)
                        .desired_width(64.)
                        .char_limit(5)
                        .hint_text("HH:MM"),
                );
                ui.checkbox(&mut self.alarm.daily, "每天重复");
            });
            ui.add(
                egui::TextEdit::multiline(&mut self.alarm.message)
                    .hint_text("提示语")
                    .desired_rows(2)
                    .desired_width(f32::INFINITY)
                    .char_limit(500),
            );
            ui.horizontal(|ui| {
                if ui
                    .button(if self.alarm.editing.is_some() {
                        "保存闹钟"
                    } else {
                        "添加闹钟"
                    })
                    .clicked()
                {
                    result = Some(self.alarm.state.save(
                        self.alarm.editing,
                        &self.alarm.time,
                        &self.alarm.message,
                        self.alarm.daily,
                        clock::now(),
                    ));
                    if result.as_ref().is_some_and(Result::is_ok) {
                        self.alarm.editing = None;
                        self.alarm.message.clear();
                    }
                }
                if self.alarm.editing.is_some() && ui.button("取消编辑").clicked() {
                    self.alarm.editing = None;
                    self.alarm.message.clear();
                }
            });
            ui.separator();
            let mut toggle = None;
            let mut delete = None;
            for alarm in self.alarm.state.alarms() {
                ui.push_id(alarm.id, |ui| {
                    egui::Frame::group(ui.style())
                        .fill(egui::Color32::WHITE)
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                let mut enabled = alarm.enabled;
                                if ui.checkbox(&mut enabled, &alarm.time).changed() {
                                    toggle = Some(alarm.id);
                                }
                                ui.weak(if alarm.daily { "每天" } else { "一次" });
                                if ui.small_button("编辑").clicked() {
                                    self.alarm.editing = Some(alarm.id);
                                    self.alarm.time = alarm.time.clone();
                                    self.alarm.message = alarm.message.clone();
                                    self.alarm.daily = alarm.daily;
                                }
                                if ui.small_button("删除").clicked() {
                                    delete = Some(alarm.id);
                                }
                            });
                            ui.add(egui::Label::new(&alarm.message).wrap());
                            if alarm.enabled
                                && let Some(next) = local_time(alarm.next_due)
                            {
                                ui.weak(format!("下次 {} {}", next.day, next.label()));
                            }
                        });
                });
            }
            if let Some(id) = toggle {
                result = Some(self.alarm.state.toggle(id, clock::now()));
            }
            if let Some(id) = delete {
                result = Some(self.alarm.state.delete(id));
                if result.as_ref().is_some_and(Result::is_ok) && self.alarm.editing == Some(id) {
                    self.alarm.editing = None;
                    self.alarm.message.clear();
                }
            }
            if self.alarm.state.alarms().is_empty() {
                ui.weak("暂无闹钟");
            }
        });
        if let Some(result) = result {
            self.alarm.error = result.err().unwrap_or_default();
        }
        if !self.alarm.error.is_empty() {
            ui.colored_label(egui::Color32::from_rgb(180, 45, 45), &self.alarm.error);
        }
    }
    pub(super) fn alarm_popup_ui(&mut self, ctx: &egui::Context) {
        if self.alarm.available && self.alarm.last_poll.elapsed() >= Duration::from_secs(1) {
            self.alarm.last_poll = Instant::now();
            if let Err(error) = self.alarm.state.poll(clock::now()) {
                self.alarm.error = error;
            }
        }
        if self.alarm.state.pending().is_empty() {
            self.alarm.popup_open = false;
            return;
        }
        let size = vec2(420., 260.);
        let monitor_size = ctx
            .input(|i| i.viewport().monitor_size)
            .unwrap_or(vec2(1920., 1080.));
        let mut monitor = egui::Rect::from_min_size(egui::Pos2::ZERO, monitor_size);
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
        let id = egui::ViewportId::from_hash_of("lexington-alarm");
        let mut builder = egui::ViewportBuilder::default()
            .with_title("Lexington 闹钟")
            .with_inner_size(size)
            .with_resizable(false)
            .with_decorations(false)
            .with_transparent(false)
            .with_always_on_top()
            .with_taskbar(false)
            // A normal window, even AlwaysOnTop, can sit below the pet's Dock
            // layer on X11. Use the same layer for this independent popup.
            .with_window_type(egui::X11WindowType::Dock);
        if !self.alarm.popup_open {
            builder = builder.with_position(monitor.center() - size / 2.);
        }
        if !self.alarm.popup_open {
            self.alarm.popup_started = Instant::now();
            self.alarm.popup_raised = false;
        }
        self.alarm.popup_open = true;
        let mut dismiss = false;
        ctx.show_viewport_immediate(id, builder, |popup, class| {
            if class == egui::ViewportClass::Embedded {
                // Test/non-native integrations use a regular egui window.
                if let Some(window) = egui::Window::new("Lexington 闹钟")
                    .order(egui::Order::Foreground)
                    .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                    .collapsible(false)
                    .resizable(false)
                    .show(popup, |ui| {
                        for reminder in self.alarm.state.pending() {
                            ui.label(&reminder.message);
                        }
                        dismiss = ui.button("知道了").clicked();
                    })
                {
                    self.input_regions.push(window.response.rect);
                }
            } else {
                // Activation before mapping can be ignored, and root startup
                // also raises the pet. Activate once after both are mapped.
                if !self.alarm.popup_raised
                    && self.alarm.popup_started.elapsed() >= Duration::from_millis(800)
                {
                    popup.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
                        egui::WindowLevel::AlwaysOnTop,
                    ));
                    popup.send_viewport_cmd(egui::ViewportCommand::Focus);
                    self.alarm.popup_raised = true;
                }
                dismiss |= popup.input(|i| i.viewport().close_requested());
                egui::CentralPanel::default()
                    .frame(
                        egui::Frame::central_panel(&popup.style())
                            .fill(egui::Color32::from_rgb(247, 248, 250))
                            .inner_margin(20),
                    )
                    .show(popup, |ui| {
                        ui.heading("闹钟");
                        ui.add_space(10.);
                        egui::ScrollArea::vertical()
                            .max_height(132.)
                            .min_scrolled_height(132.)
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.set_min_height(132.);
                                for reminder in self.alarm.state.pending() {
                                    ui.weak(&reminder.time);
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(&reminder.message).size(20.),
                                        )
                                        .wrap(),
                                    );
                                    ui.add_space(8.);
                                }
                            });
                        ui.add_space(10.);
                        dismiss |= ui
                            .add_sized([ui.available_width(), 30.], egui::Button::new("知道了"))
                            .clicked();
                        if !self.alarm.error.is_empty() {
                            ui.colored_label(egui::Color32::RED, &self.alarm.error);
                        }
                    });
            }
        });
        if dismiss {
            match self.alarm.state.dismiss() {
                Ok(()) => {
                    self.alarm.popup_open = false;
                    self.alarm.error.clear();
                    ctx.send_viewport_cmd_to(id, egui::ViewportCommand::Close);
                }
                Err(error) => {
                    self.alarm.error = error;
                    ctx.send_viewport_cmd_to(id, egui::ViewportCommand::CancelClose);
                }
            }
        }
    }
}
