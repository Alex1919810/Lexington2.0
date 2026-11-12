//! Interactive client commands, drawn in the viewport that submitted them.
use super::app::Wife;
use crate::backend::{commands::Panel, config::Config};
use eframe::egui;

pub(super) struct CommandUi {
    pub panel: Panel,
    pub development: bool,
    pub pending: bool,
    pub error: String,
    no_approval: bool,
}

pub(super) enum Action {
    Model(String, String),
    Permissions(bool),
}

impl CommandUi {
    pub fn new(mut panel: Panel, development: bool, no_approval: bool) -> Self {
        if let Panel::Model {
            current,
            effort,
            models,
        } = &mut panel
            && let Some(model) = models.iter().find(|model| model.model == *current)
            && !model
                .supported_reasoning_efforts
                .iter()
                .any(|option| option.reasoning_effort == *effort)
        {
            *effort = model.default_reasoning_effort.clone();
        }
        Self {
            panel,
            development,
            pending: false,
            error: String::new(),
            no_approval,
        }
    }
}

impl Wife {
    pub(super) fn command_panel_ui(
        &mut self,
        ctx: &egui::Context,
        development: bool,
        embedded: bool,
    ) -> Option<Action> {
        let state = self
            .command_ui
            .as_mut()
            .filter(|state| state.development == development)?;
        let mut open = true;
        let mut cancel = false;
        let mut action = None;
        let mut popup_regions = Vec::new();
        let model_panel = matches!(state.panel, Panel::Model { .. });
        let window = egui::Window::new(if model_panel { "模型与推理强度" } else { "权限与审批" })
            .id(egui::Id::new("client-command-panel"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(350.)
            .max_width((ctx.screen_rect().width() - 48.).max(180.))
            .default_pos(ctx.screen_rect().center() - egui::vec2(190., 140.))
            .show(ctx, |ui| {
                super::style::panel_style(ui);
                egui::ScrollArea::vertical()
                    .id_salt("command-options")
                    .max_height((ctx.screen_rect().height() - 160.).max(80.))
                    .show(ui, |ui| {
                        ui.add_enabled_ui(!self.busy && !state.pending, |ui| {
                            let mut valid = true;
                            match &mut state.panel {
                                Panel::Model { current, effort, models } => {
                                    let previous = current.clone();
                                    ui.label("模型");
                                    let label = models.iter().find(|model| model.model == *current).map(|model| model.display_name.as_str()).unwrap_or(current);
                                    egui::ComboBox::from_id_salt("command-model")
                                        .width(300.)
                                        .selected_text(if label.is_empty() { "请选择模型" } else { label })
                                        .show_ui(ui, |ui| {
                                            for model in models.iter() {
                                                ui.selectable_value(current, model.model.clone(), &model.display_name).on_hover_text(&model.description);
                                            }
                                            popup_regions.push(ui.min_rect() + ui.spacing().menu_margin);
                                        });
                                    if let Some(model) = models.iter().find(|model| model.model == *current) {
                                        if previous != *current {
                                            *effort = model.default_reasoning_effort.clone();
                                        }
                                        ui.label("推理强度");
                                        egui::ComboBox::from_id_salt("command-effort")
                                            .width(300.)
                                            .selected_text(effort.as_str())
                                            .show_ui(ui, |ui| {
                                                for option in &model.supported_reasoning_efforts {
                                                    ui.selectable_value(effort, option.reasoning_effort.clone(), &option.reasoning_effort).on_hover_text(&option.description);
                                                }
                                                popup_regions.push(ui.min_rect() + ui.spacing().menu_margin);
                                            });
                                        valid = model.supported_reasoning_efforts.iter().any(|option| option.reasoning_effort == *effort);
                                    } else {
                                        valid = false;
                                        ui.weak("当前模型未出现在列表中，请选择可用模型。");
                                    }
                                    ui.small("仅当前会话使用，从下一轮生效，不修改本机 Codex 配置。");
                                }
                                Panel::Permissions(summary) => {
                                    ui.label(summary.as_str());
                                    ui.separator();
                                    ui.checkbox(&mut state.no_approval, "无需审批");
                                    ui.small("关闭时使用本机 Codex 的审批策略。从下一轮生效，沙箱权限保持本机配置。");
                                }
                            }
                            ui.add_space(8.);
                            ui.horizontal(|ui| {
                                if ui.add_enabled(valid, egui::Button::new("应用")).clicked() {
                                    action = Some(match &state.panel {
                                        Panel::Model { current, effort, .. } => Action::Model(current.clone(), effort.clone()),
                                        Panel::Permissions(_) => Action::Permissions(state.no_approval),
                                    });
                                }
                                if ui.button("取消").clicked() { cancel = true; }
                            });
                        });
                        if state.pending { ui.label("正在应用模型选择…"); }
                        if self.busy { ui.weak("请等待当前任务结束。"); }
                        if !state.error.is_empty() { ui.colored_label(egui::Color32::DARK_RED, &state.error); }
                    });
            });
        if (!development || embedded)
            && open
            && let Some(window) = window
        {
            self.input_regions.push(window.response.rect);
            self.input_regions.extend(popup_regions);
        }
        if !open || cancel {
            self.command_ui = None;
        }
        action
    }

    pub(super) fn apply_command_panel(&mut self, action: Action) {
        if self.busy {
            return;
        }
        let Some(state) = self.command_ui.as_mut() else {
            return;
        };
        match action {
            Action::Model(model, effort) => {
                match self
                    .codex
                    .as_ref()
                    .ok_or_else(|| "Codex 未连接，请重启会话。".to_owned())
                    .and_then(|client| client.select_model(model, effort))
                {
                    Ok(()) => {
                        state.pending = true;
                        state.error.clear();
                    }
                    Err(error) => state.error = error,
                }
            }
            Action::Permissions(no_approval) => {
                match Config::save_no_approval(&self.root, no_approval) {
                    Ok(()) => {
                        self.config.codex.no_approval = no_approval;
                        self.codex_status = "审批设置已保存，从下一轮生效".into();
                        self.command_ui = None;
                    }
                    Err(error) => state.error = error,
                }
            }
        }
    }
}
