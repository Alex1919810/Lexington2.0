//! Native development viewport and UI session coordination.
use super::app::Wife;
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
    drafts: HashMap<PathBuf, String>,
    viewed_task: Option<u64>,
    search: String,
    dirty: bool,
    last_saved: Option<std::time::Instant>,
    picker: Option<mpsc::Receiver<Option<PathBuf>>>,
    pending_project: Option<PathBuf>,
}

impl Wife {
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
            match self.development.book.save(path) {
                Ok(()) => {
                    self.development.dirty = false;
                    self.development.last_saved = Some(std::time::Instant::now());
                }
                Err(error) => self.development.error = error,
            }
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
            turn.status = status.into();
            turn.error = error.into();
            for item in &mut turn.items {
                if item.status == "inProgress" {
                    item.status = "interrupted".into();
                }
            }
            self.save_development(true);
        }
    }
    pub(super) fn development_window(&mut self, ctx: &egui::Context) {
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
            self.save_development(true);
        }
        if !self.development.open {
            self.development.viewport_open = false;
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
                        ui.add(egui::Label::new(egui::RichText::new(&self.codex_status).size(12.).color(egui::Color32::from_gray(100))).truncate());
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
                                    }
                                }
                                if count==0 {ui.weak(if project.tasks.is_empty() {"还没有任务"} else {"没有匹配的任务"});}
                            });
                        });
                    });
                    ui.allocate_ui_with_layout(egui::vec2(ui.available_width(),height), egui::Layout::top_down(egui::Align::Min), |ui| {
                        let project = self.development.book.current().unwrap();
                        let task = project.tasks.iter().find(|task|Some(task.id)==self.development.viewed_task.or(project.current_task));
                        egui::Frame::NONE.fill(egui::Color32::WHITE).stroke(egui::Stroke::new(1_f32,border)).corner_radius(9).inner_margin(14).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            if let Some(task) = task {
                                ui.add(egui::Label::new(egui::RichText::new(&task.title).size(17.).strong()).truncate()).on_hover_text(&task.title);
                                ui.horizontal(|ui| {
                                    ui.weak(format!("{} 轮对话",task.turns.len()));
                                    let compatible = task.character==self.library.character && task.development_mode==self.config.codex.development_mode && (!self.config.codex.development_mode || (task.memory_revision==project.memory_revision && task.memory_context_stamp==crate::development::memory::context_stamp(&self.development.memory_context)));
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.add_enabled(!self.busy,egui::Button::new("删除记录").small()).clicked() {delete_task = Some(task.id);}
                                        if ui.add_enabled(!self.busy && compatible,egui::Button::new("继续此任务").small()).clicked() {resume_task = Some(task.id);}
                                    });
                                });
                            } else {ui.label(egui::RichText::new("新任务").size(17.).strong());ui.weak("在下方描述希望完成的工作。");}
                            ui.separator();
                            let detail_height = (height-260.).max(12.);
                            egui::ScrollArea::vertical().id_salt(("development-detail",task.map(|task|task.id))).max_height(detail_height).min_scrolled_height(0.).auto_shrink([false,false]).show(ui, |ui| {
                                if let Some(task) = task {task_details(ui,task,&project.path);}
                                else {ui.add_space(20.);ui.weak("任务的对话、执行记录和代码变更会显示在这里。");}
                            });
                        });
                        egui::Frame::NONE.fill(egui::Color32::WHITE).stroke(egui::Stroke::new(1_f32,border)).corner_radius(9).inner_margin(12).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            let current = self.development.book.task().map(|task|task.title.as_str()).unwrap_or("新任务");
                            ui.add(egui::Label::new(egui::RichText::new(format!("发送到：{current}")).size(12.).color(egui::Color32::from_gray(100))).truncate()).on_hover_text(current);
                            let editor_id = egui::Id::new("development-input");
                            let send_shortcut = ui.memory(|memory|memory.has_focus(editor_id)) && ui.input_mut(|input|input.consume_key(egui::Modifiers::CTRL,egui::Key::Enter));
                            let mut editor_focused = false;
                            egui::ScrollArea::vertical().id_salt("development-editor-scroll").max_height(64.).show(ui, |ui| {
                                editor_focused = ui.add(egui::TextEdit::multiline(&mut self.development.input).id(editor_id).frame(false).desired_rows(3).desired_width(f32::INFINITY).hint_text("描述需求，或补充任务信息…")).has_focus();
                            });
                            ui.horizontal(|ui| {
                                ui.weak("Ctrl+Enter 发送");
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    let enabled = !self.busy && self.development.picker.is_none() && self.development.pending_project.is_none() && !self.development.input.trim().is_empty();
                                    let button = egui::Button::new(egui::RichText::new("发送").color(egui::Color32::WHITE)).fill(accent).min_size(egui::vec2(76.,30.));
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
                None,
            );
            self.development.page = 0;
        }
        if submit {
            let text = self.development.input.trim().to_owned();
            self.send_message(ctx, text, None);
            if self.busy {
                self.development.input.clear();
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
                egui::CollapsingHeader::new(format!("{} · {}", status_text(&item.status), title))
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
                            rich_text(ui, &format!("```\n{}\n```", item.output));
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
                    });
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
                ui.colored_label(egui::Color32::DARK_RED, &turn.error);
            }
            ui.separator();
        });
    }
}
fn diff_text(ui: &mut egui::Ui, diff: &str) {
    if ui.small_button("复制 diff").clicked() {
        ui.ctx().copy_text(diff.into());
    }
    egui::ScrollArea::horizontal().show(ui, |ui| {
        for line in diff.lines() {
            let color = if line.starts_with('+') {
                egui::Color32::from_rgb(25, 125, 65)
            } else if line.starts_with('-') {
                egui::Color32::from_rgb(190, 55, 55)
            } else {
                ui.visuals().text_color()
            };
            ui.add(
                egui::Label::new(egui::RichText::new(line).monospace().color(color))
                    .selectable(true)
                    .wrap_mode(egui::TextWrapMode::Extend),
            );
        }
    });
}
fn rich_text(ui: &mut egui::Ui, text: &str) {
    let mut code = false;
    let mut block = String::new();
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            if code {
                egui::Frame::NONE
                    .fill(egui::Color32::from_rgb(242, 244, 248))
                    .corner_radius(6)
                    .inner_margin(10)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.weak("代码");
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.small_button("复制代码").clicked() {
                                        ui.ctx().copy_text(block.clone());
                                    }
                                },
                            );
                        });
                        egui::ScrollArea::horizontal().show(ui, |ui| {
                            ui.add(
                                egui::Label::new(egui::RichText::new(block.trim_end()).monospace())
                                    .selectable(true)
                                    .wrap_mode(egui::TextWrapMode::Extend),
                            );
                        });
                    });
                block.clear();
            }
            code = !code;
        } else if code {
            block.push_str(line);
            block.push('\n');
        } else {
            ui.add(egui::Label::new(line).selectable(true).wrap());
        }
    }
    if !block.is_empty() {
        ui.add(
            egui::Label::new(egui::RichText::new(block).monospace())
                .selectable(true)
                .wrap(),
        );
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{Duration, Instant},
    };

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
        app.send_message(ctx, text.into(), None);
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
        app.send_message(&ctx, "interrupt".into(), None);
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
        app.send_message(&ctx, "interrupt".into(), None);
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
            assert!(app.busy);
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
