use super::app::Wife;
use crate::{
    backend::{
        clock,
        schedule::{
            ExecutionEvent, Executor, MAX_PROMPT_CHARS, Outcome, Schedule, ScheduleState, Status,
            Target, TaskSpec,
        },
    },
    tools::paths,
};
use eframe::egui;
use std::time::{Duration, Instant};

pub(super) struct ScheduleUi {
    pub(super) open: bool,
    started: Option<Instant>,
    raised: bool,
    drag: Option<(egui::Pos2, egui::Pos2, crate::tools::platform::WindowId)>,
    pointer_down: bool,
    pub(super) state: Option<ScheduleState>,
    pub(super) error: String,
    editing: Option<u64>,
    name: String,
    prompt: String,
    kind: usize,
    time_parts: [u32; 2],
    minutes: u32,
    target: Option<Target>,
    last_poll: Instant,
    active: Option<u64>,
    pub(super) executor: Option<Executor>,
    outcome: Option<Outcome>,
    history_open: bool,
    selected_run: Option<u64>,
}
impl Default for ScheduleUi {
    fn default() -> Self {
        Self {
            open: false,
            started: None,
            raised: false,
            drag: None,
            pointer_down: false,
            state: None,
            error: String::new(),
            editing: None,
            name: String::new(),
            prompt: String::new(),
            kind: 0,
            time_parts: [9, 0],
            minutes: 60,
            target: None,
            last_poll: Instant::now() - Duration::from_secs(2),
            active: None,
            executor: None,
            outcome: None,
            history_open: false,
            selected_run: None,
        }
    }
}
pub(super) enum ScheduleAction {
    Save(TaskSpec),
    Toggle(u64),
    Delete(u64),
    Queue(u64),
    MarkRead,
    Stop,
}
impl Wife {
    pub(super) fn load_schedules(&mut self) {
        let path = paths::data_root().join(if self.dev {
            "schedules-dev.json"
        } else {
            "schedules.json"
        });
        match ScheduleState::load(path) {
            Ok(state) => self.schedule.state = Some(state),
            Err(error) => self.schedule.error = error,
        }
    }
    pub(super) fn schedule_window(&mut self, ctx: &egui::Context) {
        if !self.schedule.open {
            return;
        }
        let id = egui::ViewportId::from_hash_of("lexington-schedules");
        let mut builder = egui::ViewportBuilder::default()
            .with_title("定时任务")
            .with_inner_size(egui::vec2(640., 620.))
            .with_min_inner_size(egui::vec2(480., 380.))
            .with_resizable(true)
            .with_decorations(false)
            .with_always_on_top()
            .with_window_type(egui::X11WindowType::Dock);
        if self.schedule.started.is_none() {
            self.schedule.started = Some(Instant::now());
            self.schedule.raised = false;
            let mut monitor = egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                ctx.input(|input| input.viewport().monitor_size)
                    .unwrap_or(egui::vec2(1920., 1080.)),
            );
            #[cfg(any(target_os = "linux", windows))]
            if let Some(window) = &self.drag_window {
                let monitors = window.monitors(ctx.pixels_per_point());
                if let Some(found) = self
                    .window_position
                    .and_then(|position| {
                        monitors
                            .iter()
                            .find(|rect| rect.contains(position + ctx.screen_rect().size() / 2.))
                    })
                    .or_else(|| monitors.first())
                {
                    monitor = *found;
                }
            }
            let near = self.window_position.unwrap_or(monitor.min);
            builder = builder.with_position(egui::pos2(
                near.x.clamp(
                    monitor.left() + 8.,
                    (monitor.right() - 648.).max(monitor.left() + 8.),
                ),
                near.y.clamp(
                    monitor.top() + 8.,
                    (monitor.bottom() - 668.).max(monitor.top() + 8.),
                ),
            ));
        }
        let mut open = true;
        let mut close = false;
        let mut action = None;
        ctx.show_viewport_immediate(id, builder, |window, class| {
            if class == egui::ViewportClass::Embedded {
                if let Some(response) = egui::Window::new("定时任务")
                    .open(&mut open)
                    .order(egui::Order::Foreground)
                    .collapsible(false)
                    .default_size(egui::vec2(608., 570.))
                    .show(window, |ui| action = self.schedule_panel(ui, &mut close))
                    && open
                {
                    self.input_regions.push(response.response.rect);
                }
            } else {
                if !self.schedule.raised
                    && self
                        .schedule
                        .started
                        .is_some_and(|started| started.elapsed() >= Duration::from_millis(800))
                {
                    window.send_viewport_cmd(egui::ViewportCommand::Focus);
                    self.schedule.raised = true;
                }
                open = !window.input(|input| input.viewport().close_requested());
                egui::CentralPanel::default()
                    .frame(
                        egui::Frame::new()
                            .fill(super::style::PANEL_BG)
                            .inner_margin(12),
                    )
                    .show(window, |ui| action = self.schedule_panel(ui, &mut close));
            }
        });
        if let Some(action) = action {
            self.apply_schedule_action(action);
        }
        self.schedule.open = open && !close;
        if !self.schedule.open {
            self.schedule.started = None;
            self.schedule.drag = None;
            self.schedule.pointer_down = false;
            ctx.send_viewport_cmd_to(id, egui::ViewportCommand::Close);
        }
    }
    fn schedule_panel(&mut self, ui: &mut egui::Ui, close: &mut bool) -> Option<ScheduleAction> {
        super::style::panel_style(ui);
        let mut title_rect = egui::Rect::NOTHING;
        let mut drag_started = false;
        ui.horizontal(|ui| {
            let (rect, response) = ui.allocate_exact_size(
                egui::vec2((ui.available_width() - 36.).max(1.), 28.),
                egui::Sense::click_and_drag(),
            );
            title_rect = rect;
            drag_started = response.drag_started();
            ui.painter().text(
                rect.left_center(),
                egui::Align2::LEFT_CENTER,
                "定时任务",
                egui::FontId::proportional(18.),
                super::style::INK,
            );
            *close = ui.add_sized([28., 28.], egui::Button::new("×")).clicked();
        });
        if ui.ctx().viewport_id() != egui::ViewportId::ROOT {
            #[cfg(any(target_os = "linux", windows))]
            if let Some(window) = &self.drag_window
                && let Some((pointer, down)) = window.screen_pointer(ui.ctx().pixels_per_point())
            {
                if !down {
                    self.schedule.drag = None;
                } else if !self.schedule.pointer_down {
                    self.schedule.drag = window
                        .window_at_pointer("定时任务", ui.ctx().pixels_per_point())
                        .filter(|(_, origin)| {
                            title_rect.contains(egui::Pos2::ZERO + (pointer - *origin))
                        })
                        .map(|(native, origin)| (pointer, origin, native));
                    if self.schedule.drag.is_some() {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Focus);
                    }
                }
                self.schedule.pointer_down = down;
                if let Some((press, origin, native)) = self.schedule.drag {
                    window.move_window(
                        native,
                        origin + (pointer - press),
                        ui.ctx().pixels_per_point(),
                    );
                }
            } else if drag_started {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            #[cfg(not(any(target_os = "linux", windows)))]
            if drag_started {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
        }
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.schedule.history_open, false, "任务");
            ui.selectable_value(&mut self.schedule.history_open, true, "执行历史");
        });
        egui::ScrollArea::vertical()
            .id_salt(("scheduled-window-content", self.schedule.history_open))
            .min_scrolled_height(0.)
            .max_height(ui.available_height())
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                self.schedule_contents(ui)
            })
            .inner
    }
    fn schedule_target(&self) -> Target {
        Target {
            root: self.root.clone(),
            workspace: self.development.book.active.clone(),
            character: self.library.character.clone(),
            development_mode: self.config.codex.development_mode,
        }
    }
    fn schedule_contents(&mut self, ui: &mut egui::Ui) -> Option<ScheduleAction> {
        if self.schedule.history_open {
            return self.schedule_history_ui(ui);
        }
        let mut action = None;
        let target = self
            .schedule
            .target
            .clone()
            .unwrap_or_else(|| self.schedule_target());
        ui.small("后台自动执行，无需审批，不占用当前聊天。保持 Lexington 运行；关闭期间错过的多次执行会在下次启动合并补发一次。");
        ui.add_enabled_ui(self.schedule.state.is_some(), |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.schedule.name)
                    .hint_text("任务名称")
                    .char_limit(80)
                    .desired_width(f32::INFINITY),
            );
            ui.horizontal(|ui| {
                for (kind, label) in [(0, "单次"), (1, "每天"), (2, "固定间隔")] {
                    ui.selectable_value(&mut self.schedule.kind, kind, label);
                }
            });
            ui.horizontal(|ui| {
                if self.schedule.kind == 2 {
                    ui.label("每");
                    ui.add(egui::DragValue::new(&mut self.schedule.minutes).range(1..=525_600));
                    ui.label("分钟（保存后开始计时）");
                } else {
                    ui.label("本地时间");
                    for (index, (max, unit)) in [(23, "时"), (59, "分")].into_iter().enumerate() {
                        ui.add(
                            egui::DragValue::new(&mut self.schedule.time_parts[index])
                                .range(0..=max)
                                .speed(1.)
                                .custom_formatter(|value, _| format!("{:02}", value as u32)),
                        );
                        ui.label(unit);
                    }
                    ui.weak("已过则安排明天");
                }
            });
            egui::ScrollArea::vertical()
                .id_salt("scheduled-prompt-editor")
                .max_height(110.)
                .min_scrolled_height(0.)
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut self.schedule.prompt)
                            .hint_text("到期时发送给 Codex 的提示词")
                            .desired_rows(3)
                            .desired_width(f32::INFINITY)
                            .char_limit(MAX_PROMPT_CHARS),
                    );
                });
            ui.add(
                egui::Label::new(format!(
                    "目标：{} · {} · {}",
                    if target.character.is_empty() {
                        "默认角色"
                    } else {
                        &target.character
                    },
                    target.workspace.as_ref().unwrap_or(&target.root).display(),
                    if target.development_mode {
                        "开发模式"
                    } else {
                        "普通模式"
                    }
                ))
                .wrap(),
            );
            ui.horizontal(|ui| {
                if ui
                    .button(if self.schedule.editing.is_some() {
                        "保存任务"
                    } else {
                        "添加任务"
                    })
                    .clicked()
                {
                    action = Some(ScheduleAction::Save(TaskSpec {
                        name: self.schedule.name.clone(),
                        prompt: self.schedule.prompt.clone(),
                        target,
                        schedule: match self.schedule.kind {
                            1 => Schedule::Daily {
                                time: format!(
                                    "{:02}:{:02}",
                                    self.schedule.time_parts[0], self.schedule.time_parts[1]
                                ),
                            },
                            2 => Schedule::Interval {
                                minutes: self.schedule.minutes,
                            },
                            _ => Schedule::Once {
                                time: format!(
                                    "{:02}:{:02}",
                                    self.schedule.time_parts[0], self.schedule.time_parts[1]
                                ),
                            },
                        },
                    }));
                }
                if self.schedule.editing.is_some() && ui.button("取消编辑").clicked() {
                    self.schedule.editing = None;
                    self.schedule.target = None;
                    self.schedule.name.clear();
                    self.schedule.prompt.clear();
                }
            });
            ui.separator();
            if let Some(state) = &self.schedule.state {
                if state.tasks().is_empty() {
                    ui.weak("暂无定时任务");
                }
                for task in state.tasks() {
                    ui.push_id(task.id, |ui| {
                        super::style::card().show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal_wrapped(|ui| {
                                let mut enabled = task.enabled;
                                if ui.checkbox(&mut enabled, "启用").changed() {
                                    action = Some(ScheduleAction::Toggle(task.id));
                                }
                                ui.add(egui::Label::new(&task.spec.name).wrap());
                                if ui
                                    .add_enabled(
                                        !task.status.active(),
                                        egui::Button::new("编辑").small(),
                                    )
                                    .clicked()
                                {
                                    self.schedule.editing = Some(task.id);
                                    self.schedule.name = task.spec.name.clone();
                                    self.schedule.prompt = task.spec.prompt.clone();
                                    self.schedule.target = Some(task.spec.target.clone());
                                    match &task.spec.schedule {
                                        Schedule::Once { time } => {
                                            self.schedule.kind = 0;
                                            for (value, part) in self
                                                .schedule
                                                .time_parts
                                                .iter_mut()
                                                .zip(time.split(':'))
                                            {
                                                *value = part.parse().unwrap_or_default();
                                            }
                                        }
                                        Schedule::Daily { time } => {
                                            self.schedule.kind = 1;
                                            for (value, part) in self
                                                .schedule
                                                .time_parts
                                                .iter_mut()
                                                .zip(time.split(':'))
                                            {
                                                *value = part.parse().unwrap_or_default();
                                            }
                                        }
                                        Schedule::Interval { minutes } => {
                                            self.schedule.kind = 2;
                                            self.schedule.minutes = *minutes;
                                        }
                                    }
                                }
                                if ui
                                    .add_enabled(
                                        !task.status.active(),
                                        egui::Button::new("删除").small(),
                                    )
                                    .clicked()
                                {
                                    action = Some(ScheduleAction::Delete(task.id));
                                }
                                if ui
                                    .add_enabled(
                                        !task.status.active() && task.status != Status::Pending,
                                        egui::Button::new("立即排队").small(),
                                    )
                                    .clicked()
                                {
                                    action = Some(ScheduleAction::Queue(task.id));
                                }
                            });
                            ui.weak(match &task.spec.schedule {
                                Schedule::Once { time } => format!("单次 {time}"),
                                Schedule::Daily { time } => format!("每天 {time}"),
                                Schedule::Interval { minutes } => format!("每 {minutes} 分钟"),
                            });
                            ui.add(egui::Label::new(&task.spec.prompt).truncate())
                                .on_hover_text(&task.spec.prompt);
                            let target = &task.spec.target;
                            ui.add(
                                egui::Label::new(format!(
                                    "{} · {} · {}",
                                    target.character,
                                    target.workspace.as_ref().unwrap_or(&target.root).display(),
                                    if target.development_mode {
                                        "开发模式"
                                    } else {
                                        "普通模式"
                                    }
                                ))
                                .wrap(),
                            );
                            ui.label(task.status.label());
                            if task.status.active() && ui.small_button("中断后台任务").clicked()
                            {
                                action = Some(ScheduleAction::Stop);
                            }
                            if task.enabled
                                && let Some(next) = clock::local_time(task.next_due)
                            {
                                ui.weak(format!("下次：{} {}", next.day, next.label()));
                            }
                            if let Some(last) = task.last_run.and_then(clock::local_time) {
                                ui.weak(format!("上次提交：{} {}", last.day, last.label()));
                            }
                        });
                    });
                }
            }
        });
        if !self.schedule.error.is_empty() {
            ui.colored_label(egui::Color32::from_rgb(180, 45, 45), &self.schedule.error);
        }
        action
    }
    pub(super) fn apply_schedule_action(&mut self, action: ScheduleAction) {
        let Some(state) = &mut self.schedule.state else {
            return;
        };
        let saving = matches!(action, ScheduleAction::Save(_));
        let deleting = match &action {
            ScheduleAction::Delete(id) => Some(*id),
            _ => None,
        };
        let result = match action {
            ScheduleAction::Save(mut spec) => {
                if let Some(client) = &self.codex {
                    spec.name = client.sanitize(&spec.name);
                    spec.prompt = client.sanitize(&spec.prompt);
                }
                state.save(self.schedule.editing, spec, clock::now())
            }
            ScheduleAction::Toggle(id) => state.toggle(id, clock::now()),
            ScheduleAction::Delete(id) => state.delete(id),
            ScheduleAction::Queue(id) => state.queue(id),
            ScheduleAction::MarkRead => state.mark_read(),
            ScheduleAction::Stop => {
                if let Some(executor) = &self.schedule.executor {
                    executor.interrupt();
                }
                Ok(())
            }
        };
        if result.is_ok() && (saving || (deleting.is_some() && deleting == self.schedule.editing)) {
            self.schedule.editing = None;
            self.schedule.target = None;
            self.schedule.name.clear();
            self.schedule.prompt.clear();
        }
        self.schedule.error = result.err().unwrap_or_default();
    }
    fn schedule_history_ui(&mut self, ui: &mut egui::Ui) -> Option<ScheduleAction> {
        let mut action = None;
        ui.horizontal(|ui| {
            ui.weak("保留最近 200 次执行，删除任务仍保留历史。");
            if ui.small_button("全部标为已读").clicked() {
                action = Some(ScheduleAction::MarkRead);
            }
        });
        let Some(state) = &self.schedule.state else {
            ui.colored_label(egui::Color32::RED, &self.schedule.error);
            return action;
        };
        if state.history().is_empty() {
            ui.weak("暂无执行历史");
        }
        if let Some(run) = state
            .history()
            .iter()
            .find(|r| Some(r.id) == self.schedule.selected_run)
        {
            super::style::card().show(ui, |ui| {
                ui.heading(&run.spec.name);
                ui.label(run.status.label());
                ui.add(
                    egui::Label::new(format!(
                        "工作目录：{}",
                        run.spec
                            .target
                            .workspace
                            .as_ref()
                            .unwrap_or(&run.spec.target.root)
                            .display()
                    ))
                    .wrap(),
                );
                ui.label(format!(
                    "角色：{} · {}",
                    run.spec.target.character,
                    if run.spec.target.development_mode {
                        "开发模式"
                    } else {
                        "普通模式"
                    }
                ));
                if let Some(time) = clock::local_time(run.started_at) {
                    ui.label(format!("开始：{} {}", time.day, time.label()));
                }
                if let Some(time) = run.finished_at.and_then(clock::local_time) {
                    ui.label(format!("结束：{} {}", time.day, time.label()));
                }
                ui.collapsing("提示词", |ui| {
                    ui.add(egui::Label::new(&run.spec.prompt).wrap());
                });
                if !run.error.is_empty() {
                    ui.colored_label(egui::Color32::RED, &run.error);
                }
                ui.horizontal(|ui| {
                    ui.strong("执行回复");
                    if ui.small_button("复制回复").clicked() {
                        ui.ctx().copy_text(run.output.clone());
                    }
                });
                let lines: Vec<_> = run.output.lines().collect();
                egui::ScrollArea::both()
                    .id_salt(("scheduled-output", run.id))
                    .max_height(180.)
                    .min_scrolled_height(0.)
                    .show_rows(
                        ui,
                        ui.text_style_height(&egui::TextStyle::Monospace),
                        lines.len(),
                        |ui, range| {
                            for line in &lines[range] {
                                ui.monospace(*line);
                            }
                        },
                    );
                if run.output.chars().count() >= crate::backend::schedule::MAX_OUTPUT_CHARS {
                    ui.weak("历史回复最多保留 64000 字符，后续内容已截断。");
                }
            });
        }
        for run in state.history() {
            ui.push_id(run.id, |ui| {
                let time = clock::local_time(run.started_at)
                    .map(|t| format!("{} {}", t.day, t.label()))
                    .unwrap_or_default();
                let label = format!(
                    "{}{} · {} · {}",
                    if run.unread { "● " } else { "" },
                    time,
                    run.spec.name,
                    run.status.label()
                );
                if ui
                    .add(
                        egui::Button::new(label)
                            .selected(self.schedule.selected_run == Some(run.id))
                            .wrap(),
                    )
                    .clicked()
                {
                    self.schedule.selected_run = Some(run.id);
                }
            });
        }
        if !self.schedule.error.is_empty() {
            ui.colored_label(egui::Color32::RED, &self.schedule.error);
        }
        action
    }
    fn finish_scheduled_run(&mut self) {
        let (Some(id), Some(outcome), Some(state)) = (
            self.schedule.active,
            &self.schedule.outcome,
            &mut self.schedule.state,
        ) else {
            return;
        };
        match state.finish(id, outcome, clock::now()) {
            Ok(()) => {
                self.schedule.active = None;
                self.schedule.outcome = None;
                self.schedule.error.clear();
            }
            Err(error) => self.schedule.error = error,
        }
    }
    pub(super) fn stop_schedules(&mut self) {
        if let Some(executor) = self.schedule.executor.take() {
            self.schedule.outcome = Some(executor.interrupted());
            drop(executor);
        }
        self.finish_scheduled_run();
    }
    pub(super) fn poll_schedules(&mut self, ctx: &egui::Context) {
        if let Some(executor) = &mut self.schedule.executor {
            for event in executor.poll() {
                match event {
                    ExecutionEvent::Prompt(prompt) => self
                        .codex_prompts
                        .push_back(super::codex::PendingPrompt::scheduled(prompt)),
                    ExecutionEvent::Sent => {
                        if let (Some(id), Some(state)) =
                            (self.schedule.active, &mut self.schedule.state)
                            && let Err(error) = state.sent(id)
                        {
                            self.schedule.error = error;
                        }
                    }
                    ExecutionEvent::Finished(outcome) => self.schedule.outcome = Some(outcome),
                }
            }
            // Continue draining with a frame budget, independently of the main Client.
            ctx.request_repaint_after(Duration::from_millis(20));
        }
        if self.schedule.outcome.is_some() && self.schedule.executor.take().is_some() {
            self.codex_prompts.retain(|prompt| !prompt.scheduled);
            self.finish_scheduled_run();
        }
        if self.schedule.last_poll.elapsed() < Duration::from_secs(1) {
            return;
        }
        self.schedule.last_poll = Instant::now();
        if self.schedule.outcome.is_some() {
            self.finish_scheduled_run();
        }
        let now = clock::now();
        let Some(state) = &mut self.schedule.state else {
            return;
        };
        if let Err(error) = state.poll(now) {
            self.schedule.error = error;
            return;
        }
        if self.schedule.active.is_some() {
            return;
        }
        let Some(id) = state
            .tasks()
            .iter()
            .find(|t| t.status == Status::Pending && t.spec.target.root == self.root)
            .map(|t| t.id)
        else {
            return;
        };
        let run = match state.begin(id, now) {
            Ok(run) => run,
            Err(error) => {
                self.schedule.error = error;
                return;
            }
        };
        self.schedule.active = Some(run.id);
        let project = self
            .development
            .book
            .projects
            .iter()
            .find(|project| Some(&project.path) == run.spec.target.workspace.as_ref());
        let mut context = crate::development::context::project_manifest(
            project,
            run.spec.target.development_mode,
            &run.spec.target.character,
        );
        context.turn = 1;
        context.blocks.insert(
            0,
            crate::development::context::Block {
                id: "input".into(),
                title: "定时任务提示词".into(),
                category: "输入".into(),
                source: "定时任务".into(),
                included: true,
                scope: "本轮首个文本块".into(),
                ..Default::default()
            },
        );
        let wake = ctx.clone();
        match Executor::start(&self.config.codex, &run, context, move || {
            wake.request_repaint()
        }) {
            Ok(executor) => self.schedule.executor = Some(executor),
            Err(error) => {
                let error = self
                    .codex
                    .as_ref()
                    .map_or(error.clone(), |client| client.sanitize(&error));
                self.schedule.outcome = Some(Outcome {
                    status: Status::Failed,
                    output: String::new(),
                    error,
                });
                self.finish_scheduled_run();
            }
        }
    }
    pub(super) fn schedule_notice_ui(&mut self, ctx: &egui::Context) {
        let Some(state) = &self.schedule.state else {
            return;
        };
        let unread = state.history().iter().filter(|run| run.unread).count();
        if unread == 0 {
            return;
        }
        let latest = state.history().iter().find(|run| run.unread).unwrap();
        let mut view = false;
        let mut dismiss = false;
        let area = egui::Area::new("scheduled-completion-notice".into())
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0., 8.))
            .show(ctx, |ui| {
                super::style::card().show(ui, |ui| {
                    ui.set_width(300.);
                    ui.label(format!("定时任务有 {unread} 条执行结果"));
                    ui.add(
                        egui::Label::new(format!(
                            "{}：{}",
                            latest.spec.name,
                            latest.status.label()
                        ))
                        .truncate(),
                    );
                    ui.horizontal(|ui| {
                        view = ui.button("查看执行历史").clicked();
                        dismiss = ui.small_button("已阅").clicked();
                    });
                });
            });
        self.input_regions.push(area.response.rect);
        if view {
            self.schedule.selected_run = Some(latest.id);
            self.schedule.history_open = true;
            self.schedule.open = true;
        }
        if view || dismiss {
            self.apply_schedule_action(ScheduleAction::MarkRead);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::codex::Client;

    #[test]
    #[cfg(unix)]
    fn scheduled_background_execution_preserves_busy_chat_and_restores_unread_history() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let fixture = root.path().join("server.py");
        let script = root.path().join("codex-fixture");
        std::fs::write(
            &fixture,
            include_str!("../../tests/fixtures/codex-server.py"),
        )
        .unwrap();
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\nexec python3 -X utf8 -u '{}' schedule\n",
                fixture.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
        let ctx = egui::Context::default();
        let mut app = Wife::from_root(&ctx, false, root.path().to_path_buf());
        app.codex = Some(Client::mock("normal"));
        app.input = "interrupt".into();
        app.submit(&ctx);
        let deadline = Instant::now() + Duration::from_secs(5);
        while !app.text.contains("第1轮：interrupt") && Instant::now() < deadline {
            app.poll_codex();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(app.busy && app.text.contains("第1轮：interrupt"));
        let chat_text = app.text.clone();
        app.input = "保留主窗草稿".into();
        app.config.codex.executable = script.to_string_lossy().into();
        let path = root.path().join("schedules.json");
        let mut state = ScheduleState::load(path.clone()).unwrap();
        let workspace = root.path().join("different-project");
        std::fs::create_dir(&workspace).unwrap();
        state
            .save(
                None,
                TaskSpec {
                    name: "后台检查".into(),
                    prompt: "夜间检查结果".into(),
                    schedule: Schedule::Interval { minutes: 1 },
                    target: Target {
                        root: app.root.clone(),
                        workspace: Some(workspace),
                        character: "other-character".into(),
                        development_mode: true,
                    },
                },
                clock::now(),
            )
            .unwrap();
        let id = state.tasks()[0].id;
        state.queue(id).unwrap();
        app.schedule.state = Some(state);
        let deadline = Instant::now() + Duration::from_secs(5);
        while app
            .schedule
            .state
            .as_ref()
            .unwrap()
            .history()
            .first()
            .is_none_or(|run| run.finished_at.is_none())
            && Instant::now() < deadline
        {
            app.poll_schedules(&ctx);
            app.poll_codex();
            std::thread::sleep(Duration::from_millis(5));
        }
        let history = app.schedule.state.as_ref().unwrap().history();
        assert_eq!(history.len(), 1);
        assert!(
            history[0].status == Status::Completed,
            "{}",
            history[0].error
        );
        assert!(history[0].output.contains("夜间检查结果"));
        assert!(history[0].unread);
        assert!(app.busy);
        assert_eq!(app.text, chat_text);
        assert_eq!(app.input, "保留主窗草稿");
        assert!(!app.config.codex.no_approval);
        assert!(!app.development_turn);
        let _ = ctx.run(egui::RawInput::default(), |ctx| app.schedule_notice_ui(ctx));
        assert_eq!(app.text, chat_text);
        assert!(!app.input_regions.is_empty());
        app.apply_schedule_action(ScheduleAction::Delete(id));
        drop(app);
        let restored = ScheduleState::load(path).unwrap();
        assert!(restored.tasks().is_empty());
        assert!(restored.history()[0].unread);
        assert!(restored.history()[0].output.contains("夜间检查结果"));
    }

    #[test]
    fn scheduled_questions_survive_main_completion_and_shutdown_records_interruption() {
        let root = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        let mut app = Wife::from_root(&ctx, false, root.path().to_path_buf());
        app.codex = Some(Client::mock_events(
            vec![crate::backend::codex::Event::Finished { interrupted: false }],
            Vec::new(),
        ));
        let path = root.path().join("schedules.json");
        let mut state = ScheduleState::load(path.clone()).unwrap();
        state
            .save(
                None,
                TaskSpec {
                    name: "后台提问".into(),
                    prompt: "question".into(),
                    schedule: Schedule::Interval { minutes: 1 },
                    target: app.schedule_target(),
                },
                clock::now(),
            )
            .unwrap();
        let id = state.tasks()[0].id;
        state.queue(id).unwrap();
        let run = state.begin(id, clock::now()).unwrap();
        app.schedule.state = Some(state);
        app.schedule.active = Some(run.id);
        app.schedule.executor = Some(Executor::mock("schedule", "question"));
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.codex_prompts.is_empty() && Instant::now() < deadline {
            app.poll_schedules(&ctx);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(app.codex_prompts.len(), 1);
        assert!(app.codex_prompts[0].scheduled);
        app.poll_codex();
        assert_eq!(app.codex_prompts.len(), 1);
        app.stop_schedules();
        assert!(app.schedule.executor.is_none());
        let restored = ScheduleState::load(path).unwrap();
        assert!(restored.history()[0].status == Status::Interrupted);
        assert!(restored.history()[0].unread);
    }
    #[test]
    fn scheduled_completion_is_retained_until_history_write_succeeds() {
        let root = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        let mut app = Wife::from_root(&ctx, false, root.path().to_path_buf());
        let path = root.path().join("schedules.json");
        let mut state = ScheduleState::load(path.clone()).unwrap();
        state
            .save(
                None,
                TaskSpec {
                    name: "写入失败恢复".into(),
                    prompt: "检查".into(),
                    schedule: Schedule::Interval { minutes: 1 },
                    target: app.schedule_target(),
                },
                clock::now(),
            )
            .unwrap();
        let id = state.tasks()[0].id;
        state.queue(id).unwrap();
        let run = state.begin(id, clock::now()).unwrap();
        app.schedule.state = Some(state);
        app.schedule.active = Some(run.id);
        app.schedule.outcome = Some(Outcome {
            status: Status::Completed,
            output: "不能丢失的结果".into(),
            error: String::new(),
        });
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        app.finish_scheduled_run();
        assert_eq!(app.schedule.active, Some(run.id));
        assert!(app.schedule.outcome.is_some());
        assert!(!app.schedule.error.is_empty());
        std::fs::remove_dir(&path).unwrap();
        app.finish_scheduled_run();
        assert!(app.schedule.active.is_none() && app.schedule.outcome.is_none());
        assert_eq!(
            ScheduleState::load(path).unwrap().history()[0].output,
            "不能丢失的结果"
        );
    }
    #[test]
    fn scheduled_time_controls_save_padded_time_and_reopen_preserves_drafts() {
        let root = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        let mut app = Wife::from_root(&ctx, false, root.path().to_path_buf());
        let path = root.path().join("schedules.json");
        app.schedule.state = Some(ScheduleState::load(path.clone()).unwrap());
        app.schedule.open = true;
        app.schedule.time_parts = [9, 5];
        app.schedule.name = "早间任务".into();
        app.schedule.prompt = "检查当天任务".into();
        let mut frame = |events| {
            let mut action = None;
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(640., 620.),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default()
                        .show(ctx, |ui| action = app.schedule_panel(ui, &mut false));
                },
            );
            if let Some(action) = action {
                app.apply_schedule_action(action);
            }
            output
        };
        let output = frame(vec![]);
        for label in ["09", "05", "时", "分"] {
            assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text == label)), "missing {label}");
        }
        let point = output
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape
                    && text.galley.job.text == "添加任务"
                {
                    Some(text.pos + text.galley.size() / 2.)
                } else {
                    None
                }
            })
            .unwrap();
        frame(vec![egui::Event::PointerMoved(point)]);
        for pressed in [true, false] {
            frame(vec![egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            }]);
        }
        let state = ScheduleState::load(path).unwrap();
        assert!(
            matches!(&state.tasks()[0].spec.schedule, Schedule::Once { time } if time == "09:05")
        );
        assert!(app.schedule.name.is_empty() && app.schedule.prompt.is_empty());
        app.schedule.prompt = "关闭窗口保留草稿".into();
        app.schedule.open = false;
        app.schedule.open = true;
        assert_eq!(app.schedule.prompt, "关闭窗口保留草稿");
        assert!(!app.settings && app.codex.is_none());
    }
}
