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
    pub sent_manifest: Option<crate::development::context::Manifest>,
    pub preview_manifest: Option<crate::development::context::Manifest>,
}

impl Wife {
    pub(super) fn role_context_ui(&mut self, ui: &mut egui::Ui) {
        ui.label(format!("运行角色：{}", self.library.character));
        ui.small(
            "发送清单覆盖 Lexington 提供的输入与背景；Codex 完整历史、基础指令和实际 token 用量由 Codex 管理。",
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
            match self.codex.as_ref().unwrap().preview_context(
                self.context_ui.preview_input.clone(),
                crate::development::context::project_manifest(
                    self.development.book.current(),
                    self.config.codex.development_mode,
                    &self.library.character,
                ),
            ) {
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
        for (title, manifest) in [
            ("上次发送清单", &self.context_ui.sent_manifest),
            ("预览发送清单", &self.context_ui.preview_manifest),
        ] {
            ui.collapsing(title, |ui| {
                if let Some(manifest) = manifest {
                    manifest_ui(ui, manifest);
                } else {
                    ui.label("尚无记录。");
                }
            });
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

pub(super) fn manifest_ui(ui: &mut egui::Ui, manifest: &crate::development::context::Manifest) {
    ui.add(
        egui::Label::new(format!(
            "角色：{} · 项目：{}",
            manifest.character, manifest.workspace
        ))
        .wrap(),
    );
    ui.small(format!(
        "{} · {}",
        if manifest.preview {
            "预览，未发送"
        } else {
            "已写入 app-server 请求，不代表模型已完成处理"
        },
        super::development::timestamp_text(manifest.captured_at),
    ));
    for block in &manifest.blocks {
        ui.push_id(&block.id, |ui| {
            let title = format!(
                "{} · {} · {} 字符",
                block.delivery.label(),
                block.title,
                block.content.chars().count()
            );
            let header = egui::WidgetText::from(title.as_str()).into_galley(
                ui,
                Some(egui::TextWrapMode::Truncate),
                (ui.available_width() - ui.spacing().indent - ui.spacing().button_padding.x)
                    .max(0.),
                egui::TextStyle::Button,
            );
            egui::CollapsingHeader::new(header).show(ui, |ui| {
                ui.add(
                    egui::Label::new(format!(
                        "{} · {}\n来源：{}\n版本：{}\n{}",
                        block.category, block.scope, block.source, block.revision, block.validity
                    ))
                    .wrap(),
                );
                super::development::code_text(ui, &block.content, false);
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::development::context::{Block, Manifest};

    #[test]
    fn manifest_body_remains_readable_inside_outer_scrolling_content() {
        let ctx = egui::Context::default();
        ctx.memory_mut(|memory| memory.set_everything_is_visible(true));
        let manifest = Manifest {
            blocks: vec![Block {
                id: "background".into(),
                title: "Background".into(),
                content: "context body\n".repeat(200),
                ..Default::default()
            }],
            ..Default::default()
        };
        let mut visible = false;
        for _ in 0..3 {
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800., 600.),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        egui::ScrollArea::vertical()
                            .max_height(220.)
                            .stick_to_bottom(true)
                            .show(ui, |ui| {
                                ui.add_space(600.);
                                manifest_ui(ui, &manifest);
                            });
                    });
                },
            );
            visible = output.shapes.iter().any(|shape| matches!(&shape.shape,
                egui::epaint::Shape::Text(text) if text.galley.job.text.contains("context body")
                    && shape.clip_rect.intersects(egui::Rect::from_min_size(text.pos, text.galley.size()))));
        }
        assert!(
            visible,
            "manifest body collapsed inside the task scroll area"
        );
    }

    #[test]
    fn long_manifest_titles_and_workspace_paths_stay_within_the_panel() {
        let ctx = egui::Context::default();
        let manifest = Manifest {
            character: "a".into(),
            workspace: "/long-project-path".repeat(40),
            blocks: vec![Block {
                id: "input".into(),
                title: "long-title".repeat(100),
                content: "content".repeat(100),
                ..Default::default()
            }],
            ..Default::default()
        };
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(320., 800.),
                )),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let width = ui.available_width();
                    manifest_ui(ui, &manifest);
                    assert!(
                        ui.min_rect().width() <= width + 1.,
                        "manifest widened the panel"
                    );
                });
            },
        );
    }
}
