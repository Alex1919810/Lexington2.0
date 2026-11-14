use super::app::Wife;
use crate::backend::codex_config::{LocalCodexConfig, ProviderKind};
use eframe::egui;
use std::path::PathBuf;

#[derive(Default)]
pub(super) struct ConnectionEditor {
    home: Option<PathBuf>,
    current: Option<LocalCodexConfig>,
    draft: Option<LocalCodexConfig>,
    inactive_draft: Option<LocalCodexConfig>,
    loaded: bool,
    show_key: bool,
    error: String,
    notice: String,
}

pub(super) enum ConnectionAction {
    Reload,
    Save,
}

impl Wife {
    fn load_connection(&mut self) {
        let result = (|| {
            let home = if cfg!(test) {
                self.root.join("fixture-codex")
            } else {
                LocalCodexConfig::home()?
            };
            let config = LocalCodexConfig::read(&home)?;
            Ok::<_, String>((home, config))
        })();
        self.connection_editor = ConnectionEditor {
            loaded: true,
            ..Default::default()
        };
        match result {
            Ok((home, config)) => {
                self.connection_editor.home = Some(home);
                self.connection_editor.draft = Some(config.clone());
                self.connection_editor.current = Some(config);
            }
            Err(error) => self.connection_editor.error = error,
        }
    }

    pub(super) fn connection_settings_ui(&mut self, ui: &mut egui::Ui) -> Option<ConnectionAction> {
        if !self.connection_editor.loaded {
            self.load_connection();
        }
        let editor = &mut self.connection_editor;
        ui.label("始终继承当前环境与本机 Codex 配置。");
        ui.small("下方修改直接保存到本机 Codex，也会影响其他使用此配置的 Codex 客户端。");
        if let Some(current) = &editor.current {
            ui.strong(format!("当前继承：{}", current.summary()));
            ui.small(format!("提供方：{}", current.provider_id));
        }
        if let Some(home) = &editor.home {
            ui.add(egui::Label::new("本机 Codex 配置文件").sense(egui::Sense::hover()))
                .on_hover_text(home.join("config.toml").display().to_string());
        }
        let mut action = None;
        if let Some(draft) = &mut editor.draft {
            ui.add_space(6.);
            let mut selected = draft.kind;
            ui.horizontal(|ui| {
                ui.selectable_value(&mut selected, ProviderKind::Openai, "OpenAI 官方");
                ui.selectable_value(&mut selected, ProviderKind::Custom, "第三方 API");
            });
            if selected != draft.kind {
                let replacement = editor.inactive_draft.take().unwrap_or_else(|| {
                    let mut replacement = draft.clone();
                    replacement.kind = selected;
                    replacement.api_key.clear();
                    replacement.base_url.clear();
                    replacement.model.clear();
                    replacement.use_api_key = selected == ProviderKind::Custom;
                    replacement
                });
                editor.inactive_draft = Some(std::mem::replace(draft, replacement));
                editor.show_key = false;
            }
            if draft.kind == ProviderKind::Openai {
                ui.horizontal(|ui| {
                    ui.radio_value(&mut draft.use_api_key, false, "本机登录");
                    ui.radio_value(&mut draft.use_api_key, true, "API Key");
                });
                if !draft.use_api_key {
                    ui.small("沿用本机官方登录；未登录时需先使用 Codex 登录。不会覆盖或删除已有登录信息。");
                }
            } else {
                ui.label("API 基础地址");
                ui.add(
                    egui::TextEdit::singleline(&mut draft.base_url)
                        .hint_text("https://example.com/v1")
                        .desired_width(f32::INFINITY),
                );
                ui.small(
                    "需兼容 OpenAI Responses API；仅兼容 Chat Completions 的接口不能直接使用。",
                );
            }
            ui.label("模型名称");
            ui.add(
                egui::TextEdit::singleline(&mut draft.model)
                    .hint_text(if draft.kind == ProviderKind::Openai {
                        "留空使用 Codex 默认模型"
                    } else {
                        "填写服务商提供的模型 ID"
                    })
                    .desired_width(f32::INFINITY),
            );
            if draft.kind == ProviderKind::Custom || draft.use_api_key {
                ui.label("API Key");
                ui.add(
                    egui::TextEdit::singleline(&mut draft.api_key)
                        .password(!editor.show_key)
                        .desired_width(f32::INFINITY),
                );
                ui.checkbox(&mut editor.show_key, "显示 Key");
                ui.small("Key 将明文存入本机 Codex 配置，不写入 Lexington 的 config.json。第三方无鉴权服务可留空。");
            }
            ui.horizontal(|ui| {
                if ui.button("保存到 Codex 并应用").clicked() {
                    action = Some(ConnectionAction::Save);
                }
                if ui.button("重新读取 Codex 配置").clicked() {
                    action = Some(ConnectionAction::Reload);
                }
            });
        } else if ui.button("重新读取 Codex 配置").clicked() {
            action = Some(ConnectionAction::Reload);
        }
        if !editor.notice.is_empty() {
            ui.weak(&editor.notice);
        }
        if !editor.error.is_empty() {
            ui.colored_label(egui::Color32::from_rgb(180, 45, 45), &editor.error);
        }
        action
    }

    pub(super) fn apply_connection_action(&mut self, action: ConnectionAction) {
        match action {
            ConnectionAction::Reload => self.load_connection(),
            ConnectionAction::Save => {
                let (Some(home), Some(draft)) =
                    (&self.connection_editor.home, &self.connection_editor.draft)
                else {
                    return;
                };
                match draft.save(home) {
                    Ok(()) => {
                        self.config.codex.connection_revision =
                            self.config.codex.connection_revision.wrapping_add(1);
                        self.codex_restart_pending = true;
                        self.development.reset_sessions_pending = true;
                        self.load_connection();
                        self.connection_editor.notice = if self.busy {
                            "已保存到本机 Codex，当前任务结束后新建会话。"
                        } else {
                            "已保存到本机 Codex，将新建会话应用配置。"
                        }
                        .into();
                    }
                    Err(error) => {
                        self.connection_editor.error = error;
                        self.connection_editor.notice.clear();
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{pos2, vec2};

    fn frame(app: &mut Wife, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
        let mut action = None;
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(pos2(0., 0.), vec2(720., 960.))),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    action = app.connection_settings_ui(ui);
                });
            },
        );
        if let Some(action) = action {
            app.apply_connection_action(action);
        }
        output
    }

    fn click(app: &mut Wife, ctx: &egui::Context, label: &str) {
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
    fn connection_settings_show_inherited_source_mask_key_and_discard_unsaved_edits() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("fixture-codex");
        std::fs::create_dir_all(&home).unwrap();
        let path = home.join("config.toml");
        let source = "model_provider='fixture'\nmodel='fixture-model'\n[model_providers.fixture]\nname='Fixture'\nbase_url='https://fixture.invalid/v1'\nexperimental_bearer_token='fixture-ui-secret'\n";
        std::fs::write(&path, source).unwrap();
        let ctx = egui::Context::default();
        let mut app = Wife::from_root(&ctx, true, root.path().into());
        let output = frame(&mut app, &ctx, vec![]);
        let text = output
            .shapes
            .iter()
            .filter_map(|shape| {
                if let egui::epaint::Shape::Text(text) = &shape.shape {
                    Some(text.galley.job.text.as_str())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("当前继承：第三方 API"));
        assert!(!text.contains("fixture-ui-secret"));
        click(&mut app, &ctx, "OpenAI 官方");
        let draft = app.connection_editor.draft.as_ref().unwrap();
        assert_eq!(draft.kind, ProviderKind::Openai);
        assert!(draft.api_key.is_empty() && draft.model.is_empty());
        click(&mut app, &ctx, "第三方 API");
        let draft = app.connection_editor.draft.as_ref().unwrap();
        assert_eq!(draft.kind, ProviderKind::Custom);
        assert_eq!(draft.base_url, "https://fixture.invalid/v1");
        assert_eq!(draft.model, "fixture-model");
        assert_eq!(draft.api_key, "fixture-ui-secret");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
        assert!(!app.codex_restart_pending);
        app.settings = false;
        app.settings_ui(&ctx);
        assert!(app.connection_editor.draft.is_none());
        assert!(app.connection_editor.inactive_draft.is_none());
        frame(&mut app, &ctx, vec![]);
        assert_eq!(
            app.connection_editor.draft.as_ref().unwrap().kind,
            ProviderKind::Custom
        );
    }

    #[test]
    fn connection_settings_keep_official_and_custom_drafts_separate_when_switching() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("fixture-codex");
        std::fs::create_dir_all(&home).unwrap();
        let path = home.join("config.toml");
        let source = "model='fixture-official-model'\n[model_providers.openai]\nexperimental_bearer_token='fixture-official-key'\n";
        std::fs::write(&path, source).unwrap();
        let ctx = egui::Context::default();
        let mut app = Wife::from_root(&ctx, true, root.path().into());
        frame(&mut app, &ctx, vec![]);
        app.connection_editor.show_key = true;
        click(&mut app, &ctx, "第三方 API");
        assert!(!app.connection_editor.show_key);
        let draft = app.connection_editor.draft.as_mut().unwrap();
        assert!(draft.api_key.is_empty() && draft.model.is_empty() && draft.base_url.is_empty());
        draft.api_key = "fixture-custom-key".into();
        draft.model = "fixture-custom-model".into();
        draft.base_url = "https://fixture.invalid/v1".into();
        app.connection_editor.show_key = true;
        click(&mut app, &ctx, "OpenAI 官方");
        assert!(!app.connection_editor.show_key);
        let draft = app.connection_editor.draft.as_mut().unwrap();
        assert_eq!(draft.api_key, "fixture-official-key");
        assert_eq!(draft.model, "fixture-official-model");
        assert!(draft.base_url.is_empty() && draft.use_api_key);
        draft.api_key = "fixture-edited-official-key".into();
        draft.model = "fixture-edited-official-model".into();
        click(&mut app, &ctx, "本机登录");
        click(&mut app, &ctx, "第三方 API");
        let draft = app.connection_editor.draft.as_ref().unwrap();
        assert_eq!(draft.api_key, "fixture-custom-key");
        assert_eq!(draft.model, "fixture-custom-model");
        assert_eq!(draft.base_url, "https://fixture.invalid/v1");
        assert!(draft.use_api_key);
        click(&mut app, &ctx, "OpenAI 官方");
        let draft = app.connection_editor.draft.as_ref().unwrap();
        assert_eq!(draft.api_key, "fixture-edited-official-key");
        assert_eq!(draft.model, "fixture-edited-official-model");
        assert!(!draft.use_api_key);
        assert_eq!(
            app.connection_editor.current.as_ref().unwrap().summary(),
            "OpenAI 官方 API"
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
        assert!(!app.codex_restart_pending && !app.development.reset_sessions_pending);
        assert_eq!(app.config.codex.connection_revision, 0);
        click(&mut app, &ctx, "重新读取 Codex 配置");
        let draft = app.connection_editor.draft.as_ref().unwrap();
        assert_eq!(draft.api_key, "fixture-official-key");
        assert_eq!(draft.model, "fixture-official-model");
        assert!(draft.use_api_key);
        click(&mut app, &ctx, "第三方 API");
        let draft = app.connection_editor.draft.as_ref().unwrap();
        assert!(draft.api_key.is_empty() && draft.model.is_empty() && draft.base_url.is_empty());
    }

    #[test]
    fn connection_settings_save_native_config_and_defer_new_sessions_until_idle() {
        let root = tempfile::tempdir().unwrap();
        crate::backend::config::Config::save_no_approval(root.path(), true).unwrap();
        let ctx = egui::Context::default();
        let mut app = Wife::from_root(&ctx, true, root.path().into());
        let previous = app.config.codex.clone();
        frame(&mut app, &ctx, vec![]);
        click(&mut app, &ctx, "第三方 API");
        let draft = app.connection_editor.draft.as_mut().unwrap();
        draft.base_url = "https://fixture.invalid/v1".into();
        draft.model = "fixture-model".into();
        draft.api_key = "fixture-ui-secret".into();
        app.busy = true;
        click(&mut app, &ctx, "保存到 Codex 并应用");
        assert!(app.busy && app.codex_restart_pending);
        assert!(app.development.reset_sessions_pending);
        assert!(app.config.codex.connection_changed(&previous));
        assert!(app.connection_editor.notice.contains("当前任务结束后"));
        assert!(app.connection_editor.error.is_empty());
        let current = LocalCodexConfig::read(&root.path().join("fixture-codex")).unwrap();
        assert_eq!(current.summary(), "第三方 API");
        assert_eq!(current.api_key, "fixture-ui-secret");
        let application_config =
            std::fs::read_to_string(root.path().join("config/config.json")).unwrap();
        assert!(!application_config.contains("fixture-ui-secret"));
        app.reload_config();
        assert_eq!(app.config.codex.connection_revision, 1);
        assert!(app.config.codex.no_approval);
    }

    #[test]
    fn connection_settings_switch_official_auth_and_reject_conflicting_saves() {
        let root = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        let mut app = Wife::from_root(&ctx, true, root.path().into());
        frame(&mut app, &ctx, vec![]);
        click(&mut app, &ctx, "API Key");
        let draft = app.connection_editor.draft.as_mut().unwrap();
        draft.api_key = "fixture-official-key".into();
        click(&mut app, &ctx, "保存到 Codex 并应用");
        assert_eq!(
            app.connection_editor.current.as_ref().unwrap().summary(),
            "OpenAI 官方 API"
        );
        app.codex_restart_pending = false;
        let home = root.path().join("fixture-codex");
        let path = home.join("config.toml");
        let source = std::fs::read_to_string(&path).unwrap() + "\n# changed elsewhere\n";
        std::fs::write(&path, &source).unwrap();
        click(&mut app, &ctx, "本机登录");
        click(&mut app, &ctx, "保存到 Codex 并应用");
        assert!(app.connection_editor.error.contains("重新读取"));
        assert!(!app.codex_restart_pending);
        assert_eq!(app.config.codex.connection_revision, 1);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
        click(&mut app, &ctx, "重新读取 Codex 配置");
        click(&mut app, &ctx, "本机登录");
        click(&mut app, &ctx, "保存到 Codex 并应用");
        assert_eq!(
            app.connection_editor.current.as_ref().unwrap().summary(),
            "OpenAI 官方（本机登录）"
        );
        assert!(app.connection_editor.error.is_empty());
        assert_eq!(app.config.codex.connection_revision, 2);
        assert!(!home.join("auth.json").exists());
    }
}
