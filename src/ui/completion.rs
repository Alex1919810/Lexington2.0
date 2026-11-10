//! Text-only slash completion shared by both composers; sending stays with Codex.
use eframe::egui::{self, Id, Rect};

const COMMANDS: &[(&str, &str)] = &[
    ("/plan", "计划模式"),
    ("/compact", "压缩上下文"),
    ("/review", "代码审查"),
    ("/status", "会话状态"),
    ("/model", "模型选择"),
    ("/permissions", "权限设置"),
    ("/new", "新会话"),
    ("/resume", "恢复会话"),
    ("/fork", "分支会话"),
    ("/diff", "代码差异"),
    ("/init", "项目指令"),
    ("/skills", "技能"),
    ("/mcp", "外部工具"),
    ("/goal", "任务目标"),
    ("/fast", "快速模式"),
    ("/mention", "文件引用"),
];

#[derive(Default)]
pub(super) struct Completion {
    query: String,
    selected: usize,
    dismissed: bool,
    composing: bool,
    scroll_to_selected: bool,
    retain_focus: bool,
    pub bounds: Option<Rect>,
}

impl Completion {
    fn candidates(&mut self, text: &str) -> Vec<(&'static str, &'static str)> {
        if self.query != text {
            self.query = text.to_owned();
            self.selected = 0;
            self.dismissed = false;
            self.scroll_to_selected = true;
        }
        if self.dismissed || !text.starts_with('/') || text.chars().any(char::is_whitespace) {
            return Vec::new();
        }
        COMMANDS
            .iter()
            .copied()
            .filter(|(command, _)| command.starts_with(&text.to_ascii_lowercase()))
            .collect()
    }

    fn complete(&mut self, ctx: &egui::Context, editor: Id, text: &mut String, command: &str) {
        *text = format!("{command} ");
        if let Some(mut state) = egui::TextEdit::load_state(ctx, editor) {
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::one(
                    egui::text::CCursor::new(text.chars().count()),
                )));
            state.store(ctx, editor);
        }
        ctx.memory_mut(|memory| memory.request_focus(editor));
        self.retain_focus = true;
        self.bounds = None;
    }

    /// Run before TextEdit and the composer's send shortcut consume keyboard events.
    pub fn keyboard(&mut self, ctx: &egui::Context, editor: Id, text: &mut String) {
        let previous_bounds = self.bounds.take();
        // egui clears focus for Escape at begin_pass, before widgets can consume it.
        self.retain_focus = previous_bounds.is_some()
            && ctx.input(|input| {
                input.focused
                    && (input.key_pressed(egui::Key::Escape)
                        || (input.pointer.any_pressed()
                            && input.pointer.interact_pos().is_some_and(|point| {
                                previous_bounds.is_some_and(|bounds| bounds.contains(point))
                            })))
            })
            && ctx.memory(|memory| memory.had_focus_last_frame(editor));
        if self.retain_focus {
            ctx.memory_mut(|memory| memory.request_focus(editor));
        }
        let ime_event = ctx.input(|input| {
            let mut present = false;
            for event in &input.events {
                if let egui::Event::Ime(event) = event {
                    present = true;
                    self.composing =
                        matches!(event, egui::ImeEvent::Enabled | egui::ImeEvent::Preedit(_));
                }
            }
            present
        });
        if !ctx.input(|input| input.focused) || !ctx.memory(|memory| memory.has_focus(editor)) {
            self.composing = false;
            return;
        }
        if ime_event || self.composing {
            return;
        }
        let candidates = self.candidates(text);
        if candidates.is_empty() {
            return;
        }
        if let Some(range) =
            egui::TextEdit::load_state(ctx, editor).and_then(|s| s.cursor.char_range())
            && (range.primary.index != range.secondary.index
                || range.primary.index != text.chars().count())
        {
            return;
        }
        ctx.input_mut(|input| {
            if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
                self.selected = (self.selected + 1) % candidates.len();
                self.scroll_to_selected = true;
            }
            if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
                self.selected = (self.selected + candidates.len() - 1) % candidates.len();
                self.scroll_to_selected = true;
            }
            if input.consume_key(egui::Modifiers::NONE, egui::Key::Escape) {
                self.dismissed = true;
            }
        });
        let accept = !self.dismissed
            && ctx.input_mut(|input| {
                input.consume_key(egui::Modifiers::NONE, egui::Key::Tab)
                    || input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
            });
        if accept {
            self.complete(ctx, editor, text, candidates[self.selected].0);
        }
    }

    /// Anchor in viewport coordinates, outside the editor's hover transform and scroll area.
    pub fn show(&mut self, ui: &egui::Ui, editor: Id, anchor: Rect, text: &mut String) {
        let ctx = ui.ctx();
        // TextEdit surrenders focus on a click outside its own rectangle.
        if self.retain_focus && ctx.input(|input| input.focused) {
            ctx.memory_mut(|memory| memory.request_focus(editor));
        }
        if self.composing
            || !ctx.input(|input| input.focused)
            || !ctx.memory(|m| m.has_focus(editor))
        {
            return;
        }
        let candidates = self.candidates(text);
        if candidates.is_empty() {
            return;
        }
        ctx.memory_mut(|memory| {
            memory.set_focus_lock_filter(
                editor,
                egui::EventFilter {
                    tab: true,
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    escape: true,
                },
            )
        });
        if let Some(range) =
            egui::TextEdit::load_state(ctx, editor).and_then(|s| s.cursor.char_range())
            && (range.primary.index != range.secondary.index
                || range.primary.index != text.chars().count())
        {
            return;
        }
        let transform = ctx
            .layer_transform_to_global(ui.layer_id())
            .unwrap_or_default();
        let anchor = transform * anchor;
        let width = anchor.width().clamp(240., 380.);
        let height = candidates.len().min(5) as f32 * 26. + 62.;
        let screen = ctx.screen_rect();
        let position = egui::pos2(
            anchor.left().clamp(
                screen.left() + 4.,
                (screen.right() - width - 20.).max(screen.left() + 4.),
            ),
            (anchor.top() - height - 8.).max(screen.top() + 4.),
        );
        let mut chosen = None;
        let response = egui::Area::new(editor.with("slash-completion"))
            .order(egui::Order::Tooltip)
            .fixed_pos(position)
            .constrain_to(screen.shrink(4.))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(width);
                    ui.spacing_mut().item_spacing.y = 2.;
                    egui::ScrollArea::vertical()
                        .id_salt(editor.with("slash-list"))
                        .max_height(130.)
                        .min_scrolled_height(0.)
                        .show(ui, |ui| {
                            for (index, (command, description)) in candidates.iter().enumerate() {
                                let response = ui.add_sized(
                                    [width, 24.],
                                    egui::Button::new(format!("{command}  {description}"))
                                        .selected(index == self.selected),
                                );
                                if self.scroll_to_selected && index == self.selected {
                                    response.scroll_to_me(None);
                                }
                                if response.clicked() {
                                    chosen = Some(*command);
                                }
                            }
                        });
                    ui.small("↑↓ 选择 · Tab / Enter 补全 · Esc 收起");
                    ui.small("仅补全文本，发送后交给 Codex");
                });
            });
        self.bounds = Some(response.response.rect);
        self.scroll_to_selected = false;
        if let Some(command) = chosen {
            self.complete(ctx, editor, text, command);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(key: egui::Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }

    #[test]
    fn completion_keyboard_mouse_escape_and_ime_preserve_the_draft() {
        let ctx = egui::Context::default();
        let id = Id::new("test-completion");
        let mut completion = Completion::default();
        let mut text = String::new();
        let mut sent = false;
        let mut run = |events: Vec<egui::Event>| {
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(500., 500.),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.add_space(300.);
                        completion.keyboard(ctx, id, &mut text);
                        let editor = ui.add(egui::TextEdit::singleline(&mut text).id(id));
                        completion.show(ui, id, editor.rect, &mut text);
                        sent |= ctx.input(|input| input.key_pressed(egui::Key::Enter));
                    });
                },
            );
            (text.clone(), completion.bounds, sent, output)
        };
        run(vec![]);
        ctx.memory_mut(|memory| memory.request_focus(id));
        run(vec![egui::Event::Text("/r".into())]);
        let (_, bounds, _, _) = run(vec![]);
        assert!(bounds.is_some());
        run(vec![key(egui::Key::ArrowDown)]);
        assert_eq!(run(vec![key(egui::Key::Tab)]).0, "/resume ");
        // Replace the draft using the editor's actual select-all behavior.
        let select_all = egui::Event::Key {
            key: egui::Key::A,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        };
        run(vec![select_all.clone(), egui::Event::Text("/co".into())]);
        let (draft, bounds, _, _) = run(vec![key(egui::Key::Escape)]);
        assert_eq!(draft, "/co");
        assert!(bounds.is_none());
        let (draft, bounds, _, _) = run(vec![egui::Event::Text("m".into())]);
        assert_eq!(draft, "/com");
        assert!(bounds.is_some());
        let (draft, _, sent, _) = run(vec![key(egui::Key::Enter)]);
        assert_eq!(draft, "/compact ");
        assert!(!sent, "accepting a completion must not send");
        run(vec![select_all.clone(), egui::Event::Text("/pl".into())]);
        let (_, _, _, output) = run(vec![]);
        let point = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text.starts_with("/plan ") => {
                    Some(text.pos + egui::vec2(10., 6.))
                }
                _ => None,
            })
            .unwrap();
        for pressed in [true, false] {
            run(vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ]);
        }
        assert_eq!(run(vec![]).0, "/plan ");
        run(vec![select_all, egui::Event::Text("/pl".into())]);
        let (_, bounds, _, _) = run(vec![egui::Event::Ime(egui::ImeEvent::Preedit(
            "中文".into(),
        ))]);
        assert!(
            bounds.is_none(),
            "IME candidates own the input while composing"
        );
        let (draft, _, _, _) = run(vec![egui::Event::Ime(egui::ImeEvent::Commit(
            "中文".into(),
        ))]);
        assert!(draft.contains("中文"));
    }
}
