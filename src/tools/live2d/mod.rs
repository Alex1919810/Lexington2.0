//! Native unified 2D actors: shared idle clock, local tracks and moving hit regions.
mod layered;
mod mesh;
mod render;
use image::RgbaImage;
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[derive(Deserialize)]
struct Model {
    format: String,
    canvas: [u32; 2],
    original: String,
    plate: String,
    layers: Vec<LayerSpec>,
    restore_masks: Vec<String>,
    bones: Bones,
    head_until: u32,
    breath_roi: [u32; 4],
    breath_fields: [String; 2],
    hair_roi: [u32; 4],
    hair_mask: String,
    hair_occlusion: String,
    hair_front: String,
    hair_body_occlusion: [u32; 3],
    shared_idle: Idle,
    root_pivot: [f32; 2],
    channels: BTreeMap<String, Channel>,
    actions: BTreeMap<String, Action>,
    hit_regions: Vec<Region>,
    eyes: Eyes,
}
#[derive(Deserialize)]
struct LayerSpec {
    path: String,
    origin: [u32; 2],
}
#[derive(Deserialize)]
struct Bones {
    left: [[f32; 2]; 3],
    right: [[f32; 2]; 3],
    poles: [f32; 2],
}
#[derive(Deserialize)]
struct Eyes {
    atlas: String,
    mask: String,
    cell: [u32; 2],
    columns: u32,
    states: u32,
    origin: [u32; 2],
}
#[derive(Deserialize)]
struct Idle {
    breathing: Breathing,
    sway: Sway,
    hair: Hair,
}
#[derive(Deserialize)]
struct Breathing {
    period: f64,
}
#[derive(Deserialize)]
struct Sway {
    period: f64,
    rotation_clockwise_degrees_per_wave: f32,
    translation_x_px_per_wave: f32,
}
#[derive(Deserialize)]
struct Hair {
    period: f64,
    pivot: [f32; 2],
    primary_angle_clockwise_degrees: f32,
    secondary_angle_clockwise_degrees: f32,
}
#[derive(Deserialize)]
struct Channel {
    default: f32,
    range: [f32; 2],
}
#[derive(Deserialize)]
struct Action {
    duration: f64,
    tracks: BTreeMap<String, Vec<(f64, f32, String)>>,
}
#[derive(Deserialize)]
struct Region {
    shape: String,
    #[serde(default)]
    center: [f32; 2],
    #[serde(default)]
    radii: [f32; 2],
    #[serde(default)]
    points: Vec<[f32; 2]>,
    action: String,
    #[serde(default)]
    local_transform: String,
}
struct Event {
    action: String,
    start: f64,
}
pub(super) struct LegacyActor {
    model: Model,
    renderer: render::Renderer,
    events: Vec<Event>,
}

fn safe_path(root: &Path, name: &str) -> Result<PathBuf, String> {
    let relative = Path::new(name);
    if relative
        .components()
        .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err("Live2D 资源路径无效。".into());
    }
    let path = root
        .join(relative)
        .canonicalize()
        .map_err(|_| "Live2D 资源缺失。")?;
    if !path.starts_with(root) {
        return Err("Live2D 资源超出模型目录。".into());
    }
    Ok(path)
}
fn wave(time: f64, period: f64) -> f32 {
    let value = (time * std::f64::consts::TAU / period).sin().powi(3) as f32;
    if value.abs() < 0.0001 { 0. } else { value }
}
fn ease(kind: &str, u: f32) -> f32 {
    let q = u * u * u * (10. + u * (-15. + 6. * u));
    match kind {
        "smooth" => u * u * (3. - 2. * u),
        "quintic_squared" => q * q,
        "quintic_complement_squared" => 1. - (1. - q).powi(2),
        _ => u,
    }
}
enum ActorKind {
    Legacy(Box<LegacyActor>),
    Layered(Box<layered::LayeredActor>),
}
pub struct Actor(ActorKind);
impl Actor {
    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|_| "无法读取 Live2D 模型。")?;
        if bytes.len() > 65536 {
            return Err("Live2D 模型描述过大。".into());
        }
        let header: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|_| "Live2D 模型格式错误。")?;
        Ok(Self(if header["format"] == "lexington-layered-actor-1" {
            ActorKind::Layered(Box::new(layered::LayeredActor::load(path, &bytes)?))
        } else {
            ActorKind::Legacy(Box::new(LegacyActor::load(path)?))
        }))
    }
    pub fn render(&mut self, time: f64) -> RgbaImage {
        match &mut self.0 {
            ActorKind::Legacy(a) => a.render(time),
            ActorKind::Layered(a) => a.render(time),
        }
    }
    pub fn click(&mut self, point: [f32; 2], time: f64) -> bool {
        match &mut self.0 {
            ActorKind::Legacy(a) => a.click(point, time),
            ActorKind::Layered(a) => a.click(point, time),
        }
    }
    #[cfg(test)]
    pub fn parameters(&self, time: f64) -> BTreeMap<String, f32> {
        match &self.0 {
            ActorKind::Legacy(a) => a.parameters(time),
            ActorKind::Layered(a) => a.parameters(time),
        }
    }
}
fn parameters(
    channels: &BTreeMap<String, Channel>,
    actions: &BTreeMap<String, Action>,
    events: &[Event],
    time: f64,
) -> BTreeMap<String, f32> {
    let mut values = channels
        .iter()
        .map(|(k, v)| (k.clone(), v.default))
        .collect::<BTreeMap<_, _>>();
    for event in events {
        let action = &actions[&event.action];
        let local = time - event.start;
        if local < 0. || local >= action.duration {
            continue;
        }
        for (channel, track) in &action.tracks {
            for pair in track.windows(2) {
                let (a, b) = (&pair[0], &pair[1]);
                if local >= a.0 && local <= b.0 {
                    let u = ((local - a.0) / (b.0 - a.0)) as f32;
                    values.insert(channel.clone(), a.1 + (b.1 - a.1) * ease(&a.2, u));
                    break;
                }
            }
        }
    }
    values
}
fn trigger(
    actions: &BTreeMap<String, Action>,
    events: &mut Vec<Event>,
    action: &str,
    time: f64,
) -> bool {
    events.retain(|e| e.start + actions[&e.action].duration > time);
    let Some(next) = actions.get(action) else {
        return false;
    };
    let conflicting = events
        .iter()
        .filter(|e| {
            actions[&e.action]
                .tracks
                .keys()
                .any(|k| next.tracks.contains_key(k))
        })
        .collect::<Vec<_>>();
    if conflicting.iter().any(|e| e.start > time) {
        return false;
    }
    let start = conflicting
        .iter()
        .map(|e| e.start + actions[&e.action].duration)
        .fold(time, f64::max);
    events.push(Event {
        action: action.into(),
        start,
    });
    true
}
fn validate_motion(
    channels: &BTreeMap<String, Channel>,
    actions: &BTreeMap<String, Action>,
) -> Result<(), String> {
    if channels.is_empty() || channels.len() > 16 || actions.len() > 32 {
        return Err("Live2D 通道数量无效。".into());
    }
    for (name, channel) in channels {
        if name.len() > 80
            || !channel.default.is_finite()
            || channel.range.iter().any(|v| !v.is_finite())
            || channel.range[0] > channel.default
            || channel.range[1] < channel.default
        {
            return Err("Live2D 通道无效。".into());
        }
    }
    for action in actions.values() {
        if !action.duration.is_finite()
            || action.duration <= 0.
            || action.duration > 60.
            || action.tracks.is_empty()
        {
            return Err("Live2D 动作无效。".into());
        }
        for (name, track) in &action.tracks {
            let channel = channels.get(name).ok_or("Live2D 动作通道缺失。")?;
            if track.len() < 2
                || track.len() > 64
                || track[0].0 != 0.
                || track.last().unwrap().0 != action.duration
                || track[0].1 != channel.default
                || track.last().unwrap().1 != channel.default
                || track.windows(2).any(|p| p[0].0 >= p[1].0)
                || track.iter().any(|(t, v, e)| {
                    !t.is_finite()
                        || !v.is_finite()
                        || *v < channel.range[0]
                        || *v > channel.range[1]
                        || !matches!(
                            e.as_str(),
                            "linear" | "smooth" | "quintic_squared" | "quintic_complement_squared"
                        )
                })
            {
                return Err("Live2D 轨道无效。".into());
            }
        }
    }
    Ok(())
}
impl LegacyActor {
    pub fn load(path: &Path) -> Result<Self, String> {
        let root = path
            .parent()
            .ok_or("Live2D 路径无效。")?
            .canonicalize()
            .map_err(|_| "Live2D 目录无效。")?;
        let bytes = std::fs::read(path).map_err(|_| "无法读取 Live2D 模型。")?;
        if bytes.len() > 65536 {
            return Err("Live2D 模型描述过大。".into());
        }
        let model: Model = serde_json::from_slice(&bytes).map_err(|_| "Live2D 模型格式错误。")?;
        model.validate()?;
        let renderer = render::Renderer::load(&root, &model)?;
        Ok(Self {
            model,
            renderer,
            events: vec![],
        })
    }
    pub fn parameters(&self, time: f64) -> BTreeMap<String, f32> {
        parameters(
            &self.model.channels,
            &self.model.actions,
            &self.events,
            time,
        )
    }
    pub fn trigger(&mut self, action: &str, time: f64) -> bool {
        trigger(&self.model.actions, &mut self.events, action, time)
    }
    pub fn click(&mut self, point: [f32; 2], time: f64) -> bool {
        let point = render::inverse(render::root(&self.model, time), point);
        let values = self.parameters(time);
        let lift = values.get("bouquet_lift_px").copied().unwrap_or(0.);
        let action = self
            .model
            .hit_regions
            .iter()
            .rev()
            .find(|region| {
                let p = [
                    point[0],
                    point[1]
                        + if region.local_transform == "translate(0,-bouquet_lift_px)" {
                            lift
                        } else {
                            0.
                        },
                ];
                if region.shape == "ellipse" {
                    ((p[0] - region.center[0]) / region.radii[0]).powi(2)
                        + ((p[1] - region.center[1]) / region.radii[1]).powi(2)
                        <= 1.
                } else {
                    let mut inside = false;
                    for (a, b) in region
                        .points
                        .iter()
                        .zip(region.points.iter().cycle().skip(1))
                        .take(region.points.len())
                    {
                        if (a[1] > p[1]) != (b[1] > p[1])
                            && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]
                        {
                            inside = !inside;
                        }
                    }
                    inside
                }
            })
            .map(|r| r.action.clone());
        action.is_some_and(|a| self.trigger(&a, time))
    }
    pub fn render(&mut self, time: f64) -> RgbaImage {
        self.events
            .retain(|e| e.start + self.model.actions[&e.action].duration > time);
        let values = self.parameters(time);
        self.renderer.frame(&self.model, time, &values)
    }
}
impl Model {
    fn validate(&self) -> Result<(), String> {
        let valid = self.format == "lexington-native-actor-1"
            && self.canvas.iter().all(|&v| v > 0 && v <= 2048)
            && self.layers.len() == 5
            && self.restore_masks.len() == 2
            && self.head_until <= self.canvas[1]
            && [
                self.shared_idle.breathing.period,
                self.shared_idle.sway.period,
                self.shared_idle.hair.period,
            ]
            .iter()
            .all(|v| v.is_finite() && *v > 0.)
            && self.channels.len() <= 16
            && self.actions.len() <= 32
            && self.hit_regions.len() <= 32;
        let roi = |r: [u32; 4]| {
            r[0] < r[2] && r[1] < r[3] && r[2] <= self.canvas[0] && r[3] <= self.canvas[1]
        };
        if !valid
            || !roi(self.breath_roi)
            || !roi(self.hair_roi)
            || self.eyes.columns == 0
            || self.eyes.columns > 64
            || self.eyes.states < 2
            || self.eyes.states > 128
            || self.eyes.cell.contains(&0)
            || self.eyes.origin[0]
                .checked_add(self.eyes.cell[0])
                .is_none_or(|v| v > self.canvas[0])
            || self.eyes.origin[1]
                .checked_add(self.eyes.cell[1])
                .is_none_or(|v| v > self.canvas[1])
            || self.canvas[0] * self.canvas[1] > 1024 * 1024
        {
            return Err("Live2D 模型参数无效。".into());
        }
        validate_motion(&self.channels, &self.actions)?;
        for region in &self.hit_regions {
            if !self.actions.contains_key(&region.action)
                || !matches!(
                    region.local_transform.as_str(),
                    "" | "translate(0,-bouquet_lift_px)"
                )
                || (region.shape == "ellipse" && region.radii.iter().any(|r| *r <= 0.))
                || (region.shape == "polygon" && !(3..=64).contains(&region.points.len()))
                || !matches!(region.shape.as_str(), "ellipse" | "polygon")
            {
                return Err("Live2D 热区无效。".into());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn actor() -> LegacyActor {
        LegacyActor::load(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("resource/illustration/lexington/live2d/WeddingDress_Normal/actor.json"),
        )
        .unwrap()
    }
    #[test]
    fn local_actions_queue_independently_and_hits_follow_current_root_and_bouquet() {
        let mut actor = actor();
        let start = 1.37;
        let head = actor.model.hit_regions[0].center;
        let m = render::root(&actor.model, start);
        let point = [
            m[0] * head[0] + m[1] * head[1] + m[2],
            m[3] * head[0] + m[4] * head[1] + m[5],
        ];
        assert!(actor.click(point, start));
        assert!(actor.trigger("bouquet", start));
        let values = actor.parameters(start + 0.3);
        assert!(values["eye_openness"] < 0.001);
        assert!(values["bouquet_lift_px"] > 0.);
        assert!(actor.trigger("blink", start + 0.3));
        assert!(!actor.trigger("blink", start + 0.31));
        assert_eq!(actor.events.len(), 3);
        let time = start + 1.6;
        let m = render::root(&actor.model, time);
        let p = [237., 282.]; // In the lifted bouquet, above its neutral polygon.
        let point = [
            m[0] * p[0] + m[1] * p[1] + m[2],
            m[3] * p[0] + m[4] * p[1] + m[5],
        ];
        assert!(actor.click(point, time));
        assert!(!actor.click([5., 5.], time));
        assert_eq!(actor.parameters(start + 8.)["eye_openness"], 1.);
        assert_eq!(actor.parameters(start + 8.)["bouquet_lift_px"], 0.);
    }
    #[test]
    fn model_paths_and_invalid_tracks_are_rejected() {
        let root = tempfile::tempdir().unwrap();
        assert!(safe_path(root.path(), "../outside").is_err());
        let mut actor = actor();
        actor
            .model
            .actions
            .get_mut("blink")
            .unwrap()
            .tracks
            .get_mut("eye_openness")
            .unwrap()[1]
            .1 = 5.;
        assert!(actor.model.validate().is_err());
    }
    #[test]
    #[ignore = "release-only native pixel snapshots and timing, no user data"]
    fn live2d_reference_frames() {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join(".tools/live2d-verify");
        std::fs::create_dir_all(&directory).unwrap();
        let mut actor = actor();
        for (name, time, lift, eye) in [
            ("neutral", 0., 0., 1.),
            ("idle", 1.37, 0., 1.),
            ("peak", 1.37, 10., 1.),
            ("blink", 4.5, 0., 0.),
            ("mixed", 2.17, 6., 0.4),
        ] {
            let values = BTreeMap::from([
                ("bouquet_lift_px".into(), lift),
                ("eye_openness".into(), eye),
            ]);
            let start = std::time::Instant::now();
            let frame = actor.renderer.frame(&actor.model, time, &values);
            println!("{name}: {:?}", start.elapsed());
            frame.save(directory.join(format!("{name}.png"))).unwrap();
        }
    }
}
