//! Connected layers sharing global idle, fixed-branch arm IK and visible-alpha hits.
use super::{Action, Channel, Event, mesh, parameters, render, trigger, validate_motion, wave};
use image::RgbaImage;
use serde::Deserialize;
use std::{collections::BTreeMap, path::Path};

#[derive(Deserialize)]
struct Model {
    canvas: [u32; 2],
    original: String,
    background: String,
    foreground: String,
    #[serde(default)]
    fixed_background: bool,
    #[serde(default)]
    breath_body_only: bool,
    #[serde(default)]
    background_above_depth: bool,
    root_pivot: [f32; 2],
    sway: Sway,
    breath: Breath,
    parts: Vec<PartSpec>,
    #[serde(default)]
    rig: Option<ArmRig>,
    channels: BTreeMap<String, Channel>,
    actions: BTreeMap<String, Action>,
}
#[derive(Deserialize)]
struct Sway {
    period: f64,
    angle: f32,
    dx: f32,
}
#[derive(Deserialize)]
struct Breath {
    period: f64,
    center: [f32; 2],
    radii: [f32; 2],
    dx_scale: f32,
    dy: f32,
}
#[derive(Deserialize)]
struct PartSpec {
    image: String,
    mask: String,
    pivot: [f32; 2],
    #[serde(default)]
    channel: String,
    #[serde(default)]
    action: String,
    foreground: bool,
    #[serde(default)]
    idle: Option<PartIdle>,
    #[serde(default = "background_enabled")]
    fill_background: bool,
    #[serde(default)]
    binding: String,
    #[serde(default)]
    mesh: Option<mesh::Mesh>,
    #[serde(default)]
    front_mask: Option<String>,
}
#[derive(Deserialize)]
struct PartIdle {
    period: f64,
    angle: f32,
}
fn background_enabled() -> bool {
    true
}
impl PartSpec {
    fn angle(&self, values: &BTreeMap<String, f32>, time: f64) -> f32 {
        self.idle.as_ref().map_or_else(
            || values[&self.channel],
            // Idle hair must cross neutral at full speed. The torso's sin^3
            // envelope has zero velocity there and creates an extra pause.
            |idle| idle.angle * (time * std::f64::consts::TAU / idle.period).sin() as f32,
        )
    }
}
#[derive(Deserialize)]
struct ArmRig {
    channel: String,
    bones: [[f32; 2]; 3],
    pole: f32,
    target_delta: [f32; 2],
}
impl ArmRig {
    fn matrices(&self, values: &BTreeMap<String, f32>) -> [[f32; 6]; 2] {
        let value = values[&self.channel];
        if value == 0. {
            return [[1., 0., 0., 0., 1., 0.]; 2];
        }
        let target = [
            self.bones[2][0] + self.target_delta[0] * value,
            self.bones[2][1] + self.target_delta[1] * value,
        ];
        let elbow = render::elbow(self.bones, target, self.pole);
        [
            render::segment(self.bones[0], self.bones[1], self.bones[0], elbow),
            render::segment(self.bones[1], self.bones[2], elbow, target),
        ]
    }
    fn validate(&self, channels: &BTreeMap<String, Channel>) -> Result<(), String> {
        let channel = channels.get(&self.channel).ok_or("Live2D 骨骼通道缺失。")?;
        if self
            .bones
            .iter()
            .flatten()
            .chain(self.target_delta.iter())
            .any(|v| !v.is_finite() || v.abs() > 2048.)
            || self.pole.abs() != 1.
            || channel.default != 0.
            || self.target_delta.iter().any(|v| v.abs() > 3.)
        {
            return Err("Live2D 骨骼参数无效。".into());
        }
        let distance =
            |a: [f32; 2], b: [f32; 2]| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt();
        let a = distance(self.bones[0], self.bones[1]);
        let b = distance(self.bones[1], self.bones[2]);
        let [s, e, w] = self.bones;
        let cross = (w[0] - s[0]) * (e[1] - s[1]) - (w[1] - s[1]) * (e[0] - s[0]);
        if a < 1. || b < 1. || cross * self.pole < 0.1 {
            return Err("Live2D 骨骼弯曲侧无效。".into());
        }
        for i in 0..=16 {
            let value = channel.range[0] + (channel.range[1] - channel.range[0]) * i as f32 / 16.;
            let target = [
                w[0] + self.target_delta[0] * value,
                w[1] + self.target_delta[1] * value,
            ];
            let d = distance(s, target);
            if d >= (a + b) - 0.5 || d <= (a - b).abs() + 0.5 {
                return Err("Live2D 骨骼目标不可达。".into());
            }
        }
        Ok(())
    }
}
struct Part {
    image: RgbaImage,
    mask: RgbaImage,
    bounds: [u32; 4],
    front_mask: Option<RgbaImage>,
}
pub(super) struct LayeredActor {
    model: Model,
    original: RgbaImage,
    background: RgbaImage,
    foreground: RgbaImage,
    parts: Vec<Part>,
    events: Vec<Event>,
    // Cache only the current local pose. Idle remains global and is never baked.
    pose: Option<(Vec<f32>, RgbaImage)>,
}
fn over(bottom: [f32; 4], top: [f32; 4]) -> [f32; 4] {
    std::array::from_fn(|i| top[i] + bottom[i] * (1. - top[3]))
}
impl LayeredActor {
    pub(super) fn load(path: &Path, bytes: &[u8]) -> Result<Self, String> {
        let model: Model = serde_json::from_slice(bytes).map_err(|_| "Live2D 分层格式错误。")?;
        let finite_point = |p: [f32; 2]| p.iter().all(|v| v.is_finite() && v.abs() <= 2048.);
        if model.canvas.iter().any(|&v| v == 0 || v > 2048)
            || model.canvas[0] * model.canvas[1] > 1024 * 1024
            || model.parts.is_empty()
            || model.parts.len() > 16
            || model
                .parts
                .windows(2)
                .any(|p| p[0].foreground && !p[1].foreground)
            || !finite_point(model.root_pivot)
            || !finite_point(model.breath.center)
            || model
                .breath
                .radii
                .iter()
                .any(|v| !v.is_finite() || *v <= 0. || *v > 2048.)
            || [model.sway.period, model.breath.period]
                .iter()
                .any(|v| !v.is_finite() || *v <= 0.)
            || [
                model.sway.angle,
                model.sway.dx,
                model.breath.dy,
                model.breath.dx_scale,
            ]
            .iter()
            .any(|v| !v.is_finite() || v.abs() > 10.)
        {
            return Err("Live2D 分层参数无效。".into());
        }
        if (model.background_above_depth
            && (!model.fixed_background || !model.parts.iter().any(|p| p.front_mask.is_some())))
            || model
                .parts
                .iter()
                .filter(|p| p.front_mask.is_some())
                .count()
                > 1
            || (model.parts.iter().any(|p| p.front_mask.is_some())
                && model.parts.iter().any(|p| p.foreground))
            || model
                .parts
                .iter()
                .any(|p| p.front_mask.is_some() && p.mesh.is_some())
        {
            return Err("Live2D 前后遮挡图层无效。".into());
        }
        validate_motion(&model.channels, &model.actions)?;
        if let Some(rig) = &model.rig {
            rig.validate(&model.channels)?;
        }
        let root = path
            .parent()
            .ok_or("Live2D 路径无效。")?
            .canonicalize()
            .map_err(|_| "Live2D 目录无效。")?;
        let image = |name: &str| -> Result<RgbaImage, String> {
            let im = render::rgba(&root, name)?;
            if im.dimensions() != (model.canvas[0], model.canvas[1]) {
                return Err("Live2D 分层尺寸不匹配。".into());
            }
            Ok(im)
        };
        let mut parts = vec![];
        for spec in &model.parts {
            if !finite_point(spec.pivot) {
                return Err("Live2D 部件轴无效。".into());
            }
            if let Some(idle) = &spec.idle {
                if !idle.period.is_finite()
                    || idle.period <= 0.
                    || !idle.angle.is_finite()
                    || idle.angle.abs() > 15.
                    || !spec.channel.is_empty()
                    || !spec.action.is_empty()
                    || spec.fill_background
                {
                    return Err("Live2D 待机部件无效。".into());
                }
            } else {
                let channel = model
                    .channels
                    .get(&spec.channel)
                    .ok_or("Live2D 部件通道缺失。")?;
                let action = model
                    .actions
                    .get(&spec.action)
                    .ok_or("Live2D 部件动作缺失。")?;
                if channel.default != 0.
                    || channel.range.iter().any(|v| v.abs() > 15.)
                    || !action.tracks.contains_key(&spec.channel)
                {
                    return Err("Live2D 部件绑定无效。".into());
                }
            }
            if !matches!(spec.binding.as_str(), "" | "upper" | "lower")
                || (!spec.binding.is_empty() && model.rig.is_none())
            {
                return Err("Live2D 部件骨骼绑定无效。".into());
            }
            if let Some(mesh) = &spec.mesh {
                mesh.validate(model.canvas, model.rig.is_some())?;
            }
            if (!spec.binding.is_empty() || matches!(spec.mesh, Some(mesh::Mesh::Arm { .. })))
                && model.rig.as_ref().is_none_or(|r| r.channel != spec.channel)
            {
                return Err("Live2D 骨骼驱动通道不匹配。".into());
            }
            let mask = image(&spec.mask)?;
            let texture = image(&spec.image)?;
            let mut bounds = [model.canvas[0], model.canvas[1], 0, 0];
            let mut mask_has_pixels = false;
            for (x, y, p) in mask.enumerate_pixels() {
                mask_has_pixels |= p[0] > 0;
                if p[0] > 0 || texture.get_pixel(x, y)[3] > 0 {
                    bounds[0] = bounds[0].min(x);
                    bounds[1] = bounds[1].min(y);
                    bounds[2] = bounds[2].max(x + 1);
                    bounds[3] = bounds[3].max(y + 1);
                }
            }
            if !mask_has_pixels || bounds[0] >= bounds[2] || bounds[1] >= bounds[3] {
                return Err("Live2D 部件遮罩为空。".into());
            }
            parts.push(Part {
                image: texture,
                mask,
                bounds,
                front_mask: spec
                    .front_mask
                    .as_ref()
                    .map(|name| image(name))
                    .transpose()?,
            });
        }
        Ok(Self {
            original: image(&model.original)?,
            background: image(&model.background)?,
            foreground: image(&model.foreground)?,
            model,
            parts,
            events: vec![],
            pose: None,
        })
    }
    pub(super) fn parameters(&self, time: f64) -> BTreeMap<String, f32> {
        parameters(
            &self.model.channels,
            &self.model.actions,
            &self.events,
            time,
        )
    }
    fn root(&self, time: f64) -> [f32; 6] {
        let w = wave(time, self.model.sway.period);
        render::rotation(
            self.model.root_pivot,
            self.model.sway.angle * w,
            self.model.sway.dx * w,
        )
    }
    fn body_point(&self, mut point: [f32; 2], time: f64) -> [f32; 2] {
        let b = &self.model.breath;
        let radius = ((point[0] - b.center[0]) / b.radii[0]).powi(2)
            + ((point[1] - b.center[1]) / b.radii[1]).powi(2);
        let weight = (1. - radius).max(0.).powi(3) * wave(time, b.period);
        point[0] -= (point[0] - b.center[0]) * b.dx_scale * weight;
        point[1] -= b.dy * weight;
        point
    }
    fn part_transform(
        &self,
        spec: &PartSpec,
        values: &BTreeMap<String, f32>,
        time: f64,
    ) -> [f32; 6] {
        if !spec.binding.is_empty() {
            let bones = self.model.rig.as_ref().unwrap().matrices(values);
            return bones[usize::from(spec.binding == "lower")];
        }
        render::rotation(spec.pivot, spec.angle(values, time), 0.)
    }
    fn bones(&self, values: &BTreeMap<String, f32>) -> [[f32; 6]; 2] {
        self.model
            .rig
            .as_ref()
            .map_or([[1., 0., 0., 0., 1., 0.]; 2], |r| r.matrices(values))
    }
    pub(super) fn click(&mut self, point: [f32; 2], time: f64) -> bool {
        if !time.is_finite() || point.iter().any(|v| !v.is_finite()) {
            return false;
        }
        let point = render::inverse(self.root(time), point);
        let values = self.parameters(time);
        let bones = self.bones(&values);
        let mut hits = vec![];
        let mut front = 0_f32;
        let depth_index = self.parts.iter().position(|p| p.front_mask.is_some());
        for (spec, part) in self.model.parts.iter().zip(&self.parts) {
            let matrix = self.part_transform(spec, &values, time);
            let local = render::inverse(matrix, point);
            let alpha = if let Some(mesh) = &spec.mesh {
                if mesh.pose(matrix, bones).hit(&part.image, point) {
                    1.
                } else {
                    0.
                }
            } else {
                render::sample(&part.image, local[0], local[1], false)[3]
            };
            if let Some(mask) = &part.front_mask {
                front = alpha * render::sample(mask, local[0], local[1], false)[0];
            }
            hits.push(alpha >= 8. / 255.);
        }
        let body_point = if self.model.breath_body_only {
            self.body_point(point, time)
        } else {
            point
        };
        let body =
            render::sample(&self.foreground, body_point[0], body_point[1], false)[3] >= 8. / 255.;
        let mut selected = depth_index.filter(|i| hits[*i] && front >= 8. / 255.);
        if selected.is_none() {
            selected = self
                .model
                .parts
                .iter()
                .enumerate()
                .rev()
                .find(|(i, p)| p.foreground && hits[*i])
                .map(|(i, _)| i);
        }
        if selected.is_none() && !body {
            selected = self
                .model
                .parts
                .iter()
                .enumerate()
                .rev()
                .find(|(i, p)| !p.foreground && Some(*i) != depth_index && hits[*i])
                .map(|(i, _)| i);
            if selected.is_none() {
                selected = depth_index.filter(|i| {
                    hits[*i]
                        && !(self.model.background_above_depth
                            && render::sample(&self.background, point[0], point[1], false)[3]
                                >= 8. / 255.)
                });
            }
        }
        let action = selected.and_then(|i| {
            let part = &self.model.parts[i];
            (part.idle.is_none() && !part.action.is_empty()).then(|| part.action.clone())
        });
        action.is_some_and(|a| trigger(&self.model.actions, &mut self.events, &a, time))
    }
    pub(super) fn render(&mut self, time: f64) -> RgbaImage {
        self.events
            .retain(|e| e.start + self.model.actions[&e.action].duration > time);
        let values = self.parameters(time);
        let mut angles: Vec<_> = self
            .model
            .parts
            .iter()
            .map(|p| p.angle(&values, time))
            .collect();
        if self.model.breath_body_only {
            angles.push(wave(time, self.model.breath.period));
        }
        if self
            .pose
            .as_ref()
            .is_none_or(|(cached, _)| *cached != angles)
        {
            let pose = if !self.model.fixed_background && angles.iter().all(|a| *a == 0.) {
                self.original.clone()
            } else {
                let matrices: Vec<_> = self
                    .model
                    .parts
                    .iter()
                    .zip(&angles)
                    .map(|(s, _)| self.part_transform(s, &values, time))
                    .collect();
                let bones = self.bones(&values);
                let mesh_images = self
                    .model
                    .parts
                    .iter()
                    .zip(&self.parts)
                    .zip(&matrices)
                    .map(|((spec, part), matrix)| {
                        spec.mesh
                            .as_ref()
                            .map(|mesh| mesh.pose(*matrix, bones).raster(&part.image))
                    })
                    .collect::<Vec<_>>();
                let mut bounds = [
                    self.model.canvas[0] as f32,
                    self.model.canvas[1] as f32,
                    0_f32,
                    0_f32,
                ];
                for ((part, m), angle) in self.parts.iter().zip(&matrices).zip(&angles) {
                    if *angle == 0. && !self.model.fixed_background {
                        continue;
                    }
                    let [l, t, r, b] = part.bounds.map(|v| v as f32);
                    for p in [[l, t], [r, t], [l, b], [r, b]] {
                        let q = render::apply(*m, p);
                        bounds[0] = bounds[0].min(p[0]).min(q[0] - 2.);
                        bounds[1] = bounds[1].min(p[1]).min(q[1] - 2.);
                        bounds[2] = bounds[2].max(p[0]).max(q[0] + 2.);
                        bounds[3] = bounds[3].max(p[1]).max(q[1] + 2.);
                    }
                }
                for image in mesh_images.iter().flatten() {
                    for (x, y, p) in image.enumerate_pixels() {
                        if p[3] > 0 {
                            bounds[0] = bounds[0].min(x as f32 - 1.);
                            bounds[1] = bounds[1].min(y as f32 - 1.);
                            bounds[2] = bounds[2].max(x as f32 + 1.);
                            bounds[3] = bounds[3].max(y as f32 + 1.);
                        }
                    }
                }
                let depth_index = self.parts.iter().position(|p| p.front_mask.is_some());
                RgbaImage::from_fn(self.model.canvas[0], self.model.canvas[1], |x, y| {
                    if (x as f32) < bounds[0]
                        || (y as f32) < bounds[1]
                        || (x as f32) > bounds[2]
                        || (y as f32) > bounds[3]
                    {
                        return if self.model.fixed_background {
                            let foreground = if self.model.breath_body_only {
                                let p = self.body_point([x as f32, y as f32], time);
                                render::sample(&self.foreground, p[0], p[1], false)
                            } else {
                                render::packed(self.foreground.get_pixel(x, y))
                            };
                            render::straight(over(
                                render::packed(self.background.get_pixel(x, y)),
                                foreground,
                            ))
                        } else {
                            *self.original.get_pixel(x, y)
                        };
                    }
                    let p = [x as f32, y as f32];
                    let mut old_coverage = 0_f32;
                    let mut new_coverage = 0_f32;
                    let mut moved = [[0_f32; 4]; 16];
                    let mut front_weight = 0_f32;
                    for (i, (part, m)) in self.parts.iter().zip(&matrices).enumerate() {
                        let local = render::inverse(*m, p);
                        if let Some(mask) = &part.front_mask {
                            front_weight = render::sample(mask, local[0], local[1], false)[0];
                        }
                        if !self.model.fixed_background && self.model.parts[i].fill_background {
                            old_coverage =
                                old_coverage.max(f32::from(part.mask.get_pixel(x, y)[0]) / 255.);
                            new_coverage = new_coverage
                                .max(render::sample(&part.mask, local[0], local[1], false)[0]);
                        }
                        moved[i] = if let Some(image) = &mesh_images[i] {
                            render::packed(image.get_pixel(x, y))
                        } else {
                            render::sample(&part.image, local[0], local[1], false)
                        };
                    }
                    let uncovered = (old_coverage - new_coverage).max(0.);
                    let mut color = render::packed(self.background.get_pixel(x, y));
                    if !self.model.fixed_background {
                        color = color.map(|v| v * uncovered);
                    }
                    let mut without_depth = color;
                    if let Some(i) = depth_index {
                        color = if self.model.background_above_depth {
                            over(moved[i], color)
                        } else {
                            over(color, moved[i])
                        };
                    }
                    for (i, (_, pixel)) in self
                        .model
                        .parts
                        .iter()
                        .zip(&moved)
                        .enumerate()
                        .filter(|(_, (s, _))| !s.foreground)
                    {
                        if Some(i) != depth_index {
                            color = over(color, *pixel);
                            without_depth = over(without_depth, *pixel);
                        }
                    }
                    let foreground = if self.model.breath_body_only {
                        let p = self.body_point([x as f32, y as f32], time);
                        render::sample(&self.foreground, p[0], p[1], false)
                    } else {
                        render::packed(self.foreground.get_pixel(x, y))
                    };
                    color = over(color, foreground);
                    without_depth = over(without_depth, foreground);
                    for (_, pixel) in self
                        .model
                        .parts
                        .iter()
                        .zip(&moved)
                        .filter(|(s, _)| s.foreground)
                    {
                        color = over(color, *pixel);
                        without_depth = over(without_depth, *pixel);
                    }
                    if let Some(i) = depth_index {
                        let front = over(without_depth, moved[i]);
                        // Blend two complete depth orders, never two clipped alphas.
                        color = std::array::from_fn(|j| {
                            color[j] + (front[j] - color[j]) * front_weight
                        });
                    }
                    render::straight(color)
                })
            };
            self.pose = Some((angles, pose));
        }
        let pose = &self.pose.as_ref().unwrap().1;
        let root = self.root(time);
        RgbaImage::from_fn(self.model.canvas[0], self.model.canvas[1], |x, y| {
            let mut p = render::inverse(root, [x as f32, y as f32]);
            if !self.model.breath_body_only {
                p = self.body_point(p, time);
            }
            render::straight(render::sample(pose, p[0], p[1], false))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn actor() -> LayeredActor {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("resource/illustration/lexington/live2d/BiKiNi_Normal/actor.json");
        LayeredActor::load(&path, &std::fs::read(&path).unwrap()).unwrap()
    }
    #[test]
    fn bikini_parts_follow_current_pose_queue_independently_and_restore_idle() {
        let mut actor = actor();
        let start = 1.37;
        let p = render::apply(actor.root(start), [110., 230.]);
        assert!(actor.click(p, start));
        let p = render::apply(actor.root(start), [225., 110.]);
        assert!(actor.click(p, start));
        let values = actor.parameters(start + 0.9);
        assert!(values["board_angle"] > 0.);
        assert_eq!(values["hat_angle"], 6.);
        // Hit an actual moved hat pixel at its extreme, including the root transform.
        let t = start + 0.9;
        let p = render::apply(
            actor.part_transform(&actor.model.parts[2], &actor.parameters(t), t),
            [225., 110.],
        );
        let p = render::apply(actor.root(t), p);
        assert!(actor.click(p, t));
        assert!(!actor.click(p, t + 0.01));
        assert!(!actor.click([0., 0.], t));
        assert!(!actor.click(render::apply(actor.root(t), [246., 185.]), t));
        assert!(!actor.click([f32::NAN, 0.], t));
        assert_eq!(actor.events.len(), 3);
        let frame = actor.render(start + 0.9);
        assert_eq!(frame.dimensions(), (512, 512));
        assert!(frame.pixels().all(|p| p[3] != 0 || p.0 == [0; 4]));
        let finished = actor.render(start + 8.);
        assert_eq!(finished, self::actor().render(start + 8.));
        assert!(actor.events.is_empty());
    }
    #[test]
    fn hair_crosses_neutral_without_an_extra_stop_and_turns_smoothly() {
        let a = actor();
        let part = &a.model.parts[1];
        let values = a.parameters(0.);
        let angle = |t| part.angle(&values, t);
        for crossing in [0., 3., 6.] {
            let before = angle(crossing - 1. / 30.);
            let at = angle(crossing);
            let after = angle(crossing + 1. / 30.);
            assert!(
                (at - before).abs() > 0.2 && (after - at).abs() > 0.2,
                "hair stalls while crossing neutral at {crossing}"
            );
            assert!(
                (at - before) * (after - at) > 0.,
                "motion reverses at neutral"
            );
        }
        for turn in [1.5, 4.5] {
            assert!((angle(turn - 0.01) - angle(turn + 0.01)).abs() < 0.001);
            assert!(
                (angle(turn) - angle(turn - 0.01)).abs() < 0.001,
                "turn must decelerate continuously"
            );
        }
        for t in [0.1, 0.73, 2.84] {
            assert!((angle(t) - angle(t + 6.)).abs() < 0.00001);
        }
    }
    #[test]
    fn bikini_hair_uses_global_idle_without_changing_face_body_or_local_channels() {
        let mut actor = actor();
        let hair = &actor.model.parts[1];
        assert!(hair.idle.is_some());
        assert_eq!(hair.angle(&actor.parameters(1.5), 1.5), 6.);
        assert_eq!(hair.angle(&actor.parameters(4.5), 4.5), -6.);
        assert!(trigger(
            &actor.model.actions,
            &mut actor.events,
            "hat",
            1.37
        ));
        assert_eq!(actor.model.parts[1].angle(&actor.parameters(1.5), 1.5), 6.);
        assert!(!actor.click(render::apply(actor.root(1.5), [335., 210.]), 1.5));
        let mut still = self::actor();
        still.model.parts[1].idle.as_mut().unwrap().angle = 0.;
        let idle = still.render(1.5);
        let mut moving = self::actor();
        let frame = moving.render(1.5);
        assert_ne!(frame, idle);
        for (x, y, pixel) in frame.enumerate_pixels() {
            if x < 258 || !(115..275).contains(&y) || x > 390 {
                assert_eq!(
                    *pixel,
                    *idle.get_pixel(x, y),
                    "hair changed non-hair pixel {x},{y}"
                );
            }
        }
        // The same decomposed layers must stay continuous just off neutral.
        moving.model.parts[1].idle.as_mut().unwrap().angle = 0.001;
        let near = moving.render(1.5);
        for (a, b) in near.pixels().zip(idle.pixels()) {
            for i in 0..4 {
                let a = if i == 3 {
                    f32::from(a[i])
                } else {
                    f32::from(a[i]) * f32::from(a[3]) / 255.
                };
                let b = if i == 3 {
                    f32::from(b[i])
                } else {
                    f32::from(b[i]) * f32::from(b[3]) / 255.
                };
                assert!((a - b).abs() <= 2., "hair decomposition jumps near neutral");
            }
        }
    }
    #[test]
    fn bikini_visible_layers_and_moving_attachment_seams_stay_covered() {
        let mut a = actor();
        assert!(a.model.fixed_background);
        for point in [[222, 235], [225, 235], [226, 236]] {
            assert_eq!(
                a.foreground.get_pixel(point[0], point[1]),
                a.original.get_pixel(point[0], point[1]),
                "fixed forearm/clothing pixel extracted into a moving part"
            );
        }
        for [x, y] in [[244, 124], [246, 126], [248, 127], [250, 128], [252, 130]] {
            assert_eq!(
                a.foreground.get_pixel(x, y)[3],
                255,
                "complete crown must not have holes"
            );
        }
        for point in [[300, 186], [304, 180], [308, 178], [312, 174], [318, 194]] {
            assert!(
                a.parts[1].image.get_pixel(point[0], point[1])[3] >= 250,
                "warm hair pixel cut out"
            );
        }
        a.model.sway.angle = 0.;
        a.model.sway.dx = 0.;
        a.model.breath.dx_scale = 0.;
        a.model.breath.dy = 0.;
        assert!(trigger(&a.model.actions, &mut a.events, "board", 0.37));
        assert!(trigger(&a.model.actions, &mut a.events, "hat", 1.11));
        for i in 0..31 {
            let time = f64::from(i) / 5.;
            let image = a.render(time);
            // Follow an interior lock pixel rather than probing a fixed world
            // coordinate that the antialiased free contour crosses during idle.
            let hair_point = render::apply(
                a.part_transform(&a.model.parts[1], &a.parameters(time), time),
                [300., 210.],
            );
            assert!(
                image.get_pixel(hair_point[0].round() as u32, hair_point[1].round() as u32)[3]
                    >= 250,
                "moving hair interior opened at t={time}"
            );
            let elbow = render::apply(
                a.bones(&a.parameters(time))[0],
                a.model.rig.as_ref().unwrap().bones[1],
            );
            assert!(
                image.get_pixel(elbow[0].round() as u32, elbow[1].round() as u32)[3] >= 250,
                "moving elbow opened at t={time}"
            );
            for [x, y] in [
                [243, 124],
                [250, 128],
                [258, 134],
                [279, 200],
                [280, 204],
                [281, 208],
                [286, 155],
                [200, 290],
                [280, 279],
            ] {
                assert!(
                    image.get_pixel(x, y)[3] >= 250,
                    "open seam at {x},{y}, t={time}, alpha={}",
                    image.get_pixel(x, y)[3]
                );
            }
        }
    }
    #[test]
    fn fixed_underlay_is_not_erased_by_transparent_moving_polygon_coverage() {
        let mut a = actor();
        a.model.sway.angle = 0.;
        a.model.sway.dx = 0.;
        a.model.breath.dx_scale = 0.;
        a.model.breath.dy = 0.;
        let size = a.original.dimensions();
        a.background = RgbaImage::from_pixel(size.0, size.1, image::Rgba([20, 40, 60, 255]));
        a.original = a.background.clone();
        a.foreground = RgbaImage::new(size.0, size.1);
        for part in &mut a.parts {
            part.image = RgbaImage::new(size.0, size.1);
        }
        let frame = a.render(1.5);
        assert!(
            frame.pixels().all(|p| p[3] == 255),
            "a moving polygon erased the fixed underlay"
        );
        a.model.fixed_background = false;
        a.pose = None;
        assert!(
            a.render(1.5).pixels().any(|p| p[3] == 0),
            "fixture must expose the former polygon-subtraction defect"
        );
    }
    #[test]
    fn complete_arm_keeps_bone_lengths_and_mesh_orientation_across_the_action() {
        let a = actor();
        let rig = a.model.rig.as_ref().unwrap();
        let distance = |a: [f32; 2], b: [f32; 2]| (a[0] - b[0]).hypot(a[1] - b[1]);
        for step in 0..=60 {
            let mut values = a.parameters(0.);
            values.insert("hat_angle".into(), step as f32 / 10.);
            let bones = a.bones(&values);
            let s = render::apply(bones[0], rig.bones[0]);
            let e = render::apply(bones[0], rig.bones[1]);
            let lower_e = render::apply(bones[1], rig.bones[1]);
            let w = render::apply(bones[1], rig.bones[2]);
            assert!(distance(s, rig.bones[0]) < 0.001);
            assert!(distance(e, lower_e) < 0.001);
            assert!((distance(s, e) - distance(rig.bones[0], rig.bones[1])).abs() < 0.001);
            assert!((distance(e, w) - distance(rig.bones[1], rig.bones[2])).abs() < 0.001);
            for t in [1.5, 4.5] {
                for part in &a.model.parts {
                    if let Some(mesh) = &part.mesh {
                        assert!(
                            mesh.pose(a.part_transform(part, &values, t), bones)
                                .minimum_area_ratio()
                                > 0.5
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn rigid_hair_preserves_distances_and_fixed_stack_never_restores_source_fringe() {
        let mut a = actor();
        let hair = &a.model.parts[1];
        assert!(
            hair.mesh.is_none(),
            "free locks must not be squeezed by blend weights"
        );
        for time in [0., 0.75, 1.5, 3.75, 4.5] {
            let matrix = a.part_transform(hair, &a.parameters(time), time);
            let p = render::apply(matrix, [300., 190.]);
            let q = render::apply(matrix, [350., 230.]);
            assert!(((p[0] - q[0]).hypot(p[1] - q[1]) - 50_f32.hypot(40.)).abs() < 0.001);
        }
        a.original = RgbaImage::from_pixel(512, 512, image::Rgba([255, 0, 255, 255]));
        assert_eq!(
            a.render(0.).get_pixel(10, 450)[3],
            0,
            "fixed stack restored source pixels outside moving-part bounds"
        );
    }
    #[test]
    fn bikini_crown_depth_and_area_filtered_edges_do_not_restore_authoring_defects() {
        let a = actor();
        assert!(
            a.parts[1]
                .image
                .rows()
                .take(160)
                .flatten()
                .all(|p| p[3] == 0),
            "back-hair layer contains a second generated scalp"
        );
        assert!(
            a.foreground.get_pixel(260, 123)[3] >= 240,
            "hidden crown was clipped back to the neutral cap contact"
        );
        for [x, y] in [[279, 208], [278, 265]] {
            assert_eq!(
                a.foreground.get_pixel(x, y)[3],
                0,
                "generated breast/waist underplate protrudes beyond the source contour"
            );
        }
        let pixel = a.parts[1].image.get_pixel(291, 180);
        assert!(
            !(i16::from(pixel[0]) > i16::from(pixel[1]) + 12
                && i16::from(pixel[0]) > i16::from(pixel[2]) + 24),
            "old upper-arm skin was restored into the moving hair"
        );
        assert_eq!(
            a.parts[2].front_mask.as_ref().unwrap().get_pixel(239, 118)[0],
            255,
            "cap contact must be in front of the hidden crown overlap"
        );
        assert_eq!(
            a.original.get_pixel(264, 340)[3],
            0,
            "negative-lobe resizing restored a detached outer alpha ring"
        );
        for y in 330..512 {
            for x in 0..512 {
                assert_eq!(a.foreground.get_pixel(x, y), a.original.get_pixel(x, y));
            }
        }
    }
    #[test]
    fn static_roots_occlude_rear_brim_in_rendering_and_picking() {
        let mut a = actor();
        a.model.sway.angle = 0.;
        a.model.sway.dx = 0.;
        a.background = RgbaImage::from_pixel(512, 512, image::Rgba([160, 145, 170, 255]));
        a.foreground = RgbaImage::new(512, 512);
        for (i, part) in a.parts.iter_mut().enumerate() {
            if i != 2 {
                part.image = RgbaImage::new(512, 512);
            }
        }
        a.parts[2].front_mask.as_mut().unwrap().fill(0);
        assert_eq!(a.render(0.), a.background);
        assert!(
            !a.click([225., 110.], 0.),
            "rear brim must not receive clicks through static roots"
        );
        a.parts[2].front_mask.as_mut().unwrap().fill(255);
        a.pose = None;
        assert_ne!(a.render(0.), a.background);
        assert!(
            a.click([225., 110.], 0.),
            "front brim remains visible and interactive"
        );
    }
    #[test]
    fn cap_lift_keeps_the_temple_covered_and_does_not_carry_a_hair_fringe() {
        let mut a = actor();
        let cap = a.parts[2].image.get_pixel(260, 123);
        assert!(
            i16::from(cap[0]) > i16::from(cap[2]) + 30,
            "gray source hair was restored onto the moving cap contact"
        );
        for value in [0., 3., 6.] {
            a.model.channels.get_mut("hat_angle").unwrap().default = value;
            let frame = a.render(0.);
            for [x, y] in [[283, 143], [286, 145], [289, 147], [291, 150]] {
                assert!(
                    frame.get_pixel(x, y)[3] >= 250,
                    "temple opened at cap control {value}, point {x},{y}"
                );
            }
        }
    }
    #[test]
    fn hat_depth_orders_preserve_alpha_instead_of_splitting_the_texture() {
        let mut back = actor();
        let mut front = actor();
        back.parts[2].front_mask.as_mut().unwrap().fill(0);
        front.parts[2].front_mask.as_mut().unwrap().fill(255);
        let back = back.render(0.);
        let front = front.render(0.);
        assert_ne!(
            back, front,
            "fixture must include actual hat/body occlusion"
        );
        for (a, b) in back.pixels().zip(front.pixels()) {
            assert!(
                a[3].abs_diff(b[3]) <= 1,
                "changing depth punched an alpha seam"
            );
        }
    }
    #[test]
    fn original_face_and_torso_survive_hat_lift_and_hidden_hat_does_not_receive_clicks() {
        for value in [0., 3., 6.] {
            let mut a = actor();
            a.model.channels.get_mut("hat_angle").unwrap().default = value;
            let frame = a.render(0.);
            for (left, top, right, bottom) in [(235, 145, 262, 170), (236, 240, 264, 270)] {
                for y in top..bottom {
                    for x in left..right {
                        if a.original.get_pixel(x, y)[3] == 255 {
                            assert_eq!(frame.get_pixel(x, y), a.original.get_pixel(x, y));
                        }
                    }
                }
            }
        }
        let mut a = actor();
        assert!(
            !a.click([285., 155.], 0.),
            "hair must occlude the complete hidden brim"
        );
        assert!(a.click([225., 110.], 0.), "visible hat remains interactive");
    }
    #[test]
    #[ignore = "native layered pose snapshots from published resources"]
    fn bikini_v2_review_frames() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("resource/illustration/lexington/live2d/BiKiNi_Normal/actor.json");
        let output = Path::new(env!("CARGO_MANIFEST_DIR")).join(".tools/bikini-v2-review");
        std::fs::create_dir_all(&output).unwrap();
        for (name, value) in [("neutral", 0.), ("middle", 3.), ("extreme", 6.)] {
            let mut a = LayeredActor::load(&path, &std::fs::read(&path).unwrap()).unwrap();
            a.model.channels.get_mut("hat_angle").unwrap().default = value;
            let values = a.parameters(0.);
            let bones = a.bones(&values);
            for part in &a.model.parts {
                if let Some(mesh) = &part.mesh {
                    let matrix = a.part_transform(part, &values, 0.);
                    let ratio = mesh.pose(matrix, bones).minimum_area_ratio();
                    assert!(ratio > 0.5, "folded or collapsed mesh: {ratio}");
                }
            }
            a.render(0.)
                .save(output.join(format!("{name}.png")))
                .unwrap();
        }
    }
    #[test]
    #[ignore = "release continuous seam audit, no user configuration"]
    fn bikini_continuous_seam_frames() {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join(".tools/bikini-seams");
        std::fs::create_dir_all(&directory).unwrap();
        let mut a = actor();
        assert!(trigger(&a.model.actions, &mut a.events, "board", 0.37));
        assert!(trigger(&a.model.actions, &mut a.events, "hat", 1.11));
        for i in 0..181 {
            let time = f64::from(i) / 30.;
            a.render(time)
                .save(directory.join(format!("{i:03}.png")))
                .unwrap();
        }
    }
    #[test]
    fn bikini_loading_rejects_bad_bindings_geometry_and_paths() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("resource/illustration/lexington/live2d/BiKiNi_Normal/actor.json");
        let original: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        for (field, value) in [
            ("canvas", serde_json::json!([0, 512])),
            ("fixed_background", serde_json::json!(false)),
            ("parts", serde_json::json!([])),
            ("foreground", serde_json::json!("../static.png")),
        ] {
            let mut m = original.clone();
            m[field] = value;
            assert!(LayeredActor::load(&path, &serde_json::to_vec(&m).unwrap()).is_err());
        }
        let mut m = original.clone();
        m["parts"][0]["channel"] = serde_json::json!("missing");
        assert!(LayeredActor::load(&path, &serde_json::to_vec(&m).unwrap()).is_err());
        for (key, value) in [
            ("period", serde_json::json!(0)),
            ("angle", serde_json::json!(30)),
        ] {
            let mut m = original.clone();
            m["parts"][1]["idle"][key] = value;
            assert!(LayeredActor::load(&path, &serde_json::to_vec(&m).unwrap()).is_err());
        }
        let mut m = original.clone();
        m["parts"][1]["fill_background"] = serde_json::json!(true);
        assert!(LayeredActor::load(&path, &serde_json::to_vec(&m).unwrap()).is_err());
        for (pointer, value) in [
            ("/rig/pole", serde_json::json!(-1)),
            ("/rig", serde_json::Value::Null),
            ("/parts/3/mesh/vertices/0/2", serde_json::json!(2)),
            ("/parts/3/mesh/indices/0", serde_json::json!(99999)),
        ] {
            let mut m = original.clone();
            *m.pointer_mut(pointer).unwrap() = value;
            assert!(LayeredActor::load(&path, &serde_json::to_vec(&m).unwrap()).is_err());
        }
        let mut m = original;
        m["actions"]["hat"]["tracks"]["hat_angle"][1][1] = serde_json::json!(999);
        assert!(LayeredActor::load(&path, &serde_json::to_vec(&m).unwrap()).is_err());
    }
    #[test]
    #[ignore = "release native snapshots for visual comparison"]
    fn bikini_native_reference_frames() {
        let output = Path::new(env!("CARGO_MANIFEST_DIR")).join(".tools/bikini-native");
        std::fs::create_dir_all(&output).unwrap();
        for (name, t, board, hat) in [
            ("neutral", 0., false, false),
            ("idle", 1.6, false, false),
            ("board", 1.6, true, false),
            ("hat", 1.6, false, true),
            ("mixed", 1.6, true, true),
            ("hair_positive", 1.5, false, false),
            ("hair_negative", 4.5, false, false),
        ] {
            let mut a = actor();
            if board {
                assert!(trigger(&a.model.actions, &mut a.events, "board", 0.7));
            }
            if hat {
                assert!(trigger(&a.model.actions, &mut a.events, "hat", 0.7));
            }
            let start = std::time::Instant::now();
            a.render(t)
                .save(output.join(format!("{name}.png")))
                .unwrap();
            println!("{name}: {:?}", start.elapsed());
        }
    }
}
