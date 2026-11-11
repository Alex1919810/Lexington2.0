use super::{app::Wife, style};
use crate::{backend::memo::MemoState, tools::paths};
use eframe::egui;
use std::time::{Duration, Instant};

#[derive(Default)]
pub(super) struct MemoUi {
    pub open: bool,
    state: Option<MemoState>,
    draft: String,
    error: String,
    filter: u8,
    action: Option<Action>,
    started: Option<Instant>,
    raised: bool,
}

enum Action {
    Add,
    Complete(u64, bool),
    Delete(u64),
    Reload,
}

impl Wife {
    pub(super) fn open_memos(&mut self) {
        if self.memo.state.is_none() {
            self.memo.action = Some(Action::Reload);
            self.apply_memo_action();
        }
        self.memo.open = true;
    }

    pub(super) fn memo_window(&mut self, ctx: &egui::Context) {
        if !self.memo.open {
            return;
        }
        let id = egui::ViewportId::from_hash_of("lexington-memos");
        let size = egui::vec2(500., 520.);
        let mut builder = egui::ViewportBuilder::default()
            .with_title("备忘录")
            .with_inner_size(size)
            .with_min_inner_size(egui::vec2(380., 300.))
            .with_resizable(true)
            .with_always_on_top();
        if self.memo.started.is_none() {
            self.memo.started = Some(Instant::now());
            self.memo.raised = false;
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
            // Leave room for the native title bar and screen edges on first open.
            let near = self.window_position.unwrap_or(monitor.min);
            builder = builder.with_position(egui::pos2(
                near.x.clamp(
                    monitor.left() + 8.,
                    (monitor.right() - size.x - 8.).max(monitor.left() + 8.),
                ),
                near.y.clamp(
                    monitor.top() + 8.,
                    (monitor.bottom() - size.y - 48.).max(monitor.top() + 8.),
                ),
            ));
        }
        let mut open = true;
        ctx.show_viewport_immediate(id, builder, |memo, class| {
            if class == egui::ViewportClass::Embedded {
                if let Some(window) = egui::Window::new("备忘录")
                    .open(&mut open)
                    .order(egui::Order::Foreground)
                    .collapsible(false)
                    .default_size(egui::vec2(468., 470.))
                    .show(memo, |ui| self.memo_ui(ui))
                    && open
                {
                    self.input_regions.push(window.response.rect);
                }
            } else {
                if !self.memo.raised
                    && self
                        .memo
                        .started
                        .is_some_and(|started| started.elapsed() >= Duration::from_millis(800))
                {
                    memo.send_viewport_cmd(egui::ViewportCommand::Focus);
                    self.memo.raised = true;
                }
                open = !memo.input(|input| input.viewport().close_requested());
                egui::CentralPanel::default()
                    .frame(egui::Frame::new().fill(style::PANEL_BG).inner_margin(12))
                    .show(memo, |ui| self.memo_ui(ui));
            }
        });
        self.apply_memo_action();
        self.memo.open = open;
        if !open {
            self.memo.started = None;
            ctx.send_viewport_cmd_to(id, egui::ViewportCommand::Close);
        }
    }

    fn memo_ui(&mut self, ui: &mut egui::Ui) {
        style::panel_style(ui);
        let memo = &mut self.memo;
        ui.horizontal(|ui| {
            ui.heading("备忘录");
            if let Some(state) = &memo.state {
                let remaining = state.items().iter().filter(|item| !item.completed).count();
                ui.label(egui::RichText::new(format!("{remaining} 项待办")).color(style::MUTED));
            }
        });
        if !memo.error.is_empty() {
            egui::ScrollArea::vertical()
                .id_salt("memo-error")
                .max_height(45.)
                .min_scrolled_height(0.)
                .show(ui, |ui| {
                    ui.colored_label(egui::Color32::from_rgb(174, 54, 54), &memo.error);
                });
        }
        if memo.state.is_none() {
            if ui.button("重新读取").clicked() {
                memo.action = Some(Action::Reload);
            }
            return;
        }
        style::card().inner_margin(10).show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("memo-draft-scroll")
                .max_height(56.)
                .min_scrolled_height(0.)
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut memo.draft)
                            .id_salt("memo-draft")
                            .hint_text("记下要做的事…")
                            .desired_rows(2)
                            .desired_width(f32::INFINITY),
                    );
                });
            ui.horizontal(|ui| {
                let count = memo.draft.trim().chars().count();
                if ui
                    .add_enabled(count > 0, egui::Button::new("添加事项"))
                    .clicked()
                {
                    memo.action = Some(Action::Add);
                }
                ui.label(
                    egui::RichText::new("勾选完成 · 自动保存")
                        .small()
                        .color(style::MUTED),
                );
            });
        });
        ui.horizontal(|ui| {
            for (filter, label) in [(0, "全部"), (1, "待办"), (2, "已完成")] {
                ui.selectable_value(&mut memo.filter, filter, label);
            }
        });
        egui::ScrollArea::vertical()
            .id_salt(("memo-items", memo.filter))
            .auto_shrink([false, false])
            .min_scrolled_height(0.)
            .show(ui, |ui| {
                let mut shown = false;
                for item in memo
                    .state
                    .as_ref()
                    .unwrap()
                    .items()
                    .iter()
                    .filter(|item| memo.filter == 0 || (memo.filter == 2) == item.completed)
                {
                    shown = true;
                    ui.push_id(item.id, |ui| {
                        style::card().inner_margin(10).show(ui, |ui| {
                            ui.horizontal_top(|ui| {
                                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                                let mut completed = item.completed;
                                let mut text = egui::RichText::new(&item.text);
                                if completed {
                                    text = text.strikethrough().color(style::MUTED);
                                }
                                // Reserve a fixed right column so long text wraps before Delete.
                                let width = (ui.available_width() - 68.).max(40.);
                                if ui
                                    .allocate_ui_with_layout(
                                        egui::vec2(width, 0.),
                                        egui::Layout::top_down(egui::Align::Min),
                                        |ui| {
                                            ui.set_min_width(width);
                                            ui.add(egui::Checkbox::new(&mut completed, text))
                                                .changed()
                                        },
                                    )
                                    .inner
                                {
                                    memo.action = Some(Action::Complete(item.id, completed));
                                }
                                if ui
                                    .button("删除")
                                    .on_hover_text("彻底删除这条事项")
                                    .clicked()
                                {
                                    memo.action = Some(Action::Delete(item.id));
                                }
                            });
                        });
                    });
                }
                if !shown {
                    ui.label(
                        egui::RichText::new(match memo.filter {
                            1 => "没有待办事项。",
                            2 => "还没有已完成事项。",
                            _ => "还没有事项，写下第一条备忘吧。",
                        })
                        .color(style::MUTED),
                    );
                }
            });
    }

    fn apply_memo_action(&mut self) {
        let Some(action) = self.memo.action.take() else {
            return;
        };
        let result = match action {
            Action::Reload => {
                let path = paths::data_root().join(if self.dev {
                    "memos-dev.json"
                } else {
                    "memos.json"
                });
                MemoState::load(path).map(|state| self.memo.state = Some(state))
            }
            action => {
                let Some(state) = self.memo.state.as_mut() else {
                    return;
                };
                match action {
                    Action::Add => state
                        .add(&self.memo.draft)
                        .map(|()| self.memo.draft.clear()),
                    Action::Complete(id, completed) => state.set_completed(id, completed),
                    Action::Delete(id) => state.delete(id),
                    Action::Reload => unreachable!(),
                }
            }
        };
        self.memo.error = result.err().unwrap_or_default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(app: &mut Wife, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(380., 300.),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| app.memo_ui(ui));
            },
        );
        app.apply_memo_action();
        output
    }

    fn click(app: &mut Wife, ctx: &egui::Context, label: &str) {
        let output = frame(app, ctx, vec![]);
        let position = output
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape
                    && text.galley.job.text == label
                {
                    Some(text.pos + text.galley.size() / 2.)
                } else {
                    None
                }
            })
            .unwrap_or_else(|| panic!("missing memo control: {label}"));
        assert!(
            position.x < 380. && position.y < 300.,
            "control outside viewport: {label}"
        );
        frame(app, ctx, vec![egui::Event::PointerMoved(position)]);
        for pressed in [true, false] {
            frame(
                app,
                ctx,
                vec![egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                }],
            );
        }
    }

    #[test]
    fn memo_ui_adds_strikes_restores_filters_and_deletes_without_touching_chat() {
        let directory = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        let mut app = Wife::from_root(&ctx, true, directory.path().to_path_buf());
        let path = directory.path().join("memos.json");
        app.memo.state = Some(MemoState::load(path.clone()).unwrap());
        app.open_memos();
        app.text = "聊天输出".into();
        app.input = "聊天草稿".into();
        app.busy = true;
        app.memo.draft = "测试备忘".into();
        click(&mut app, &ctx, "添加事项");
        assert!(app.memo.draft.is_empty());
        assert_eq!(MemoState::load(path.clone()).unwrap().items().len(), 1);
        click(&mut app, &ctx, "测试备忘");
        assert!(app.memo.state.as_ref().unwrap().items()[0].completed);
        let output = frame(&mut app, &ctx, vec![]);
        assert!(output.shapes.iter().any(|shape| {
            matches!(&shape.shape, egui::Shape::Text(text)
                if text.galley.job.text == "测试备忘"
                && text.galley.job.sections.iter().all(|section| section.format.strikethrough != egui::Stroke::NONE))
        }));
        click(&mut app, &ctx, "待办");
        let output = frame(&mut app, &ctx, vec![]);
        assert!(!output.shapes.iter().any(|shape| {
            matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text == "测试备忘")
        }));
        click(&mut app, &ctx, "已完成");
        click(&mut app, &ctx, "全部");
        click(&mut app, &ctx, "测试备忘");
        assert!(!MemoState::load(path.clone()).unwrap().items()[0].completed);
        click(&mut app, &ctx, "删除");
        assert!(MemoState::load(path).unwrap().items().is_empty());
        app.settings = false;
        app.memo.open = false;
        app.memo.draft = "关闭窗口保留草稿".into();
        app.open_memos();
        assert!(app.memo.open && !app.settings);
        assert_eq!(app.memo.draft, "关闭窗口保留草稿");
        assert_eq!(app.text, "聊天输出");
        assert_eq!(app.input, "聊天草稿");
        assert!(app.busy && app.codex.is_none() && app.codex_messages.is_empty());
    }

    #[test]
    fn memo_ui_wraps_long_items_and_preserves_draft_and_checks_on_save_failure() {
        let directory = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        let mut app = Wife::from_root(&ctx, true, directory.path().to_path_buf());
        let path = directory.path().join("memos.json");
        let mut state = MemoState::load(path.clone()).unwrap();
        let long_text = "很长的备忘内容需要在窄窗口内换行".repeat(4);
        state.add(&long_text).unwrap();
        app.memo.state = Some(state);
        app.memo.draft = "多行草稿\n".repeat(30);
        let output = frame(&mut app, &ctx, vec![]);
        let text = output
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape
                    && text.galley.job.text == long_text
                {
                    Some(text)
                } else {
                    None
                }
            })
            .unwrap();
        assert!(text.galley.rows.len() > 1);
        assert!(text.pos.x + text.galley.size().x <= 380.);
        assert!(
            text.pos.y < 300.,
            "first memo must be visible at the minimum size"
        );
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        let id = app.memo.state.as_ref().unwrap().items()[0].id;
        app.memo.action = Some(Action::Complete(id, true));
        app.apply_memo_action();
        assert!(!app.memo.state.as_ref().unwrap().items()[0].completed);
        app.memo.draft = "保存失败保留我".into();
        click(&mut app, &ctx, "添加事项");
        assert_eq!(app.memo.draft, "保存失败保留我");
        assert!(!app.memo.error.is_empty());
        assert_eq!(app.memo.state.as_ref().unwrap().items().len(), 1);
    }

    #[test]
    fn memo_and_schedule_context_menu_entries_open_independent_windows() {
        for label in ["备忘录", "定时任务"] {
            let directory = tempfile::tempdir().unwrap();
            let ctx = egui::Context::default();
            let mut app = Wife::from_root(&ctx, true, directory.path().to_path_buf());
            app.memo.state = Some(MemoState::load(directory.path().join("memos.json")).unwrap());
            let mut run = |events| {
                ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(500., 500.),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ctx| {
                        egui::CentralPanel::default().show(ctx, |ui| {
                            let response =
                                ui.allocate_response(egui::vec2(100., 100.), egui::Sense::click());
                            app.context_menu(ctx, &response);
                        });
                    },
                )
            };
            run(vec![]);
            let point = egui::pos2(40., 40.);
            run(vec![egui::Event::PointerMoved(point)]);
            for pressed in [true, false] {
                run(vec![egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Secondary,
                    pressed,
                    modifiers: Default::default(),
                }]);
            }
            let output = run(vec![]);
            let point = output
                .shapes
                .iter()
                .find_map(|shape| {
                    if let egui::Shape::Text(text) = &shape.shape
                        && text.galley.job.text == label
                    {
                        Some(text.pos + text.galley.size() / 2.)
                    } else {
                        None
                    }
                })
                .expect("memo entry in pet context menu");
            run(vec![egui::Event::PointerMoved(point)]);
            for pressed in [true, false] {
                run(vec![egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                }]);
            }
            assert_eq!(app.memo.open, label == "备忘录");
            assert_eq!(app.schedule.open, label == "定时任务");
            assert!(!app.settings && !app.character_memory_ui.open);
            assert!(app.codex.is_none());
        }
    }
}
