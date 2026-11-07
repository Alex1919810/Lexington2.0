use super::app::Pet;
use super::style::{self, draw_bubble};
use eframe::egui::{self, Color32, Rect, vec2};
use eframe::egui::{Sense, Stroke};

impl Pet {
    pub(super) fn submit(&mut self, ctx: &egui::Context) {
        if self.input.trim().is_empty() || self.busy {
            return;
        }
        self.send_message(ctx, self.input.trim().to_owned(), false);
    }
    #[cfg(target_os = "linux")]
    pub(super) fn poll_handoff(&mut self, ctx: &egui::Context) {
        if !self.busy
            && let Some(request) = self.inbox.as_ref().and_then(|inbox| inbox.poll())
        {
            self.last_activity = std::time::Instant::now();
            self.send_message(ctx, request.prompt(), true);
            if !self.error.is_empty() {
                self.text = self.error.clone();
                self.full_text = true;
            }
        }
    }
    fn send_message(&mut self, ctx: &egui::Context, text: String, external: bool) {
        let result = self
            .codex
            .as_ref()
            .ok_or_else(|| "Codex 未启动，请在设置中重启会话。".to_owned())
            .and_then(|codex| codex.submit(text, self.config.codex.no_approval));
        match result {
            Ok(()) => {
                self.busy = true;
                self.external_turn = external;
                self.codex_status = "Lexington正在思考".into();
                self.error.clear();
                self.codex_messages.clear();
                self.text.clear();
            }
            Err(error) => self.error = error,
        }
        ctx.request_repaint();
    }
    pub(super) fn input_ui(
        &mut self,
        ctx: &egui::Context,
        screen: Rect,
        center: egui::Pos2,
        image_height: f32,
    ) {
        let width = self.config.dialog.width;
        let error_height = if self.error.is_empty() || self.busy {
            0.
        } else {
            ctx.fonts(|fonts| {
                let mut job = egui::text::LayoutJob::simple(
                    self.error.clone(),
                    style::font(13.),
                    Color32::from_rgb(231, 76, 60),
                    width - 28.,
                );
                job.sections[0].format.line_height = Some(19.5);
                fonts.layout_job(job).size().y.clamp(19.5, 100.)
            })
        };
        let height = 50.
            + if error_height > 0. {
                error_height + 8.
            } else {
                0.
            };
        // Preserve the old 450px component's local boundary checks before translation.
        let origin = center - vec2(225., 225.);
        let mut position = origin
            + vec2(
                (225. - width / 2.).clamp(10., (screen.width() - 10. - width).max(10.)),
                (225. + image_height / 2. + self.config.dialog.offset_y).clamp(
                    10.,
                    (screen.height() - 10. - self.config.dialog.max_height).max(10.),
                ),
            );
        let id = egui::Id::new("message-input");
        let input_transform = ctx
            .layer_transform_to_global(egui::LayerId::new(egui::Order::Foreground, "input".into()))
            .unwrap_or_default();
        // Keep the full editor/error panel inside the native viewport after hover scaling.
        let global = input_transform * Rect::from_min_size(position, vec2(width, height));
        let top = global.top().clamp(
            screen.top() + 4.,
            (screen.bottom() - global.height() - 4.).max(screen.top() + 4.),
        );
        position = input_transform.inverse() * egui::pos2(global.left(), top);
        let rect = Rect::from_min_size(position, vec2(width, height));
        let pointer = ctx.input(|i| i.pointer.hover_pos());
        #[cfg(target_os = "linux")]
        // Transparent areas receive no native motion events. Poll for hover
        // discovery without claiming clicks or stealing another app's focus.
        let pointer = self.drag_window.as_ref().map_or(pointer, |window| {
            window.pointer_position(ctx.pixels_per_point()).or(pointer)
        });
        let hovered = pointer.is_some_and(|p| {
            (input_transform * rect).contains(p)
                || (input_transform * Rect::from_center_size(center, vec2(450., 450.))).contains(p)
        });
        self.input_bounds = Some(input_transform * rect);
        // egui can retain a TextEdit focus ID after another native window gains
        // focus. It must not keep the pet's input visible (or consume keys).
        if !ctx.input(|i| i.focused) {
            ctx.memory_mut(|m| m.surrender_focus(id));
        }
        let focused = ctx.memory(|m| m.has_focus(id));
        let alpha = ctx.animate_value_with_time(
            egui::Id::new("input-alpha"),
            if hovered || focused || self.busy {
                1.
            } else {
                0.
            },
            0.3,
        );
        if alpha > 0.01 {
            self.input_regions.push(input_transform * rect);
        }
        egui::Area::new("input".into())
            .constrain(false)
            .order(egui::Order::Foreground)
            .fixed_pos(position)
            .show(ctx, |ui| {
                ui.set_opacity(alpha);
                let rect = Rect::from_min_size(ui.min_rect().min, rect.size());
                draw_bubble(ui, rect, &mut self.input_bg, true);
                ui.set_min_size(rect.size());
                ui.scope_builder(
                    egui::UiBuilder::new().max_rect(rect.shrink2(vec2(14., 10.))),
                    |ui| {
                        if self.busy {
                            ui.horizontal_centered(|ui| {
                                ui.spacing_mut().item_spacing.x = 8.;
                                let (rect, _) =
                                    ui.allocate_exact_size(vec2(16., 16.), Sense::hover());
                                let center = rect.center();
                                let points: Vec<_> = (0..=32)
                                    .map(|n| {
                                        let angle = n as f32 / 32. * std::f32::consts::TAU;
                                        center + vec2(angle.cos(), angle.sin()) * 7.
                                    })
                                    .collect();
                                ui.painter().add(egui::Shape::line(
                                    points,
                                    Stroke::new(2_f32, Color32::from_black_alpha(26)),
                                ));
                                let angle =
                                    ui.input(|i| i.time) as f32 * std::f32::consts::TAU / 0.8;
                                let arc: Vec<_> = (0..=8)
                                    .map(|n| {
                                        let a = angle + n as f32 / 8. * std::f32::consts::FRAC_PI_2;
                                        center + vec2(a.cos(), a.sin()) * 7.
                                    })
                                    .collect();
                                ui.painter().add(egui::Shape::line(
                                    arc,
                                    Stroke::new(2_f32, Color32::from_gray(51)),
                                ));
                                ui.label(
                                    egui::RichText::new("Lexington正在思考")
                                        .font(style::font(13.))
                                        .color(Color32::from_gray(102)),
                                );
                            });
                        } else {
                            let edit = ui.add_sized(
                                vec2(width - 28., 30.),
                                egui::TextEdit::singleline(&mut self.input)
                                    .id(id)
                                    .desired_width(width - 28.)
                                    .font(style::font(14.))
                                    .text_color(Color32::from_gray(51))
                                    .hint_text(
                                        egui::RichText::new("输入消息...")
                                            .color(Color32::from_gray(153)),
                                    )
                                    .margin(egui::Margin::ZERO)
                                    .vertical_align(egui::Align::Center)
                                    .frame(false),
                            );
                            if error_height > 0. {
                                // Errors accompany the editor, so a failed connection cannot eat later keystrokes.
                                egui::ScrollArea::vertical()
                                    .id_salt("input-error")
                                    .max_height(error_height)
                                    .drag_to_scroll(false)
                                    .show(ui, |ui| {
                                        ui.add(
                                            egui::Label::new(
                                                egui::RichText::new(&self.error)
                                                    .font(style::font(13.))
                                                    .line_height(Some(19.5))
                                                    .color(Color32::from_rgb(231, 76, 60)),
                                            )
                                            .wrap(),
                                        );
                                    });
                            }
                            if (edit.has_focus() || edit.lost_focus())
                                && ui.input(|i| i.key_pressed(egui::Key::Enter))
                            {
                                self.submit(ctx);
                            }
                        }
                    },
                );
            });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.busy {
                if let Some(codex) = &self.codex {
                    codex.interrupt();
                }
                self.codex_status = "正在停止…".into();
            } else {
                self.input.clear();
                self.error.clear();
                ctx.memory_mut(|m| m.surrender_focus(id));
            }
        }
    }
}
