use super::app::Pet;
use crate::roleplay::{self, RoleCard, Rule, WorldEntry};
use eframe::egui;

#[derive(Default)]
pub(super) struct RoleEditor {
    pub profile: Option<crate::roleplay::UserProfile>,
    pub pending_profile: Option<(String, crate::roleplay::UserProfile)>,
    pub character: String,
    pub draft: Option<RoleCard>,
    pub section: usize,
    pub error: String,
    pub notice: String,
    pub pending_clear: Option<String>,
    pub clear_confirmed: bool,
}

fn editor(ui: &mut egui::Ui, label: &str, text: &mut String) {
    ui.label(label);
    ui.add(
        egui::TextEdit::multiline(text)
            .desired_width(f32::INFINITY)
            .desired_rows(4),
    );
}

impl Pet {
    pub(super) fn roleplay_file_stamp(&self) -> [Option<(std::time::SystemTime, u64)>; 2] {
        ["roleplay.json", "roleplay-memory.json"].map(|name| {
            std::fs::metadata(self.root.join("config").join(name))
                .ok()
                .and_then(|metadata| metadata.modified().ok().map(|time| (time, metadata.len())))
        })
    }

    pub(super) fn reload_roleplay(&mut self) {
        self.roleplay_stamp = self.roleplay_file_stamp();
        match roleplay::session_card(&self.root, &self.library.character) {
            Ok(card) => {
                self.codex_restart_pending |=
                    self.codex_character.is_some() && card != self.codex_role_card;
                self.role_editor.error.clear();
            }
            Err(error) => self.role_editor.error = error,
        }
    }

    pub(super) fn role_card_ui(&mut self, ui: &mut egui::Ui) -> Option<(String, RoleCard)> {
        if self.role_editor.character.is_empty() {
            self.role_editor.character = self.library.character.clone();
        }
        let previous = self.role_editor.character.clone();
        egui::ComboBox::from_id_salt("role-card-character")
            .selected_text(if previous.is_empty() {
                "请选择角色"
            } else {
                &previous
            })
            .show_ui(ui, |ui| {
                for name in self.library.characters() {
                    ui.selectable_value(&mut self.role_editor.character, name.clone(), name);
                }
            });
        if previous != self.role_editor.character {
            self.role_editor.profile = None;
            self.role_editor.pending_profile = None;
            self.role_editor.pending_clear = None;
            self.role_editor.clear_confirmed = false;
            self.role_editor.draft = None;
            self.role_editor.error.clear();
            self.role_editor.notice.clear();
        }
        if self.role_editor.character.is_empty() {
            ui.label("请先添加角色立绘资源。");
            return None;
        }
        if self.role_editor.draft.is_none() && self.role_editor.error.is_empty() {
            let result =
                RoleCard::load(&self.root, &self.role_editor.character).and_then(
                    |card| match card {
                        Some(card) => Ok(card),
                        None => Ok(RoleCard {
                            name: self.role_editor.character.clone(),
                            description: roleplay::agent_instructions(
                                &self.root,
                                &self.role_editor.character,
                            )?
                            .unwrap_or_default(),
                            ..Default::default()
                        }),
                    },
                );
            match result {
                Ok(card) => self.role_editor.draft = Some(card),
                Err(error) => self.role_editor.error = error,
            }
        }
        ui.small("切换编辑角色或关闭设置会放弃未保存的修改。");
        let mut save = false;
        ui.horizontal_wrapped(|ui| {
            save = ui
                .add_enabled(
                    self.role_editor.draft.is_some() && self.role_editor.section != 3,
                    egui::Button::new("保存并应用"),
                )
                .clicked();
            if ui.button("取消修改").clicked() {
                self.role_editor.clear_confirmed = false;
                self.role_editor.pending_clear = None;
                self.role_editor.profile = None;
                self.role_editor.draft = None;
                self.role_editor.error.clear();
                self.role_editor.notice.clear();
            }
        });
        ui.small(if self.busy {
            "当前任务结束后，新会话将应用保存的设定。"
        } else {
            "保存当前角色的设定会新建会话。"
        });
        if !self.role_editor.error.is_empty() {
            ui.colored_label(egui::Color32::DARK_RED, &self.role_editor.error);
        }
        if !self.role_editor.notice.is_empty() {
            ui.label(&self.role_editor.notice);
        }
        ui.horizontal_wrapped(|ui| {
            for (index, title) in [
                "角色卡",
                "对话示例",
                "世界书",
                "用户与记忆",
                "附加规则",
                "上下文",
                "长期记忆",
            ]
            .iter()
            .enumerate()
            {
                ui.selectable_value(&mut self.role_editor.section, index, *title);
            }
        });
        if self.role_editor.section == 6 {
            self.role_vector_ui(ui);
            return save
                .then(|| {
                    self.role_editor
                        .draft
                        .clone()
                        .map(|card| (self.role_editor.character.clone(), card))
                })
                .flatten();
        }
        if self.role_editor.section == 5 || self.role_editor.section == 3 {
            if self.role_editor.section == 5 {
                self.role_context_ui(ui);
            } else {
                self.role_memory_ui(ui);
            }
            return save
                .then(|| {
                    (
                        self.role_editor.character.clone(),
                        self.role_editor.draft.clone(),
                    )
                })
                .and_then(|(name, card)| card.map(|card| (name, card)));
        }
        let card = self.role_editor.draft.as_mut()?;
        match self.role_editor.section {
            0 => {
                ui.label("角色名称");
                ui.text_edit_singleline(&mut card.name);
                editor(ui, "身份与描述", &mut card.description);
                editor(ui, "性格与语气", &mut card.personality);
                editor(ui, "场景", &mut card.scenario);
                editor(ui, "与用户的关系", &mut card.relationship);
            }
            1 => {
                ui.small("使用“用户：… / 角色：…”编写示例；示例不会作为已发生的对话。");
                editor(ui, "对话示例与代表台词", &mut card.dialogue_examples);
            }
            2 => {
                ui.small("扫描最近对话；始终启用、关键词命中和持续效果均受本轮预算限制。");
                ui.horizontal(|ui| {
                    ui.label("默认扫描条数");
                    ui.add(egui::DragValue::new(&mut card.context.scan_messages).range(0..=32));
                });
                ui.horizontal(|ui| {
                    ui.label("本轮字符预算");
                    ui.add(egui::DragValue::new(&mut card.context.turn_budget).range(0..=16384));
                });
                if ui
                    .add_enabled(
                        card.world_book.len() < 64,
                        egui::Button::new("新增世界书条目"),
                    )
                    .clicked()
                {
                    card.world_book.push(WorldEntry {
                        name: "新条目".into(),
                        ..Default::default()
                    });
                }
                let default_depth = card.context.scan_messages;
                let mut remove = None;
                for (index, entry) in card.world_book.iter_mut().enumerate() {
                    ui.push_id(("world", index), |ui| {
                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut entry.enabled, "启用");
                            ui.checkbox(&mut entry.always_on, "始终启用");
                            if ui.small_button("删除").clicked() {
                                remove = Some(index);
                            }
                        });
                        ui.label("条目名称");
                        ui.text_edit_singleline(&mut entry.name);
                        ui.label("触发关键词");
                        ui.text_edit_singleline(&mut entry.keywords);
                        let mut custom_depth = entry.scan_depth.is_some();
                        if ui.checkbox(&mut custom_depth, "单独设置扫描条数").changed() {
                            entry.scan_depth = custom_depth.then_some(default_depth);
                        }
                        if let Some(depth) = &mut entry.scan_depth {
                            ui.add(egui::DragValue::new(depth).range(0..=32));
                        }
                        ui.horizontal_wrapped(|ui| {
                            ui.label("优先级");
                            ui.add(egui::DragValue::new(&mut entry.priority).range(-100..=100));
                            ui.label("持续轮数");
                            ui.add(egui::DragValue::new(&mut entry.sticky_turns).range(0..=32));
                            ui.label("冷却轮数");
                            ui.add(egui::DragValue::new(&mut entry.cooldown_turns).range(0..=32));
                        });
                        editor(ui, "背景内容", &mut entry.content);
                    });
                }
                if let Some(index) = remove {
                    card.world_book.remove(index);
                }
            }
            _ => {
                ui.horizontal(|ui| {
                    ui.label("静态字符预算");
                    ui.add(
                        egui::DragValue::new(&mut card.context.static_budget).range(256..=65536),
                    );
                });
                ui.small("启用的规则按列表顺序加载；规则不能增加软件权限。");
                if ui
                    .add_enabled(card.rules.len() < 64, egui::Button::new("新增规则"))
                    .clicked()
                {
                    card.rules.push(Rule {
                        name: "新规则".into(),
                        ..Default::default()
                    });
                }
                let mut remove = None;
                let mut movement = None;
                let count = card.rules.len();
                for (index, rule) in card.rules.iter_mut().enumerate() {
                    ui.push_id(("rule", index), |ui| {
                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut rule.enabled, "启用");
                            if ui
                                .add_enabled(index > 0, egui::Button::new("上移"))
                                .clicked()
                            {
                                movement = Some((index, index - 1));
                            }
                            if ui
                                .add_enabled(index + 1 < count, egui::Button::new("下移"))
                                .clicked()
                            {
                                movement = Some((index, index + 1));
                            }
                            if ui.small_button("删除").clicked() {
                                remove = Some(index);
                            }
                        });
                        ui.label("规则名称");
                        ui.text_edit_singleline(&mut rule.name);
                        editor(ui, "规则内容", &mut rule.content);
                    });
                }
                if let Some(index) = remove {
                    card.rules.remove(index);
                } else if let Some((a, b)) = movement {
                    card.rules.swap(a, b);
                }
            }
        }
        save.then(|| (self.role_editor.character.clone(), card.clone()))
    }

    pub(super) fn save_role_card(&mut self, character: &str, card: &RoleCard) {
        match roleplay::save_role_card(&self.root, character, card) {
            Ok(()) => {
                self.reload_roleplay();
                self.role_editor.error.clear();
                self.role_editor.notice = "角色设定已保存。".into();
            }
            Err(error) => self.role_editor.error = error,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{pos2, vec2};

    fn frame(app: &mut Pet, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
        let mut action = None;
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(pos2(0., 0.), vec2(500., 1400.))),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    action = app.role_card_ui(ui);
                });
            },
        );
        if let Some((character, card)) = action {
            app.save_role_card(&character, &card);
        }
        app.apply_profile_action();
        app.apply_vector_action();
        output
    }

    fn click(app: &mut Pet, ctx: &egui::Context, label: &str) {
        let output = frame(app, ctx, vec![]);
        let position = output
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::epaint::Shape::Text(text) = &shape.shape
                    && text.galley.job.text == label
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

    #[test]
    fn role_card_settings_edit_rules_save_cancel_and_defer_active_session_restart() {
        let root =
            std::env::temp_dir().join(format!("lexington-role-ui-{}", rand::random::<u64>()));
        let ctx = egui::Context::default();
        let mut app = Pet::from_root(&ctx, true, root.clone());
        app.library.character = "a".into();
        frame(&mut app, &ctx, vec![]);
        click(&mut app, &ctx, "附加规则");
        for _ in 0..2 {
            click(&mut app, &ctx, "新增规则");
        }
        let card = app.role_editor.draft.as_mut().unwrap();
        assert_eq!(card.rules.len(), 2);
        card.rules[0].content = "first".into();
        card.rules[1].content = "second".into();
        click(&mut app, &ctx, "下移");
        assert_eq!(
            app.role_editor.draft.as_ref().unwrap().rules[0].content,
            "second"
        );
        click(&mut app, &ctx, "启用");
        assert!(!app.role_editor.draft.as_ref().unwrap().rules[0].enabled);
        click(&mut app, &ctx, "删除");
        assert_eq!(app.role_editor.draft.as_ref().unwrap().rules.len(), 1);
        app.codex_character = Some("a".into());
        app.busy = true;
        click(&mut app, &ctx, "保存并应用");
        assert!(app.role_editor.error.is_empty());
        assert!(app.busy && app.codex_restart_pending);
        assert_eq!(
            RoleCard::load(&root, "a").unwrap().unwrap().rules[0].content,
            "first"
        );
        app.codex_role_card = RoleCard::load(&root, "a").unwrap();
        app.codex_restart_pending = false;
        app.save_role_card(
            "b",
            &RoleCard {
                name: "Other".into(),
                ..Default::default()
            },
        );
        assert!(
            !app.codex_restart_pending,
            "editing another card must keep this session"
        );
        let saved = std::fs::read(root.join("config/roleplay.json")).unwrap();
        std::fs::write(root.join("config/roleplay.json"), "broken").unwrap();
        app.reload_roleplay();
        assert!(!app.role_editor.error.is_empty() && !app.codex_restart_pending);
        assert!(app.codex_role_card.is_some());
        std::fs::write(root.join("config/roleplay.json"), saved).unwrap();
        app.role_editor.draft.as_mut().unwrap().memory = "unsaved memory".into();
        click(&mut app, &ctx, "取消修改");
        frame(&mut app, &ctx, vec![]);
        assert!(app.role_editor.draft.as_ref().unwrap().memory.is_empty());
        click(&mut app, &ctx, "世界书");
        click(&mut app, &ctx, "新增世界书条目");
        assert_eq!(app.role_editor.draft.as_ref().unwrap().world_book.len(), 1);
        app.settings = false;
        app.settings_ui(&ctx);
        assert!(app.role_editor.draft.is_none());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn role_memory_ui_candidates_require_explicit_confirmation_and_can_be_revoked() {
        let root =
            std::env::temp_dir().join(format!("lexington-memory-ui-{}", rand::random::<u64>()));
        let card = RoleCard {
            name: "a".into(),
            ..Default::default()
        };
        roleplay::save_role_card(&root, "a", &card).unwrap();
        let ctx = egui::Context::default();
        let mut app = Pet::from_root(&ctx, true, root.clone());
        app.library.character = "a".into();
        app.codex_character = Some("a".into());
        app.codex_role_card = Some(card);
        app.role_editor.section = 3;
        app.context_ui.last_input = "我喜欢温热的茶".into();
        frame(&mut app, &ctx, vec![]);
        click(&mut app, &ctx, "从最近输入创建候选");
        click(&mut app, &ctx, "保存用户与记忆");
        assert!(!app.codex_restart_pending);
        let profile = crate::roleplay::UserProfile::load(&root, "a", None).unwrap();
        assert_eq!(profile.memories.len(), 1);
        assert!(!profile.memories[0].confirmed);
        click(&mut app, &ctx, "确认记忆");
        click(&mut app, &ctx, "保存用户与记忆");
        assert!(app.codex_restart_pending);
        assert!(
            crate::roleplay::session_card(&root, "a")
                .unwrap()
                .unwrap()
                .instructions()
                .contains("温热的茶")
        );
        click(&mut app, &ctx, "撤销确认");
        click(&mut app, &ctx, "保存用户与记忆");
        assert!(
            !crate::roleplay::session_card(&root, "a")
                .unwrap()
                .unwrap()
                .instructions()
                .contains("温热的茶")
        );
        click(&mut app, &ctx, "删除记忆");
        click(&mut app, &ctx, "保存用户与记忆");
        assert!(
            crate::roleplay::UserProfile::load(&root, "a", None)
                .unwrap()
                .memories
                .is_empty()
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn vector_settings_validate_save_and_purge_only_the_selected_character() {
        let root =
            std::env::temp_dir().join(format!("lexington-vector-ui-{}", rand::random::<u64>()));
        let card = RoleCard {
            name: "a".into(),
            ..Default::default()
        };
        roleplay::save_role_card(&root, "a", &card).unwrap();
        let ctx = egui::Context::default();
        let mut app = Pet::from_root(&ctx, true, root.clone());
        app.library.character = "a".into();
        app.codex_character = Some("a".into());
        app.codex_role_card = Some(card);
        app.role_editor.section = 6;
        frame(&mut app, &ctx, vec![]);
        assert!(
            app.role_editor
                .draft
                .as_ref()
                .unwrap()
                .vector_memory
                .enabled
        );
        click(&mut app, &ctx, "远程接口");
        click(&mut app, &ctx, "保存并应用");
        assert!(!app.role_editor.error.is_empty());
        assert!(
            RoleCard::load(&root, "a")
                .unwrap()
                .unwrap()
                .vector_memory
                .source
                == crate::roleplay::EmbeddingSource::Local
        );
        app.role_editor.draft.as_mut().unwrap().vector_memory.model = "fixture".into();
        click(&mut app, &ctx, "保存并应用");
        assert!(app.role_editor.error.is_empty());
        assert!(app.codex_restart_pending);
        let saved = RoleCard::load(&root, "a").unwrap().unwrap();
        assert!(saved.vector_memory.enabled);
        click(&mut app, &ctx, "内置本地模型");
        click(&mut app, &ctx, "启用历史对话向量记忆");
        click(&mut app, &ctx, "保存并应用");
        let disabled = RoleCard::load(&root, "a").unwrap().unwrap();
        assert!(!disabled.vector_memory.enabled);
        assert!(disabled.vector_memory.source == crate::roleplay::EmbeddingSource::Local);
        assert!(!saved.instructions().contains("127.0.0.1"));
        let store = crate::tools::vector_store::VectorStore::open(
            &root.join("config/roleplay-vectors.sqlite3"),
        )
        .unwrap();
        for character in ["a", "b"] {
            store
                .put(
                    character,
                    &crate::tools::vector_store::Document {
                        id: "fixture".into(),
                        text: "history".into(),
                        created_at: 1,
                    },
                )
                .unwrap();
        }
        click(&mut app, &ctx, "清空此角色的历史记忆");
        assert_eq!(store.pending("a", "", 8).unwrap().len(), 1);
        click(&mut app, &ctx, "确认删除当前编辑角色的历史向量记忆");
        app.busy = true;
        click(&mut app, &ctx, "清空此角色的历史记忆");
        assert_eq!(store.pending("a", "", 8).unwrap().len(), 1);
        app.busy = false;
        click(&mut app, &ctx, "清空此角色的历史记忆");
        assert!(store.pending("a", "", 8).unwrap().is_empty());
        assert_eq!(store.pending("b", "", 8).unwrap().len(), 1);
        assert!(!app.role_editor.clear_confirmed);
        app.role_editor.clear_confirmed = true;
        click(&mut app, &ctx, "取消修改");
        assert!(!app.role_editor.clear_confirmed);
        drop(store);
        drop(app);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn context_inspector_previews_without_model_turns_and_sends_applied_scene() {
        let root =
            std::env::temp_dir().join(format!("lexington-context-ui-{}", rand::random::<u64>()));
        let mut card = RoleCard::default();
        card.world_book.push(WorldEntry {
            name: "tea".into(),
            keywords: "tea".into(),
            content: "tea background".into(),
            ..Default::default()
        });
        roleplay::save_role_card(&root, "a", &card).unwrap();
        let ctx = egui::Context::default();
        let mut app = Pet::from_root(&ctx, true, root.clone());
        app.library.character = "a".into();
        app.codex = Some(crate::backend::codex::Client::mock_session(
            "context",
            None,
            Some(crate::roleplay::RoleplaySession::new(card)),
        ));
        app.role_editor.section = 5;
        frame(&mut app, &ctx, vec![]);
        click(&mut app, &ctx, "临时场景（新会话清空）");
        app.context_ui.scene.style = "warm conversation".into();
        click(&mut app, &ctx, "应用临时场景");
        app.context_ui.preview_input = "world-tea".into();
        click(&mut app, &ctx, "预览上下文（不调用模型）");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while app.context_ui.preview.is_none() && std::time::Instant::now() < deadline {
            app.poll_codex();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let preview = app.context_ui.preview.as_ref().unwrap();
        assert!(preview.preview && preview.turn == 1);
        assert!(
            preview.text(false).contains("tea background")
                && preview.text(false).contains("warm conversation")
        );
        assert!(app.text.is_empty() && !app.busy);
        click(&mut app, &ctx, "预览结果");
        app.input = "world-tea".into();
        app.submit(&ctx);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while app.busy && std::time::Instant::now() < deadline {
            app.poll_codex();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(!app.busy);
        assert_eq!(app.context_ui.last.as_ref().unwrap().turn, 1);
        assert!(app.text.contains("warm conversation"));
        click(&mut app, &ctx, "清空临时场景");
        assert!(app.context_ui.scene.style.is_empty());
        drop(app);
        std::fs::remove_dir_all(root).unwrap();
    }
}
