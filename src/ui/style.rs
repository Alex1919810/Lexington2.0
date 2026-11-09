use super::assets::Picture;
use eframe::egui::{self, Color32, Pos2, Rect, Stroke, pos2};

pub(super) fn install(ctx: &egui::Context) {
    let installed = egui::Id::new("ime-commit-compatibility-installed");
    if !ctx.data(|data| data.get_temp::<bool>(installed).unwrap_or(false)) {
        ctx.data_mut(|data| data.insert_temp(installed, true));
        ctx.on_begin_pass(
            "ime-commit-compatibility",
            std::sync::Arc::new(|ctx| {
                let state_id = egui::Id::new(("ime-composition", ctx.viewport_id()));
                let focused = ctx.memory(|memory| memory.focused());
                let mut composing = ctx
                    .data(|data| data.get_temp::<Option<egui::Id>>(state_id))
                    .flatten()
                    .filter(|id| Some(*id) == focused);
                ctx.input_mut(|input| {
                    if !input.focused {
                        composing = None;
                    }
                    for event in &mut input.events {
                        match event {
                            egui::Event::Ime(
                                egui::ImeEvent::Enabled | egui::ImeEvent::Preedit(_),
                            ) => {
                                composing = focused;
                            }
                            egui::Event::Ime(egui::ImeEvent::Commit(text)) => {
                                // egui 0.31 checks Commit against the last preedit cursor.
                                // XIM may send only Commit, leaving that cursor at the
                                // first word. Insert these candidates as ordinary text.
                                if composing.is_none() {
                                    *event = egui::Event::Text(std::mem::take(text));
                                }
                                composing = None;
                            }
                            egui::Event::Ime(egui::ImeEvent::Disabled) => composing = None,
                            _ => {}
                        }
                    }
                });
                ctx.data_mut(|data| data.insert_temp(state_id, composing));
            }),
        );
    }
    let mut fonts = egui::FontDefinitions::default();
    let mut cjk = egui::FontData::from_static(include_bytes!(
        "../../resource/fonts/NotoSansCJK-Regular.ttc"
    ));
    cjk.index = 2; // Simplified Chinese face, matching the original renderer on Linux.
    fonts.font_data.insert("cjk".into(), cjk.into());
    fonts.font_data.insert(
        "serif".into(),
        egui::FontData::from_static(include_bytes!(
            "../../resource/fonts/LiberationSerif-Regular.ttf"
        ))
        .into(),
    );
    fonts.font_data.insert(
        "arial".into(),
        egui::FontData::from_static(include_bytes!(
            "../../resource/fonts/LiberationSans-Regular.ttf"
        ))
        .into(),
    );
    fonts.font_data.insert(
        "math".into(),
        egui::FontData::from_static(include_bytes!(
            "../../resource/fonts/NotoSansMath-Regular.ttf"
        ))
        .into(),
    );
    fonts.families.insert(
        egui::FontFamily::Name("browser".into()),
        vec!["serif".into(), "cjk".into()],
    );
    fonts.families.insert(
        egui::FontFamily::Name("arial".into()),
        vec!["arial".into(), "cjk".into(), "math".into()],
    );
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts.families.entry(family).or_default().push("cjk".into());
    }
    ctx.set_fonts(fonts);
    ctx.set_visuals(egui::Visuals::light());
}

pub fn font(size: f32) -> egui::FontId {
    egui::FontId::new(size, egui::FontFamily::Name("browser".into()))
}

pub(super) const PANEL_BG: Color32 = Color32::from_rgb(243, 246, 248);
pub(super) const INK: Color32 = Color32::from_rgb(37, 55, 70);
pub(super) const MUTED: Color32 = Color32::from_rgb(100, 116, 130);
pub(super) const ACCENT: Color32 = Color32::from_rgb(35, 112, 116);

/// Apply only to the current panel, leaving the pet and other viewports unchanged.
pub(super) fn panel_style(ui: &mut egui::Ui) {
    let style = ui.style_mut();
    for (kind, size) in [
        (egui::TextStyle::Heading, 24.),
        (egui::TextStyle::Body, 14.),
        (egui::TextStyle::Button, 14.),
        (egui::TextStyle::Small, 12.),
    ] {
        style
            .text_styles
            .insert(kind, egui::FontId::proportional(size));
    }
    style.spacing.item_spacing = egui::vec2(10., 10.);
    style.spacing.button_padding = egui::vec2(12., 7.);
    style.spacing.interact_size.y = 30.;
    style.visuals.override_text_color = Some(INK);
    style.visuals.panel_fill = PANEL_BG;
    style.visuals.extreme_bg_color = Color32::from_rgb(249, 251, 252);
    style.visuals.faint_bg_color = Color32::from_rgb(238, 244, 246);
    style.visuals.selection.bg_fill = Color32::from_rgb(218, 237, 235);
    style.visuals.selection.stroke = Stroke::new(1_f32, ACCENT);
    for widget in [
        &mut style.visuals.widgets.noninteractive,
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
        &mut style.visuals.widgets.open,
    ] {
        widget.corner_radius = egui::CornerRadius::same(7);
        widget.bg_stroke = Stroke::new(1_f32, Color32::from_rgb(219, 228, 233));
        widget.fg_stroke = Stroke::new(1_f32, INK);
    }
    style.visuals.widgets.inactive.weak_bg_fill = Color32::WHITE;
    style.visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(232, 242, 242);
    style.visuals.widgets.active.weak_bg_fill = Color32::from_rgb(218, 237, 235);
}

pub(super) fn card() -> egui::Frame {
    egui::Frame::new()
        .fill(Color32::WHITE)
        .stroke(Stroke::new(1_f32, Color32::from_rgb(222, 230, 235)))
        .corner_radius(12)
        .inner_margin(16)
}

pub fn bubble_tail(rect: egui::Rect, input: bool) -> Vec<Pos2> {
    let x = if input {
        rect.center().x
    } else {
        rect.left() + 28.
    };
    vec![
        pos2(x - 8., rect.top()),
        pos2(x, rect.top() - 8.),
        pos2(x + 8., rect.top()),
    ]
}

pub(super) fn draw_bubble(
    ui: &mut egui::Ui,
    rect: Rect,
    background: &mut Option<Picture>,
    input: bool,
) {
    draw_shadow(ui.painter(), rect, 12., 10., 38);
    if let Some(image) = background {
        let size = image.size();
        let target = rect.size();
        let ratio = (target.x / size.x).max(target.y / size.y);
        let uv_size = target / (size * ratio);
        let uv = Rect::from_center_size(pos2(0.5, 0.5), uv_size);
        egui::Image::new((image.texture().id(), size))
            .uv(uv)
            .corner_radius(12)
            .paint_at(ui, rect);
    } else {
        ui.painter()
            .rect_filled(rect, 12, Color32::from_white_alpha(242));
    }
    ui.painter().add(egui::Shape::convex_polygon(
        bubble_tail(rect, input),
        Color32::from_white_alpha(242),
        Stroke::NONE,
    ));
}
pub(super) fn draw_shadow(painter: &egui::Painter, rect: Rect, radius: f32, blur: f32, alpha: u8) {
    painter.add(
        egui::epaint::Shadow {
            offset: [0, 2],
            blur: (blur * 2.) as u8,
            spread: 0,
            color: Color32::from_black_alpha(alpha),
        }
        .as_shape(rect, radius as u8),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ime_commit_only_continues_in_singleline_and_multiline_editors() {
        for multiline in [false, true] {
            let ctx = egui::Context::default();
            install(&ctx);
            let id = egui::Id::new("ime-editor");
            let mut text = String::new();
            let run = |text: &mut String, events| {
                ctx.run(
                    egui::RawInput {
                        events,
                        ..Default::default()
                    },
                    |ctx| {
                        egui::CentralPanel::default().show(ctx, |ui| {
                            let editor = if multiline {
                                egui::TextEdit::multiline(text)
                            } else {
                                egui::TextEdit::singleline(text)
                            };
                            ui.add(editor.id(id));
                        });
                    },
                )
            };
            run(&mut text, vec![]);
            ctx.memory_mut(|memory| memory.request_focus(id));
            run(&mut text, vec![]);
            let mut expected = String::new();
            for word in ["你好", "世界", "中文", "测试"] {
                // XIM can commit a whole candidate without an Enabled/Preedit pair.
                run(
                    &mut text,
                    vec![
                        egui::Event::Ime(egui::ImeEvent::Disabled),
                        egui::Event::Ime(egui::ImeEvent::Commit(word.into())),
                        egui::Event::Ime(egui::ImeEvent::Disabled),
                    ],
                );
                expected.push_str(word);
                assert_eq!(text, expected, "multiline={multiline}");
                assert!(ctx.memory(|memory| memory.has_focus(id)));
            }
            // Normal preedit still replaces its marked text, even across frames.
            run(&mut text, vec![egui::Event::Ime(egui::ImeEvent::Enabled)]);
            for preedit in ["lian", "lianxu"] {
                run(
                    &mut text,
                    vec![egui::Event::Ime(egui::ImeEvent::Preedit(preedit.into()))],
                );
            }
            run(
                &mut text,
                vec![
                    egui::Event::Ime(egui::ImeEvent::Commit("连续".into())),
                    egui::Event::Ime(egui::ImeEvent::Disabled),
                ],
            );
            expected.push_str("连续");
            assert_eq!(text, expected);
            // A bare commit must also replace the selected Unicode range.
            let mut state = egui::TextEdit::load_state(&ctx, id).unwrap();
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::two(
                    egui::text::CCursor::new(2),
                    egui::text::CCursor::new(4),
                )));
            state.store(&ctx, id);
            run(
                &mut text,
                vec![egui::Event::Ime(egui::ImeEvent::Commit("替换".into()))],
            );
            assert_eq!(text, "你好替换中文测试连续");
        }
    }
}
