use super::app::Wife;
use crate::backend::codex::{Client, Event, Prompt};
use eframe::egui;
use serde_json::json;
use std::time::{Duration, Instant};

pub(super) struct PendingPrompt {
    pub request: Prompt,
    pub open: bool,
    answers: Vec<String>,
    started: Option<Instant>,
    raised: bool,
}
impl PendingPrompt {
    pub(super) fn reopen(&mut self) {
        self.open = true;
        self.started = None;
        self.raised = false;
    }
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
        self.complete_development_supplement(Err("会话已重启，未确认的补充保留在草稿中。".into()));
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
        self.development_turn = false;
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
    // Return whether another frame is needed to continue draining queued events.
    pub(super) fn poll_codex(&mut self) -> bool {
        let started = Instant::now();
        let mut processed = 0;
        let mut changed = false;
        let mut development_changed = false;
        let mut main_dirty = false;
        while processed < 64 && started.elapsed() < Duration::from_millis(4) {
            let Some(event) = self.codex.as_ref().and_then(Client::poll) else {
                break;
            };
            processed += 1;
            // Preserve stream order, and flush assembled secrets before any event
            // that can persist records or queue character memory (including finish).
            if !matches!(&event, Event::Activity(_) | Event::Text { .. }) {
                if development_changed {
                    self.sanitize_development_stream();
                    development_changed = false;
                }
                if main_dirty {
                    self.sanitize_main_stream();
                    main_dirty = false;
                }
            }
            match event {
                Event::Steered(result) => self.complete_development_supplement(result),
                Event::TestResult(result) => self.receive_test_result(*result),
                Event::Context(plan) => {
                    if plan.preview {
                        self.context_ui.preview = Some(*plan);
                    } else {
                        self.context_ui.last = Some(*plan);
                    }
                }
                Event::Manifest(manifest) => {
                    if manifest.preview {
                        self.context_ui.preview_manifest = Some(*manifest);
                    } else {
                        if self.busy
                            && let Some(turn) = self.development.book.turn_mut()
                        {
                            turn.set_manifest((*manifest).clone());
                            self.save_development(true);
                        }
                        self.context_ui.sent_manifest = Some(*manifest);
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
                        development_changed = true;
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
                        development_changed = true;
                    }
                    if self.development_turn {
                        continue;
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
                    main_dirty = true;
                    changed = true;
                }
                Event::CommandFinished => {
                    self.finish_development("completed", "");
                    self.busy = false;
                    self.dialog_clicks = 0;
                    self.codex_status = "已连接".into();
                    if !self.development_turn {
                        self.input.clear();
                    }
                    self.last_activity = Instant::now();
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
                    if !interrupted && !self.development.session_mode && !self.development_turn {
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
                    if !interrupted && !self.external_turn && !self.development_turn {
                        self.input.clear();
                    }
                    self.external_turn = false;
                    self.codex_prompts.clear();
                    self.last_activity = Instant::now();
                }
                Event::Failed(error) | Event::Stopped(error) => {
                    self.complete_development_supplement(Err(error.clone()));
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
                    self.codex_prompts.push_back(PendingPrompt {
                        request,
                        answers,
                        open: true,
                        started: None,
                        raised: false,
                    });
                }
            }
        }
        if development_changed {
            self.sanitize_development_stream();
        }
        if main_dirty {
            self.sanitize_main_stream();
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
        processed == 64 || started.elapsed() >= Duration::from_millis(4)
    }
    fn sanitize_development_stream(&mut self) {
        if let Some(turn) = self.development.book.turn_mut()
            && let Some(client) = &self.codex
        {
            turn.sanitize(|text| client.sanitize(text));
            turn.flush_events();
        }
        self.save_development(false);
    }
    fn sanitize_main_stream(&mut self) {
        if let Some(client) = &self.codex {
            for (_, text) in &mut self.codex_messages {
                *text = client.sanitize(text);
            }
        }
    }
    pub(super) fn codex_prompt_ui(&mut self, ctx: &egui::Context) {
        let Some(prompt) = self.codex_prompts.front_mut() else {
            return;
        };
        if !prompt.open {
            if !self.development_turn
                && let Some(window) = egui::Window::new("Lexington 正在等待你")
                    .id("codex-prompt-reopen".into())
                    .collapsible(false)
                    .resizable(false)
                    .show(ctx, |ui| {
                        if ui.button("查看待处理请求").clicked() {
                            prompt.reopen();
                        }
                    })
            {
                self.input_regions.push(window.response.rect);
            }
            if !prompt.open {
                return;
            }
        }
        let first_open = prompt.started.is_none();
        prompt.started.get_or_insert_with(Instant::now);
        let id = egui::ViewportId::from_hash_of("lexington-codex-prompt");
        let size = egui::vec2(520., 460.);
        let mut builder = egui::ViewportBuilder::default()
            .with_title(&prompt.request.title)
            .with_inner_size(size)
            .with_min_inner_size([400., 300.])
            .with_transparent(false)
            .with_resizable(true)
            .with_always_on_top()
            .with_window_type(egui::X11WindowType::Dock);
        if first_open {
            let mut monitor = egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                ctx.input(|input| input.viewport().monitor_size)
                    .unwrap_or(egui::vec2(1920., 1080.)),
            );
            #[cfg(any(target_os = "linux", windows))]
            if let Some(window) = &self.drag_window {
                let monitors = window.monitors(ctx.pixels_per_point());
                if let Some(found) = monitors
                    .iter()
                    .find(|rect| {
                        rect.contains(
                            self.window_position.unwrap_or_default()
                                + ctx.screen_rect().size() / 2.,
                        )
                    })
                    .or_else(|| monitors.first())
                {
                    monitor = *found;
                }
            }
            builder = builder.with_position(monitor.center() - size / 2.);
        }
        let mut answer = None;
        let mut stop = false;
        let mut open = true;
        let mut defer = false;
        let development_turn = self.development_turn;
        ctx.show_viewport_immediate(id, builder, |popup, class| {
            if class != egui::ViewportClass::Embedded {
                if popup.input(|input| input.viewport().close_requested()) {
                    open = false;
                }
                if !prompt.raised
                    && prompt
                        .started
                        .is_some_and(|started| started.elapsed() >= Duration::from_millis(800))
                {
                    popup.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
                        egui::WindowLevel::AlwaysOnTop,
                    ));
                    popup.send_viewport_cmd(egui::ViewportCommand::Focus);
                    prompt.raised = true;
                }
            }
            let mut contents = |ui: &mut egui::Ui| {
                super::style::panel_style(ui);
                ui.heading(&prompt.request.title);
                ui.label(if development_turn {
                    "开发任务正在等待你处理，回答后继续执行。"
                } else {
                    "当前任务正在等待你处理，回答后继续执行。"
                });
                ui.separator();
                egui::ScrollArea::vertical()
                    .id_salt(("codex-prompt-body", prompt.request.id.to_string()))
                    .max_height((ui.available_height() - 52.).max(0.))
                    .min_scrolled_height(0.)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if !prompt.request.details.is_empty() {
                            ui.add(egui::Label::new(&prompt.request.details).wrap());
                        }
                        for (index, (question, answer)) in prompt
                            .request
                            .questions
                            .iter()
                            .zip(&mut prompt.answers)
                            .enumerate()
                        {
                            ui.push_id(index, |ui| {
                                ui.separator();
                                ui.add(egui::Label::new(&question.text).wrap());
                                for option in &question.options {
                                    if ui
                                        .add(
                                            egui::Button::new(option)
                                                .selected(*answer == *option)
                                                .wrap(),
                                        )
                                        .clicked()
                                    {
                                        *answer = option.clone();
                                    }
                                }
                                ui.add(
                                    egui::TextEdit::singleline(answer)
                                        .password(question.secret)
                                        .desired_width(f32::INFINITY)
                                        .hint_text("输入回答…"),
                                );
                            });
                        }
                    });
                ui.separator();
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
                        if ui.button("停止任务").clicked() {
                            stop = true;
                        }
                    }
                    if ui.button("稍后处理").clicked() {
                        defer = true;
                    }
                });
            };
            if class == egui::ViewportClass::Embedded {
                if let Some(window) = egui::Window::new(&prompt.request.title)
                    .id("codex-prompt".into())
                    .open(&mut open)
                    .order(egui::Order::Foreground)
                    .collapsible(false)
                    .default_size([488., 420.])
                    .min_size([368., 260.])
                    .show(popup, &mut contents)
                {
                    self.input_regions.push(window.response.rect);
                }
            } else {
                egui::CentralPanel::default()
                    .frame(
                        egui::Frame::new()
                            .fill(super::style::PANEL_BG)
                            .inner_margin(16),
                    )
                    .show(popup, &mut contents);
            }
        });
        prompt.open = open && !defer;
        if !prompt.open || answer.is_some() {
            prompt.started = None;
            prompt.raised = false;
            ctx.send_viewport_cmd_to(id, egui::ViewportCommand::Close);
        }
        if stop && let Some(codex) = &self.codex {
            codex.interrupt();
        }
        if let Some(answer) = answer
            && let Some(prompt) = self.codex_prompts.pop_front()
            && let Some(codex) = &self.codex
        {
            codex.reply(prompt.request.id, answer);
        }
    }
}
