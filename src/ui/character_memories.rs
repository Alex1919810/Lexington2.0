use super::app::Wife;
use crate::{
    backend::{
        codex::{Client, Event},
        config::CodexConfig,
    },
    roleplay::character_memory::{self, Episode, Extraction},
};
use eframe::egui;
use std::{
    collections::{BTreeMap, VecDeque},
    time::{Duration, Instant},
};

#[derive(Default)]
pub(super) struct CharacterMemoryUi {
    pub running: bool,
    pub open: bool,
    window_started: Option<Instant>,
    window_raised: bool,
    browser: MemoryBrowser,
    queue: VecDeque<Queued>,
    active: Option<Active>,
    pub status: String,
}
struct Queued {
    source: Extraction,
    connection: CodexConfig,
}
struct Active {
    client: Client,
    source: Extraction,
    messages: BTreeMap<String, String>,
    final_text: String,
    started: Instant,
    connection: CodexConfig,
}
#[derive(Default)]
pub(super) struct MemoryBrowser {
    character: String,
    loaded_for: Option<String>,
    pub records: Vec<Episode>,
    pub query: String,
    category: String,
    important_only: bool,
    pub pending_delete: Option<(String, String)>,
    shown: usize,
    error: String,
}

impl Wife {
    pub(super) fn open_character_memories(&mut self) {
        if !self.character_memory_ui.open {
            self.character_memory_ui.browser = MemoryBrowser {
                character: self.library.character.clone(),
                ..Default::default()
            };
        }
        self.character_memory_ui.open = true;
    }

    pub(super) fn character_memories_window(&mut self, ctx: &egui::Context) {
        if !self.character_memory_ui.open {
            return;
        }
        let id = egui::ViewportId::from_hash_of("lexington-character-memories");
        let mut builder = egui::ViewportBuilder::default()
            .with_title("人物记忆")
            .with_inner_size(egui::vec2(620., 480.))
            .with_min_inner_size(egui::vec2(400., 300.))
            .with_decorations(true)
            .with_resizable(true)
            .with_always_on_top();
        if self.character_memory_ui.window_started.is_none() {
            self.character_memory_ui.window_started = Some(Instant::now());
            self.character_memory_ui.window_raised = false;
            if let Some(position) = self.window_position {
                builder = builder.with_position(position);
            }
        }
        let mut open = true;
        ctx.show_viewport_immediate(id, builder, |memory, class| {
            if class == egui::ViewportClass::Embedded {
                if let Some(window) = egui::Window::new("人物记忆")
                    .open(&mut open)
                    .order(egui::Order::Foreground)
                    .collapsible(false)
                    .default_size(egui::vec2(596., 430.))
                    .show(memory, |ui| {
                        egui::ScrollArea::vertical()
                            .drag_to_scroll(false)
                            .show(ui, |ui| self.character_memories_ui(ui));
                    })
                    && open
                {
                    self.input_regions.push(window.response.rect);
                }
            } else {
                if !self.character_memory_ui.window_raised
                    && self
                        .character_memory_ui
                        .window_started
                        .is_some_and(|started| started.elapsed() >= Duration::from_millis(800))
                {
                    memory.send_viewport_cmd(egui::ViewportCommand::Focus);
                    self.character_memory_ui.window_raised = true;
                }
                open = !memory.input(|i| i.viewport().close_requested());
                egui::CentralPanel::default().show(memory, |ui| {
                    egui::ScrollArea::vertical()
                        .drag_to_scroll(false)
                        .show(ui, |ui| self.character_memories_ui(ui));
                });
            }
        });
        self.apply_character_memory_action();
        self.character_memory_ui.open = open;
        if !open {
            self.character_memory_ui.window_started = None;
            ctx.send_viewport_cmd_to(id, egui::ViewportCommand::Close);
        }
    }

    pub(super) fn queue_character_memory(&mut self) {
        if self.codex_restart_pending {
            return;
        }
        let Some(character) = self.codex_character.clone().filter(|s| !s.is_empty()) else {
            return;
        };
        let assistant = self
            .codex_messages
            .iter()
            .map(|(_, text)| text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if self.context_ui.last_input.trim().is_empty() || assistant.trim().is_empty() {
            return;
        }
        if self.character_memory_ui.queue.len() >= 4 {
            self.character_memory_ui.status =
                "人物记忆提炼队列已满，本轮跳过；正常对话不受影响。".into();
            return;
        }
        self.character_memory_ui.queue.push_back(Queued {
            source: Extraction::new(character, &self.context_ui.last_input, &assistant),
            connection: self.config.codex.clone(),
        });
    }

    pub(super) fn poll_character_memories(&mut self, ctx: &egui::Context) {
        if !self.character_memory_ui.running {
            return;
        }
        if let Some(active) = &self.character_memory_ui.active
            && (active.connection.connection_changed(&self.config.codex)
                || active.started.elapsed() > Duration::from_secs(90))
        {
            self.character_memory_ui.active = None;
            self.character_memory_ui.status = "后台人物记忆提炼已停止；正常对话不受影响。".into();
        }
        let mut result = None;
        if let Some(active) = &mut self.character_memory_ui.active {
            while let Some(event) = active.client.poll() {
                match event {
                    Event::Text { id, text, replace } => {
                        if replace {
                            active.final_text = text.clone();
                            active.messages.insert(id, text);
                        } else {
                            active.messages.entry(id).or_default().push_str(&text);
                        }
                        if active.messages.values().map(String::len).sum::<usize>() > 32768 {
                            result = Some(Err("人物记忆提炼结果过大，未保存。".to_owned()));
                            break;
                        }
                    }
                    Event::Finished { interrupted: false } => {
                        result = Some(character_memory::store_extracted(
                            &self.root,
                            &active.source,
                            &active.final_text,
                        ));
                        break;
                    }
                    Event::Finished { interrupted: true }
                    | Event::Failed(_)
                    | Event::Stopped(_)
                    | Event::Prompt(_) => {
                        result = Some(Err(
                            "人物记忆提炼失败，未保存；后续对话仍会继续提炼。".into()
                        ));
                        break;
                    }
                    _ => (),
                }
            }
        }
        if let Some(result) = result {
            let active = self.character_memory_ui.active.take().unwrap();
            self.character_memory_ui.status = result.map_or_else(
                |error| error,
                |count| format!("{}：本轮新增 {count} 条人物记忆。", active.source.character),
            );
            if self.character_memory_ui.browser.character == active.source.character {
                self.character_memory_ui.browser.loaded_for = None;
            }
        }
        if self.character_memory_ui.active.is_none()
            && let Some(queued) = self.character_memory_ui.queue.pop_front()
        {
            if queued.connection.connection_changed(&self.config.codex) {
                self.character_memory_ui.status =
                    "连接设置已变更，已跳过旧连接下排队的记忆提炼。".into();
                return;
            }
            let source = queued.source;
            let wake = ctx.clone();
            match Client::start_structured(
                &self.config.codex,
                &self.root,
                character_memory::INSTRUCTIONS.into(),
                character_memory::output_schema(),
                move || wake.request_repaint(),
            ) {
                Ok(client) => match client.submit(source.prompt(), false) {
                    Ok(()) => {
                        self.character_memory_ui.status =
                            format!("正在后台提炼 {} 的人物记忆…", source.character);
                        self.character_memory_ui.active = Some(Active {
                            client,
                            source,
                            messages: Default::default(),
                            final_text: String::new(),
                            started: Instant::now(),
                            connection: self.config.codex.clone(),
                        });
                    }
                    Err(_) => {
                        self.character_memory_ui.status =
                            "无法启动人物记忆提炼；正常对话不受影响。".into()
                    }
                },
                Err(_) => {
                    self.character_memory_ui.status =
                        "无法连接人物记忆提炼服务；请检查连接设置。".into()
                }
            }
        }
        if self.character_memory_ui.active.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    pub(super) fn character_memories_ui(&mut self, ui: &mut egui::Ui) {
        let browser = &mut self.character_memory_ui.browser;
        let previous = browser.character.clone();
        egui::ComboBox::from_id_salt("memory-character")
            .selected_text(&browser.character)
            .show_ui(ui, |ui| {
                for name in self.library.characters() {
                    ui.selectable_value(&mut browser.character, name.clone(), name);
                }
            });
        if previous != browser.character {
            *browser = MemoryBrowser {
                character: browser.character.clone(),
                ..Default::default()
            };
        }
        let character = browser.character.clone();
        if character.is_empty() {
            ui.label("暂无记忆");
            return;
        }
        let refresh = ui.button("刷新人物记忆").clicked();
        if browser.loaded_for.as_deref() != Some(&character) || refresh {
            match character_memory::list(&self.root, &character) {
                Ok(records) => {
                    browser.records = records;
                    browser.loaded_for = Some(character.clone());
                    browser.error.clear();
                    browser.shown = 50;
                }
                Err(error) => browser.error = error,
            }
        }
        if !browser.error.is_empty() {
            ui.colored_label(egui::Color32::DARK_RED, &browser.error);
        }
        ui.add(
            egui::TextEdit::singleline(&mut browser.query).hint_text("搜索事件、参与者或原文依据"),
        );
        ui.horizontal_wrapped(|ui| {
            egui::ComboBox::from_id_salt("character-memory-category")
                .selected_text(if browser.category.is_empty() {
                    "所有事件"
                } else {
                    &browser.category
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut browser.category, String::new(), "所有事件");
                    for category in character_memory::CATEGORIES {
                        ui.selectable_value(&mut browser.category, category.into(), category);
                    }
                });
            ui.checkbox(&mut browser.important_only, "只看重要记忆");
        });
        let query = browser.query.trim().to_lowercase();
        let records: Vec<_> = browser
            .records
            .iter()
            .filter(|event| {
                (browser.category.is_empty() || event.category == browser.category)
                    && (!browser.important_only || event.importance >= 2)
                    && (query.is_empty()
                        || format!(
                            "{}\n{}\n{}\n{}",
                            event.title,
                            event.description,
                            event.participants.join(" "),
                            event.evidence
                        )
                        .to_lowercase()
                        .contains(&query))
            })
            .collect();
        ui.label(format!("{} 条人物记忆", records.len()));
        if records.is_empty() {
            ui.small("暂无记忆");
        }
        for event in records.iter().take(browser.shown) {
            ui.separator();
            egui::CollapsingHeader::new(format!(
                "{} · {}",
                format_time(event.recorded_at),
                event.title
            ))
            .id_salt(&event.id)
            .show(ui, |ui| {
                ui.label(format!(
                    "{} · {} · {}",
                    event.category,
                    event.perspective,
                    match event.importance {
                        3 => "里程碑",
                        2 => "重要",
                        _ => "日常",
                    }
                ));
                ui.label(&event.description);
                ui.label(format!("事件时间：{}", event.happened_at));
                ui.label(format!("参与者：{}", event.participants.join("、")));
                ui.label("模型提炼依据");
                ui.label(&event.evidence);
                ui.collapsing("对应对话", |ui| {
                    ui.label(format!(
                        "用户：{}\n\n角色：{}",
                        event.source_user, event.source_assistant
                    ));
                });
                if ui.button("删除此条记忆").clicked() {
                    browser.pending_delete = Some((character.clone(), event.id.clone()));
                }
            });
        }
        if records.len() > browser.shown && ui.button("显示更多人物记忆").clicked() {
            browser.shown += 50;
        }
    }

    pub(super) fn apply_character_memory_action(&mut self) {
        let browser = &mut self.character_memory_ui.browser;
        if let Some((character, id)) = browser.pending_delete.take() {
            match character_memory::delete(&self.root, &character, &id) {
                Ok(()) => {
                    browser.loaded_for = None;
                    browser.error.clear();
                }
                Err(error) => browser.error = error,
            }
        }
    }
}

fn format_time(timestamp: i64) -> String {
    crate::tools::time::local(timestamp).map_or_else(
        || "时间未知".into(),
        |date| {
            format!(
                "{:04}-{:02}-{:02} {:02}:{:02}",
                date.tm_year + 1900,
                date.tm_mon + 1,
                date.tm_mday,
                date.tm_hour,
                date.tm_min
            )
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn frame(app: &mut Wife, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(700., 1800.),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    app.character_memories_ui(ui);
                });
            },
        );
        app.apply_character_memory_action();
        output
    }
    fn click(app: &mut Wife, ctx: &egui::Context, label: &str) {
        let position = frame(app, ctx, vec![])
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::epaint::Shape::Text(text) = &shape.shape
                    && text.galley.job.text.contains(label)
                {
                    Some(text.pos + text.galley.size() / 2.)
                } else {
                    None
                }
            })
            .unwrap_or_else(|| panic!("missing control: {label}"));
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
    fn fixture_response(source: &Extraction) -> String {
        json!({"events":[{"title":"温茶偏好","description":"用户喜欢温热的茶。","category":"偏好","happenedAt":"未说明","participants":["用户","a"],"importance":2,"perspective":"用户陈述","evidence":source.user}]}).to_string()
    }

    #[test]
    fn memory_browser_searches_and_deletes_without_model_requests_or_chat_changes() {
        let root = std::env::temp_dir().join(format!(
            "lexington-memory-browser-{}",
            rand::random::<u64>()
        ));
        let ctx = egui::Context::default();
        ctx.style_mut(|style| style.animation_time = 0.0);
        let mut app = Wife::from_root(&ctx, true, root.clone());
        app.library.character = "a".into();
        app.open_character_memories();
        app.text = "正常聊天保持原样".into();
        let source = Extraction::new("a".into(), "我喜欢温热的茶", "收到。");
        character_memory::store_extracted(&root, &source, &fixture_response(&source)).unwrap();
        frame(&mut app, &ctx, vec![]);
        assert_eq!(app.character_memory_ui.browser.records.len(), 1);
        app.settings_ui(&ctx);
        assert!(app.character_memory_ui.open);
        assert_eq!(app.character_memory_ui.browser.records.len(), 1);
        app.character_memory_ui.browser.query = "不存在的词".into();
        let output = frame(&mut app, &ctx, vec![]);
        assert!(output.shapes.iter().any(|shape|matches!(&shape.shape,egui::epaint::Shape::Text(text) if text.galley.job.text=="0 条人物记忆")));
        assert!(!output.shapes.iter().any(|shape|matches!(&shape.shape,egui::epaint::Shape::Text(text) if text.galley.job.text.contains("自动提炼") || text.galley.job.text.contains("保存并应用"))));
        app.character_memory_ui.browser.query.clear();
        click(&mut app, &ctx, "温茶偏好");
        click(&mut app, &ctx, "删除此条记忆");
        assert!(character_memory::list(&root, "a").unwrap().is_empty());
        assert_eq!(app.text, "正常聊天保持原样");
        assert!(app.codex.is_none() && app.character_memory_ui.active.is_none());
        drop(app);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn only_successful_conversations_enter_the_background_queue_without_a_switch() {
        let root =
            std::env::temp_dir().join(format!("lexington-memory-queue-{}", rand::random::<u64>()));
        let ctx = egui::Context::default();
        let mut app = Wife::from_root(&ctx, true, root.clone());
        app.codex_character = Some("a".into());
        // Old optional fields are preserved as unknown data, but no longer control extraction.
        app.codex_role_card =
            Some(serde_json::from_value(json!({"characterMemory":{"enabled":false}})).unwrap());
        app.codex = Some(Client::mock("normal"));
        for (message, count) in [("我喜欢温热的茶", 1), ("fail", 1), ("明天一起喝茶", 2)]
        {
            app.input = message.into();
            app.submit(&ctx);
            let deadline = Instant::now() + Duration::from_secs(5);
            while app.busy && Instant::now() < deadline {
                app.poll_codex();
                std::thread::sleep(Duration::from_millis(5));
            }
            assert!(!app.busy);
            assert_eq!(app.character_memory_ui.queue.len(), count);
        }
        assert_eq!(app.character_memory_ui.queue[0].source.character, "a");
        assert_eq!(
            app.character_memory_ui.queue[0].source.user,
            "我喜欢温热的茶"
        );
        assert!(!app.text.contains("\"events\""));
        assert!(app.character_memory_ui.active.is_none());
        drop(app);
        if root.exists() {
            std::fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn connection_change_discards_queued_text_before_using_a_new_provider() {
        let root = std::env::temp_dir().join(format!(
            "lexington-memory-connection-{}",
            rand::random::<u64>()
        ));
        let ctx = egui::Context::default();
        let mut app = Wife::from_root(&ctx, true, root);
        app.character_memory_ui.queue.push_back(Queued {
            source: Extraction::new("a".into(), "private fixture text", "reply"),
            connection: app.config.codex.clone(),
        });
        app.config.codex.development_mode = true;
        app.character_memory_ui.running = true;
        app.poll_character_memories(&ctx);
        assert!(app.character_memory_ui.active.is_none());
        assert!(app.character_memory_ui.queue.is_empty());
        assert!(app.character_memory_ui.status.contains("连接设置已变更"));
    }

    #[test]
    fn background_extraction_saves_automatically_and_never_enters_dialogue_output() {
        for mode in ["memory-success", "memory-malformed", "memory-fail"] {
            let root = std::env::temp_dir().join(format!(
                "lexington-memory-background-{}",
                rand::random::<u64>()
            ));
            let ctx = egui::Context::default();
            let mut app = Wife::from_root(&ctx, true, root.clone());
            std::fs::create_dir_all(&root).unwrap();
            app.text = "正常角色回复".into();
            app.input = "用户草稿".into();
            app.codex_messages = vec![("normal".into(), "正常角色回复".into())];
            let source = Extraction::new("a".into(), "我喜欢温热的茶", "正常角色回复");
            let client = Client::mock_structured(mode);
            client.submit(source.prompt(), false).unwrap();
            app.character_memory_ui.running = true;
            app.character_memory_ui.active = Some(Active {
                client,
                source,
                messages: Default::default(),
                final_text: String::new(),
                started: Instant::now(),
                connection: app.config.codex.clone(),
            });
            let deadline = Instant::now() + Duration::from_secs(5);
            while app.character_memory_ui.active.is_some() && Instant::now() < deadline {
                app.poll_character_memories(&ctx);
                std::thread::sleep(Duration::from_millis(5));
            }
            assert!(app.character_memory_ui.active.is_none());
            assert_eq!(
                character_memory::list(&root, "a").unwrap().len(),
                usize::from(mode == "memory-success")
            );
            assert_eq!(app.text, "正常角色回复");
            assert_eq!(app.input, "用户草稿");
            assert_eq!(
                app.codex_messages,
                vec![("normal".into(), "正常角色回复".into())]
            );
            assert!(!app.busy);
            drop(app);
            std::fs::remove_dir_all(root).unwrap();
        }
    }
}
