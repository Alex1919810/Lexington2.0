use super::app::Wife;
use crate::backend::codex::{Client, Event, Prompt};
use eframe::egui;
use serde_json::json;
use std::time::Instant;

pub(super) struct PendingPrompt {
    request: Prompt,
    answers: Vec<String>,
}
impl Wife {
    pub(super) fn start_codex(&mut self, ctx: &egui::Context) {
        self.interrupt_development_test("会话已重建。");
        self.finish_development("interrupted", "会话已重建。");
        if self.development.reset_sessions_pending {
            for project in &mut self.development.book.projects {
                project.current_task = None;
            }
            self.development.reset_sessions_pending = false;
        }
        let plan = if self.config.codex.development_mode {
            self.development
                .book
                .current()
                .map(|project| {
                    crate::development::memory::context(&project.path, &project.memories)
                })
                .unwrap_or_default()
        } else {
            Default::default()
        };
        let compatible = self.development.book.task().is_none_or(|task| {
            task.character == self.library.character
                && task.development_mode == self.config.codex.development_mode
                && (!self.config.codex.development_mode
                    || self.development.book.current().is_some_and(|project| {
                        project.memory_revision == task.memory_revision
                            && task.memory_context_stamp
                                == crate::development::memory::context_stamp(&plan.0)
                    }))
        });
        if !compatible && let Some(project) = self.development.book.current_mut() {
            project.current_task = None;
        }
        self.development.memory_context = plan.0;
        self.development.memory_ui.report = plan.1;
        self.development.thread_id = None;
        self.development.session_mode = self.config.codex.development_mode;
        self.context_ui = Default::default();
        self.codex_restart_pending = false;
        // Drop the previous process before replacing it, so restarts cannot leave orphan servers.
        self.codex = None;
        self.codex_character = Some(self.library.character.clone());
        self.codex_role_card = crate::roleplay::session_card(&self.root, &self.library.character)
            .ok()
            .flatten();
        self.roleplay_stamp = self.roleplay_file_stamp();
        self.codex_messages.clear();
        self.text.clear();
        self.turn_ui_hidden = false;
        self.dialog_clicks = 0;
        self.busy = false;
        self.external_turn = false;
        self.codex_prompts.clear();
        self.error.clear();
        let ctx = ctx.clone();
        let mut config = self.config.codex.clone();
        config.working_directory = self.development.book.active.clone();
        let resume = self
            .development
            .book
            .task()
            .and_then(|task| task.thread_id.clone());
        match Client::start_in_workspace(
            &config,
            &self.root,
            &self.library.character,
            resume,
            move || ctx.request_repaint(),
        ) {
            Ok(client) => {
                self.codex = Some(client);
                self.codex_status = "正在连接…".into();
            }
            Err(error) => {
                self.codex_status = "启动失败".into();
                self.error = error;
            }
        }
    }
    pub(super) fn poll_codex(&mut self) {
        let mut changed = false;
        while let Some(event) = self.codex.as_ref().and_then(Client::poll) {
            match event {
                Event::TestResult(result) => self.receive_test_result(*result),
                Event::Context(plan) => {
                    if plan.preview {
                        self.context_ui.preview = Some(*plan);
                    } else {
                        self.context_ui.last = Some(*plan);
                    }
                }
                Event::ThreadReady(id) => {
                    self.development.thread_id = Some(id.clone());
                    if let Some(task) = self.development.book.task_mut() {
                        task.thread_id = Some(id);
                        self.save_development(true);
                    }
                }
                Event::Activity(activity) => {
                    if self.busy
                        && let Some(turn) = self.development.book.turn_mut()
                    {
                        turn.activity(activity);
                        if let Some(client) = &self.codex {
                            turn.sanitize(|text| client.sanitize(text));
                        }
                        self.save_development(false);
                    }
                }
                Event::Ready => {
                    if !self.busy {
                        self.codex_status = "已连接".into();
                    }
                }
                Event::Text { id, text, replace } => {
                    if self.busy
                        && let Some(turn) = self.development.book.turn_mut()
                    {
                        turn.text(id.clone(), text.clone(), replace);
                        if let Some(client) = &self.codex {
                            turn.sanitize(|text| client.sanitize(text));
                        }
                        self.save_development(false);
                    }
                    let index = self
                        .codex_messages
                        .iter()
                        .position(|(item, _)| item == &id)
                        .unwrap_or_else(|| {
                            self.codex_messages.push((id, String::new()));
                            self.codex_messages.len() - 1
                        });
                    if replace {
                        self.codex_messages[index].1 = text;
                    } else {
                        self.codex_messages[index].1.push_str(&text);
                    }
                    if let Some(client) = &self.codex {
                        self.codex_messages[index].1 =
                            client.sanitize(&self.codex_messages[index].1);
                    }
                    changed = true;
                }
                Event::Finished { interrupted } => {
                    self.finish_development(
                        if interrupted {
                            "interrupted"
                        } else {
                            "completed"
                        },
                        "",
                    );
                    if !interrupted && !self.development.session_mode {
                        self.queue_character_memory();
                    }
                    self.busy = false;
                    self.dialog_clicks = 0;
                    self.codex_status = if interrupted {
                        "已停止"
                    } else {
                        "已连接"
                    }
                    .into();
                    if !interrupted && !self.external_turn {
                        self.input.clear();
                    }
                    self.external_turn = false;
                    self.codex_prompts.clear();
                    self.last_activity = Instant::now();
                }
                Event::Failed(error) | Event::Stopped(error) => {
                    self.interrupt_development_test(&error);
                    self.finish_development("failed", &error);
                    self.busy = false;
                    self.dialog_clicks = 0;
                    if self.external_turn && !self.turn_ui_hidden {
                        self.text = error.clone();
                        self.full_text = true;
                        self.last_activity = Instant::now();
                    }
                    self.external_turn = false;
                    self.codex_status = "连接或任务失败".into();
                    self.codex_prompts.clear();
                    self.error = error;
                }
                Event::Notice(message) => self.codex_status = message,
                Event::Prompt(request) => {
                    let answers = vec![String::new(); request.questions.len()];
                    self.codex_prompts
                        .push_back(PendingPrompt { request, answers });
                }
            }
        }
        if changed {
            if !self.turn_ui_hidden {
                self.text = self
                    .codex_messages
                    .iter()
                    .map(|(_, text)| text.as_str())
                    .collect::<Vec<_>>()
                    .join("\n\n");
            }
            self.full_text = true; // Codex already streams text; do not apply a second typewriter delay.
            self.last_activity = Instant::now();
        }
    }
    pub(super) fn codex_prompt_ui(&mut self, ctx: &egui::Context) {
        let Some(prompt) = self.codex_prompts.front_mut() else {
            return;
        };
        let mut answer = None;
        if let Some(window) = egui::Window::new(&prompt.request.title)
            .id("codex-prompt".into())
            .collapsible(false)
            .resizable(true)
            .default_width(340.)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(300.)
                    .show(ui, |ui| {
                        if !prompt.request.details.is_empty() {
                            ui.label(&prompt.request.details);
                        }
                        for (question, answer) in
                            prompt.request.questions.iter().zip(&mut prompt.answers)
                        {
                            ui.separator();
                            ui.label(&question.text);
                            for option in &question.options {
                                ui.selectable_value(answer, option.clone(), option);
                            }
                            ui.add(
                                egui::TextEdit::singleline(answer)
                                    .password(question.secret)
                                    .hint_text("输入回答…"),
                            );
                        }
                    });
                ui.horizontal(|ui| {
                    if prompt.request.questions.is_empty() {
                        if ui.button("允许本次").clicked() {
                            answer = Some(json!({"decision":"accept"}));
                        }
                        if ui.button("拒绝").clicked() {
                            answer = Some(json!({"decision":"decline"}));
                        }
                    } else {
                        if ui
                            .add_enabled(
                                prompt.answers.iter().all(|a| !a.trim().is_empty()),
                                egui::Button::new("提交回答"),
                            )
                            .clicked()
                        {
                            let answers: serde_json::Map<_, _> = prompt
                                .request
                                .questions
                                .iter()
                                .zip(&prompt.answers)
                                .map(|(q, a)| (q.id.clone(), json!({"answers":[a]})))
                                .collect();
                            answer = Some(json!({"answers":answers}));
                        }
                        if ui.button("停止任务").clicked()
                            && let Some(codex) = &self.codex
                        {
                            codex.interrupt();
                        }
                    }
                });
            })
        {
            self.input_regions.push(window.response.rect);
        }
        if let Some(answer) = answer
            && let Some(prompt) = self.codex_prompts.pop_front()
            && let Some(codex) = &self.codex
        {
            codex.reply(prompt.request.id, answer);
        }
    }
}
