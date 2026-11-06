use super::app::Pet;
use super::style::{self, draw_bubble};
use eframe::egui::{self, Color32, Rect, pos2, vec2};
use std::time::Instant;

impl Pet {
    pub(super) fn say(&mut self, text: String) {
        self.text = text;
        self.typed_at = Instant::now();
        self.last_activity = Instant::now();
        self.full_text = false;
        self.dialog_clicks = 0;
    }
    pub(super) fn click_pet(&mut self) {
        if self.text.is_empty() {
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
        if self.text.is_empty() {
            return None;
        }
        let width = self.config.dialog.width;
        let position = pos2(
            center.x + self.config.dialog.offset_x - width,
            center.y + self.config.dialog.offset_y,
        );
        let shown = if self.full_text {
            self.text.clone()
        } else {
            self.text
                .chars()
                .take((self.typed_at.elapsed().as_secs_f64() / 0.05) as usize + 1)
                .collect()
        };
        let result = egui::Area::new("dialog".into())
            .constrain(false)
            .order(egui::Order::Foreground)
            .fixed_pos(position)
            .show(ctx, |ui| {
                let galley = egui::WidgetText::from(
                    egui::RichText::new(&self.text)
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
        Some(result.inner)
    }
}
