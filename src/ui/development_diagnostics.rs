use super::app::Wife;
use eframe::egui;
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct DiagnosticsUi {
    pub search: String,
    pub errors_only: bool,
    pub stale: HashMap<String, bool>,
    pub refresh: bool,
    pub analyze: bool,
}
impl Wife {
    pub(super) fn development_diagnostics_ui(&mut self, ui: &mut egui::Ui) {
        let Some(project) = self.development.book.current() else {
            return;
        };
        ui.heading("项目诊断");
        ui.small("在 VS Code 中运行“发送项目诊断到开发”，或通过右键发送选区时一并同步。");
        let Some(snapshot) = &project.diagnostics else {
            ui.add_space(12.);
            ui.weak("尚未收到这个项目的诊断。");
            return;
        };
        if std::mem::take(&mut self.development.diagnostics_ui.refresh) {
            self.development.diagnostics_ui.stale = snapshot
                .documents
                .iter()
                .map(|doc| (doc.uri.clone(), doc.stale()))
                .collect();
        }
        ui.horizontal(|ui| {
            ui.label(format!(
                "{} 个问题 · 快照时间 {}",
                snapshot.count(),
                super::development::timestamp_text(snapshot.captured_at)
            ));
            if ui.button("检查来源").clicked() {
                self.development.diagnostics_ui.refresh = true;
            }
            if ui
                .add_enabled(
                    !self.busy && self.config.codex.development_mode && snapshot.count() > 0,
                    egui::Button::new("分析诊断"),
                )
                .clicked()
            {
                self.development.diagnostics_ui.analyze = true;
            }
        });
        if snapshot.truncated {
            ui.weak("来源问题较多，仅同步了大小限制内的部分问题。");
        }
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.development.diagnostics_ui.search)
                    .hint_text("搜索诊断内容或文件…")
                    .desired_width(280.),
            );
            ui.checkbox(&mut self.development.diagnostics_ui.errors_only, "只看错误");
        });
        ui.separator();
        for document in &snapshot.documents {
            let stale = self
                .development
                .diagnostics_ui
                .stale
                .get(&document.uri)
                .copied()
                .unwrap_or(false);
            let title = document
                .path
                .as_ref()
                .map(|path| {
                    path.strip_prefix(&project.path)
                        .unwrap_or(path)
                        .display()
                        .to_string()
                })
                .unwrap_or_else(|| document.uri.clone());
            let issues: Vec<_> = document
                .diagnostics
                .iter()
                .filter(|issue| {
                    (!self.development.diagnostics_ui.errors_only || issue.severity == "error")
                        && (self.development.diagnostics_ui.search.is_empty()
                            || issue
                                .message
                                .contains(&self.development.diagnostics_ui.search)
                            || title.contains(&self.development.diagnostics_ui.search))
                })
                .collect();
            if issues.is_empty() {
                continue;
            }
            ui.push_id(&document.uri, |ui| {
                ui.strong(&title);
                if stale {
                    ui.colored_label(
                        egui::Color32::DARK_RED,
                        "磁盘文件已变化，旧诊断不再自动载入。",
                    );
                }
                if document.dirty {
                    ui.small(format!(
                        "来自未保存的编辑器版本 {}",
                        document.version.map(|v| v.to_string()).unwrap_or_default()
                    ));
                }
                for issue in issues {
                    let color = if issue.severity == "error" {
                        egui::Color32::from_rgb(180, 55, 55)
                    } else {
                        egui::Color32::from_gray(90)
                    };
                    ui.colored_label(
                        color,
                        format!(
                            "{}:{} · {} {}",
                            issue.start_line, issue.start_column, issue.source, issue.code
                        ),
                    );
                    ui.add(egui::Label::new(&issue.message).selectable(true).wrap());
                    if let Some(path) = &document.path
                        && path.is_file()
                        && let Some(path) = path.to_str()
                    {
                        let mut url = url::Url::parse("vscode://file").unwrap();
                        url.set_path(&format!(
                            "{path}:{}:{}",
                            issue.start_line, issue.start_column
                        ));
                        ui.hyperlink_to("打开对应位置", url.as_str());
                    }
                    ui.add_space(6.);
                }
                ui.separator();
            });
        }
    }
}
