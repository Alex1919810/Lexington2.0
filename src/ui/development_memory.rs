use super::app::Wife;
use crate::development::memory::ProjectMemory;
use eframe::egui;

pub(super) enum MemoryAction {
    Save(ProjectMemory),
    Delete(u64),
    Toggle(u64),
}
#[derive(Default)]
pub(super) struct MemoryUi {
    pub editor: Option<ProjectMemory>,
    pub action: Option<MemoryAction>,
    pub report: Vec<(u64, String)>,
    pub refresh: bool,
    pub search: String,
}
impl Wife {
    pub(super) fn development_memory_ui(&mut self, ui: &mut egui::Ui) {
        let Some(project) = self.development.book.current() else {
            return;
        };
        ui.heading("项目记忆");
        ui.small("只在 development 模式中载入已核对且启用的内容；依据变化后需重新核对。");
        if std::mem::take(&mut self.development.memory_ui.refresh) {
            self.development.memory_ui.report =
                crate::development::memory::context(&project.path, &project.memories).1;
            if !self.config.codex.development_mode {
                for (_, status) in &mut self.development.memory_ui.report {
                    if status == "可载入" {
                        *status = "可载入（当前模式未载入）".into();
                    }
                }
            }
        }
        ui.horizontal(|ui| {
            if ui.button("新增记忆").clicked() {
                self.development.memory_ui.editor = Some(ProjectMemory {
                    id: rand::random(),
                    category: "项目约定".into(),
                    enabled: true,
                    ..Default::default()
                });
            }
            if ui.button("从当前任务摘录").clicked()
                && let Some(task) = self.development.book.task()
            {
                let content = task
                    .turns
                    .last()
                    .map(|turn| {
                        turn.messages
                            .iter()
                            .map(|(_, text)| text.as_str())
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
                    .unwrap_or_default();
                self.development.memory_ui.editor = Some(ProjectMemory {
                    id: rand::random(),
                    title: task.title.chars().take(120).collect(),
                    category: "技术决策".into(),
                    content: content.chars().take(6000).collect(),
                    source: format!("任务 {}", task.id),
                    enabled: true,
                    ..Default::default()
                });
            }
            if ui.button("检查依据").clicked() {
                self.development.memory_ui.refresh = true;
            }
        });
        if let Some(editor) = &mut self.development.memory_ui.editor {
            let mut close = false;
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.label("标题");
                let mut changed = ui.text_edit_singleline(&mut editor.title).changed();
                egui::ComboBox::from_id_salt("project-memory-category")
                    .selected_text(&editor.category)
                    .show_ui(ui, |ui| {
                        for category in ["项目约定", "架构", "构建与测试", "技术决策", "待解决问题"]
                        {
                            changed |= ui
                                .selectable_value(&mut editor.category, category.into(), category)
                                .changed();
                        }
                    });
                ui.label("内容");
                changed |= ui
                    .add(
                        egui::TextEdit::multiline(&mut editor.content)
                            .desired_rows(5)
                            .desired_width(f32::INFINITY),
                    )
                    .changed();
                ui.label("来源说明（例如任务或文档）");
                changed |= ui.text_edit_singleline(&mut editor.source).changed();
                ui.label("依据文件（项目内相对路径，可留空）");
                changed |= ui
                    .add(
                        egui::TextEdit::singleline(&mut editor.source_file)
                            .hint_text("例如 docs/MODULES.md"),
                    )
                    .changed();
                if changed {
                    editor.confirmed = false;
                }
                ui.checkbox(&mut editor.confirmed, "我已核对内容，允许用于开发任务");
                ui.checkbox(&mut editor.enabled, "启用");
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(!self.busy, egui::Button::new("保存记忆"))
                        .clicked()
                    {
                        self.development.memory_ui.action =
                            Some(MemoryAction::Save(editor.clone()));
                    }
                    close = ui.button("取消修改").clicked();
                });
            });
            if close {
                self.development.memory_ui.editor = None;
            }
        }
        ui.add(
            egui::TextEdit::singleline(&mut self.development.memory_ui.search)
                .hint_text("搜索项目记忆…"),
        );
        for memory in project.memories.iter().filter(|m| {
            m.title.contains(&self.development.memory_ui.search)
                || m.content.contains(&self.development.memory_ui.search)
        }) {
            ui.push_id(memory.id, |ui| {
                egui::CollapsingHeader::new(format!("{} · {}", memory.category, memory.title))
                    .show(ui, |ui| {
                        ui.add(egui::Label::new(&memory.content).selectable(true).wrap());
                        ui.small(format!("来源：{}", memory.source));
                        let state = self
                            .development
                            .memory_ui
                            .report
                            .iter()
                            .find(|(id, _)| *id == memory.id)
                            .map(|(_, state)| state.as_str())
                            .unwrap_or("尚未检查");
                        ui.small(format!(
                            "{state} · {}",
                            if memory.enabled { "启用" } else { "停用" }
                        ));
                        ui.add_enabled_ui(!self.busy, |ui| {
                            ui.horizontal(|ui| {
                                if ui.button("编辑并核对").clicked() {
                                    let mut draft = memory.clone();
                                    draft.confirmed = false;
                                    self.development.memory_ui.editor = Some(draft);
                                }
                                if ui
                                    .button(if memory.enabled { "停用" } else { "启用" })
                                    .clicked()
                                {
                                    self.development.memory_ui.action =
                                        Some(MemoryAction::Toggle(memory.id));
                                }
                                if ui.button("删除").clicked() {
                                    self.development.memory_ui.action =
                                        Some(MemoryAction::Delete(memory.id));
                                }
                            });
                        });
                    });
            });
        }
    }
    pub(super) fn apply_project_memory(&mut self, ctx: &egui::Context) {
        let Some(action) = self.development.memory_ui.action.take() else {
            return;
        };
        if self.busy {
            self.development.error = "请在当前任务结束后保存项目记忆。".into();
            return;
        }
        let saving = matches!(&action, MemoryAction::Save(_));
        let mut candidate = self.development.book.clone();
        let Some(project) = candidate.current_mut() else {
            return;
        };
        match action {
            MemoryAction::Save(mut memory) => {
                if let Some(client) = &self.codex {
                    memory.title = client.sanitize(&memory.title);
                    memory.content = client.sanitize(&memory.content);
                    memory.source = client.sanitize(&memory.source);
                    memory.source_file = client.sanitize(&memory.source_file);
                }
                if let Err(error) = memory.prepare(&project.path) {
                    self.development.error = error;
                    return;
                }
                if let Some(saved) = project.memories.iter_mut().find(|m| m.id == memory.id) {
                    *saved = memory;
                } else if project.memories.len() < 128 {
                    project.memories.push(memory);
                } else {
                    self.development.error = "单个项目最多保存 128 条记忆。".into();
                    return;
                }
            }
            MemoryAction::Delete(id) => project.memories.retain(|m| m.id != id),
            MemoryAction::Toggle(id) => {
                if let Some(memory) = project.memories.iter_mut().find(|m| m.id == id) {
                    memory.enabled = !memory.enabled;
                }
            }
        }
        project.memory_revision = project.memory_revision.saturating_add(1);
        if let Some(path) = &self.development.storage
            && let Err(error) = candidate.save(path)
        {
            self.development.error = error;
            return;
        }
        self.development.book = candidate;
        if saving {
            self.development.memory_ui.editor = None;
        }
        self.development.memory_ui.refresh = true;
        self.development.error.clear();
        if self.config.codex.development_mode && self.codex.is_some() {
            self.new_development_task(ctx);
        }
    }
}
