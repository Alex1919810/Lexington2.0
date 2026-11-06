use eframe::egui::{self, Color32, Mesh, Painter, Pos2, Shape, Stroke, pos2, vec2};

pub fn font(size: f32) -> egui::FontId {
    egui::FontId::new(size, egui::FontFamily::Name("browser".into()))
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

#[cfg(test)]
mod tests {
    use super::*;
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
