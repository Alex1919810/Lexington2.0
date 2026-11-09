use super::app::Wife;
use crate::roleplay::{ContextPlan, SceneState};
use eframe::egui;

#[derive(Default)]
pub(super) struct ContextUi {
    pub last_input: String,
    pub last: Option<ContextPlan>,
    pub preview: Option<ContextPlan>,
    pub preview_input: String,
    pub scene: SceneState,
    pub notice: String,
}

impl Wife {
    pub(super) fn role_context_ui(&mut self, ui: &mut egui::Ui) {
        ui.label(format!("运行角色：{}", self.library.character));
        ui.small(
            "仅显示本软件附加的角色内容；Codex 完整历史、基础指令和实际 token 用量由 Codex 管理。",
        );
        ui.collapsing("临时场景（新会话清空）", |ui| {
            for (title, value) in [
                ("当前活动", &mut self.context_ui.scene.activity),
                ("表达风格", &mut self.context_ui.scene.style),
                ("尚待完成", &mut self.context_ui.scene.next_step),
            ] {
                ui.label(title);
                ui.text_edit_singleline(value);
            }
            if ui
                .add_enabled(self.codex.is_some(), egui::Button::new("应用临时场景"))
                .clicked()
            {
                self.context_ui.notice = self
                    .codex
                    .as_ref()
                    .unwrap()
                    .set_scene(self.context_ui.scene.clone())
                    .map_or_else(|error| error, |_| "临时场景将在下一轮使用。".into());
            }
            if ui.button("清空临时场景").clicked() {
                self.context_ui.scene = SceneState::default();
                if let Some(client) = &self.codex {
                    self.context_ui.notice = client
                        .set_scene(SceneState::default())
                        .map_or_else(|error| error, |_| "临时场景已清空。".into());
                }
            }
        });
        ui.label("预览下一轮输入");
        ui.add(
            egui::TextEdit::multiline(&mut self.context_ui.preview_input)
                .desired_rows(2)
                .desired_width(f32::INFINITY),
        );
        if ui
            .add_enabled(
                self.codex.is_some(),
                egui::Button::new("预览上下文（不调用模型）"),
            )
            .clicked()
        {
            match self
                .codex
                .as_ref()
                .unwrap()
                .preview_context(self.context_ui.preview_input.clone())
            {
                Ok(()) => {
                    self.context_ui.notice =
                        "正在按已完成的历史预览；预览不会推进轮次或持续效果。".into()
                }
                Err(error) => self.context_ui.notice = error,
            }
        }
        if !self.context_ui.notice.is_empty() {
            ui.label(&self.context_ui.notice);
        }
        for (title, report) in [
            ("上次实际发送", &self.context_ui.last),
            ("预览结果", &self.context_ui.preview),
        ] {
            ui.collapsing(title, |ui| {
                let Some(report) = report else {
                    ui.label("尚无记录。");
                    return;
                };
                ui.label(format!(
                    "第 {} 轮 · 静态 {}/{} 字符 · 本轮 {}/{} 字符",
                    report.turn,
                    report.static_used,
                    report.static_budget,
                    report.turn_used,
                    report.turn_budget
                ));
                if report.static_used > report.static_budget {
                    ui.colored_label(
                        egui::Color32::DARK_RED,
                        "核心设定超过预算，已完整保留；请精简核心设定。",
                    );
                }
                ui.small(
                    "静态块在新会话时发送一次；本轮块随消息附加。历史中的旧背景不会自动删除。",
                );
                for block in &report.blocks {
                    ui.push_id(&block.id, |ui| {
                        ui.collapsing(
                            format!(
                                "{} {} · {} · {} 字符",
                                if block.included { "✓" } else { "−" },
                                block.title,
                                block.category,
                                block.cost
                            ),
                            |ui| {
                                ui.label(&block.reason);
                                ui.label(if block.persistent {
                                    "作用域：本会话静态设定"
                                } else {
                                    "作用域：本轮"
                                });
                                ui.add(egui::Label::new(&block.content).selectable(true).wrap());
                            },
                        );
                    });
                }
            });
        }
    }
}
