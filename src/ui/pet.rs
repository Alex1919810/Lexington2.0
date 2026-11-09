use super::app::Wife;
use eframe::egui::{self, Color32, Mesh, Painter, Pos2, Rect, Sense, Shape, Stroke, pos2, vec2};
use std::time::{Duration, Instant};

impl Wife {
    pub(super) fn pet_ui(
        &mut self,
        ctx: &egui::Context,
        screen: Rect,
    ) -> (egui::Vec2, egui::emath::TSTransform, bool) {
        let time = ctx.input(|i| i.time);
        let base = screen.center();
        let hover = ctx.input(|i| {
            i.pointer.hover_pos().is_some_and(|p| {
                Rect::from_center_size(base, vec2(450., 450.)).contains(p)
                    || self.input_bounds.is_some_and(|r| r.contains(p))
                    || self.dialog_bounds.is_some_and(|r| r.contains(p))
            })
        });
        let scale = ctx.animate_value_with_time(
            egui::Id::new("pet-scale"),
            if hover { 1.05 } else { 1. },
            0.3,
        );
        let lift = ctx.animate_value_with_time(
            egui::Id::new("pet-lift"),
            if hover { -5. } else { 0. },
            0.3,
        );
        let live = self.picture.as_ref().is_some_and(|p| p.is_live2d());
        let transform = transform(
            base,
            if live {
                lift
            } else {
                (time * 2.).sin() as f32 * 5. + lift
            },
            scale,
        );
        if live {
            ctx.request_repaint_after(Duration::from_secs_f64(1. / 30.));
        }
        for (order, id) in [
            (egui::Order::Background, "pet"),
            (egui::Order::Foreground, "dialog"),
            (egui::Order::Foreground, "input"),
        ] {
            ctx.set_transform_layer(egui::LayerId::new(order, id.into()), transform);
        }
        let center = base;
        let size = self
            .picture
            .as_ref()
            .map(|p| {
                let s = p.size();
                s * (450. / s.x.max(s.y)).min(1.)
            })
            .unwrap_or(vec2(120., 120.));
        let pet_rect = Rect::from_center_size(center, size);
        let mut clicked = false;
        egui::Area::new("pet".into())
            .fixed_pos(screen.min)
            .order(egui::Order::Background)
            .show(ctx, |ui| {
                ui.set_min_size(screen.size());
                if let Some(pic) = self.picture.as_mut() {
                    ui.painter().image(
                        pic.texture().id(),
                        pet_rect,
                        Rect::from_min_max(pos2(0., 0.), pos2(1., 1.)),
                        Color32::WHITE,
                    );
                    self.input_regions
                        .extend(pic.opaque_regions.iter().map(|region| {
                            transform
                                * Rect::from_min_max(
                                    pet_rect.min + region.min.to_vec2() * size,
                                    pet_rect.min + region.max.to_vec2() * size,
                                )
                        }));
                } else {
                    fallback(ui.painter(), center, time);
                    let offset = -5. + 5. * (time as f32 * std::f32::consts::PI).cos();
                    for y in -60..60 {
                        let radius = if y < 12 { 72. } else { 48. };
                        let half_width = 60. * (1. - ((y as f32 - 12.) / radius).powi(2)).sqrt();
                        self.input_regions.push(
                            transform
                                * Rect::from_min_max(
                                    center + vec2(-half_width, y as f32 + offset),
                                    center + vec2(half_width, y as f32 + 1. + offset),
                                ),
                        );
                    }
                }
                let shadow_phase = (1. - (time * std::f64::consts::PI).cos()) as f32 / 2.;
                ui.painter().add(egui::Shape::ellipse_filled(
                    pos2(base.x, base.y + 225.),
                    vec2(40. + shadow_phase * 10., 10.),
                    Color32::from_black_alpha((7.65 - shadow_phase * 2.55).round() as u8),
                ));
                let menu = ui.interact(
                    Rect::from_center_size(center, vec2(450., 450.)),
                    "context-menu".into(),
                    Sense::click(),
                );
                self.context_menu(ctx, &menu);
                // Match the original circular hit region (radius = displayed image height * 0.3).
                let radius = size.y * 0.3;
                let hit_rect = if live {
                    pet_rect
                } else {
                    Rect::from_center_size(center, vec2(radius * 2., radius * 2.))
                };
                let inside = ui.input(|i| {
                    i.pointer.interact_pos().is_some_and(|p| {
                        if live {
                            hit_rect.contains(transform.inverse() * p)
                        } else {
                            (transform.inverse() * p).distance(center) <= radius
                        }
                    })
                });
                let response =
                    ui.interact(hit_rect, egui::Id::new("pet-drag"), Sense::click_and_drag());
                if inside {
                    ctx.set_cursor_icon(egui::CursorIcon::Move);
                }
                if ctx.input(|i| i.pointer.primary_pressed()) {
                    self.pet_pressed = inside.then(Instant::now);
                    self.drag_sent = false;
                    #[cfg(any(target_os = "linux", windows))]
                    if inside
                        && response.is_pointer_button_down_on()
                        && let Some(window) = &mut self.drag_window
                    {
                        window.arm(ctx.pixels_per_point());
                    }
                }
                let moved = ctx.input(|i| {
                    i.pointer
                        .press_origin()
                        .zip(i.pointer.interact_pos())
                        .is_some_and(|(start, end)| {
                            (if live {
                                hit_rect.contains(transform.inverse() * start)
                            } else {
                                (transform.inverse() * start).distance(center) <= radius
                            }) && start.distance(end) > 5.
                        })
                });
                if response.dragged_by(egui::PointerButton::Primary) && moved && !self.drag_sent {
                    #[cfg(any(target_os = "linux", windows))]
                    if self.drag_window.is_none() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                    }
                    #[cfg(not(any(target_os = "linux", windows)))]
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                    self.drag_sent = true;
                }
                clicked = response.clicked()
                    && inside
                    && !self.drag_sent
                    && self
                        .pet_pressed
                        .is_some_and(|at| at.elapsed() < Duration::from_millis(300));
                if clicked
                    && live
                    && let Some(point) = ctx.input(|i| i.pointer.interact_pos())
                {
                    let local = (transform.inverse() * point - pet_rect.min) / size;
                    if let Some(pic) = self.picture.as_mut() {
                        let dimensions = pic.size();
                        pic.click_part([local.x * dimensions.x, local.y * dimensions.y]);
                    }
                    // Body-part actions remain independent of conversation visibility.
                }
                self.context_menu(ctx, &response);
            });
        if clicked && (!live || self.busy || self.turn_ui_hidden) {
            self.click_pet();
        }
        (size, transform, clicked)
    }
}
/// The original body has 60% top and 40% bottom elliptical corner radii.
fn body_points(center: Pos2) -> Vec<Pos2> {
    (0..=96)
        .map(|i| {
            let angle = std::f32::consts::TAU * i as f32 / 96.;
            let y = angle.sin();
            center + vec2(angle.cos() * 60., 12. + y * if y < 0. { 72. } else { 48. })
        })
        .collect()
}

pub fn fallback(painter: &Painter, center: Pos2, time: f64) {
    let center = center + vec2(0., -5. + 5. * (time as f32 * std::f32::consts::PI).cos());
    let points = body_points(center);
    for n in (1..=10).rev() {
        let extent = n as f32;
        let outline: Vec<_> = points
            .iter()
            .map(|p| center + (*p - center) * (1. + extent / 60.) + vec2(0., 10.))
            .collect();
        painter.add(Shape::closed_line(
            outline,
            Stroke::new(2_f32, Color32::from_black_alpha((11 - n) as u8)),
        ));
    }
    let mut mesh = Mesh::default();
    let color = |p: Pos2| {
        let t = ((p.x - center.x + p.y - center.y + 120.) / 240.).clamp(0., 1.);
        Color32::from_rgb(
            255,
            (107. + 36. * t).round() as u8,
            (157. + 14. * t).round() as u8,
        )
    };
    mesh.colored_vertex(center, color(center));
    for &point in &points {
        mesh.colored_vertex(point, color(point));
    }
    for index in 1..points.len() as u32 {
        mesh.add_triangle(0, index, index + 1);
    }
    painter.add(Shape::mesh(mesh));
    // Blink anchors at the eye's top, just as the CSS height animation did.
    let phase = time.rem_euclid(3.) as f32 / 3.;
    let blink = if phase < 0.9 {
        0.
    } else if phase < 0.95 {
        (phase - 0.9) / 0.05
    } else {
        (1. - phase) / 0.05
    };
    let height = 12. - 10. * blink;
    for x in [-12., 12.] {
        painter.rect_filled(
            egui::Rect::from_center_size(center + vec2(x, -18. + height / 2.), vec2(12., height)),
            6,
            Color32::from_gray(51),
        );
    }
    let mouth: Vec<_> = (0..=24)
        .map(|n| {
            let angle = n as f32 / 24. * std::f32::consts::PI;
            center + vec2(angle.cos() * 13.5, 9. + angle.sin() * 13.5)
        })
        .collect();
    painter.add(Shape::line(
        mouth,
        Stroke::new(3_f32, Color32::from_gray(51)),
    ));
}

pub fn transform(center: Pos2, offset: f32, scale: f32) -> egui::emath::TSTransform {
    egui::emath::TSTransform {
        translation: center.to_vec2() * (1. - scale) + vec2(0., offset),
        scaling: scale,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::style::bubble_tail;
    #[test]
    fn css_body_bounds_and_tail_anchors() {
        let points = body_points(pos2(0., 0.));
        assert!(points.iter().any(|p| (p.y + 60.).abs() < 0.001));
        assert!(points.iter().any(|p| (p.y - 60.).abs() < 0.001));
        let rect = egui::Rect::from_min_size(pos2(300., 500.), vec2(200., 50.));
        assert_eq!(bubble_tail(rect, true)[1], pos2(400., 492.));
        assert_eq!(bubble_tail(rect, false)[1], pos2(328., 492.));
    }
    #[test]
    fn hover_transform_scales_children_around_container_center() {
        let center = pos2(400., 300.);
        let transform = transform(center, -5., 1.05);
        assert_eq!(transform * center, pos2(400., 295.));
        assert!((transform * pos2(300., 550.)).distance(pos2(295., 557.5)) < 0.001);
        assert!((transform.inverse() * (transform * center)).distance(center) < 0.001);
    }
}
