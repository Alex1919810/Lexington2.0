use super::app::Wife;
use super::style::{self, draw_bubble};
use eframe::egui::{self, Color32, Rect, pos2, vec2};
use std::time::Instant;

impl Wife {
    pub(super) fn say(&mut self, text: String) {
        self.text = text;
        self.typed_at = Instant::now();
        self.last_activity = Instant::now();
        self.full_text = false;
        self.dialog_clicks = 0;
    }
    pub(super) fn click_pet(&mut self) {
        if self.busy && self.development_turn {
            return;
        }
        if self.turn_ui_hidden {
            self.turn_ui_hidden = false;
            self.text = self
                .codex_messages
                .iter()
                .map(|(_, text)| text.as_str())
                .collect::<Vec<_>>()
                .join("\n\n");
            self.full_text = true;
            self.dialog_clicks = 0;
            self.last_activity = Instant::now();
            return;
        }
        if self.busy {
            self.dialog_clicks += 1;
            if self.dialog_clicks == 2 {
                self.turn_ui_hidden = true;
                self.text.clear();
                self.dialog_clicks = 0;
            }
            return;
        }
        if self.text.is_empty() {
            // Preset lines are fallback content, including after an AI bubble was dismissed.
            if self.busy
                || self
                    .codex_messages
                    .iter()
                    .any(|(_, text)| !text.trim().is_empty())
            {
                return;
            }
            if !self.library.texts.is_empty() {
                self.say(
                    self.library.texts[rand::random_range(0..self.library.texts.len())].clone(),
                );
            }
        } else if self.dialog_clicks == 0 {
            self.full_text = true;
            self.dialog_clicks = 1;
        } else {
            self.text.clear();
            self.dialog_clicks = 0;
        }
    }
    pub(super) fn bubble(&mut self, ctx: &egui::Context, center: egui::Pos2) -> Option<Rect> {
        let waiting = self.text.is_empty()
            && self.busy
            && !self.development_turn
            && !self.turn_ui_hidden
            && self.development.tests_ui.running.is_none();
        if self.text.is_empty() && !waiting {
            return None;
        }
        let text = if waiting {
            "Lexington正在思考"
        } else {
            &self.text
        };
        let width = self.config.dialog.width;
        let position = pos2(
            center.x + self.config.dialog.offset_x - width,
            center.y + self.config.dialog.offset_y,
        );
        let shown = if self.full_text || waiting {
            text.to_owned()
        } else {
            text.chars()
                .take((self.typed_at.elapsed().as_secs_f64() / 0.05) as usize + 1)
                .collect()
        };
        let result = egui::Area::new("dialog".into())
            .constrain(false)
            .order(egui::Order::Foreground)
            .fixed_pos(position)
            .show(ctx, |ui| {
                let galley = egui::WidgetText::from(
                    egui::RichText::new(text)
                        .font(style::font(14.))
                        .line_height(Some(21.)),
                )
                .into_galley(
                    ui,
                    Some(egui::TextWrapMode::Wrap),
                    width - 28.,
                    style::font(14.),
                );
                let height = (galley.size().y + 20.).min(self.config.dialog.max_height);
                let rect = Rect::from_min_size(ui.min_rect().min, vec2(width, height));
                draw_bubble(ui, rect, &mut self.dialog_bg, false);
                ui.set_min_size(rect.size());
                ui.scope_builder(
                    egui::UiBuilder::new().max_rect(rect.shrink2(vec2(14., 10.))),
                    |ui| {
                        egui::ScrollArea::vertical()
                            .max_height((height - 20.).max(1.))
                            .show(ui, |ui| {
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(shown)
                                            .font(style::font(14.))
                                            .line_height(Some(21.))
                                            .color(Color32::from_gray(51)),
                                    )
                                    .wrap(),
                                );
                            });
                    },
                );
                rect
            });
        self.input_regions.push(
            ctx.layer_transform_to_global(result.response.layer_id)
                .unwrap_or_default()
                * result.inner,
        );
        Some(result.inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::codex::Client;
    use std::time::Duration;

    #[test]
    fn preset_lines_do_not_replace_pending_or_dismissed_ai_output() {
        let root = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        let mut app = Wife::from_root(&ctx, true, root.path().into());
        app.library.texts = vec!["预设台词夹具".into()];
        app.click_pet();
        assert_eq!(app.text, "预设台词夹具");
        app.click_pet();
        app.click_pet();
        assert!(app.text.is_empty());
        app.codex = Some(Client::mock("normal"));
        app.input = "AI 回复夹具".into();
        app.submit(&ctx);
        assert!(app.busy);
        app.click_pet();
        assert!(app.text.is_empty());
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.busy && Instant::now() < deadline {
            app.poll_codex();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!app.busy);
        assert!(app.text.contains("AI 回复夹具"));
        let reply = app.text.clone();
        app.click_pet();
        assert_eq!(app.text, reply);
        app.click_pet();
        assert!(app.text.is_empty());
        for _ in 0..3 {
            app.click_pet();
            assert!(app.text.is_empty());
        }
        // A failed next turn with no AI text still permits the ordinary fallback.
        app.input = "fail".into();
        app.submit(&ctx);
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.busy && Instant::now() < deadline {
            app.poll_codex();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!app.busy);
        assert!(app.error.contains("模拟请求失败"));
        app.click_pet();
        assert_eq!(app.text, "预设台词夹具");
    }
}
