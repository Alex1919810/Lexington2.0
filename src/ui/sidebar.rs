use super::app::Pet;
use super::style::draw_shadow;
use eframe::egui::{self, Color32, Rect, pos2, vec2};
use eframe::egui::{FontId, Sense};

impl Pet {
    pub(super) fn sidebar(&mut self, ctx: &egui::Context, screen: Rect) -> bool {
        let mut handled = false;
        let x = if screen.width() / 2. - 315. < 0. {
            10.
        } else {
            screen.width() / 2. - 315.
        };
        let bounds = Rect::from_min_size(pos2(x, screen.center().y - 155.), vec2(50., 360.));
        let hovered = ctx.input(|i| i.pointer.hover_pos().is_some_and(|p| bounds.contains(p)));
        let alpha = ctx.animate_value_with_time(
            egui::Id::new("sidebar-alpha"),
            if self.pinned || hovered { 1. } else { 0.2 },
            0.3,
        );
        egui::Area::new("sidebar".into())
            .fixed_pos(bounds.min + vec2(5., 10.))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = vec2(0., 10.);
                ui.set_opacity(alpha);
                let labels = [
                    if self.pinned { "📌" } else { "📍" },
                    "↻",
                    "▶",
                    if self.library.format == "gif" {
                        "G"
                    } else {
                        "P"
                    },
                    "👤",
                    "⏰",
                    "L",
                ];
                let can_toggle_damage = self.library.damage_counterpart().is_some();
                let tips = [
                    "切换侧边栏显示模式",
                    if !can_toggle_damage {
                        "当前立绘没有对应的正常／战损版本"
                    } else if self.library.damaged {
                        "切换为对应的正常立绘"
                    } else {
                        "切换为对应的战损立绘"
                    },
                    "下一张立绘",
                    "切换正常/Q版立绘",
                    "切换角色",
                    if self.clock_state.running() {
                        "点击终止计时"
                    } else {
                        "上班打卡"
                    },
                    "切换live2d",
                ];
                for n in [0, 1, 2, 3, 6, 4, 5] {
                    let enabled = match n {
                        1 => can_toggle_damage,
                        2 => self.library.selected.is_some(),
                        3 | 4 => !self.library.illustrations.is_empty(),
                        6 => self.library.can_toggle_live2d(),
                        _ => true,
                    };
                    let color = if n == 0 {
                        Color32::from_rgba_unmultiplied(150, 100, 255, 204)
                    } else if n == 1 && self.library.damaged {
                        Color32::from_rgba_unmultiplied(255, 152, 0, 204)
                    } else if (n == 5 && self.clock_state.running())
                        || (n == 6 && self.library.format == "live2d")
                    {
                        Color32::from_rgba_unmultiplied(76, 175, 80, 204)
                    } else {
                        Color32::from_rgba_unmultiplied(100, 150, 255, 204)
                    };
                    let response = ui
                        .allocate_response(
                            vec2(40., 40.),
                            if enabled {
                                Sense::click()
                            } else {
                                Sense::hover()
                            },
                        )
                        .on_hover_text(tips[n]);
                    let scale = ctx.animate_value_with_time(
                        egui::Id::new(("button-scale", n)),
                        if response.is_pointer_button_down_on() {
                            0.95
                        } else if response.hovered() && enabled {
                            1.1
                        } else {
                            1.
                        },
                        0.2,
                    );
                    let rect =
                        Rect::from_center_size(response.rect.center(), vec2(40., 40.) * scale);
                    self.input_regions.push(rect);
                    let painter = ui.painter();
                    let color = if response.hovered() && enabled {
                        Color32::from_rgb(color.r(), color.g(), color.b())
                    } else {
                        color
                    };
                    let opacity = if enabled { 1. } else { 0.5 };
                    draw_shadow(painter, rect, 8., 8., (51. * opacity) as u8);
                    painter.rect_filled(rect, 8, color.gamma_multiply(opacity));
                    match n {
                        0 | 4 | 5 => {
                            let icon = if n == 0 {
                                usize::from(!self.pinned)
                            } else if n == 4 {
                                2
                            } else {
                                3
                            };
                            painter.image(
                                self.icons[icon].id(),
                                Rect::from_center_size(rect.center(), vec2(22.5, 21.25) * scale),
                                Rect::from_min_max(pos2(0., 0.), pos2(1., 1.)),
                                Color32::WHITE.gamma_multiply(opacity),
                            );
                        }
                        _ => {
                            painter.text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                labels[n],
                                FontId::new(18. * scale, egui::FontFamily::Name("arial".into())),
                                Color32::WHITE.gamma_multiply(opacity),
                            );
                        }
                    }
                    if response.hovered() {
                        ui.ctx().set_cursor_icon(if enabled {
                            egui::CursorIcon::PointingHand
                        } else {
                            egui::CursorIcon::NotAllowed
                        });
                    }
                    if response.clicked() {
                        handled = true;
                        match n {
                            0 => self.pinned = !self.pinned,
                            1 => {
                                self.library.toggle_damage();
                                self.load_picture(ctx);
                            }
                            2 => {
                                self.library.next_illustration();
                                self.load_picture(ctx);
                            }
                            3 => {
                                self.library.toggle_format();
                                self.load_picture(ctx);
                            }
                            4 => {
                                self.library.next_character(&self.root);
                                self.load_picture(ctx);
                            }
                            5 => self.clock(),
                            6 => {
                                self.library.toggle_live2d();
                                self.load_picture(ctx);
                            }
                            _ => {}
                        }
                    }
                }
            });
        handled
    }
    pub(super) fn clock(&mut self) {
        if !self.clock_storage_ok {
            self.settings = true;
            self.settings_page = 2;
            return;
        }
        match self
            .clock_state
            .toggle(&self.config.clock_in, crate::backend::clock::now())
        {
            Ok(message) => {
                self.clock_error.clear();
                self.clock_notice = false;
                self.say(message.clone());
                if self.clock_state.running() {
                    self.queue_clock_weather(message);
                }
            }
            Err(error) => {
                self.clock_error = error;
                self.settings = true;
                self.settings_page = 2;
            }
        }
    }
}
