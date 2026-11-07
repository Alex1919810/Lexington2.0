use super::app::Pet;
use crate::backend::clock::{self, local_time};
use eframe::egui;
use std::time::{Duration, Instant};

pub(super) enum ClockAction {
    Edit,
    Undo,
    Manual,
}

impl Pet {
    pub(super) fn poll_clock(&mut self) {
        let now = clock::now();
        for sample in self
            .activity_monitor
            .as_ref()
            .map(|m| m.poll())
            .unwrap_or_default()
        {
            self.activity_status = sample.status;
            if self.clock_storage_ok {
                let idle = if sample.sampled.elapsed() <= Duration::from_secs(5) {
                    sample.idle
                } else {
                    None
                };
                match self.clock_state.observe(
                    &self.config.clock_in,
                    now - sample.sampled.elapsed().as_secs() as i64,
                    sample.sampled,
                    idle,
                ) {
                    Ok(true) => {
                        self.clock_notice = true;
                        self.clock_error.clear();
                        let time = self
                            .clock_state
                            .started()
                            .and_then(local_time)
                            .map(|t| t.label())
                            .unwrap_or_default();
                        let message = format!("已记录今天 {time} 开始工作");
                        if !self.busy {
                            self.say(message.clone());
                        }
                        self.queue_clock_weather(message);
                    }
                    Ok(false) => {}
                    Err(error) => self.clock_error = error,
                }
            }
        }
        if self.clock_storage_ok && self.last_clock_poll.elapsed() >= Duration::from_secs(1) {
            self.last_clock_poll = Instant::now();
            match self.clock_state.poll(&self.config.clock_in, now) {
                Ok(Some(message)) => self.say(message),
                Ok(None) => {}
                Err(error) => self.clock_error = error,
            }
        }
        if self.clock_state.started().is_none() {
            self.clock_notice = false;
        }
    }
    pub(super) fn clock_settings_ui(&mut self, ui: &mut egui::Ui) -> (bool, Option<ClockAction>) {
        let mut auto = self.config.clock_in.auto_start;
        let changed = ui.add_enabled(self.clock_storage_ok, egui::Checkbox::new(&mut auto, "自动开始工作计时"))
            .on_hover_text(format!("解锁或恢复操作后持续活跃 {} 秒，起始时间回溯到首次操作；闲置满 1 分钟、锁屏或检测中断会重新确认。", self.config.clock_in.confirmation_seconds)).changed();
        ui.add(egui::Label::new(&self.activity_status).wrap());
        ui.separator();
        let start = self
            .clock_state
            .started()
            .and_then(local_time)
            .map(|t| t.label());
        ui.label(
            start
                .as_ref()
                .map_or_else(|| "今天尚未记录".into(), |t| format!("今天 {t} 开始")),
        );
        let mut action = None;
        ui.add_enabled_ui(self.clock_storage_ok, |ui| {
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.clock_edit)
                        .hint_text("HH:MM")
                        .desired_width(64.)
                        .char_limit(5),
                );
                if ui.button("保存时间").clicked() {
                    action = Some(ClockAction::Edit);
                }
            });
            ui.horizontal(|ui| {
                if ui
                    .button(if self.clock_state.running() {
                        "取消打卡"
                    } else {
                        "现在打卡"
                    })
                    .clicked()
                {
                    action = Some(ClockAction::Manual);
                }
                if ui
                    .add_enabled(start.is_some(), egui::Button::new("撤销今日记录"))
                    .on_hover_text("今天不再自动打卡，仍可手动开始")
                    .clicked()
                {
                    action = Some(ClockAction::Undo);
                }
            });
        });
        if !self.clock_error.is_empty() {
            ui.colored_label(egui::Color32::from_rgb(180, 45, 45), &self.clock_error);
        }
        (changed, action)
    }
    pub(super) fn clock_action(&mut self, action: ClockAction) {
        if !self.clock_storage_ok {
            return;
        }
        let result = match action {
            ClockAction::Edit => self
                .clock_state
                .edit(&self.clock_edit, clock::now())
                .map(|_| "起始时间已修改"),
            ClockAction::Undo => self
                .clock_state
                .undo(clock::now())
                .map(|_| "已撤销，今天不再自动打卡"),
            ClockAction::Manual => {
                self.clock();
                return;
            }
        };
        match result {
            Ok(message) => {
                self.clock_error.clear();
                self.clock_notice = false;
                self.say(message.into());
            }
            Err(error) => self.clock_error = error,
        }
    }
    pub(super) fn clock_notice_ui(&mut self, ctx: &egui::Context) {
        let mut modify = false;
        let mut undo = false;
        let mut dismiss = false;
        if let Some(window) = egui::Window::new("已自动打卡")
            .open(&mut self.clock_notice)
            .order(egui::Order::Foreground)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                let time = self
                    .clock_state
                    .started()
                    .and_then(local_time)
                    .map(|t| t.label())
                    .unwrap_or_default();
                ui.label(format!("今天 {time} 开始工作"));
                ui.horizontal(|ui| {
                    modify = ui.button("修改时间").clicked();
                    undo = ui.button("撤销").clicked();
                    dismiss = ui.button("知道了").clicked();
                });
            })
            && self.clock_notice
            && !modify
            && !undo
            && !dismiss
        {
            self.input_regions.push(window.response.rect);
        }
        if modify {
            self.clock_edit = self
                .clock_state
                .started()
                .and_then(local_time)
                .map(|t| t.label())
                .unwrap_or_default();
            self.settings_page = 2;
            self.settings = true;
        }
        if modify || undo || dismiss {
            self.clock_notice = false;
        }
        if undo {
            self.clock_action(ClockAction::Undo);
        }
    }
}
