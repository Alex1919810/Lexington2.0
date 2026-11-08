use super::app::Pet;
use crate::{
    roleplay::{self, RoleCard},
    tools::file_dialog,
};
use eframe::egui;
use std::{path::PathBuf, sync::mpsc, time::Duration};

#[derive(Default)]
pub(super) struct NewCharacter {
    card: RoleCard,
    artworks: Vec<PathBuf>,
    picker: Option<mpsc::Receiver<Option<Vec<PathBuf>>>>,
    pick_requested: bool,
    create_requested: bool,
}

#[derive(Default)]
pub(super) struct CharacterCreation {
    worker: Option<mpsc::Receiver<Result<String, String>>>,
}
impl CharacterCreation {
    pub fn busy(&self) -> bool {
        self.worker.is_some()
    }
}

impl Pet {
    pub(super) fn new_character_ui(&mut self, ui: &mut egui::Ui) {
        let busy = self.character_creation.busy();
        let Some(draft) = &mut self.role_editor.creating else {
            return;
        };
        let mut cancel = false;
        ui.add_enabled_ui(!busy, |ui| {
            ui.label("角色名称");
            ui.text_edit_singleline(&mut draft.card.name);
            ui.label("身份与描述");
            ui.add(
                egui::TextEdit::multiline(&mut draft.card.description)
                    .desired_width(f32::INFINITY)
                    .desired_rows(3),
            );
            if ui
                .add_enabled(
                    draft.picker.is_none(),
                    egui::Button::new("导入立绘（PNG / GIF）"),
                )
                .clicked()
            {
                draft.pick_requested = true;
            }
            let dropped = ui.ctx().input(|i| {
                i.raw
                    .dropped_files
                    .iter()
                    .filter_map(|file| file.path.clone())
                    .collect::<Vec<_>>()
            });
            for path in dropped {
                if !draft.artworks.contains(&path) {
                    draft.artworks.push(path);
                }
            }
            let mut remove = None;
            for (index, path) in draft.artworks.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(path.file_name().unwrap_or_default().to_string_lossy());
                    if ui.small_button("移除").clicked() {
                        remove = Some(index);
                    }
                });
            }
            if let Some(index) = remove {
                draft.artworks.remove(index);
            }
            ui.label(format!(
                "立绘（至少一张）：已选 {} 张",
                draft.artworks.len()
            ));
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        !draft.card.name.trim().is_empty()
                            && !draft.artworks.is_empty()
                            && draft.picker.is_none(),
                        egui::Button::new("创建角色卡"),
                    )
                    .clicked()
                {
                    draft.create_requested = true;
                }
                cancel = ui.button("取消新建").clicked();
            });
        });
        if busy {
            ui.label("正在创建…");
        }
        if !self.role_editor.error.is_empty() {
            ui.colored_label(egui::Color32::DARK_RED, &self.role_editor.error);
        }
        if cancel {
            self.role_editor.creating = None;
            self.role_editor.error.clear();
        }
    }

    pub(super) fn apply_new_character_action(&mut self, ctx: &egui::Context) {
        let Some(draft) = &mut self.role_editor.creating else {
            return;
        };
        if std::mem::take(&mut draft.pick_requested) && draft.picker.is_none() {
            let (tx, rx) = mpsc::channel();
            let wake = ctx.clone();
            std::thread::spawn(move || {
                let _ = tx.send(file_dialog::pick_illustrations());
                wake.request_repaint();
            });
            draft.picker = Some(rx);
        }
        if let Some(picker) = &draft.picker {
            match picker.try_recv() {
                Ok(selection) => {
                    for path in selection.unwrap_or_default() {
                        if !draft.artworks.contains(&path) {
                            draft.artworks.push(path);
                        }
                    }
                    draft.picker = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    draft.picker = None;
                    self.role_editor.error = "无法打开立绘选择器，可拖入 PNG / GIF 文件。".into();
                }
                Err(mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(Duration::from_millis(100));
                }
            }
        }
        if std::mem::take(&mut draft.create_requested) && !self.character_creation.busy() {
            let root = self.root.clone();
            let card = draft.card.clone();
            let artworks = draft.artworks.clone();
            let (tx, rx) = mpsc::channel();
            let wake = ctx.clone();
            self.role_editor.error.clear();
            std::thread::spawn(move || {
                let _ = tx.send(roleplay::create_character(&root, &card, &artworks));
                wake.request_repaint();
            });
            self.character_creation.worker = Some(rx);
        }
    }

    pub(super) fn poll_character_creation(&mut self, ctx: &egui::Context) {
        let Some(worker) = &self.character_creation.worker else {
            return;
        };
        let result = match worker.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(Duration::from_millis(100));
                return;
            }
            Err(mpsc::TryRecvError::Disconnected) => Err("角色创建未完成，请重试。".into()),
        };
        self.character_creation.worker = None;
        match result {
            Ok(character) => {
                let previous = self.library.selected_path().map(PathBuf::from);
                self.library.refresh(&self.root);
                if previous.as_deref() != self.library.selected_path() {
                    self.load_picture(ctx);
                }
                if self.settings {
                    self.role_editor = super::role_card::RoleEditor {
                        character,
                        notice: "角色卡已创建。".into(),
                        ..Default::default()
                    };
                }
            }
            Err(error) => self.role_editor.error = error,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn frame(app: &mut Pet, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(600., 1200.),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    app.role_card_ui(ui);
                });
            },
        )
    }
    fn click(app: &mut Pet, ctx: &egui::Context, label: &str) {
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
            .expect("creation control");
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
    fn new_card_imports_artwork_becomes_selectable_and_survives_closing_settings() {
        let root = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        let mut app = Pet::from_root(&ctx, true, root.path().into());
        let path = root.path().join("立绘.png");
        image::RgbaImage::from_pixel(4, 4, image::Rgba([1, 2, 3, 255]))
            .save(&path)
            .unwrap();
        app.settings = true;
        click(&mut app, &ctx, "新建角色卡");
        let draft = app.role_editor.creating.as_mut().unwrap();
        draft.card.name = "新角色".into();
        draft.card.description = "新的人格".into();
        click(&mut app, &ctx, "创建角色卡");
        assert!(!app.role_editor.creating.as_ref().unwrap().create_requested);
        assert!(!root.path().join("config/characters").exists());
        let (tx, rx) = mpsc::channel();
        tx.send(Some(vec![path])).unwrap();
        app.role_editor.creating.as_mut().unwrap().picker = Some(rx);
        app.apply_new_character_action(&ctx);
        click(&mut app, &ctx, "创建角色卡");
        app.apply_new_character_action(&ctx);
        // A requested creation completes even if settings clears its draft meanwhile.
        app.settings = false;
        app.settings_ui(&ctx);
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.character_creation.busy() && Instant::now() < deadline {
            app.poll_character_creation(&ctx);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!app.character_creation.busy());
        assert_eq!(app.library.characters(), vec!["新角色"]);
        assert!(app.picture.is_some());
        assert!(app.codex.is_none());
        assert!(
            roleplay::agent_instructions(root.path(), "新角色")
                .unwrap()
                .unwrap()
                .contains("新的人格")
        );
    }
}
