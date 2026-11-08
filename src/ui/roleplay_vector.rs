use super::app::Pet;
use eframe::egui;

impl Pet {
    pub(super) fn role_vector_ui(&mut self, ui: &mut egui::Ui) {
        let Some(card) = &mut self.role_editor.draft else {
            return;
        };
        let settings = &mut card.vector_memory;
        ui.checkbox(&mut settings.enabled, "启用历史对话向量记忆");
        ui.small("启用后保存成功完成的对话，并发送当前输入与历史片段到下方 Embedding 服务生成向量。固定记忆仍需手动确认。");
        ui.label("Embedding 基础地址（兼容 /embeddings 接口）");
        ui.text_edit_singleline(&mut settings.base_url);
        ui.label("Embedding 模型名称");
        ui.text_edit_singleline(&mut settings.model);
        ui.label("API Key 环境变量名称（本地无鉴权可留空）");
        ui.text_edit_singleline(&mut settings.api_key_env);
        ui.small("此处只填写变量名；密钥不写入角色卡。修改环境变量后需重启软件。");
        ui.horizontal(|ui| {
            ui.label("最多召回片段数");
            ui.add(egui::DragValue::new(&mut settings.recall_limit).range(1..=8));
        });
        ui.horizontal(|ui| {
            ui.label("最大余弦距离（越小越严格）");
            ui.add(
                egui::DragValue::new(&mut settings.max_distance)
                    .range(0.0..=2.0)
                    .speed(0.01),
            );
        });
        ui.small("按角色隔离，每个角色最多保留 2000 个片段；每次最多为 8 个历史片段补索引。更换模型或地址后逐批重建，失败时继续普通对话。实际注入仍受本轮上下文预算限制。");
        ui.small(
            "数据库保存在 config/roleplay-vectors.sqlite3。停用会停止保存与检索，并保留已有数据。",
        );
        ui.separator();
        ui.checkbox(
            &mut self.role_editor.clear_confirmed,
            "确认删除当前编辑角色的历史向量记忆",
        );
        if ui
            .add_enabled(
                self.role_editor.clear_confirmed && !self.busy,
                egui::Button::new("清空此角色的历史记忆"),
            )
            .clicked()
        {
            self.role_editor.pending_clear = Some(self.role_editor.character.clone());
            self.role_editor.clear_confirmed = false;
        }
    }

    pub(super) fn apply_vector_action(&mut self) {
        if let Some(character) = self.role_editor.pending_clear.take() {
            match crate::roleplay::clear_vector_memory(&self.root, &character) {
                Ok(()) => {
                    if self.codex_character.as_deref() == Some(&character) {
                        self.codex_restart_pending = true;
                    }
                    self.role_editor.notice =
                        "此角色的历史片段与向量已清空；固定记忆保留，当前角色会新建会话。".into();
                }
                Err(error) => self.role_editor.error = error,
            }
        }
    }
}
