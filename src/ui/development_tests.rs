use super::app::Wife;
use crate::development::testing::{TestDefinition, TestRequest, TestRun};
use eframe::egui;
use std::{collections::VecDeque, path::PathBuf};

pub(super) struct TestEditor {
    pub definition: TestDefinition,
    pub program: String,
    pub arguments: String,
}
pub(super) enum TestAction {
    Save(TestDefinition),
    Delete(u64),
    Run(TestDefinition),
    RetryFailed,
    Analyze(u64),
}
#[derive(Default)]
pub(super) struct TestsUi {
    pub editor: Option<TestEditor>,
    pub action: Option<TestAction>,
    pub queue: VecDeque<(PathBuf, TestDefinition)>,
    pub running: Option<(PathBuf, u64)>,
}
impl Wife {
    pub(super) fn development_tests_ui(&mut self, ui: &mut egui::Ui) {
        let Some(project) = self.development.book.current() else {
            return;
        };
        ui.heading("测试管理");
        ui.small("按项目保存命令，通过本机 Codex 执行并沿用权限设置；每个参数单独一行，不自动拼接 shell。");
        ui.horizontal(|ui| {
            if ui.button("添加测试").clicked() {
                self.development.tests_ui.editor = Some(TestEditor {
                    definition: TestDefinition {
                        id: rand::random(),
                        timeout_seconds: 300,
                        ..Default::default()
                    },
                    program: String::new(),
                    arguments: String::new(),
                });
            }
            if ui
                .add_enabled(!self.busy, egui::Button::new("重跑失败项"))
                .clicked()
            {
                self.development.tests_ui.action = Some(TestAction::RetryFailed);
            }
            if self.development.tests_ui.running.is_some() {
                ui.spinner();
                ui.label("正在运行；可使用窗口上方的“停止任务”。");
            }
        });
        if let Some(editor) = &mut self.development.tests_ui.editor {
            let mut cancel = false;
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.label("名称");
                ui.text_edit_singleline(&mut editor.definition.name);
                ui.label("程序（例如 cargo）");
                ui.text_edit_singleline(&mut editor.program);
                ui.label("参数（每行一项，例如 test、--locked、模块名）");
                ui.add(
                    egui::TextEdit::multiline(&mut editor.arguments)
                        .desired_rows(4)
                        .desired_width(f32::INFINITY),
                );
                ui.horizontal(|ui| {
                    ui.label("超时（秒）");
                    ui.add(
                        egui::DragValue::new(&mut editor.definition.timeout_seconds)
                            .range(1..=3600),
                    );
                });
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(!self.busy, egui::Button::new("保存测试"))
                        .clicked()
                    {
                        let mut definition = editor.definition.clone();
                        definition.command = std::iter::once(editor.program.trim().to_owned())
                            .chain(editor.arguments.lines().map(str::to_owned))
                            .collect();
                        self.development.tests_ui.action = Some(TestAction::Save(definition));
                    }
                    cancel = ui.button("取消修改").clicked();
                });
            });
            if cancel {
                self.development.tests_ui.editor = None;
            }
        }
        for definition in &project.tests {
            ui.push_id(definition.id, |ui| {
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.strong(&definition.name);
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(definition.display_command()).monospace(),
                        )
                        .wrap()
                        .selectable(true),
                    );
                    if let Some(run) = project
                        .test_runs
                        .iter()
                        .rev()
                        .find(|run| run.definition_id == definition.id)
                    {
                        ui.small(format!(
                            "最近记录：{} · {}",
                            super::development::status_text(&run.status),
                            super::development::timestamp_text(run.started_at)
                        ));
                    }
                    ui.add_enabled_ui(!self.busy, |ui| {
                        ui.horizontal(|ui| {
                            if ui.button("运行").clicked() {
                                self.development.tests_ui.action =
                                    Some(TestAction::Run(definition.clone()));
                            }
                            if ui.button("编辑").clicked() {
                                self.development.tests_ui.editor = Some(TestEditor {
                                    definition: definition.clone(),
                                    program: definition
                                        .command
                                        .first()
                                        .cloned()
                                        .unwrap_or_default(),
                                    arguments: definition
                                        .command
                                        .iter()
                                        .skip(1)
                                        .cloned()
                                        .collect::<Vec<_>>()
                                        .join("\n"),
                                });
                            }
                            if ui.button("删除配置").clicked() {
                                self.development.tests_ui.action =
                                    Some(TestAction::Delete(definition.id));
                            }
                        });
                    });
                });
            });
        }
        ui.separator();
        ui.strong("运行记录（最近 20 次，结果不代表后来修改的代码）");
        for run in project.test_runs.iter().rev() {
            ui.push_id(run.id, |ui| {
                egui::CollapsingHeader::new(format!(
                    "{} · {} · {}",
                    super::development::status_text(&run.status),
                    run.name,
                    super::development::timestamp_text(run.started_at)
                ))
                .default_open(run.status == "running")
                .show(ui, |ui| {
                    ui.label(format!(
                        "退出码：{} · {} ms",
                        run.exit_code
                            .map(|c| c.to_string())
                            .unwrap_or_else(|| "未返回".into()),
                        run.duration_ms
                    ));
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(
                                serde_json::to_string(&run.command).unwrap_or_default(),
                            )
                            .monospace(),
                        )
                        .wrap(),
                    );
                    if !run.error.is_empty() {
                        ui.colored_label(egui::Color32::DARK_RED, &run.error);
                    }
                    for (name, text) in [("标准输出", &run.stdout), ("标准错误", &run.stderr)]
                    {
                        if !text.is_empty() {
                            ui.collapsing(name, |ui| {
                                ui.add(
                                    egui::Label::new(egui::RichText::new(text).monospace())
                                        .selectable(true)
                                        .wrap(),
                                );
                            });
                        }
                    }
                    if ui
                        .add_enabled(
                            !self.busy && run.status == "failed",
                            egui::Button::new("分析失败记录"),
                        )
                        .clicked()
                    {
                        self.development.tests_ui.action = Some(TestAction::Analyze(run.id));
                    }
                });
            });
        }
    }
    pub(super) fn apply_development_test(&mut self, ctx: &egui::Context) {
        let Some(action) = self.development.tests_ui.action.take() else {
            return;
        };
        if self.busy {
            self.development.error = "当前执行通道正忙。".into();
            return;
        }
        match action {
            TestAction::Run(definition) => {
                if let Some(path) = self.development.book.active.clone() {
                    self.development
                        .tests_ui
                        .queue
                        .push_back((path, definition));
                }
            }
            TestAction::RetryFailed => {
                if let Some(project) = self.development.book.current() {
                    for definition in &project.tests {
                        if project
                            .test_runs
                            .iter()
                            .rev()
                            .find(|run| run.definition_id == definition.id)
                            .is_some_and(|run| run.status == "failed")
                        {
                            self.development
                                .tests_ui
                                .queue
                                .push_back((project.path.clone(), definition.clone()));
                        }
                    }
                }
            }
            TestAction::Analyze(id) => {
                if let Some(run) = self
                    .development
                    .book
                    .current()
                    .and_then(|project| project.test_runs.iter().find(|run| run.id == id))
                {
                    let output = [&run.stdout, &run.stderr].map(|text| {
                        text.chars()
                            .rev()
                            .take(6000)
                            .collect::<Vec<_>>()
                            .into_iter()
                            .rev()
                            .collect::<String>()
                    });
                    let text = format!(
                        "请分析这次测试失败的原因，先核对当前代码，不把历史结果当作当前状态。\n命令：{}\n退出码：{:?}\n{}\n{}\n{}",
                        serde_json::to_string(&run.command).unwrap_or_default(),
                        run.exit_code,
                        run.error,
                        output[0],
                        output[1]
                    );
                    self.send_message(ctx, text, super::input::MessageSource::Development);
                    self.development.page = 0;
                }
            }
            TestAction::Save(definition) => {
                if let Err(error) = definition.validate() {
                    self.development.error = error;
                    return;
                }
                if self.codex.as_ref().is_some_and(|client| {
                    definition
                        .command
                        .iter()
                        .any(|arg| client.sanitize(arg) != *arg)
                }) {
                    self.development.error =
                        "请通过环境提供凭据，不要把凭据保存到测试参数中。".into();
                    return;
                }
                let mut candidate = self.development.book.clone();
                let Some(project) = candidate.current_mut() else {
                    return;
                };
                if let Some(saved) = project
                    .tests
                    .iter_mut()
                    .find(|item| item.id == definition.id)
                {
                    *saved = definition;
                } else if project.tests.len() < 64 {
                    project.tests.push(definition);
                } else {
                    self.development.error = "单个项目最多配置 64 项测试。".into();
                    return;
                }
                if let Some(path) = &self.development.storage
                    && let Err(error) = candidate.save(path)
                {
                    self.development.error = error;
                    return;
                }
                self.development.book = candidate;
                self.development.tests_ui.editor = None;
                self.development.error.clear();
            }
            TestAction::Delete(id) => {
                let mut candidate = self.development.book.clone();
                if let Some(project) = candidate.current_mut() {
                    project.tests.retain(|test| test.id != id);
                }
                if let Some(path) = &self.development.storage
                    && let Err(error) = candidate.save(path)
                {
                    self.development.error = error;
                    return;
                }
                self.development.book = candidate;
            }
        }
    }
    pub(super) fn poll_development_test_queue(&mut self, ctx: &egui::Context) {
        if self.busy || self.development.tests_ui.running.is_some() {
            return;
        }
        let Some((path, definition)) = self.development.tests_ui.queue.pop_front() else {
            return;
        };
        if self.development.book.active.as_ref() != Some(&path) {
            self.development.tests_ui.queue.clear();
            self.development.error = "项目已切换，已取消旧项目的待运行测试。".into();
            return;
        }
        if self.codex_restart_pending {
            self.start_codex(ctx);
        }
        let id = rand::random();
        let result = self
            .codex
            .as_ref()
            .ok_or_else(|| "Codex 未启动。".to_owned())
            .and_then(|client| {
                client.run_test(TestRequest {
                    id,
                    definition: definition.clone(),
                })
            });
        if let Err(error) = result {
            self.development.error = error;
            self.development.tests_ui.queue.clear();
            return;
        }
        let mut record = TestRun {
            id,
            definition_id: definition.id,
            name: definition.name,
            command: definition.command,
            started_at: crate::development::testing::now(),
            status: "running".into(),
            ..Default::default()
        };
        if let Some(client) = &self.codex {
            record.sanitize(|text| client.sanitize(text));
        }
        if let Some(project) = self.development.book.current_mut() {
            if project.test_runs.len() >= 20 {
                project.test_runs.remove(0);
            }
            project.test_runs.push(record);
        }
        self.development.tests_ui.running = Some((path, id));
        self.busy = true;
        self.codex_status = "正在运行测试…".into();
        self.save_development(true);
    }
    pub(super) fn receive_test_result(&mut self, mut result: TestRun) {
        let Some((path, id)) = self.development.tests_ui.running.take() else {
            return;
        };
        if result.id != id {
            self.development.tests_ui.running = Some((path, id));
            return;
        }
        if let Some(client) = &self.codex {
            result.sanitize(|text| client.sanitize(text));
        }
        if let Some(record) = self
            .development
            .book
            .projects
            .iter_mut()
            .find(|project| project.path == path)
            .and_then(|project| project.test_runs.iter_mut().find(|run| run.id == id))
        {
            record.status = result.status;
            record.duration_ms = result.duration_ms;
            record.exit_code = result.exit_code;
            record.stdout = result.stdout;
            record.stderr = result.stderr;
            record.error = result.error;
            if record.status == "cancelled" {
                self.development.tests_ui.queue.clear();
            }
        }
        self.busy = false;
        self.codex_status = "测试已结束".into();
        self.save_development(true);
    }
    pub(super) fn interrupt_development_test(&mut self, error: &str) {
        if let Some((_, id)) = &self.development.tests_ui.running {
            self.receive_test_result(TestRun {
                id: *id,
                status: "interrupted".into(),
                error: error.into(),
                ..Default::default()
            });
        }
        self.development.tests_ui.queue.clear();
    }
}
