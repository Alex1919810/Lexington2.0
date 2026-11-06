use super::assets::Picture;
use eframe::egui::{self, Color32, Pos2, Rect, Stroke, pos2};

pub(super) fn install(ctx: &egui::Context) {
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
