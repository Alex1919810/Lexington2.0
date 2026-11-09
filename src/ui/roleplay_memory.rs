use super::app::Wife;
use crate::roleplay::{Memory, UserProfile};
use eframe::egui;

impl Wife {
    pub(super) fn role_memory_ui(&mut self, ui: &mut egui::Ui) {
        if self.role_editor.profile.is_none() {
            match UserProfile::load(
                &self.root,
                &self.role_editor.character,
                self.role_editor.draft.as_ref(),
            ) {
                Ok(profile) => self.role_editor.profile = Some(profile),
                Err(error) => {
                    ui.colored_label(egui::Color32::DARK_RED, error);
                    return;
                }
            }
        }
        ui.small("用户人设与记忆独立保存。候选不会发送给模型；确认后也需点击保存才生效。");
        let profile = self.role_editor.profile.as_mut().unwrap();
        ui.label("用户人设（称呼、身份、偏好）");
        ui.add(
            egui::TextEdit::multiline(&mut profile.persona)
                .desired_width(f32::INFINITY)
                .desired_rows(3),
        );
        if ui.button("保存用户与记忆").clicked() {
            self.role_editor.pending_profile =
                Some((self.role_editor.character.clone(), profile.clone()));
        }
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    profile.memories.len() < 64,
                    egui::Button::new("新增记忆候选"),
                )
                .clicked()
            {
                profile.memories.push(Memory::default());
            }
            if ui
                .add_enabled(
                    profile.memories.len() < 64
                        && !self.context_ui.last_input.is_empty()
                        && self.role_editor.character == self.library.character,
                    egui::Button::new("从最近输入创建候选"),
                )
                .clicked()
            {
                profile.memories.push(Memory {
                    text: self.context_ui.last_input.chars().take(2000).collect(),
                    source: "本次对话的用户输入（请核对并提炼）".into(),
                    ..Default::default()
                });
            }
        });
        let mut remove = None;
        for (index, memory) in profile.memories.iter_mut().enumerate() {
            ui.push_id(&memory.id, |ui| {
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    ui.checkbox(&mut memory.enabled, "启用");
                    if ui
                        .button(if memory.confirmed {
                            "撤销确认"
                        } else {
                            "确认记忆"
                        })
                        .clicked()
                    {
                        memory.confirmed = !memory.confirmed;
                    }
                    if ui.small_button("删除记忆").clicked() {
                        remove = Some(index);
                    }
                });
                ui.label(if memory.confirmed {
                    "已确认：保存后可加载"
                } else {
                    "候选：不加载给模型"
                });
                if ui
                    .add(
                        egui::TextEdit::multiline(&mut memory.text)
                            .desired_width(f32::INFINITY)
                            .desired_rows(3),
                    )
                    .changed()
                {
                    memory.confirmed = false;
                }
                ui.label("记录来源（仅供核对，不发送给模型）");
                ui.text_edit_singleline(&mut memory.source);
            });
        }
        if let Some(index) = remove {
            profile.memories.remove(index);
        }
    }

    pub(super) fn apply_profile_action(&mut self) {
        if let Some((character, profile)) = self.role_editor.pending_profile.take() {
            match profile.save(&self.root, &character) {
                Ok(()) => {
                    self.reload_roleplay();
                    self.role_editor.notice =
                        "用户人设与记忆已保存；当前角色的有效内容改变时会新建会话。".into();
                }
                Err(error) => self.role_editor.error = error,
            }
        }
    }
}
