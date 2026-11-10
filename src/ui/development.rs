//! Native development viewport and UI session coordination.
use super::{app::Wife, input::MessageSource};
use crate::development::{Task, Workspaces};
use eframe::egui;
use std::{collections::HashMap, path::PathBuf, sync::mpsc, time::Duration};

#[derive(Default)]
pub(super) struct DevelopmentUi {
    pub open: bool,
    pub page: u8,
    pub diagnostics_ui: super::development_diagnostics::DiagnosticsUi,
    pub memory_ui: super::development_memory::MemoryUi,
    pub tests_ui: super::development_tests::TestsUi,
    pub memory_context: String,
    viewport_open: bool,
    branch: String,
    branch_checked: Option<std::time::Instant>,
    branch_project: Option<PathBuf>,
    pub book: Workspaces,
    pub storage: Option<PathBuf>,
    pub storage_ok: bool,
    pub reset_sessions_pending: bool,
    pub session_mode: bool,
    pub thread_id: Option<String>,
    pub error: String,
    path_input: String,
    input: String,
    completion: super::completion::Completion,
    supplement: Option<PendingSupplement>,
    drafts: HashMap<PathBuf, String>,
    viewed_task: Option<u64>,
    detail_task: Option<(PathBuf, u64)>,
    search: String,
    dirty: bool,
    last_saved: Option<std::time::Instant>,
    save_worker: Option<std::thread::JoinHandle<Result<(), String>>>,
    picker: Option<mpsc::Receiver<Option<PathBuf>>>,
    pending_project: Option<PathBuf>,
}

struct PendingSupplement {
    draft: String,
    project: PathBuf,
    task: u64,
    turn: usize,
}

impl Wife {
    pub(super) fn complete_development_supplement(&mut self, result: Result<(), String>) {
        let Some(pending) = self.development.supplement.take() else {
            return;
        };
        if let Err(error) = result {
            self.development.error = error;
            return;
        }
        let text = self.codex.as_ref().map_or_else(
            || pending.draft.trim().to_owned(),
            |client| client.sanitize(pending.draft.trim()),
        );
        if let Some(turn) = self
            .development
            .book
            .projects
            .iter_mut()
            .find(|project| project.path == pending.project)
            .and_then(|project| {
                project
                    .tasks
                    .iter_mut()
                    .find(|task| task.id == pending.task)
            })
            .and_then(|task| task.turns.get_mut(pending.turn))
        {
            turn.supplement(text);
        }
        let draft = if self.development.book.active.as_ref() == Some(&pending.project) {
            Some(&mut self.development.input)
        } else {
            self.development.drafts.get_mut(&pending.project)
        };
        if let Some(draft) = draft
            && *draft == pending.draft
        {
            draft.clear();
        }
        self.development.error.clear();
        self.save_development(true);
    }
    pub(super) fn load_development_workspaces(&mut self) {
        let path = crate::tools::paths::data_root().join(if self.dev {
            "development-dev.json"
        } else {
            "development.json"
        });
        match Workspaces::read(&path) {
            Ok(book) => {
                self.development.book = book;
                self.development.storage_ok = true;
            }
            Err(error) => self.development.error = error,
        }
        self.development.storage = Some(path);
    }
    pub(super) fn save_development(&mut self, force: bool) {
        self.development.dirty = true;
        self.complete_development_save(force);
        if self.development.save_worker.is_some() {
            return;
        }
        if !force
            && self
                .development
                .last_saved
                .is_some_and(|saved| saved.elapsed().as_secs() < 2)
        {
            return;
        }
        if self.development.storage_ok
            && let Some(path) = &self.development.storage
        {
            self.development.last_saved = Some(std::time::Instant::now());
            if force {
                match self.development.book.save(path) {
                    Ok(()) => self.development.dirty = false,
                    Err(error) => self.development.error = error,
                }
            } else {
                let path = path.clone();
                let book = self.development.book.clone();
                self.development.save_worker = Some(std::thread::spawn(move || book.save(&path)));
                self.development.dirty = false;
            }
        }
    }
    fn complete_development_save(&mut self, wait: bool) {
        if self
            .development
            .save_worker
            .as_ref()
            .is_some_and(|worker| wait || worker.is_finished())
            && let Some(worker) = self.development.save_worker.take()
            && let Err(error) = worker
                .join()
                .unwrap_or_else(|_| Err("无法保存 development 记录。".into()))
        {
            self.development.error = error;
            self.development.dirty = true;
        }
    }
    pub(super) fn switch_development_workspace(
        &mut self,
        path: Option<PathBuf>,
        ctx: &egui::Context,
    ) -> Result<(), String> {
        if self.busy {
            return Err("当前任务仍在运行，请先停止或等待完成。".into());
        }
        let path = path
            .map(|path| self.development.book.add(&path))
            .transpose()?;
        if path == self.development.book.active {
            return Ok(());
        }
        if let Some(previous) = &self.development.book.active {
            self.development.drafts.insert(
                previous.clone(),
                std::mem::take(&mut self.development.input),
            );
        }
        self.development.book.active = path.clone();
        self.development.diagnostics_ui = Default::default();
        self.development.diagnostics_ui.refresh = true;
        self.development.memory_ui = Default::default();
        self.development.memory_ui.refresh = true;
        self.development.tests_ui = Default::default();
        self.development.input = path
            .as_ref()
            .and_then(|p| self.development.drafts.remove(p))
            .unwrap_or_default();
        self.development.viewed_task = self.development.book.current().and_then(|p| p.current_task);
        self.start_codex(ctx);
        self.save_development(true);
        Ok(())
    }
    pub(super) fn new_development_task(&mut self, ctx: &egui::Context) {
        if self.busy {
            return;
        }
        if let Some(project) = self.development.book.current_mut() {
            project.current_task = None;
        }
        self.development.viewed_task = None;
        self.start_codex(ctx);
        self.save_development(true);
    }
    pub(super) fn record_development_start(&mut self, text: String) {
        self.development
            .book
            .begin(text, &self.library.character, self.development.session_mode);
        if let Some(task) = self.development.book.task_mut() {
            task.thread_id = self.development.thread_id.clone();
            task.memory_context_stamp =
                crate::development::memory::context_stamp(&self.development.memory_context);
        }
        self.development.viewed_task = self.development.book.current().and_then(|p| p.current_task);
        self.save_development(true);
    }
    pub(super) fn finish_development(&mut self, status: &str, error: &str) {
        if let Some(turn) = self.development.book.turn_mut()
            && turn.status == "running"
        {
            turn.finish(status, error);
            self.save_development(true);
        }
    }
    pub(super) fn development_window(&mut self, ctx: &egui::Context) {
        self.complete_development_save(false);
        self.poll_development_test_queue(ctx);
        if let Some(picker) = &self.development.picker {
            match picker.try_recv() {
                Ok(selection) => {
                    self.development.picker = None;
                    self.development.pending_project = selection;
                    if self.development.open {
                        ctx.send_viewport_cmd_to(
                            egui::ViewportId::from_hash_of("lexington-development"),
                            egui::ViewportCommand::Focus,
                        );
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.development.picker = None;
                    self.development.error = "无法打开项目选择器，请重试或手动输入路径。".into();
                }
                Err(mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(Duration::from_millis(100))
                }
            }
        }
        if !self.busy
            && let Some(path) = self.development.pending_project.take()
        {
            self.development.error = self
                .switch_development_workspace(Some(path), ctx)
                .err()
                .unwrap_or_default();
        }
        if self.development.dirty
            && self
                .development
                .last_saved
                .is_none_or(|saved| saved.elapsed().as_secs() >= 2)
        {
            self.save_development(false);
        }
        if !self.development.open {
            self.development.viewport_open = false;
            self.development.detail_task = None;
            return;
        }
        if self.development.branch_project != self.development.book.active
            || self
                .development
                .branch_checked
                .is_none_or(|checked| checked.elapsed().as_secs() >= 5)
        {
            self.development.branch = self
                .development
                .book
                .active
                .as_ref()
                .and_then(|path| crate::development::branch(path))
                .unwrap_or_else(|| "非 Git 项目".into());
            self.development.branch_project = self.development.book.active.clone();
            self.development.branch_checked = Some(std::time::Instant::now());
        }
        let id = egui::ViewportId::from_hash_of("lexington-development");
        let builder = egui::ViewportBuilder::default()
            .with_title("Lexington development")
            .with_inner_size([1040., 740.])
            .with_min_inner_size([740., 500.])
            .with_transparent(false)
            .with_resizable(true)
            .with_window_level(egui::WindowLevel::Normal);
        let mut open = true;
        let mut action = None;
        let mut submit = false;
        let mut new_task = false;
        let mut resume_task = None;
        let mut delete_task = None;
        let mut pick_project = false;
        ctx.show_viewport_immediate(id, builder, |child, class| {
            let mut contents = |ui: &mut egui::Ui| {
                ui.spacing_mut().item_spacing = egui::vec2(10., 8.);
                ui.spacing_mut().button_padding = egui::vec2(12., 6.);
                ui.style_mut().text_styles.insert(egui::TextStyle::Body, egui::FontId::proportional(14.));
                ui.style_mut().text_styles.insert(egui::TextStyle::Button, egui::FontId::proportional(14.));
                let accent = egui::Color32::from_rgb(57, 97, 185);
                let border = egui::Color32::from_rgb(222, 227, 236);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("development").size(23.).strong());
                    let (label, color) = if self.config.codex.development_mode {
                        ("开发模式已开启", accent)
                    } else {
                        ("普通模式", egui::Color32::from_gray(110))
                    };
                    ui.label(egui::RichText::new(label).size(12.).color(color));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.busy && ui.button("停止任务").clicked() && let Some(client) = &self.codex {
                            client.interrupt();
                        }
                        let pending = self.codex_prompts.len();
                        if let Some(prompt) = self.codex_prompts.front_mut() {
                            let question = !prompt.request.questions.is_empty();
                            let label = if question {"回答问题"} else {"处理审批"};
                            if ui.button(egui::RichText::new(format!("{label}（{pending}）")).strong().color(egui::Color32::from_rgb(139, 80, 14))).on_hover_text(&prompt.request.title).clicked() {
                                prompt.reopen();
                            }
                            ui.label(egui::RichText::new(if question {"等待你回答"} else {"等待你确认"}).strong().color(egui::Color32::from_rgb(176, 102, 16)));
                        } else {
                            if self.busy {
                                ui.add(egui::Spinner::new().size(16.).color(accent)).on_hover_text("任务执行中");
                            }
                            ui.add(egui::Label::new(egui::RichText::new(&self.codex_status).size(12.).color(egui::Color32::from_gray(100))).truncate());
                        }
                    });
                });
                egui::Frame::NONE.fill(egui::Color32::WHITE).stroke(egui::Stroke::new(1_f32, border)).corner_radius(9).inner_margin(12).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let can_switch = !self.busy && self.development.picker.is_none() && self.development.pending_project.is_none();
                    ui.horizontal(|ui| {
                        ui.strong("项目");
                        ui.add_enabled_ui(can_switch, |ui| {
                            let selected = self.development.book.active.as_ref().map(|path|path.file_name().unwrap_or(path.as_os_str()).to_string_lossy().into_owned()).unwrap_or_else(||"普通会话".into());
                            egui::ComboBox::from_id_salt("development-workspace")
                                .width((ui.available_width()-245.).clamp(180., 460.))
                                .selected_text(selected)
                                .show_ui(ui, |ui| {
                                    if ui.selectable_label(self.development.book.active.is_none(), "普通会话").clicked() {action = Some(None);}
                                    for project in &self.development.book.projects {
                                        let name = project.path.file_name().unwrap_or(project.path.as_os_str()).to_string_lossy();
                                        if ui.selectable_label(Some(&project.path)==self.development.book.active.as_ref(), name).on_hover_text(project.path.display().to_string()).clicked() {
                                            action = Some(Some(project.path.clone()));
                                        }
                                    }
                                });
                            if ui.button("选择项目…").on_hover_text("通过文件浏览器选择项目文件夹").clicked() {pick_project = true;}
                            if ui.button("新建任务").clicked() {new_task = true;}
                        });
                    });
                    if let Some(path) = &self.development.book.active {
                        ui.horizontal(|ui| {
                            let text = path.display().to_string();
                            ui.add_sized([ui.available_width()-145., 18.], egui::Label::new(egui::RichText::new(&text).size(12.).color(egui::Color32::from_gray(115))).halign(egui::Align::Min).truncate()).on_hover_text(text);
                            ui.add_sized([135.,18.],egui::Label::new(egui::RichText::new(&self.development.branch).size(12.).color(accent)).truncate()).on_hover_text(&self.development.branch);
                        });
                    } else {
                        ui.weak("选择项目文件夹，开始管理开发任务。");
                    }
                    ui.collapsing("手动输入路径", |ui| {
                        ui.add_enabled_ui(can_switch, |ui| {
                            ui.horizontal(|ui| {
                                ui.add(egui::TextEdit::singleline(&mut self.development.path_input).desired_width((ui.available_width()-105.).max(120.)).hint_text("项目目录的绝对路径"));
                                if ui.button("添加项目").clicked() {action = Some(Some(PathBuf::from(self.development.path_input.trim())));}
                            });
                        });
                    });
                    if self.development.picker.is_some() {ui.small("正在选择项目文件夹…");}
                    if let Some(path) = &self.development.pending_project {ui.small(format!("任务结束后切换到：{}", path.display()));}
                });
                if !self.development.error.is_empty() || !self.error.is_empty() {
                    egui::ScrollArea::vertical().id_salt("development-errors").max_height(60.).show(ui, |ui| {
                        for error in [&self.development.error, &self.error] {
                            if !error.is_empty() {ui.colored_label(egui::Color32::from_rgb(175, 55, 55), error);}
                        }
                    });
                }
                if self.development.book.current().is_none() {
                    self.development.detail_task = None;
                    ui.add_space(55.);
                    ui.vertical_centered(|ui| {
                        ui.heading("从一个项目开始");
                        ui.add_space(8.);
                        ui.label("选择项目文件夹，在这里查看任务、执行记录和代码变更。");
                        ui.add_space(14.);
                        if ui.add_enabled(!self.busy && self.development.picker.is_none(), egui::Button::new("选择项目文件夹…").fill(egui::Color32::from_rgb(225, 234, 252))).clicked() {pick_project = true;}
                    });
                    return;
                }
                ui.horizontal(|ui| {
                    for (page,label) in [(0,"任务"),(1,"诊断"),(2,"测试"),(3,"项目记忆")] {
                        if ui.selectable_value(&mut self.development.page,page,label).clicked() {
                            self.development.diagnostics_ui.refresh=true;self.development.memory_ui.refresh=true;
                        }
                    }
                });
                if self.development.page!=0 {
                    self.development.detail_task = None;
                    let page=self.development.page;
                    egui::Frame::NONE.fill(egui::Color32::WHITE).corner_radius(9).inner_margin(14).show(ui,|ui| {
                        ui.set_width(ui.available_width());
                        egui::ScrollArea::vertical().id_salt(("development-page",page)).max_height(ui.available_height()).auto_shrink([false,false]).show(ui,|ui| {
                            match page {1=>self.development_diagnostics_ui(ui),2=>self.development_tests_ui(ui),_=>self.development_memory_ui(ui)}
                        });
                    });
                    return;
                }
                let height = ui.available_height().max(180.);
                let sidebar_width = (ui.available_width()*0.235).clamp(190., 245.);
                ui.allocate_ui_with_layout(egui::vec2(ui.available_width(),height), egui::Layout::left_to_right(egui::Align::Min), |ui| {
                    ui.allocate_ui_with_layout(egui::vec2(sidebar_width,height), egui::Layout::top_down(egui::Align::Min), |ui| {
                        egui::Frame::NONE.fill(egui::Color32::from_rgb(237, 241, 247)).corner_radius(9).inner_margin(12).show(ui, |ui| {
                            ui.set_min_height(height-24.);
                            ui.set_width(sidebar_width-24.);
                            ui.horizontal(|ui| {
                                ui.strong("任务");
                                ui.weak(self.development.book.current().unwrap().tasks.len().to_string());
                            });
                            ui.add(egui::TextEdit::singleline(&mut self.development.search).desired_width(f32::INFINITY).hint_text("搜索任务…"));
                            egui::ScrollArea::vertical().id_salt("development-tasks").max_height((height-105.).max(60.)).auto_shrink([false,false]).show(ui, |ui| {
                                let project = self.development.book.current().unwrap();
                                let mut count = 0;
                                for task in project.tasks.iter().rev().filter(|task|task.title.contains(&self.development.search)) {
                                    count += 1;
                                    let selected = self.development.viewed_task.or(project.current_task)==Some(task.id);
                                    let response = ui.add_sized([ui.available_width(),36.], egui::Button::new(&task.title).selected(selected).truncate());
                                    if response.on_hover_text(format!("{}\n{} · {} 轮",task.title, task.turns.last().map(|turn|status_text(&turn.status)).unwrap_or("未开始"),task.turns.len())).clicked() {
                                        self.development.viewed_task = Some(task.id);
                                        self.development.detail_task = None;
                                        let compatible = task.character==self.library.character && task.development_mode==self.config.codex.development_mode && (!self.config.codex.development_mode || (task.memory_revision==project.memory_revision && task.memory_context_stamp==crate::development::memory::context_stamp(&self.development.memory_context)));
                                        if !self.busy && compatible {
                                            resume_task = Some(task.id);
                                            self.development.error.clear();
                                        } else if project.current_task != Some(task.id) {
                                            self.development.error = if self.busy {"当前任务仍在运行，此记录仅供查看；任务结束后点击即可切换。"} else {"此任务与当前角色、开发模式或项目记忆不兼容，仅供查看。"}.into();
                                        }
                                    }
                                }
                                if count==0 {ui.weak(if project.tasks.is_empty() {"还没有任务"} else {"没有匹配的任务"});}
                            });
                        });
                    });
                    ui.allocate_ui_with_layout(egui::vec2(ui.available_width(),height), egui::Layout::top_down(egui::Align::Min), |ui| {
                        let project = self.development.book.current().unwrap();
                        let task = project.tasks.iter().find(|task|Some(task.id)==self.development.viewed_task.or(project.current_task));
                        let detail_task = task.map(|task| (project.path.clone(), task.id));
                        let scroll_to_bottom = self.development.detail_task != detail_task;
                        self.development.detail_task = detail_task;
                        egui::Frame::NONE.fill(egui::Color32::WHITE).stroke(egui::Stroke::new(1_f32,border)).corner_radius(9).inner_margin(14).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            if let Some(task) = task {
                                ui.add(egui::Label::new(egui::RichText::new(&task.title).size(17.).strong()).truncate()).on_hover_text(&task.title);
                                ui.horizontal(|ui| {
                                    ui.weak(format!("{} 轮对话",task.turns.len()));
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.add_enabled(!self.busy,egui::Button::new("删除记录").small()).clicked() {delete_task = Some(task.id);}
                                    });
                                });
                            } else {ui.label(egui::RichText::new("新任务").size(17.).strong());ui.weak("在下方描述希望完成的工作。");}
                            ui.separator();
                            let detail_height = (height-260.).max(12.);
                            egui::ScrollArea::vertical().id_salt(("development-detail",&project.path,task.map(|task|task.id))).max_height(detail_height).min_scrolled_height(0.).auto_shrink([false,false]).stick_to_bottom(true).animated(!scroll_to_bottom).show(ui, |ui| {
                                if let Some(task) = task {task_details(ui,task,&project.path);}
                                else {ui.add_space(20.);ui.weak("任务的对话、执行记录和代码变更会显示在这里。");}
                                if scroll_to_bottom {ui.scroll_to_cursor(Some(egui::Align::BOTTOM));}
                            });
                        });
                        egui::Frame::NONE.fill(egui::Color32::WHITE).stroke(egui::Stroke::new(1_f32,border)).corner_radius(9).inner_margin(12).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            let current = self.development.book.task().map(|task|task.title.as_str()).unwrap_or("新任务");
                            ui.add(egui::Label::new(egui::RichText::new(format!("发送到：{current}")).size(12.).color(egui::Color32::from_gray(100))).truncate()).on_hover_text(current);
                            let editor_id = egui::Id::new("development-input");
                            self.development.completion.keyboard(child, editor_id, &mut self.development.input);
                            let send_shortcut = ui.memory(|memory|memory.has_focus(editor_id)) && ui.input_mut(|input|input.consume_key(egui::Modifiers::NONE,egui::Key::Enter));
                            let mut editor_focused = false;
                            let mut editor_rect = egui::Rect::NOTHING;
                            egui::ScrollArea::vertical().id_salt("development-editor-scroll").max_height(64.).show(ui, |ui| {
                                let editor = ui.add(egui::TextEdit::multiline(&mut self.development.input).id(editor_id).return_key(egui::KeyboardShortcut::new(egui::Modifiers::CTRL,egui::Key::Enter)).frame(false).desired_rows(3).desired_width(f32::INFINITY).hint_text("描述需求，/ 补全指令…"));
                                editor_focused = editor.has_focus();
                                editor_rect = editor.rect;
                            });
                            self.development.completion.show(ui, editor_id, editor_rect, &mut self.development.input);
                            if class == egui::ViewportClass::Embedded && let Some(rect) = self.development.completion.bounds { self.input_regions.push(rect); }
                            ui.horizontal(|ui| {
                                ui.weak("Enter 发送 · Ctrl+Enter 换行");
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    let enabled = self.codex_prompts.is_empty() && (!self.busy || (self.development_turn && self.development.tests_ui.running.is_none())) && self.development.supplement.is_none() && self.development.picker.is_none() && self.development.pending_project.is_none() && !self.development.input.trim().is_empty();
                                    let label = if !self.codex_prompts.is_empty() {"等待处理"} else if self.development.supplement.is_some() {"确认中…"} else if self.busy && self.development_turn {"补充任务"} else {"发送"};
                                    let button = egui::Button::new(egui::RichText::new(label).color(egui::Color32::WHITE)).fill(accent).min_size(egui::vec2(76.,30.));
                                    if ui.add_enabled(enabled,button).clicked() || (enabled && editor_focused && send_shortcut) {submit = true;}
                                });
                            });
                        });
                    });
                });
            };
            if class==egui::ViewportClass::Embedded {
                if let Some(window) = egui::Window::new("development").open(&mut open).default_size([1000.,700.]).frame(egui::Frame::window(&child.style()).fill(egui::Color32::from_rgb(247,249,252)).inner_margin(16)).show(child,&mut contents) {
                    self.input_regions.push(window.response.rect);
                }
            } else {
                if child.input(|input|input.viewport().close_requested()) {open = false;}
                egui::CentralPanel::default().frame(egui::Frame::central_panel(&child.style()).fill(egui::Color32::from_rgb(247,249,252)).inner_margin(16)).show(child,&mut contents);
            }
        });
        self.development.viewport_open = true;
        self.development.open = open;
        if !open {
            self.development.detail_task = None;
            ctx.send_viewport_cmd_to(id, egui::ViewportCommand::Close);
        }
        if pick_project && !self.busy && self.development.picker.is_none() {
            let initial = self.development.book.active.clone();
            let (sender, receiver) = mpsc::channel();
            let wake = ctx.clone();
            ctx.send_viewport_cmd_to(
                id,
                egui::ViewportCommand::WindowLevel(egui::WindowLevel::Normal),
            );
            std::thread::spawn(move || {
                let selection =
                    crate::tools::file_dialog::pick_project_directory(initial.as_deref());
                let _ = sender.send(selection);
                wake.request_repaint();
            });
            self.development.picker = Some(receiver);
        }
        if let Some(path) = action {
            self.development.error = self
                .switch_development_workspace(path, ctx)
                .err()
                .unwrap_or_default();
        }
        if new_task {
            self.new_development_task(ctx);
        }
        if let Some(id) = resume_task
            && !self.busy
            && let Some(project) = self.development.book.current_mut()
            && project.current_task != Some(id)
        {
            project.current_task = Some(id);
            self.start_codex(ctx);
            self.save_development(true);
        }
        if let Some(id) = delete_task
            && let Some(project) = self.development.book.current_mut()
        {
            let active = project.current_task == Some(id);
            project.tasks.retain(|t| t.id != id);
            if active {
                project.current_task = None;
            }
            self.development.viewed_task = None;
            if active {
                self.start_codex(ctx);
            }
            self.save_development(true);
        }
        self.apply_project_memory(ctx);
        self.apply_development_test(ctx);
        if std::mem::take(&mut self.development.diagnostics_ui.analyze) {
            self.send_message(
                ctx,
                "请根据当前项目诊断定位原因，先核对对应的代码和未保存版本，给出修复建议。".into(),
                MessageSource::Development,
            );
            self.development.page = 0;
        }
        if submit {
            let text = self.development.input.trim().to_owned();
            if self.busy {
                if let (Some(project), Some(task)) = (
                    self.development.book.active.clone(),
                    self.development
                        .book
                        .task()
                        .filter(|task| !task.turns.is_empty()),
                ) {
                    let pending = PendingSupplement {
                        draft: self.development.input.clone(),
                        project,
                        task: task.id,
                        turn: task.turns.len() - 1,
                    };
                    let result = self
                        .codex
                        .as_ref()
                        .ok_or_else(|| "Codex 未启动。".to_owned())
                        .and_then(|client| client.steer(text));
                    match result {
                        Ok(()) => self.development.supplement = Some(pending),
                        Err(error) => self.development.error = error,
                    }
                } else {
                    self.development.error = "当前没有可补充的开发任务。".into();
                }
            } else {
                self.send_message(ctx, text, MessageSource::Development);
                if self.busy {
                    self.development.input.clear();
                }
            }
        }
    }
}

pub(super) fn timestamp_text(timestamp: u64) -> String {
    if timestamp == 0 {
        return "未记录".into();
    }
    i64::try_from(timestamp)
        .ok()
        .and_then(crate::tools::time::local)
        .map(|date| {
            format!(
                "{}-{:02}-{:02} {:02}:{:02}",
                date.tm_year + 1900,
                date.tm_mon + 1,
                date.tm_mday,
                date.tm_hour,
                date.tm_min
            )
        })
        .unwrap_or_else(|| "时间未知".into())
}

pub(super) fn status_text(status: &str) -> &str {
    match status {
        "running" | "inProgress" | "in_progress" => "进行中",
        "completed" => "已结束",
        "interrupted" => "已中断",
        "passed" => "通过",
        "cancelled" => "已取消",
        "failed" => "失败",
        "declined" => "已拒绝",
        "pending" => "待处理",
        _ => status,
    }
}

fn task_details(ui: &mut egui::Ui, task: &Task, project: &std::path::Path) {
    for (index, turn) in task.turns.iter().enumerate() {
        ui.push_id(index, |ui| {
            ui.label(format!(
                "第 {} 轮 · {} · {}",
                index + 1,
                status_text(&turn.status),
                task.character
            ));
            ui.label(
                egui::RichText::new("你")
                    .strong()
                    .color(egui::Color32::from_rgb(57, 97, 185)),
            );
            egui::Frame::NONE
                .fill(egui::Color32::from_rgb(242, 246, 253))
                .corner_radius(7)
                .inner_margin(10)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    rich_text(ui, &turn.user);
                });
            if let Some(manifest) = &turn.manifest {
                ui.collapsing("本轮发送清单", |ui| {
                    super::roleplay_context::manifest_ui(ui, manifest);
                });
            }
            ui.collapsing("事件记录与离线核对", |ui| {
                if let Some(journal) = &turn.journal {
                    ui.small(format!(
                        "已保留 {} 条观察记录；流式内容按处理批次保存快照，不重新执行工具。",
                        journal.events.len()
                    ));
                    if journal.compacted {
                        ui.small("较早事件已合并为检查点，仍可核对当前任务快照。");
                    }
                    if journal.truncated {
                        ui.colored_label(
                            egui::Color32::DARK_RED,
                            "任务快照超过日志容量，事件记录不完整。",
                        );
                    }
                    let watermark = (task.id, journal.events.last().map(|event| event.seq));
                    let result_id = ui.id().with((project, "replay"));
                    if ui.button("离线核对本轮记录").clicked() {
                        ui.data_mut(|data| {
                            data.insert_temp(result_id, (watermark, turn.verify_replay()))
                        });
                    }
                    if let Some((checked, result)) = ui.data(|data| {
                        data.get_temp::<((u64, Option<u64>), Result<(), String>)>(result_id)
                    }) && checked == watermark
                    {
                        ui.label(
                            result.map_or_else(
                                |error| error,
                                |_| "事件回放与当前任务快照一致。".into(),
                            ),
                        );
                    }
                    let row_height = ui.text_style_height(&egui::TextStyle::Body);
                    // This list lives inside the task's outer scrolling content.
                    // Reserve its bounded height even when its current position
                    // is below that viewport; available_height may be negative.
                    let timeline_height = ((row_height + ui.spacing().item_spacing.y)
                        * journal.events.len().min(8) as f32)
                        .min(160.);
                    egui::ScrollArea::vertical()
                        .max_height(160.)
                        .min_scrolled_height(timeline_height)
                        .show_rows(ui, row_height, journal.events.len(), |ui, rows| {
                            for record in &journal.events[rows] {
                                ui.label(format!(
                                    "#{} · {} · {}",
                                    record.seq,
                                    timestamp_text(record.time_ms / 1000),
                                    record.data.label()
                                ));
                            }
                        });
                } else {
                    ui.label("旧任务保留原有历史，没有可回放的事件记录。");
                }
            });
            if let Some(plan) = turn.plan["plan"].as_array() {
                ui.collapsing("计划", |ui| {
                    for step in plan {
                        ui.label(format!(
                            "{}  {}",
                            status_text(step["status"].as_str().unwrap_or_default()),
                            step["step"].as_str().unwrap_or_default()
                        ));
                    }
                });
            }
            for item in turn
                .items
                .iter()
                .filter(|i| i.kind != "agentMessage" && i.kind != "reasoning")
            {
                let title = if item.command.is_empty() {
                    match item.kind.as_str() {
                        "fileChange" => "文件变更",
                        "commandExecution" => "执行命令",
                        "mcpToolCall" | "dynamicToolCall" => "工具调用",
                        "webSearch" => "网页搜索",
                        kind => kind,
                    }
                } else {
                    item.command.as_str()
                };
                // CollapsingHeader always extends its text, even inside a vertical
                // ScrollArea. Bound the galley before handing it the command title.
                let title = format!("{} · {}", status_text(&item.status), title);
                let header = egui::WidgetText::from(title.as_str()).into_galley(
                    ui,
                    Some(egui::TextWrapMode::Truncate),
                    (ui.available_width() - ui.spacing().indent - ui.spacing().button_padding.x)
                        .max(0.),
                    egui::TextStyle::Button,
                );
                egui::CollapsingHeader::new(header)
                    .id_salt(&item.id)
                    .default_open(item.status == "inProgress")
                    .show(ui, |ui| {
                        if !item.cwd.is_empty() {
                            ui.label(format!("目录：{}", item.cwd));
                        }
                        if let Some(code) = item.exit_code {
                            ui.label(format!("退出码：{code}"));
                        }
                        if let Some(ms) = item.duration_ms {
                            ui.label(format!("耗时：{ms} ms"));
                        }
                        if item.truncated {
                            ui.weak("输出过长，仅保留最后 256 KiB。");
                        }
                        if !item.output.is_empty() {
                            code_text(ui, &item.output, false);
                        }
                        for change in &item.changes {
                            ui.strong(format!("{} · {}", change.kind, change.path));
                            let path = project.join(&change.path);
                            if path.is_file()
                                && let Some(path) = path.to_str()
                            {
                                let mut link = url::Url::parse("vscode://file").unwrap();
                                link.set_path(path);
                                ui.hyperlink_to("在 VS Code 打开", link.as_str());
                            }
                            diff_text(ui, &change.diff);
                        }
                    })
                    .header_response
                    .on_hover_text(title);
            }
            if !turn.diff.is_empty() {
                ui.collapsing("本轮累计变更", |ui| diff_text(ui, &turn.diff));
            }
            ui.add_space(6.);
            ui.label(
                egui::RichText::new("Lexington")
                    .strong()
                    .color(egui::Color32::from_rgb(57, 97, 185)),
            );
            for (_, text) in &turn.messages {
                rich_text(ui, text);
            }
            if !turn.error.is_empty() {
                ui.scope(|ui| {
                    ui.visuals_mut().override_text_color = Some(egui::Color32::DARK_RED);
                    output_text(ui, &turn.error, false, false);
                });
            }
            // Confirmed inputs belong at the live end of the task, where the
            // bottom-following scroll area can reveal them after a long reply.
            for (supplement_index, text) in turn.supplements.iter().enumerate() {
                ui.push_id(("supplement", supplement_index), |ui| {
                    ui.label(
                        egui::RichText::new("你 · 运行中补充")
                            .strong()
                            .color(egui::Color32::from_rgb(57, 97, 185)),
                    );
                    egui::Frame::NONE
                        .fill(egui::Color32::from_rgb(242, 246, 253))
                        .corner_radius(7)
                        .inner_margin(10)
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            rich_text(ui, text);
                        });
                });
            }
            ui.separator();
        });
    }
}
fn diff_text(ui: &mut egui::Ui, diff: &str) {
    code_text(ui, diff, true);
}
pub(super) fn code_text(ui: &mut egui::Ui, text: &str, diff: bool) {
    // Frame children share a persistent Ui ID. Distinguish each block before
    // entering the frame so sibling code/output scroll areas keep separate state.
    let scroll_id = ui.next_auto_id().with("code-output-scroll");
    egui::Frame::NONE
        .fill(egui::Color32::from_rgb(242, 244, 248))
        .corner_radius(6)
        .inner_margin(10)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.weak(if diff { "diff" } else { "代码 / 输出" });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .small_button(if diff { "复制 diff" } else { "复制代码" })
                        .clicked()
                    {
                        ui.ctx().copy_text(text.into());
                    }
                });
            });
            // A changing 256 KiB galley is expensive even when most of it is
            // clipped. Layout only visible fixed-height rows, including blank ones.
            let lines: Vec<_> = text.lines().collect();
            let row_height = ui.text_style_height(&egui::TextStyle::Monospace);
            let minimum_height =
                ((row_height + ui.spacing().item_spacing.y) * lines.len().min(8) as f32).min(240.);
            egui::ScrollArea::both()
                .id_salt(scroll_id)
                .max_height(240.)
                .min_scrolled_height(minimum_height)
                .auto_shrink([false, true])
                .stick_to_bottom(true)
                .show_rows(ui, row_height, lines.len(), |ui, rows| {
                    ui.push_id(rows.start, |ui| {
                        output_text(ui, &lines[rows].join("\n"), true, diff);
                    });
                });
        });
}
fn output_text(ui: &mut egui::Ui, mut text: &str, code: bool, diff: bool) {
    let id = ui.next_auto_id();
    let focused = ui.memory(|memory| memory.has_focus(id));
    let previous =
        egui::TextEdit::load_state(ui.ctx(), id).and_then(|state| state.cursor.char_range());
    let secondary =
        ui.input(|input| input.pointer.secondary_pressed() || input.pointer.secondary_released());
    let mut layouter = |ui: &egui::Ui, text: &str, width: f32| {
        let font = if code {
            egui::TextStyle::Monospace
        } else {
            egui::TextStyle::Body
        }
        .resolve(ui.style());
        let mut job = egui::text::LayoutJob::default();
        if code {
            for line in text.split_inclusive('\n') {
                let color = if diff && line.starts_with('+') {
                    egui::Color32::from_rgb(25, 125, 65)
                } else if diff && line.starts_with('-') {
                    egui::Color32::from_rgb(190, 55, 55)
                } else {
                    ui.visuals().text_color()
                };
                job.append(
                    line,
                    0.,
                    egui::TextFormat {
                        font_id: font.clone(),
                        color,
                        line_height: Some(
                            ui.text_style_height(&egui::TextStyle::Monospace)
                                + ui.spacing().item_spacing.y,
                        ),
                        ..Default::default()
                    },
                );
            }
        } else {
            job.append(
                text,
                0.,
                egui::TextFormat {
                    font_id: font,
                    color: ui.visuals().text_color(),
                    ..Default::default()
                },
            );
            job.wrap.max_width = width;
        }
        ui.fonts(|fonts| fonts.layout_job(job))
    };
    // &str is a read-only TextBuffer: selection and keyboard copy remain available.
    let mut output = egui::TextEdit::multiline(&mut text)
        .id(id)
        .frame(false)
        .margin(0)
        .desired_rows(1)
        .desired_width(ui.available_width())
        .min_size(egui::Vec2::ZERO)
        .layouter(&mut layouter)
        .show(ui);
    if secondary {
        // egui treats any pointer press as a new cursor; preserve the original
        // selection while opening the context menu, including a right-click within it.
        output
            .state
            .cursor
            .set_char_range(if focused { previous } else { None });
        output.state.clone().store(ui.ctx(), id);
    }
    let range = output.state.cursor.char_range();
    output.response.context_menu(|ui| {
        let selected = range
            .filter(|range| range.primary.index != range.secondary.index)
            .map(|range| {
                let [start, end] = range.sorted();
                text.chars()
                    .skip(start.index)
                    .take(end.index - start.index)
                    .collect::<String>()
            })
            .unwrap_or_default();
        if ui
            .add_enabled(!selected.is_empty(), egui::Button::new("复制"))
            .clicked()
        {
            ui.ctx().copy_text(selected);
            ui.close_menu();
        }
    });
}

fn rich_text(ui: &mut egui::Ui, text: &str) {
    let mut code = false;
    let mut block = String::new();
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            if code {
                code_text(ui, block.trim_end(), false);
            } else if !block.is_empty() {
                output_text(ui, block.trim_end_matches('\n'), false, false);
            }
            block.clear();
            code = !code;
        } else {
            block.push_str(line);
            block.push('\n');
        }
    }
    if !block.is_empty() {
        if code {
            code_text(ui, block.trim_end(), false);
        } else {
            output_text(ui, block.trim_end_matches('\n'), false, false);
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{Duration, Instant},
    };

    #[test]
    fn development_event_bursts_yield_frames_and_sanitize_before_final_save() {
        use crate::backend::codex::{Client, Event};
        for development_turn in [true, false] {
            let root = tempfile::tempdir().unwrap();
            let ctx = egui::Context::default();
            let mut app = Wife::from_root(&ctx, false, root.path().into());
            let path = app.development.book.add(root.path()).unwrap();
            app.development.book.active = Some(path);
            app.development
                .book
                .begin("burst".into(), "Lexington", false);
            app.development.storage = Some(root.path().join("history.json"));
            app.development.storage_ok = true;
            app.busy = true;
            app.development_turn = development_turn;
            // Exercise a secret assembled across event-batch boundaries, in both
            // tool output and replies. No provider configuration is read.
            let mut events = Vec::new();
            for index in 0..512 {
                let text = match index {
                    31 => "fixture-",
                    32 => "secret",
                    _ => "x",
                };
                events.push(Event::Activity(crate::development::Activity::Output {
                    id: "tool".into(),
                    delta: text.into(),
                }));
                events.push(Event::Text {
                    id: "reply".into(),
                    text: text.into(),
                    replace: false,
                });
            }
            events.push(Event::Finished { interrupted: false });
            app.codex = Some(Client::mock_events(events, vec!["fixture-secret".into()]));
            assert!(
                app.poll_codex(),
                "a queued burst must request another frame"
            );
            assert!(app.busy, "one poll drained the entire event burst");
            assert!(app.development.book.turn_mut().unwrap().messages[0].1.len() <= 64);
            let mut frames = 1;
            while app.busy && frames < 1100 {
                app.poll_codex();
                frames += 1;
            }
            assert!(!app.busy && frames > 1);
            let expected = format!("{}[hidden]{}", "x".repeat(31), "x".repeat(479));
            let saved = Workspaces::read(app.development.storage.as_ref().unwrap()).unwrap();
            let turn = &saved.task().unwrap().turns[0];
            assert_eq!(turn.status, "completed");
            turn.verify_replay().unwrap();
            assert!(
                !fs::read_to_string(app.development.storage.as_ref().unwrap())
                    .unwrap()
                    .contains("fixture-secret")
            );
            assert_eq!(turn.messages[0].1, expected);
            assert_eq!(turn.items[0].output, expected);
            if development_turn {
                assert!(app.text.is_empty());
            } else {
                assert_eq!(app.text, expected);
            }
        }
    }

    #[test]
    fn development_background_save_preserves_newer_edits_and_final_record() {
        let root = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        let mut app = Wife::from_root(&ctx, false, root.path().into());
        let path = app.development.book.add(root.path()).unwrap();
        app.development.book.active = Some(path);
        app.development
            .book
            .begin("save".into(), "Lexington", false);
        let path = root.path().join("history.json");
        app.development.storage = Some(path.clone());
        app.development.storage_ok = true;
        // Hold a real snapshot write in flight until the UI has consumed another
        // change. A frame must never join this worker while it is still running.
        let (release, wait) = mpsc::channel();
        let book = app.development.book.clone();
        let destination = path.clone();
        app.development.save_worker = Some(std::thread::spawn(move || {
            wait.recv_timeout(Duration::from_secs(5)).unwrap();
            book.save(&destination)
        }));
        app.development
            .book
            .turn_mut()
            .unwrap()
            .text("reply".into(), "latest".into(), false);
        app.save_development(false);
        assert!(app.development.dirty && app.development.save_worker.is_some());
        release.send(()).unwrap();
        app.finish_development("completed", "");
        assert!(app.development.save_worker.is_none());
        assert!(!app.development.dirty);
        let saved = Workspaces::read(&path).unwrap();
        assert_eq!(saved.task().unwrap().turns[0].messages[0].1, "latest");
        assert_eq!(saved.task().unwrap().turns[0].status, "completed");

        // Background failures keep the dirty state for retry and protect the
        // damaged store just as synchronous writes do.
        fs::write(&path, "broken").unwrap();
        app.development.last_saved = None;
        app.save_development(false);
        app.complete_development_save(true);
        assert!(app.development.dirty && app.development.error.contains("损坏"));
        assert_eq!(fs::read_to_string(&path).unwrap(), "broken");
    }

    #[test]
    fn event_timeline_keeps_rows_visible_inside_scrolled_task_details() {
        let ctx = egui::Context::default();
        ctx.style_mut(|style| style.animation_time = 0.);
        ctx.memory_mut(|memory| memory.set_everything_is_visible(true));
        let root = tempfile::tempdir().unwrap();
        let mut book = Workspaces::default();
        book.active = Some(book.add(root.path()).unwrap());
        book.begin("long input line\n".repeat(40), "a", true);
        let turn = book.turn_mut().unwrap();
        for index in 0..4 {
            turn.text("reply".into(), format!("Reply {index}"), true);
            turn.flush_events();
        }
        turn.finish("completed", "");
        let task = book.task().unwrap();
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
                                task_details(ui, task, root.path());
                            });
                    });
                },
            );
            visible = output.shapes.iter().any(|shape| {
                matches!(&shape.shape,
                egui::epaint::Shape::Text(text) if text.galley.job.text.starts_with("#5 ·")
                    && shape.clip_rect.contains(text.pos + text.galley.size() / 2.))
            });
        }
        assert!(
            visible,
            "timeline rows collapsed inside the outer task scroll area"
        );
    }

    #[test]
    fn multiple_output_blocks_do_not_emit_unclipped_id_warnings() {
        let ctx = egui::Context::default();
        ctx.options_mut(|options| options.warn_on_id_clash = true);
        ctx.style_mut(|style| style.animation_time = 0.);
        let reply = format!(
            "First block\n```rust\n{}\n```\nSecond block\n```text\n{}\n```",
            "first output\n".repeat(30),
            "second output\n".repeat(40),
        );
        let task = Task {
            turns: vec![crate::development::Turn {
                messages: vec![("reply".into(), reply.clone()), ("next".into(), reply)],
                ..Default::default()
            }],
            ..Default::default()
        };
        for offset in [0., 160., 400., 800., 0.] {
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
                            .vertical_scroll_offset(offset)
                            .show(ui, |ui| {
                                task_details(ui, &task, std::path::Path::new("/fixture"));
                            });
                    });
                },
            );
            for shape in output.shapes {
                if let egui::Shape::Text(text) = shape.shape {
                    assert!(
                        !text.galley.job.text.contains("use of ScrollArea ID"),
                        "unclipped warning at scroll offset {offset}: {}",
                        text.galley.job.text
                    );
                }
            }
        }
    }

    #[test]
    fn large_execution_output_only_draws_visible_rows() {
        let ctx = egui::Context::default();
        ctx.memory_mut(|memory| memory.set_everything_is_visible(true));
        let log = "command output with some details\n".repeat(6000);
        let task = Task {
            turns: vec![crate::development::Turn {
                items: vec![crate::development::ExecutionItem {
                    id: "large-output".into(),
                    kind: "commandExecution".into(),
                    status: "inProgress".into(),
                    output: log.clone(),
                    ..Default::default()
                }],
                diff: log,
                ..Default::default()
            }],
            ..Default::default()
        };
        for frame in 0..3 {
            let started = Instant::now();
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
                        let scroll = egui::ScrollArea::vertical().show(ui, |ui| {
                            task_details(ui, &task, std::path::Path::new("/fixture"));
                        });
                        assert!(
                            scroll.content_size.y < 2000.,
                            "large logs expanded the full task: {}",
                            scroll.content_size.y
                        );
                    });
                },
            );
            eprintln!(
                "large output frame {frame}: {:?}, {} shapes",
                started.elapsed(),
                output.shapes.len()
            );
            assert!(
                output.shapes.len() < 500,
                "offscreen log rows were rendered: {}",
                output.shapes.len()
            );
        }
    }

    fn fixture(root: &std::path::Path, mode: &str) -> String {
        use std::os::unix::fs::PermissionsExt;
        let path = root.join(format!("codex-{mode}"));
        fs::write(
            &path,
            format!(
                "#!/usr/bin/env python3\nimport sys\nsys.argv.insert(1, {mode:?})\n{}",
                include_str!("../../tests/fixtures/codex-server.py")
            ),
        )
        .unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        path.display().to_string()
    }
    fn send(app: &mut Wife, ctx: &egui::Context, text: &str) {
        app.send_message(ctx, text.into(), MessageSource::Main);
        assert!(app.busy, "{}", app.error);
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.busy && Instant::now() < deadline {
            app.poll_codex();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!app.busy, "task timed out");
        assert!(app.error.is_empty(), "{}", app.error);
    }

    #[test]
    fn development_questions_show_waiting_and_resume_after_reply_or_stop() {
        use std::sync::{Arc, Mutex};
        let root = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        ctx.set_embed_viewports(false);
        let development_id = egui::ViewportId::from_hash_of("lexington-development");
        let prompt_id = egui::ViewportId::from_hash_of("lexington-codex-prompt");
        let events = Arc::new(Mutex::new((prompt_id, Vec::<egui::Event>::new())));
        let shapes = Arc::new(Mutex::new(HashMap::new()));
        let renderer_events = events.clone();
        let renderer_shapes = shapes.clone();
        egui::Context::set_immediate_viewport_renderer(move |ctx, mut viewport| {
            let bounds = egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                if viewport.ids.this == development_id {
                    egui::vec2(740., 500.)
                } else {
                    egui::vec2(400., 300.)
                },
            );
            let mut events = renderer_events.lock().unwrap();
            let mut input = egui::RawInput {
                viewport_id: viewport.ids.this,
                screen_rect: Some(bounds),
                events: if events.0 == viewport.ids.this {
                    std::mem::take(&mut events.1)
                } else {
                    Vec::new()
                },
                ..Default::default()
            };
            drop(events);
            input
                .viewports
                .insert(viewport.ids.this, Default::default());
            let output = ctx.run(input, &mut viewport.viewport_ui_cb);
            renderer_shapes
                .lock()
                .unwrap()
                .insert(viewport.ids.this, output.shapes);
        });
        let mut app = Wife::from_root(&ctx, false, root.path().into());
        app.config.codex.executable = fixture(root.path(), "development-off");
        app.development.storage = Some(root.path().join("history.json"));
        app.development.storage_ok = true;
        app.switch_development_workspace(Some(root.path().into()), &ctx)
            .unwrap();
        app.development.open = true;
        let frame = |app: &mut Wife| {
            shapes.lock().unwrap().clear();
            let _ = ctx.run(Default::default(), |ctx| {
                app.development_window(ctx);
                app.codex_prompt_ui(ctx);
            });
        };
        let point = |viewport, label: &str| {
            shapes
                .lock()
                .unwrap()
                .get(&viewport)
                .unwrap()
                .iter()
                .find_map(|shape| {
                    if let egui::Shape::Text(text) = &shape.shape
                        && text.galley.job.text == label
                    {
                        let rect = text.galley.rect.translate(text.pos.to_vec2());
                        assert!(
                            shape.clip_rect.contains_rect(rect),
                            "{label} is clipped: {rect:?}"
                        );
                        return Some(rect.center());
                    }
                    None
                })
                .unwrap_or_else(|| panic!("missing {label} in {viewport:?}"))
        };
        let click = |app: &mut Wife, viewport, label| {
            for _ in 0..3 {
                frame(app);
            }
            let pos = point(viewport, label);
            for pressed in [true, false] {
                *events.lock().unwrap() = (
                    viewport,
                    vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                );
                frame(app);
            }
        };
        for (request, waiting, button, response) in [
            ("question", "等待你回答", "回答问题（1）", "提交回答"),
            ("approve", "等待你确认", "处理审批（1）", "拒绝"),
            ("question", "等待你回答", "回答问题（1）", "停止任务"),
        ] {
            app.send_message(&ctx, request.into(), MessageSource::Development);
            let deadline = Instant::now() + Duration::from_secs(5);
            while app.codex_prompts.is_empty() && app.busy && Instant::now() < deadline {
                app.poll_codex();
                std::thread::sleep(Duration::from_millis(5));
            }
            assert_eq!(app.codex_prompts.len(), 1, "{}", app.error);
            app.development.input = "继续保留的草稿".into();
            for _ in 0..3 {
                frame(&mut app);
            }
            point(development_id, waiting);
            point(development_id, "等待处理");
            if request == "question" {
                click(&mut app, prompt_id, "提交回答");
                assert_eq!(app.codex_prompts.len(), 1, "empty answer was submitted");
                click(&mut app, prompt_id, "A");
            }
            click(&mut app, prompt_id, "稍后处理");
            assert!(!app.codex_prompts.front().unwrap().open);
            frame(&mut app);
            assert!(!shapes.lock().unwrap().contains_key(&prompt_id));
            point(development_id, waiting);
            // Enter in the task editor must not steer the answer into the model turn.
            click(&mut app, development_id, "继续保留的草稿");
            *events.lock().unwrap() = (
                development_id,
                vec![egui::Event::Key {
                    key: egui::Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
            frame(&mut app);
            assert!(app.development.supplement.is_none());
            assert_eq!(app.development.input, "继续保留的草稿");
            click(&mut app, development_id, button);
            assert!(app.codex_prompts.front().unwrap().open);
            // Closing the development window must not hide the independent prompt.
            app.development.open = false;
            frame(&mut app);
            point(prompt_id, response);
            click(&mut app, prompt_id, response);
            let deadline = Instant::now() + Duration::from_secs(5);
            while app.busy && Instant::now() < deadline {
                app.poll_codex();
                std::thread::sleep(Duration::from_millis(5));
            }
            assert!(
                !app.busy,
                "reply or interrupt did not finish: {}",
                app.error
            );
            assert!(app.error.is_empty(), "{}", app.error);
            assert!(app.codex_prompts.is_empty());
            assert_eq!(app.development.input, "继续保留的草稿");
            assert!(app.text.is_empty() && app.codex_messages.is_empty());
            let saved = Workspaces::read(app.development.storage.as_ref().unwrap()).unwrap();
            let turn = saved.task().unwrap().turns.last().unwrap();
            assert_eq!(
                turn.status,
                if response == "停止任务" {
                    "interrupted"
                } else {
                    "completed"
                }
            );
            if response == "提交回答" {
                assert_eq!(turn.messages[0].1, "收到选择 A");
            }
            app.development.open = true;
        }
    }

    #[test]
    fn clicking_tasks_switches_send_target_and_resumes_only_compatible_idle_sessions() {
        let root = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        let mut app = Wife::from_root(&ctx, false, root.path().into());
        app.config.codex.executable = fixture(root.path(), "development-on");
        app.config.codex.development_mode = true;
        app.development.storage = Some(root.path().join("history.json"));
        app.development.storage_ok = true;
        app.switch_development_workspace(Some(root.path().into()), &ctx)
            .unwrap();
        send(&mut app, &ctx, "Task A");
        let a = app.development.book.task().unwrap().id;
        let thread = app.development.thread_id.clone().unwrap();
        app.new_development_task(&ctx);
        send(&mut app, &ctx, "Task B");
        let b = app.development.book.task().unwrap().id;
        app.development.input = "保留草稿".into();
        app.development.open = true;
        let frame = |app: &mut Wife, events| {
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1200., 900.),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| app.development_window(ctx),
            )
        };
        let click_task = |app: &mut Wife, title: &str| {
            let mut output = frame(app, vec![]);
            for _ in 0..3 {
                output = frame(app, vec![]);
            }
            assert!(!output.shapes.iter().any(|shape| {
                matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text == "继续此任务")
            }));
            let point = output
                .shapes
                .iter()
                .filter_map(|shape| {
                    if let egui::Shape::Text(text) = &shape.shape
                        && text.galley.job.text == title
                    {
                        let rect = text.galley.rect.translate(text.pos.to_vec2());
                        if shape.clip_rect.contains_rect(rect) {
                            return Some(rect);
                        }
                    }
                    None
                })
                .min_by(|a, b| a.left().total_cmp(&b.left()))
                .expect("task list button")
                .center();
            for pressed in [true, false] {
                let _ = frame(
                    app,
                    vec![
                        egui::Event::PointerMoved(point),
                        egui::Event::PointerButton {
                            pos: point,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                );
            }
        };
        // Reselecting the current task must keep the connected session.
        click_task(&mut app, "Task B");
        assert_eq!(app.development.thread_id.as_deref(), Some(thread.as_str()));
        click_task(&mut app, "Task A");
        assert_eq!(app.development.book.task().unwrap().id, a);
        assert_eq!(app.development.viewed_task, Some(a));
        assert_eq!(app.development.input, "保留草稿");
        let saved = Workspaces::read(app.development.storage.as_ref().unwrap()).unwrap();
        assert_eq!(saved.task().unwrap().id, a);
        send(&mut app, &ctx, "resume-status");
        assert!(app.text.starts_with("thread/resume:"));
        assert_eq!(app.development.book.task().unwrap().turns.len(), 2);
        assert_eq!(app.development.thread_id.as_deref(), Some(thread.as_str()));
        let output = frame(&mut app, vec![]);
        assert!(output.shapes.iter().any(|shape| {
            matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text == "发送到：Task A")
        }));

        app.send_message(&ctx, "interrupt".into(), MessageSource::Development);
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.development.book.turn_mut().unwrap().messages.is_empty()
            && Instant::now() < deadline
        {
            app.poll_codex();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(app.busy);
        click_task(&mut app, "Task B");
        assert!(app.busy);
        assert_eq!(app.development.book.task().unwrap().id, a);
        assert_eq!(app.development.viewed_task, Some(b));
        assert!(app.development.error.contains("仍在运行"));
        app.codex.as_ref().unwrap().interrupt();
        while app.busy && Instant::now() < deadline {
            app.poll_codex();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!app.busy);
        click_task(&mut app, "Task B");
        assert_eq!(app.development.book.task().unwrap().id, b);
        assert!(app.development.error.is_empty());

        let original = app.development.book.current().unwrap().tasks[0].clone();
        for reason in 0..4 {
            let task = &mut app.development.book.current_mut().unwrap().tasks[0];
            *task = original.clone();
            match reason {
                0 => task.character = "Other".into(),
                1 => task.development_mode = false,
                2 => task.memory_revision += 1,
                _ => task.memory_context_stamp = "outdated".into(),
            }
            click_task(&mut app, "Task A");
            assert_eq!(app.development.viewed_task, Some(a));
            assert_eq!(app.development.book.task().unwrap().id, b);
            assert!(app.development.error.contains("不兼容"));
            assert_eq!(app.development.input, "保留草稿");
        }
    }

    #[test]
    fn opening_tasks_scrolls_to_latest_output_and_preserves_manual_reading() {
        let root = tempfile::tempdir().unwrap();
        let a = root.path().join("a");
        let b = root.path().join("b");
        fs::create_dir(&a).unwrap();
        fs::create_dir(&b).unwrap();
        let ctx = egui::Context::default();
        let mut app = Wife::from_root(&ctx, false, root.path().into());
        let a = app.development.book.add(&a).unwrap();
        let b = app.development.book.add(&b).unwrap();
        app.development.book.active = Some(a.clone());
        let mut ids = Vec::new();
        for name in ["A", "B"] {
            app.development.book.current_mut().unwrap().current_task = None;
            app.development
                .book
                .begin(format!("Task {name}"), "Lexington", false);
            ids.push(app.development.book.task().unwrap().id);
            let turn = app.development.book.turn_mut().unwrap();
            turn.status = "completed".into();
            turn.messages = (0..60)
                .map(|index| (index.to_string(), format!("Output {name} line {index}")))
                .collect();
            turn.messages.push(("tail".into(), format!("TAIL-{name}")));
        }
        let mut other = app.development.book.current().unwrap().tasks[0].clone();
        other.turns[0].messages.last_mut().unwrap().1 = "TAIL-C".into();
        app.development
            .book
            .projects
            .iter_mut()
            .find(|project| project.path == b)
            .unwrap()
            .tasks
            .push(other);
        app.development.viewed_task = Some(ids[0]);
        // While a task is running, history remains browseable without switching.
        app.busy = true;
        app.development.open = true;
        let time = std::cell::Cell::new(0.);
        let frame = |app: &mut Wife, events| {
            time.set(time.get() + 1. / 60.);
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1200., 900.),
                    )),
                    time: Some(time.get()),
                    events,
                    ..Default::default()
                },
                |ctx| app.development_window(ctx),
            )
        };
        let visible = |output: &egui::FullOutput, expected: &str| {
            output.shapes.iter().find_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape
                    && text.galley.job.text == expected
                {
                    let rect = text.galley.rect.translate(text.pos.to_vec2());
                    if shape.clip_rect.contains_rect(rect) {
                        return Some(rect);
                    }
                }
                None
            })
        };
        let scroll_up = |app: &mut Wife, tail: egui::Rect| {
            let _ = frame(
                app,
                vec![
                    egui::Event::PointerMoved(tail.center()),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::vec2(0., 400.),
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
            let mut output = frame(app, vec![]);
            for _ in 0..30 {
                output = frame(app, vec![]);
            }
            output
        };
        let mut output = frame(&mut app, vec![]);
        for _ in 0..3 {
            output = frame(&mut app, vec![]);
        }
        let tail = visible(&output, "TAIL-A").expect("opening a long task shows its last output");
        app.development.book.current_mut().unwrap().tasks[0].turns[0]
            .messages
            .extend((0..10).map(|index| (format!("live-{index}"), format!("LIVE-{index}"))));
        for _ in 0..3 {
            output = frame(&mut app, vec![]);
        }
        assert!(
            visible(&output, "LIVE-9").is_some(),
            "follow new output at the bottom"
        );
        let tail = visible(&output, "LIVE-9").unwrap_or(tail);
        output = scroll_up(&mut app, tail);
        assert!(
            visible(&output, "TAIL-A").is_none(),
            "manual scroll must not be pulled back to the bottom"
        );
        app.development
            .book
            .current_mut()
            .unwrap()
            .tasks
            .iter_mut()
            .find(|task| task.id == ids[0])
            .unwrap()
            .turns[0]
            .messages
            .push(("new".into(), "TAIL-A-NEW".into()));
        for _ in 0..3 {
            output = frame(&mut app, vec![]);
        }
        assert!(visible(&output, "TAIL-A-NEW").is_none());
        // Returning to the bottom resumes following future output.
        let _ = frame(
            &mut app,
            vec![
                egui::Event::PointerMoved(tail.center()),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0., -1000.),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        for _ in 0..60 {
            output = frame(&mut app, vec![]);
        }
        assert!(visible(&output, "TAIL-A-NEW").is_some());
        app.development.book.current_mut().unwrap().tasks[0].turns[0]
            .messages
            .push(("resumed".into(), "FOLLOW-RESUMED".into()));
        for _ in 0..3 {
            output = frame(&mut app, vec![]);
        }
        assert!(visible(&output, "FOLLOW-RESUMED").is_some());
        assert_eq!(
            app.development.book.task().unwrap().id,
            ids[1],
            "browsing while busy must not change the send target"
        );
        for (id, marker) in [(ids[1], "TAIL-B"), (ids[0], "TAIL-A-NEW")] {
            app.development.viewed_task = Some(id);
            for _ in 0..3 {
                output = frame(&mut app, vec![]);
            }
            assert!(visible(&output, marker).is_some(), "switch to {marker}");
        }
        output = scroll_up(&mut app, visible(&output, "TAIL-A-NEW").unwrap());
        assert!(visible(&output, "TAIL-A-NEW").is_none());
        app.development.open = false;
        let _ = frame(&mut app, vec![]);
        app.development.open = true;
        for _ in 0..3 {
            output = frame(&mut app, vec![]);
        }
        assert!(visible(&output, "TAIL-A-NEW").is_some());
        output = scroll_up(&mut app, visible(&output, "TAIL-A-NEW").unwrap());
        assert!(visible(&output, "TAIL-A-NEW").is_none());
        app.development.page = 1;
        let _ = frame(&mut app, vec![]);
        app.development.page = 0;
        for _ in 0..3 {
            output = frame(&mut app, vec![]);
        }
        assert!(visible(&output, "TAIL-A-NEW").is_some());
        output = scroll_up(&mut app, visible(&output, "TAIL-A-NEW").unwrap());
        let button = output
            .shapes
            .iter()
            .filter_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape
                    && text.galley.job.text == "Task A"
                {
                    let rect = text.galley.rect.translate(text.pos.to_vec2());
                    if shape.clip_rect.contains_rect(rect) {
                        return Some(rect);
                    }
                }
                None
            })
            .min_by(|a, b| a.left().total_cmp(&b.left()))
            .unwrap()
            .center();
        for pressed in [true, false] {
            let _ = frame(
                &mut app,
                vec![
                    egui::Event::PointerMoved(button),
                    egui::Event::PointerButton {
                        pos: button,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
        for _ in 0..3 {
            output = frame(&mut app, vec![]);
        }
        assert!(
            visible(&output, "TAIL-A-NEW").is_some(),
            "reselecting the same task scrolls to the end"
        );
        // Equal task IDs in different projects must still count as a new task view.
        app.development.book.active = Some(b);
        for _ in 0..3 {
            output = frame(&mut app, vec![]);
        }
        assert!(visible(&output, "TAIL-C").is_some());
    }

    #[test]
    fn task_output_stays_within_scroll_width() {
        use crate::development::{ExecutionItem, FileChange, Turn};

        let long = "very_long_output_or_path_".repeat(80);
        for (name, turn) in [
            (
                "prose",
                Turn {
                    user: long.clone(),
                    messages: vec![("reply".into(), long.clone())],
                    ..Default::default()
                },
            ),
            (
                "code",
                Turn {
                    messages: vec![("reply".into(), format!("```rust\n{long}\n```\nafter code"))],
                    ..Default::default()
                },
            ),
            (
                "streaming code",
                Turn {
                    messages: vec![("reply".into(), format!("```rust\n{long}"))],
                    ..Default::default()
                },
            ),
            (
                "command",
                Turn {
                    items: vec![ExecutionItem {
                        id: "command".into(),
                        kind: "commandExecution".into(),
                        command: long.clone(),
                        cwd: long.clone(),
                        output: long.clone(),
                        status: "inProgress".into(),
                        changes: vec![FileChange {
                            path: long.clone(),
                            kind: "update".into(),
                            diff: format!("+{long}"),
                        }],
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            ),
            (
                "diff",
                Turn {
                    diff: format!("+{long}\n-{long}"),
                    ..Default::default()
                },
            ),
        ] {
            let ctx = egui::Context::default();
            ctx.memory_mut(|memory| memory.set_everything_is_visible(true));
            let task = Task {
                character: "Lexington".into(),
                turns: vec![turn],
                ..Default::default()
            };
            for _ in 0..3 {
                let _ = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(400., 300.),
                        )),
                        ..Default::default()
                    },
                    |ctx| {
                        egui::CentralPanel::default().show(ctx, |ui| {
                            let bounds = ui.available_rect_before_wrap();
                            let scroll = egui::ScrollArea::vertical()
                                .max_height(180.)
                                .min_scrolled_height(0.)
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    task_details(ui, &task, std::path::Path::new("/fixture"))
                                });
                            assert!(
                                scroll.inner_rect.right() <= bounds.right() + 1.,
                                "{name}: {:?} exceeds {bounds:?}",
                                scroll.inner_rect
                            );
                            assert!(
                                scroll.content_size.x <= bounds.width() + 1.,
                                "{name}: content width {} exceeds {}",
                                scroll.content_size.x,
                                bounds.width()
                            );
                        });
                    },
                );
            }
        }
    }

    #[test]
    fn development_long_output_keeps_editor_inside_small_viewport() {
        use crate::development::{ExecutionItem, Turn};
        let root = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        let mut app = Wife::from_root(&ctx, false, root.path().into());
        let path = app.development.book.add(root.path()).unwrap();
        app.development.book.active = Some(path);
        app.development
            .book
            .begin("Long output".into(), "Lexington", false);
        let long = "long command output ".repeat(100);
        *app.development.book.turn_mut().unwrap() = Turn {
            user: long.clone(),
            messages: vec![("reply".into(), format!("{long}\n```\n{long}\n```"))],
            items: vec![ExecutionItem {
                id: "command".into(),
                kind: "commandExecution".into(),
                command: long.clone(),
                output: long,
                status: "inProgress".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        app.development.open = true;
        app.development.input = "draft".into();
        ctx.set_embed_viewports(false);
        egui::Context::set_immediate_viewport_renderer(|ctx, mut viewport| {
            let bounds = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(740., 500.));
            let mut input = egui::RawInput {
                viewport_id: viewport.ids.this,
                screen_rect: Some(bounds),
                ..Default::default()
            };
            input
                .viewports
                .insert(viewport.ids.this, Default::default());
            let output = ctx.run(input, &mut viewport.viewport_ui_cb);
            let mut send_visible = false;
            let mut draft_visible = false;
            for shape in output.shapes {
                if let egui::Shape::Text(text) = shape.shape
                    && ["发送", "draft"].contains(&text.galley.job.text.as_str())
                {
                    let rect = text.galley.rect.translate(text.pos.to_vec2());
                    assert!(
                        bounds.contains_rect(rect),
                        "editor content outside viewport: {rect:?}"
                    );
                    assert!(
                        shape.clip_rect.contains_rect(rect),
                        "editor content clipped: {rect:?}"
                    );
                    send_visible |= text.galley.job.text == "发送";
                    draft_visible |= text.galley.job.text == "draft";
                }
            }
            assert!(send_visible && draft_visible);
        });
        for _ in 0..3 {
            let _ = ctx.run(Default::default(), |ctx| app.development_window(ctx));
        }
    }

    #[test]
    fn confirmed_supplements_are_visible_at_the_live_output_end_and_after_reload() {
        let root = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        ctx.style_mut(|style| style.animation_time = 0.);
        let mut app = Wife::from_root(&ctx, false, root.path().into());
        let path = app.development.book.add(root.path()).unwrap();
        app.development.book.active = Some(path.clone());
        app.development
            .book
            .begin("Original input".into(), "Lexington", false);
        app.development.book.turn_mut().unwrap().text(
            "reply".into(),
            "Long streamed reply\n".repeat(80),
            true,
        );
        app.development.book.turn_mut().unwrap().flush_events();
        app.development.storage = Some(root.path().join("history.json"));
        app.development.storage_ok = true;
        app.development.open = true;
        app.development_turn = true;
        app.busy = true;
        let frame = |app: &mut Wife| {
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1200., 900.),
                    )),
                    ..Default::default()
                },
                |ctx| app.development_window(ctx),
            )
        };
        for _ in 0..3 {
            let _ = frame(&mut app);
        }
        for draft in ["First supplement", "Second supplement\n中文补充🙂"] {
            app.development.input = draft.into();
            app.development.supplement = Some(PendingSupplement {
                draft: draft.into(),
                project: path.clone(),
                task: app.development.book.task().unwrap().id,
                turn: 0,
            });
            app.codex = Some(crate::backend::codex::Client::mock_events(
                vec![crate::backend::codex::Event::Steered(Ok(()))],
                vec![],
            ));
            app.poll_codex();
            assert!(app.development.input.is_empty());
            for _ in 0..3 {
                let _ = frame(&mut app);
            }
            let output = frame(&mut app);
            assert!(output.shapes.iter().any(|shape| {
                matches!(&shape.shape, egui::Shape::Text(text)
                    if text.galley.job.text == draft
                        && shape.clip_rect.contains_rect(text.galley.rect.translate(text.pos.to_vec2())))
            }), "confirmed supplement is outside the live output viewport: {draft}");
        }
        app.finish_development("completed", "");
        app.busy = false;
        app.development.book = Workspaces::read(app.development.storage.as_ref().unwrap()).unwrap();
        let turn = &app.development.book.task().unwrap().turns[0];
        assert_eq!(turn.user, "Original input");
        assert_eq!(turn.messages[0].1, "Long streamed reply\n".repeat(80));
        assert_eq!(
            turn.supplements,
            ["First supplement", "Second supplement\n中文补充🙂"]
        );
        turn.verify_replay().unwrap();
        app.development.detail_task = None;
        for _ in 0..3 {
            let _ = frame(&mut app);
        }
        let output = frame(&mut app);
        assert!(output.shapes.iter().any(|shape| {
            matches!(&shape.shape, egui::Shape::Text(text)
                if text.galley.job.text == "Second supplement\n中文补充🙂"
                    && shape.clip_rect.contains_rect(text.galley.rect.translate(text.pos.to_vec2())))
        }), "reloaded supplement is outside the task output viewport");
        assert!(app.text.is_empty() && app.codex_messages.is_empty());
    }

    #[test]
    fn running_task_accepts_supplements_and_preserves_new_drafts_and_failures() {
        let root = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        let mut app = Wife::from_root(&ctx, false, root.path().into());
        let path = app.development.book.add(root.path()).unwrap();
        app.development.book.active = Some(path);
        app.development.storage = Some(root.path().join("history.json"));
        app.development.storage_ok = true;
        app.codex = Some(crate::backend::codex::Client::mock("normal"));
        app.development.open = true;
        app.send_message(&ctx, "interrupt".into(), MessageSource::Development);
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.development.book.turn_mut().unwrap().messages.is_empty()
            && Instant::now() < deadline
        {
            app.poll_codex();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(app.busy && !app.development.book.turn_mut().unwrap().messages.is_empty());
        let frame = |app: &mut Wife, events| {
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1200., 900.),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| app.development_window(ctx),
            )
        };
        for _ in 0..3 {
            let _ = frame(&mut app, vec![]);
        }
        ctx.memory_mut(|memory| memory.request_focus("development-input".into()));
        let enter = |modifiers| egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        };
        let _ = frame(&mut app, vec![egui::Event::Text("补充中文".into())]);
        let _ = frame(&mut app, vec![enter(egui::Modifiers::CTRL)]);
        let _ = frame(&mut app, vec![egui::Event::Text("第二行".into())]);
        assert_eq!(app.development.input, "补充中文\n第二行");
        assert!(app.development.supplement.is_none());
        let _ = frame(&mut app, vec![enter(egui::Modifiers::NONE)]);
        assert!(app.development.supplement.is_some());
        assert_eq!(app.development.input, "补充中文\n第二行");
        // Editing while the ACK is pending must survive its later arrival.
        let _ = frame(&mut app, vec![egui::Event::Text("后续草稿".into())]);
        let deadline = Instant::now() + Duration::from_secs(5);
        while (app.development.supplement.is_some()
            || app.development.book.turn_mut().unwrap().messages.len() < 2)
            && Instant::now() < deadline
        {
            app.poll_codex();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(app.busy && app.development.supplement.is_none());
        assert_eq!(app.development.input, "补充中文\n第二行后续草稿");
        assert!(app.text.is_empty() && app.codex_messages.is_empty());
        assert_eq!(app.development.book.task().unwrap().turns.len(), 1);
        assert_eq!(
            app.development.book.turn_mut().unwrap().supplements,
            ["补充中文\n第二行"]
        );
        for (text, expected_count) in [("reject", 1), ("finish-before-ack", 2)] {
            app.development.input = text.into();
            let _ = frame(&mut app, vec![enter(egui::Modifiers::NONE)]);
            let deadline = Instant::now() + Duration::from_secs(5);
            while app.development.supplement.is_some() && Instant::now() < deadline {
                app.poll_codex();
                std::thread::sleep(Duration::from_millis(5));
            }
            assert!(app.development.supplement.is_none());
            assert_eq!(
                app.development.book.turn_mut().unwrap().supplements.len(),
                expected_count
            );
            if text == "reject" {
                assert!(app.busy);
                assert_eq!(app.development.input, text);
                assert!(app.development.error.contains("补充被拒绝"));
            } else {
                assert!(!app.busy && app.development.input.is_empty());
            }
        }
        let saved = Workspaces::read(app.development.storage.as_ref().unwrap()).unwrap();
        assert_eq!(saved.task().unwrap().turns.len(), 1);
        assert_eq!(saved.task().unwrap().turns[0].status, "completed");
        assert_eq!(
            saved.task().unwrap().turns[0].supplements,
            ["补充中文\n第二行", "finish-before-ack"]
        );
        // A disconnected server also leaves an unconfirmed supplement editable.
        app.send_message(&ctx, "interrupt".into(), MessageSource::Development);
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.development.book.turn_mut().unwrap().messages.is_empty()
            && Instant::now() < deadline
        {
            app.poll_codex();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!app.development.book.turn_mut().unwrap().messages.is_empty());
        app.development.input = "disconnect".into();
        let _ = frame(&mut app, vec![enter(egui::Modifiers::NONE)]);
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.busy && Instant::now() < deadline {
            app.poll_codex();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!app.busy && app.development.supplement.is_none());
        assert_eq!(app.development.input, "disconnect");
        assert!(
            app.development
                .book
                .turn_mut()
                .unwrap()
                .supplements
                .is_empty()
        );
    }

    #[test]
    fn supplement_ack_uses_origin_task_and_redacts_saved_text() {
        let root = tempfile::tempdir().unwrap();
        let a = root.path().join("a");
        let b = root.path().join("b");
        fs::create_dir(&a).unwrap();
        fs::create_dir(&b).unwrap();
        let ctx = egui::Context::default();
        let mut app = Wife::from_root(&ctx, false, root.path().into());
        let a = app.development.book.add(&a).unwrap();
        let b = app.development.book.add(&b).unwrap();
        app.development.book.active = Some(a.clone());
        app.development.book.begin("A".into(), "Lexington", false);
        app.development.supplement = Some(PendingSupplement {
            draft: "fixture-secret".into(),
            project: a.clone(),
            task: app.development.book.task().unwrap().id,
            turn: 0,
        });
        app.development
            .drafts
            .insert(a.clone(), "fixture-secret".into());
        app.development.book.active = Some(b);
        app.development.book.begin("B".into(), "Lexington", false);
        app.development.input = "B 的新草稿".into();
        app.codex = Some(crate::backend::codex::Client::mock_events(
            vec![crate::backend::codex::Event::Steered(Ok(()))],
            vec!["fixture-secret".into()],
        ));
        app.poll_codex();
        assert_eq!(app.development.input, "B 的新草稿");
        assert!(
            app.development
                .book
                .turn_mut()
                .unwrap()
                .supplements
                .is_empty()
        );
        assert!(app.development.drafts[&a].is_empty());
        let original = app
            .development
            .book
            .projects
            .iter()
            .find(|project| project.path == a)
            .unwrap();
        assert_eq!(original.tasks[0].turns[0].supplements, ["[hidden]"]);
        assert!(app.development.supplement.is_none());
    }

    #[test]
    fn output_context_menu_copies_only_the_dragged_unicode_selection() {
        for (kind, reverse, select) in [
            (0, false, true),
            (0, true, true),
            (1, false, true),
            (2, false, true),
            (0, false, false),
        ] {
            let ctx = egui::Context::default();
            super::super::style::install(&ctx);
            let text = if kind == 2 {
                "+before 选择🙂中文\n-下一行 after"
            } else {
                "before 选择🙂中文\n下一行 after"
            };
            let selected = if kind == 2 {
                "选择🙂中文\n-下一行"
            } else {
                "选择🙂中文\n下一行"
            };
            let frame = |events| {
                ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(800., 600.),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ctx| {
                        egui::CentralPanel::default().show(ctx, |ui| {
                            if kind == 0 {
                                rich_text(ui, text);
                            } else {
                                code_text(ui, text, kind == 2);
                            }
                        });
                    },
                )
            };
            let mut geometry = None;
            for _ in 0..3 {
                let output = frame(vec![]);
                for shape in output.shapes {
                    if let egui::Shape::Text(shape) = shape.shape
                        && shape.galley.job.text == text
                    {
                        geometry = Some((shape.pos, shape.galley));
                    }
                }
            }
            let (origin, galley) = geometry.unwrap();
            let start = text[..text.find(selected).unwrap()].chars().count();
            let end = start + selected.chars().count();
            let point = |index| {
                origin
                    + galley
                        .pos_from_ccursor(egui::text::CCursor::new(index))
                        .center()
                        .to_vec2()
            };
            let (anchor, caret) = if reverse {
                (point(end), point(start))
            } else {
                (point(start), point(end))
            };
            let pointer = |pos, button, pressed| {
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ]
            };
            if select {
                let _ = frame(pointer(anchor, egui::PointerButton::Primary, true));
                let _ = frame(vec![egui::Event::PointerMoved(caret)]);
                let _ = frame(pointer(caret, egui::PointerButton::Primary, false));
                let _ = frame(vec![]);
            }
            let menu_at = point(start + 1);
            let _ = frame(pointer(menu_at, egui::PointerButton::Secondary, true));
            let mut output = frame(pointer(menu_at, egui::PointerButton::Secondary, false));
            let mut menu_point = None;
            for _ in 0..3 {
                assert!(
                    output
                        .platform_output
                        .commands
                        .iter()
                        .all(|command| !matches!(command, egui::OutputCommand::CopyText(_))),
                    "opening a menu must not copy"
                );
                for shape in output.shapes {
                    if let egui::Shape::Text(shape) = shape.shape
                        && shape.galley.job.text == "复制"
                    {
                        menu_point =
                            Some(shape.galley.rect.translate(shape.pos.to_vec2()).center());
                    }
                }
                output = frame(vec![]);
            }
            let menu_point = menu_point.expect("copy context menu");
            let mut copied = Vec::new();
            for pressed in [true, false] {
                let output = frame(pointer(menu_point, egui::PointerButton::Primary, pressed));
                for command in output.platform_output.commands {
                    if let egui::OutputCommand::CopyText(text) = command {
                        copied.push(text);
                    }
                }
            }
            assert_eq!(
                copied,
                if select {
                    vec![selected.to_owned()]
                } else {
                    vec![]
                },
                "kind {kind}, reverse {reverse}, select {select}"
            );
            // The output is read-only even while its selection has focus.
            let output = frame(vec![egui::Event::Text("cannot edit".into())]);
            assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(shape) if shape.galley.job.text == text)));
        }
    }

    #[test]
    fn slash_completion_in_both_composers_sends_only_the_completed_raw_text() {
        for development in [false, true] {
            for (draft, complete) in [
                ("/pl", true),
                ("/unknown 中文", false),
                ("/tmp/文件.txt", false),
            ] {
                let root = tempfile::tempdir().unwrap();
                let ctx = egui::Context::default();
                let mut app = Wife::from_root(&ctx, false, root.path().into());
                let project = app.development.book.add(root.path()).unwrap();
                app.development.book.active = Some(project);
                app.development.open = development;
                app.codex = Some(crate::backend::codex::Client::mock("normal"));
                let editor = egui::Id::new(if development {
                    "development-input"
                } else {
                    "message-input"
                });
                let render = |app: &mut Wife, events| {
                    ctx.run(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(800., 600.),
                            )),
                            events,
                            ..Default::default()
                        },
                        |ctx| {
                            if development {
                                app.development_window(ctx);
                            } else {
                                app.input_regions.clear();
                                app.input_ui(
                                    ctx,
                                    ctx.screen_rect(),
                                    ctx.screen_rect().center(),
                                    200.,
                                );
                            }
                        },
                    )
                };
                for _ in 0..3 {
                    render(&mut app, vec![]);
                }
                ctx.memory_mut(|memory| memory.request_focus(editor));
                render(&mut app, vec![egui::Event::Text(draft.into())]);
                render(&mut app, vec![]);
                if complete {
                    render(
                        &mut app,
                        vec![egui::Event::Key {
                            key: egui::Key::Tab,
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers: Default::default(),
                        }],
                    );
                    assert!(!app.busy);
                    assert_eq!(
                        if development {
                            &app.development.input
                        } else {
                            &app.input
                        },
                        "/plan "
                    );
                    render(&mut app, vec![egui::Event::Text("检查中文需求".into())]);
                }
                let expected = if complete {
                    "/plan 检查中文需求"
                } else {
                    draft
                };
                assert_eq!(
                    if development {
                        &app.development.input
                    } else {
                        &app.input
                    },
                    expected
                );
                assert!(
                    ctx.memory(|m| m.has_focus(editor)),
                    "editor lost focus before send"
                );
                render(
                    &mut app,
                    vec![egui::Event::Key {
                        key: egui::Key::Enter,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: Default::default(),
                    }],
                );
                assert!(
                    app.busy,
                    "development={development}, draft={draft}, error={}",
                    app.error
                );
                assert_eq!(app.context_ui.last_input, expected);
                assert_eq!(app.development.book.turn_mut().unwrap().user, expected);
                let deadline = Instant::now() + Duration::from_secs(5);
                while app.busy && Instant::now() < deadline {
                    app.poll_codex();
                    std::thread::sleep(Duration::from_millis(5));
                }
                assert!(!app.busy && app.error.is_empty(), "{}", app.error);
                let reply = format!("第1轮：{expected}");
                assert_eq!(
                    app.development.book.turn_mut().unwrap().messages[0].1,
                    reply
                );
                if !development {
                    assert_eq!(app.text, reply);
                }
            }
        }
    }

    #[test]
    fn development_enter_sends_and_ctrl_enter_inserts_newline() {
        let root = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        let mut app = Wife::from_root(&ctx, false, root.path().into());
        let path = app.development.book.add(root.path()).unwrap();
        app.development.book.active = Some(path);
        app.codex = Some(crate::backend::codex::Client::mock("normal"));
        app.development.open = true;
        for _ in 0..3 {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1200., 900.),
                    )),
                    ..Default::default()
                },
                |ctx| app.development_window(ctx),
            );
        }
        let editor_id = egui::Id::new("development-input");
        for (focused, events, expected, sent) in [
            (
                true,
                vec![egui::Event::Text("第一行".into())],
                "第一行",
                false,
            ),
            (
                true,
                vec![egui::Event::Key {
                    key: egui::Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::CTRL,
                }],
                "第一行\n",
                false,
            ),
            (
                true,
                vec![egui::Event::Text("第二行".into())],
                "第一行\n第二行",
                false,
            ),
            (
                false,
                vec![egui::Event::Key {
                    key: egui::Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                "第一行\n第二行",
                false,
            ),
            (
                true,
                vec![egui::Event::Key {
                    key: egui::Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                "",
                true,
            ),
        ] {
            ctx.memory_mut(|memory| {
                if focused {
                    memory.request_focus(editor_id);
                } else {
                    memory.surrender_focus(editor_id);
                }
            });
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1200., 900.),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| app.development_window(ctx),
            );
            assert_eq!(app.development.input, expected);
            assert_eq!(app.busy, sent);
            assert_eq!(app.development_turn, sent);
        }
        assert_eq!(
            app.development.book.turn_mut().unwrap().user,
            "第一行\n第二行"
        );
    }

    #[test]
    fn development_input_keeps_streams_and_waiting_out_of_main_window() {
        for development_mode in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let ctx = egui::Context::default();
            let mut app = Wife::from_root(&ctx, false, root.path().into());
            app.config.codex.development_mode = development_mode;
            let path = app.development.book.add(root.path()).unwrap();
            app.development.book.active = Some(path);
            app.development.storage = Some(root.path().join("history.json"));
            app.development.storage_ok = true;
            app.codex = Some(crate::backend::codex::Client::mock("normal"));
            app.input = "主窗口草稿".into();
            app.development.open = true;
            app.development.input = format!("visibility:{}", root.path().display());
            for _ in 0..3 {
                let _ = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(1200., 900.),
                        )),
                        ..Default::default()
                    },
                    |ctx| app.development_window(ctx),
                );
            }
            ctx.memory_mut(|memory| memory.request_focus("development-input".into()));
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1200., 900.),
                    )),
                    modifiers: egui::Modifiers::NONE,
                    events: vec![egui::Event::Key {
                        key: egui::Key::Enter,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    }],
                    ..Default::default()
                },
                |ctx| app.development_window(ctx),
            );
            assert!(app.busy && app.development_turn);
            assert!(app.development.input.is_empty());
            // Closing the development viewport must not redirect the active turn.
            app.development.open = false;
            let main_frame = |app: &mut Wife| {
                let output = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(800., 600.),
                        )),
                        ..Default::default()
                    },
                    |ctx| {
                        assert!(app.bubble(ctx, ctx.screen_rect().center()).is_none());
                        app.input_ui(ctx, ctx.screen_rect(), ctx.screen_rect().center(), 450.);
                    },
                );
                assert!(app.text.is_empty() && app.codex_messages.is_empty());
                assert!(output.shapes.iter().all(|shape| {
                    !matches!(&shape.shape, egui::Shape::Text(text)
                        if text.galley.job.text.contains("Lexington正在思考"))
                }));
            };
            main_frame(&mut app);
            assert!(app.input_bounds.is_none());
            for (phase, expected) in [
                ("first", "首段"),
                ("next", "首段后续"),
                ("finish", "首段后续最终"),
            ] {
                fs::write(root.path().join(phase), "").unwrap();
                let deadline = Instant::now() + Duration::from_secs(5);
                while Instant::now() < deadline {
                    app.poll_codex();
                    let turn = app.development.book.turn_mut().unwrap();
                    if turn.messages.iter().any(|(_, text)| text == expected)
                        && (phase != "finish" || !app.busy)
                    {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                assert!(
                    app.development
                        .book
                        .turn_mut()
                        .unwrap()
                        .messages
                        .iter()
                        .any(|(_, text)| text == expected)
                );
                if app.busy {
                    app.click_pet();
                    app.click_pet();
                    assert!(!app.turn_ui_hidden);
                }
                main_frame(&mut app);
            }
            assert!(!app.busy);
            assert_eq!(app.input, "主窗口草稿");
            let history = Workspaces::read(app.development.storage.as_ref().unwrap()).unwrap();
            let turn = &history.task().unwrap().turns[0];
            assert_eq!(turn.status, "completed");
            assert_eq!(turn.messages.len(), 1);
            assert_eq!(turn.messages[0].1, "首段后续最终");
            app.send_message(&ctx, "fail".into(), MessageSource::Development);
            let deadline = Instant::now() + Duration::from_secs(5);
            while app.busy && Instant::now() < deadline {
                app.poll_codex();
                std::thread::sleep(Duration::from_millis(5));
            }
            assert!(!app.busy && app.error.contains("模拟请求失败"));
            main_frame(&mut app);
            assert_eq!(app.input, "主窗口草稿");
            app.submit(&ctx);
            assert!(!app.development_turn);
            let deadline = Instant::now() + Duration::from_secs(5);
            while app.busy && Instant::now() < deadline {
                app.poll_codex();
                std::thread::sleep(Duration::from_millis(5));
            }
            assert!(!app.busy && app.error.is_empty());
            assert!(app.text.contains("主窗口草稿"));
            assert!(app.input.is_empty());
        }
    }

    #[test]
    fn development_workspaces_resume_separate_threads_and_render_execution_history() {
        let root = tempfile::tempdir().unwrap();
        let a = root.path().join("项目 A");
        let b = root.path().join("项目 B");
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        let ctx = egui::Context::default();
        let mut app = Wife::from_root(&ctx, false, root.path().into());
        app.config.codex.executable = fixture(root.path(), "development-on");
        app.config.codex.development_mode = true;
        app.development.storage = Some(root.path().join("history.json"));
        app.development.storage_ok = true;
        app.switch_development_workspace(Some(a.clone()), &ctx)
            .unwrap();
        send(&mut app, &ctx, "execution");
        let task = app.development.book.task().unwrap();
        assert_eq!(task.turns[0].status, "completed");
        let command = task.turns[0]
            .items
            .iter()
            .find(|i| i.kind == "commandExecution")
            .unwrap();
        assert_eq!(command.exit_code, Some(0));
        assert_eq!(command.output, "第一段\n第二段\n");
        assert_eq!(
            task.turns[0]
                .items
                .iter()
                .filter(|i| i.kind == "fileChange")
                .count(),
            1
        );
        assert!(task.turns[0].diff.contains("+after"));
        app.development.input = "A 的草稿".into();
        app.switch_development_workspace(Some(b.clone()), &ctx)
            .unwrap();
        assert!(app.development.input.is_empty());
        send(&mut app, &ctx, "resume-status");
        assert!(app.text.starts_with("thread/start:"));
        assert!(app.text.contains("项目 B"));
        app.switch_development_workspace(Some(a), &ctx).unwrap();
        assert_eq!(app.development.input, "A 的草稿");
        send(&mut app, &ctx, "resume-status");
        assert!(app.text.starts_with("thread/resume:"));
        assert!(app.text.contains("项目 A"));
        assert_eq!(app.development.book.task().unwrap().turns.len(), 2);
        app.development.open = true;
        let mut rendered = String::new();
        for _ in 0..3 {
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1200., 900.),
                    )),
                    ..Default::default()
                },
                |ctx| app.development_window(ctx),
            );
            for shape in output.shapes {
                if let egui::epaint::Shape::Text(text) = shape.shape {
                    rendered.push_str(&text.galley.job.text);
                }
            }
        }
        assert!(rendered.contains("development"));
        assert!(rendered.contains("execution"));
        let loaded = Workspaces::read(app.development.storage.as_ref().unwrap()).unwrap();
        assert_eq!(loaded.projects.len(), 2);
        assert_eq!(loaded.task().unwrap().turns.len(), 2);
        app.new_development_task(&ctx);
        send(&mut app, &ctx, "resume-status");
        assert!(app.text.starts_with("thread/start:"));
        assert_eq!(app.development.book.current().unwrap().tasks.len(), 2);
    }

    #[test]
    fn development_mode_switch_defers_then_starts_clean_without_development_instructions() {
        let root = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        crate::backend::config::Config::save_development_mode(root.path(), true).unwrap();
        let mut app = Wife::from_root(&ctx, false, root.path().into());
        app.config.codex.executable = fixture(root.path(), "development-on");
        app.switch_development_workspace(Some(root.path().into()), &ctx)
            .unwrap();
        app.send_message(&ctx, "interrupt".into(), MessageSource::Main);
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.text.is_empty() && Instant::now() < deadline {
            app.poll_codex();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(app.busy && !app.text.is_empty());
        crate::backend::config::Config::save_development_mode(root.path(), false).unwrap();
        app.reload_config();
        assert!(app.codex_restart_pending && app.busy);
        app.codex.as_ref().unwrap().interrupt();
        while app.busy && Instant::now() < deadline {
            app.poll_codex();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!app.busy);
        assert_eq!(
            app.development.book.task().unwrap().turns[0].status,
            "interrupted"
        );
        app.config.codex.executable = fixture(root.path(), "development-off");
        app.start_codex(&ctx);
        send(&mut app, &ctx, "resume-status");
        assert!(app.text.starts_with("thread/start:"));
        assert!(!app.development.book.task().unwrap().development_mode);
        assert_eq!(app.development.book.current().unwrap().tasks.len(), 2);
    }

    #[test]
    fn project_picker_cancel_and_busy_completion_preserve_workspace_and_drafts() {
        let root = tempfile::tempdir().unwrap();
        let a = root.path().join("项目 A");
        let b = root.path().join("项目 B");
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        let ctx = egui::Context::default();
        let mut app = Wife::from_root(&ctx, false, root.path().into());
        app.config.codex.executable = fixture(root.path(), "development-on");
        app.config.codex.development_mode = true;
        let (sender, receiver) = mpsc::channel();
        app.development.picker = Some(receiver);
        sender.send(Some(a.clone())).unwrap();
        app.development_window(&ctx);
        assert_eq!(app.development.book.active, Some(a.canonicalize().unwrap()));
        app.development.input = "项目 A 的草稿".into();
        let (sender, receiver) = mpsc::channel();
        app.development.picker = Some(receiver);
        sender.send(None).unwrap();
        app.development_window(&ctx);
        assert_eq!(app.development.book.projects.len(), 1);
        assert_eq!(app.development.input, "项目 A 的草稿");
        app.send_message(&ctx, "interrupt".into(), MessageSource::Main);
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.text.is_empty() && Instant::now() < deadline {
            app.poll_codex();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(app.busy);
        let (sender, receiver) = mpsc::channel();
        app.development.picker = Some(receiver);
        sender.send(Some(b.clone())).unwrap();
        app.development_window(&ctx);
        assert_eq!(app.development.pending_project, Some(b.clone()));
        assert_eq!(app.development.book.active, Some(a.canonicalize().unwrap()));
        app.codex.as_ref().unwrap().interrupt();
        while app.busy && Instant::now() < deadline {
            app.poll_codex();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!app.busy);
        app.development_window(&ctx);
        assert_eq!(app.development.book.active, Some(b.canonicalize().unwrap()));
        assert!(app.development.input.is_empty());
        assert_eq!(
            app.development
                .drafts
                .get(&a.canonicalize().unwrap())
                .map(String::as_str),
            Some("项目 A 的草稿")
        );
        let (sender, receiver) = mpsc::channel();
        app.development.picker = Some(receiver);
        drop(sender);
        app.development_window(&ctx);
        assert!(app.development.error.contains("无法打开项目选择器"));
    }

    #[test]
    fn project_context_is_mode_gated_and_source_changes_start_a_clean_thread() {
        use crate::development::{
            diagnostics::{Diagnostic, DiagnosticSnapshot, DocumentDiagnostics},
            memory::ProjectMemory,
            testing::TestDefinition,
        };
        use crate::ui::development_memory::MemoryAction;
        let root = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        let file = root.path().join("source.rs");
        fs::write(&file, "original source").unwrap();
        let mut app = Wife::from_root(&ctx, false, root.path().into());
        app.config.codex.executable = fixture(root.path(), "development-on");
        app.config.codex.development_mode = true;
        app.development.storage = Some(root.path().join("history.json"));
        app.development.storage_ok = true;
        app.switch_development_workspace(Some(root.path().into()), &ctx)
            .unwrap();
        app.development.memory_ui.action = Some(MemoryAction::Save(ProjectMemory {
            id: 1,
            title: "模块规则".into(),
            content: "fixture-memory-evidence".into(),
            source_file: "source.rs".into(),
            enabled: true,
            confirmed: true,
            ..Default::default()
        }));
        app.apply_project_memory(&ctx);
        let mut snapshot = DiagnosticSnapshot {
            captured_at: 10,
            documents: vec![DocumentDiagnostics {
                uri: url::Url::from_file_path(&file).unwrap().to_string(),
                path: Some(file.clone()),
                diagnostics: vec![Diagnostic {
                    severity: "error".into(),
                    message: "fixture-diagnostic".into(),
                    start_line: 1,
                    start_column: 1,
                    end_line: 1,
                    end_column: 2,
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        };
        snapshot.capture(root.path()).unwrap();
        let project = app.development.book.current_mut().unwrap();
        project.diagnostics = Some(snapshot.clone());
        project.tests.push(TestDefinition {
            id: 2,
            name: "fixture-test-name".into(),
            command: vec!["fixture-test".into(), "pass".into()],
            timeout_seconds: 10,
        });
        send(&mut app, &ctx, "world-inspect");
        let wire: serde_json::Value = serde_json::from_str(&app.text).unwrap();
        assert_eq!(wire[0]["text"], "world-inspect");
        let background = wire[1]["text"].as_str().unwrap();
        assert!(
            background.contains("fixture-memory-evidence")
                && background.contains("fixture-diagnostic")
                && background.contains("fixture-test-name")
        );
        let old_task = app.development.book.task().unwrap().id;
        fs::write(&file, "changed source").unwrap();
        send(&mut app, &ctx, "world-updated");
        assert_ne!(app.development.book.task().unwrap().id, old_task);
        let wire: serde_json::Value = serde_json::from_str(&app.text).unwrap();
        let background = wire[1]["text"].as_str().unwrap();
        assert!(
            !background.contains("fixture-memory-evidence")
                && !background.contains("fixture-diagnostic")
        );
        app.config.codex.development_mode = false;
        app.development.reset_sessions_pending = true;
        app.config.codex.executable = fixture(root.path(), "development-off");
        app.start_codex(&ctx);
        snapshot.capture(root.path()).unwrap();
        app.development.book.current_mut().unwrap().diagnostics = Some(snapshot);
        send(&mut app, &ctx, "world-disabled");
        let wire: serde_json::Value = serde_json::from_str(&app.text).unwrap();
        assert_eq!(wire.as_array().unwrap().len(), 1);
        app.development.memory_ui.editor = Some(ProjectMemory {
            id: 9,
            title: "candidate".into(),
            content: "candidate text".into(),
            ..Default::default()
        });
        app.development.memory_ui.action = Some(MemoryAction::Save(
            app.development.memory_ui.editor.clone().unwrap(),
        ));
        fs::write(app.development.storage.as_ref().unwrap(), "broken").unwrap();
        app.apply_project_memory(&ctx);
        assert_eq!(app.development.book.current().unwrap().memories.len(), 1);
        assert!(app.development.memory_ui.editor.is_some());
        assert!(app.development.error.contains("损坏"));
    }

    #[test]
    fn test_manager_records_actual_results_retries_failures_and_cancels_without_chat() {
        use crate::development::testing::TestDefinition;
        use crate::ui::development_tests::TestAction;
        let root = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        let mut app = Wife::from_root(&ctx, false, root.path().into());
        app.config.codex.executable = fixture(root.path(), "development-off");
        app.switch_development_workspace(Some(root.path().into()), &ctx)
            .unwrap();
        for (id, arg) in [(1, "pass"), (2, "fail"), (3, "wait")] {
            let definition = TestDefinition {
                id,
                name: format!("fixture-{arg}"),
                command: vec![
                    "fixture-test".into(),
                    arg.into(),
                    "literal argument with spaces".into(),
                ],
                timeout_seconds: 10,
            };
            app.development.tests_ui.action = Some(TestAction::Save(definition.clone()));
            app.apply_development_test(&ctx);
            app.development.tests_ui.action = Some(TestAction::Run(definition));
            app.apply_development_test(&ctx);
            app.poll_development_test_queue(&ctx);
            assert!(app.busy, "{} / {}", app.development.error, app.error);
            if arg == "wait" {
                std::thread::sleep(Duration::from_millis(40));
                app.codex.as_ref().unwrap().interrupt();
            }
            let deadline = Instant::now() + Duration::from_secs(5);
            while app.busy && Instant::now() < deadline {
                app.poll_codex();
                std::thread::sleep(Duration::from_millis(5));
            }
            assert!(!app.busy);
            let run = app
                .development
                .book
                .current()
                .unwrap()
                .test_runs
                .last()
                .unwrap();
            assert_eq!(
                run.status,
                match arg {
                    "pass" => "passed",
                    "fail" => "failed",
                    _ => "cancelled",
                }
            );
        }
        assert!(app.text.is_empty());
        assert!(app.development.book.task().is_none());
        app.development.tests_ui.action = Some(TestAction::RetryFailed);
        app.apply_development_test(&ctx);
        assert_eq!(app.development.tests_ui.queue.len(), 1);
        assert_eq!(app.development.tests_ui.queue[0].1.id, 2);
        app.poll_development_test_queue(&ctx);
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.busy && Instant::now() < deadline {
            app.poll_codex();
            std::thread::sleep(Duration::from_millis(5));
        }
        let project = app.development.book.current().unwrap();
        assert_eq!(project.test_runs.len(), 4);
        assert_eq!(project.test_runs.last().unwrap().exit_code, Some(1));
        assert_eq!(project.test_runs.last().unwrap().stderr, "fixture failure");
    }
}
