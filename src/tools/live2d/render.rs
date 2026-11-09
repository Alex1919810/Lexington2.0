use super::{Model, safe_path, wave};
use image::{Rgba, RgbaImage};
use std::{collections::BTreeMap, path::Path};
type Matrix = [f32; 6];
struct Layer {
    image: RgbaImage,
    origin: [u32; 2],
}
struct Pose {
    lift: f32,
    image: RgbaImage,
    protected: Vec<f32>,
    factor: Vec<f32>,
}
pub(super) struct Renderer {
    original: RgbaImage,
    plate: RgbaImage,
    layers: Vec<Layer>,
    masks: Vec<Vec<f32>>,
    fields: [Vec<f32>; 2],
    hair_mask: Vec<f32>,
    hair_front: Vec<f32>,
    hair_occlusion: Vec<f32>,
    atlas: RgbaImage,
    eye_mask: Vec<f32>,
    pose: Option<Pose>,
}
pub(super) fn rgba(root: &Path, name: &str) -> Result<RgbaImage, String> {
    let path = safe_path(root, name)?;
    let (w, h) = image::image_dimensions(&path).map_err(|_| "Live2D 图片无效。")?;
    if w == 0 || h == 0 || w > 4096 || h > 4096 {
        return Err("Live2D 图片尺寸无效。".into());
    }
    image::open(path)
        .map(|im| im.to_rgba8())
        .map_err(|_| "无法解码 Live2D 图片。".into())
}
fn mask(root: &Path, name: &str, size: [u32; 2]) -> Result<Vec<f32>, String> {
    let path = safe_path(root, name)?;
    if image::image_dimensions(&path).map_err(|_| "Live2D 遮罩无效。")? != (size[0], size[1]) {
        return Err("Live2D 遮罩尺寸不匹配。".into());
    }
    Ok(image::open(path)
        .map_err(|_| "Live2D 遮罩无法解码。")?
        .to_luma16()
        .pixels()
        .map(|p| f32::from(p[0]) / 65535.)
        .collect())
}
fn floats(root: &Path, name: &str, len: usize) -> Result<Vec<f32>, String> {
    let path = safe_path(root, name)?;
    if std::fs::metadata(&path)
        .map_err(|_| "Live2D 数值资源缺失。")?
        .len()
        != len as u64 * 4
    {
        return Err("Live2D 数值资源尺寸错误。".into());
    }
    let data = std::fs::read(path).map_err(|_| "Live2D 数值资源读取失败。")?;
    let values = data
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes(*b))
        .collect::<Vec<_>>();
    if values.iter().any(|v| !v.is_finite() || v.abs() > 100.) {
        return Err("Live2D 数值资源无效。".into());
    }
    Ok(values)
}
pub(super) fn packed(p: &Rgba<u8>) -> [f32; 4] {
    let a = f32::from(p[3]) / 255.;
    [
        f32::from(p[0]) / 255. * a,
        f32::from(p[1]) / 255. * a,
        f32::from(p[2]) / 255. * a,
        a,
    ]
}
pub(super) fn straight(p: [f32; 4]) -> Rgba<u8> {
    let a = p[3].clamp(0., 1.);
    if (a * 255.).round() == 0. {
        return Rgba([0; 4]);
    }
    Rgba([
        if a > 1e-7 {
            (p[0] / a * 255.).round().clamp(0., 255.) as u8
        } else {
            0
        },
        if a > 1e-7 {
            (p[1] / a * 255.).round().clamp(0., 255.) as u8
        } else {
            0
        },
        if a > 1e-7 {
            (p[2] / a * 255.).round().clamp(0., 255.) as u8
        } else {
            0
        },
        (a * 255.).round() as u8,
    ])
}
fn weights(x: f32, y: f32) -> [(i32, i32, f32); 4] {
    let x = (x * 32.).round() / 32.;
    let y = (y * 32.).round() / 32.;
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let a = x - ix as f32;
    let b = y - iy as f32;
    [
        (ix, iy, (1. - a) * (1. - b)),
        (ix + 1, iy, a * (1. - b)),
        (ix, iy + 1, (1. - a) * b),
        (ix + 1, iy + 1, a * b),
    ]
}
pub(super) fn sample(image: &RgbaImage, x: f32, y: f32, clamp: bool) -> [f32; 4] {
    let mut out = [0.; 4];
    for (mut x, mut y, w) in weights(x, y) {
        if clamp {
            x = x.clamp(0, image.width() as i32 - 1);
            y = y.clamp(0, image.height() as i32 - 1);
        }
        if x >= 0 && y >= 0 && x < image.width() as i32 && y < image.height() as i32 {
            let p = packed(image.get_pixel(x as u32, y as u32));
            for c in 0..4 {
                out[c] += p[c] * w;
            }
        }
    }
    out
}
fn scalar(data: &[f32], w: u32, h: u32, x: f32, y: f32) -> f32 {
    weights(x, y)
        .iter()
        .map(|&(x, y, a)| {
            data[(y.clamp(0, h as i32 - 1) as u32 * w + x.clamp(0, w as i32 - 1) as u32) as usize]
                * a
        })
        .sum()
}
pub(super) fn apply(m: Matrix, p: [f32; 2]) -> [f32; 2] {
    [
        m[0] * p[0] + m[1] * p[1] + m[2],
        m[3] * p[0] + m[4] * p[1] + m[5],
    ]
}
pub(super) fn inverse(m: Matrix, p: [f32; 2]) -> [f32; 2] {
    let p = [p[0] - m[2], p[1] - m[5]];
    [m[0] * p[0] + m[3] * p[1], m[1] * p[0] + m[4] * p[1]]
}
pub(super) fn rotation(p: [f32; 2], angle: f32, dx: f32) -> Matrix {
    let (s, c) = angle.to_radians().sin_cos();
    [
        c,
        -s,
        p[0] - c * p[0] + s * p[1] + dx,
        s,
        c,
        p[1] - s * p[0] - c * p[1],
    ]
}
pub(super) fn root(m: &Model, time: f64) -> Matrix {
    let v = wave(time, m.shared_idle.sway.period);
    rotation(
        m.root_pivot,
        m.shared_idle.sway.rotation_clockwise_degrees_per_wave * v,
        m.shared_idle.sway.translation_x_px_per_wave * v,
    )
}
pub(super) fn segment(a: [f32; 2], b: [f32; 2], c: [f32; 2], d: [f32; 2]) -> Matrix {
    let angle = (d[1] - c[1]).atan2(d[0] - c[0]) - (b[1] - a[1]).atan2(b[0] - a[0]);
    let (s, co) = angle.sin_cos();
    [
        co,
        -s,
        c[0] - co * a[0] + s * a[1],
        s,
        co,
        c[1] - s * a[0] - co * a[1],
    ]
}
pub(super) fn elbow(b: [[f32; 2]; 3], target: [f32; 2], pole: f32) -> [f32; 2] {
    let len = |a: [f32; 2], b: [f32; 2]| {
        (((a[0] - b[0]) as f64).powi(2) + ((a[1] - b[1]) as f64).powi(2)).sqrt()
    };
    let l1 = len(b[0], b[1]);
    let l2 = len(b[1], b[2]);
    let distance = len(target, b[0]).max(1e-6);
    let u = [
        (target[0] - b[0][0]) as f64 / distance,
        (target[1] - b[0][1]) as f64 / distance,
    ];
    let p = (l1 * l1 - l2 * l2 + distance * distance) / (2. * distance);
    let h = (l1 * l1 - p * p).max(0.).sqrt() * f64::from(pole);
    [
        (f64::from(b[0][0]) + p * u[0] - h * u[1]) as f32,
        (f64::from(b[0][1]) + p * u[1] + h * u[0]) as f32,
    ]
}
impl Renderer {
    pub(super) fn load(root: &Path, m: &Model) -> Result<Self, String> {
        let original = rgba(root, &m.original)?;
        let plate = rgba(root, &m.plate)?;
        if original.dimensions() != (m.canvas[0], m.canvas[1])
            || plate.dimensions() != original.dimensions()
        {
            return Err("Live2D 画布尺寸不匹配。".into());
        }
        let mut layers = vec![];
        for spec in &m.layers {
            let image = rgba(root, &spec.path)?;
            if spec.origin[0]
                .checked_add(image.width())
                .is_none_or(|v| v > m.canvas[0])
                || spec.origin[1]
                    .checked_add(image.height())
                    .is_none_or(|v| v > m.canvas[1])
            {
                return Err("Live2D 图层超出画布。".into());
            }
            layers.push(Layer {
                image,
                origin: spec.origin,
            });
        }
        let masks = m
            .restore_masks
            .iter()
            .map(|s| mask(root, s, m.canvas))
            .collect::<Result<Vec<_>, _>>()?;
        let [x, y, r, b] = m.breath_roi;
        let count = ((r - x) * (b - y)) as usize;
        let fields = [
            floats(root, &m.breath_fields[0], count)?,
            floats(root, &m.breath_fields[1], count)?,
        ];
        let [x, y, r, b] = m.hair_roi;
        let size = [r - x, b - y];
        let atlas = rgba(root, &m.eyes.atlas)?;
        if atlas.width() < m.eyes.columns * m.eyes.cell[0]
            || atlas.height() < m.eyes.states.div_ceil(m.eyes.columns) * m.eyes.cell[1]
        {
            return Err("Live2D 眼部图集尺寸无效。".into());
        }
        Ok(Self {
            original,
            plate,
            layers,
            masks,
            fields,
            hair_mask: mask(root, &m.hair_mask, size)?,
            hair_occlusion: mask(root, &m.hair_occlusion, size)?,
            hair_front: floats(root, &m.hair_front, (size[0] * size[1] * 4) as usize)?,
            eye_mask: mask(root, &m.eyes.mask, m.eyes.cell)?,
            atlas,
            pose: None,
        })
    }
    fn draw(image: &mut RgbaImage, layer: &Layer, m: Matrix, protected: &mut [f32]) {
        let corners = [
            [0., 0.],
            [layer.image.width() as f32, 0.],
            [0., layer.image.height() as f32],
            [layer.image.width() as f32, layer.image.height() as f32],
        ]
        .map(|p| {
            apply(
                m,
                [p[0] + layer.origin[0] as f32, p[1] + layer.origin[1] as f32],
            )
        });
        let left = corners
            .iter()
            .map(|p| p[0])
            .fold(f32::INFINITY, f32::min)
            .floor()
            .max(0.) as u32;
        let top = corners
            .iter()
            .map(|p| p[1])
            .fold(f32::INFINITY, f32::min)
            .floor()
            .max(0.) as u32;
        let right = (corners.iter().map(|p| p[0]).fold(0., f32::max).ceil() + 1.)
            .min(image.width() as f32) as u32;
        let bottom = (corners.iter().map(|p| p[1]).fold(0., f32::max).ceil() + 1.)
            .min(image.height() as f32) as u32;
        for y in top..bottom {
            for x in left..right {
                let p = inverse(m, [x as f32, y as f32]);
                let f = packed(&straight(sample(
                    &layer.image,
                    p[0] - layer.origin[0] as f32,
                    p[1] - layer.origin[1] as f32,
                    false,
                )));
                let index = (y * image.width() + x) as usize;
                protected[index] = protected[index].max(f[3]);
                if f[3] > 0. {
                    let b = packed(image.get_pixel(x, y));
                    image.put_pixel(
                        x,
                        y,
                        straight(std::array::from_fn(|c| f[c] + b[c] * (1. - f[3]))),
                    );
                }
            }
        }
    }
    fn pose(&mut self, m: &Model, lift: f32) {
        if self.pose.as_ref().is_some_and(|p| p.lift == lift) {
            return;
        }
        let l = m.bones.left;
        let r = m.bones.right;
        let wl = [l[2][0], l[2][1] - lift];
        let wr = [r[2][0], r[2][1] - lift];
        let el = elbow(l, wl, m.bones.poles[0]);
        let er = elbow(r, wr, m.bones.poles[1]);
        let transforms = [
            segment(l[0], l[1], l[0], el),
            segment(r[0], r[1], r[0], er),
            segment(l[1], l[2], el, wl),
            segment(r[1], r[2], er, wr),
            [1., 0., 0., 0., 1., -lift],
        ];
        let mut image = self.plate.clone();
        let mut protected = vec![0.; (m.canvas[0] * m.canvas[1]) as usize];
        for (index, (&matrix, layer)) in transforms.iter().zip(&self.layers).enumerate() {
            if index == 2 {
                for mask in &self.masks {
                    for (i, p) in image.pixels_mut().enumerate() {
                        if mask[i] > 0. {
                            let a = packed(p);
                            let b = packed(
                                self.original
                                    .get_pixel(i as u32 % m.canvas[0], i as u32 / m.canvas[0]),
                            );
                            *p = straight(std::array::from_fn(|c| {
                                a[c] * (1. - mask[i]) + b[c] * mask[i]
                            }));
                        }
                    }
                }
            }
            Self::draw(&mut image, layer, matrix, &mut protected);
        }
        let head = (m.head_until * m.canvas[0] * 4) as usize;
        image.as_mut()[..head].copy_from_slice(&self.original.as_raw()[..head]);
        let factor = self.breath_factor(m, &protected);
        self.pose = Some(Pose {
            lift,
            image,
            protected,
            factor,
        });
    }
    fn breath_factor(&self, m: &Model, protected: &[f32]) -> Vec<f32> {
        let [x0, y0, x1, y1] = m.breath_roi;
        let w = m.canvas[0] as i32;
        let h = m.canvas[1] as i32;
        let [hx, hy, hr, hb] = m.hair_roi;
        let blocked = |x: i32, y: i32| {
            let x = x.clamp(0, w - 1) as u32;
            let y = y.clamp(0, h - 1) as u32;
            protected[(y * w as u32 + x) as usize] > 0.005
                || (x >= hx
                    && x < hr
                    && y >= hy
                    && y < hb
                    && self.hair_mask[((y - hy) * (hr - hx) + x - hx) as usize] > 0.005)
        };
        let width = x1 - x0;
        let height = y1 - y0;
        // Precompute dilation on the small torso ROI plus the Gaussian border.
        let ew = width + 8;
        let eh = height + 8;
        let mut dilated = vec![0.; (ew * eh) as usize];
        for y in 0..eh {
            for x in 0..ew {
                if (-3..=3).any(|dy| {
                    (-3..=3).any(|dx| {
                        blocked(x0 as i32 + x as i32 - 4 + dx, y0 as i32 + y as i32 - 4 + dy)
                    })
                }) {
                    dilated[(y * ew + x) as usize] = 1.;
                }
            }
        }
        let mut kernel = (-4..=4)
            .map(|v| (-(v * v) as f32 / (2. * 1.2 * 1.2)).exp())
            .collect::<Vec<_>>();
        let sum = kernel.iter().sum::<f32>();
        for v in &mut kernel {
            *v /= sum;
        }
        let mut horizontal = vec![0.; (width * eh) as usize];
        for y in 0..eh {
            for x in 0..width {
                horizontal[(y * width + x) as usize] = (0..9)
                    .map(|k| kernel[k] * dilated[(y * ew + x + k as u32) as usize])
                    .sum::<f32>();
            }
        }
        let mut factor = vec![0.; (width * height) as usize];
        for y in 0..height {
            for x in 0..width {
                factor[(y * width + x) as usize] = 1.
                    - (0..9)
                        .map(|k| kernel[k] * horizontal[((y + k as u32) * width + x) as usize])
                        .sum::<f32>();
            }
        }
        factor
    }
    pub(super) fn frame(
        &mut self,
        m: &Model,
        time: f64,
        values: &BTreeMap<String, f32>,
    ) -> RgbaImage {
        let lift = (values.get("bouquet_lift_px").copied().unwrap_or(0.) * 1e6).round() / 1e6;
        self.pose(m, lift);
        let pose = self.pose.as_ref().unwrap();
        let mut image = pose.image.clone();
        let phase = wave(time, m.shared_idle.breathing.period);
        let [x0, y0, x1, y1] = m.breath_roi;
        let bw = x1 - x0;
        let bh = y1 - y0;
        if phase != 0. {
            let crop = image::imageops::crop_imm(&pose.image, x0, y0, bw, bh).to_image();
            let fields: [Vec<f32>; 2] = std::array::from_fn(|c| {
                self.fields[c]
                    .iter()
                    .zip(&pose.factor)
                    .map(|(a, b)| a * b)
                    .collect()
            });
            for y in 0..bh {
                for x in 0..bw {
                    let index = (y * bw + x) as usize;
                    let mut p = [
                        x as f32 - phase * fields[0][index],
                        y as f32 - phase * fields[1][index],
                    ];
                    for _ in 0..3 {
                        p = [
                            x as f32 - phase * scalar(&fields[0], bw, bh, p[0], p[1]),
                            y as f32 - phase * scalar(&fields[1], bw, bh, p[0], p[1]),
                        ];
                    }
                    if pose.protected[((y + y0) * m.canvas[0] + x + x0) as usize] <= 0.005 {
                        image.put_pixel(x + x0, y + y0, straight(sample(&crop, p[0], p[1], true)));
                    }
                }
            }
        }
        let hp = &m.shared_idle.hair;
        let a = wave(time, hp.period);
        let t = time * std::f64::consts::TAU / hp.period;
        let b = ((t - 0.45).sin().powi(3) * (t / 2.).sin().powi(2)) as f32;
        let angle = hp.primary_angle_clockwise_degrees * a
            + hp.secondary_angle_clockwise_degrees * if b.abs() < 0.0001 { 0. } else { b };
        if angle != 0. {
            let before = image.clone();
            let matrix = rotation(hp.pivot, angle, 0.);
            let [hx, hy, hr, hb] = m.hair_roi;
            let hw = hr - hx;
            let hh = hb - hy;
            for y in hy..hb {
                for x in hx..hr {
                    let index = ((y - hy) * hw + x - hx) as usize;
                    let full = (y * m.canvas[0] + x) as usize;
                    if self.hair_occlusion[index] > 0.5
                        || pose.protected[full] > 0.005
                        || (x > m.hair_body_occlusion[0]
                            && y > m.hair_body_occlusion[1]
                            && y < m.hair_body_occlusion[2]
                            && before.get_pixel(x, y)[3] > 0)
                    {
                        continue;
                    }
                    let base = packed(before.get_pixel(x, y));
                    let mask = self.hair_mask[index];
                    let alpha = base[3] * (1. - mask) / (1. - base[3] * mask).max(1e-7);
                    let mut back = std::array::from_fn::<_, 4, _>(|c| {
                        if c == 3 {
                            alpha
                        } else {
                            if base[3] > 0. {
                                base[c] / base[3] * alpha
                            } else {
                                0.
                            }
                        }
                    });
                    let p = inverse(matrix, [x as f32, y as f32]);
                    let mut front = [0.; 4];
                    for (sx, sy, w) in weights(p[0] - hx as f32, p[1] - hy as f32) {
                        if sx >= 0 && sy >= 0 && sx < hw as i32 && sy < hh as i32 {
                            let start = ((sy as u32 * hw + sx as u32) * 4) as usize;
                            for (c, v) in front.iter_mut().enumerate() {
                                *v += self.hair_front[start + c] * w;
                            }
                        }
                    }
                    let coverage = 1. - back[3];
                    for c in 0..4 {
                        back[c] += front[c] * coverage;
                    }
                    image.put_pixel(x, y, straight(back));
                }
            }
        }
        let openness = values.get("eye_openness").copied().unwrap_or(1.);
        if openness < 0.99999 {
            let eyes = &m.eyes;
            let level = openness.clamp(0., 1.) * (eyes.states - 1) as f32;
            let lo = level.floor() as u32;
            let hi = (lo + 1).min(eyes.states - 1);
            let mix = level - lo as f32;
            for y in 0..eyes.cell[1] {
                for x in 0..eyes.cell[0] {
                    let mask = self.eye_mask[(y * eyes.cell[0] + x) as usize];
                    if mask > 0. {
                        let a = self.atlas.get_pixel(
                            (lo % eyes.columns) * eyes.cell[0] + x,
                            (lo / eyes.columns) * eyes.cell[1] + y,
                        );
                        let b = self.atlas.get_pixel(
                            (hi % eyes.columns) * eyes.cell[0] + x,
                            (hi / eyes.columns) * eyes.cell[1] + y,
                        );
                        let p = image.get_pixel_mut(x + eyes.origin[0], y + eyes.origin[1]);
                        for c in 0..3 {
                            p[c] = ((f32::from(a[c]) * (1. - mix) + f32::from(b[c]) * mix) * mask
                                + f32::from(p[c]) * (1. - mask))
                                .round()
                                .clamp(0., 255.) as u8;
                        }
                    }
                }
            }
        }
        let matrix = root(m, time);
        if matrix == [1., 0., 0., 0., 1., 0.] {
            return image;
        }
        RgbaImage::from_fn(m.canvas[0], m.canvas[1], |x, y| {
            let p = inverse(matrix, [x as f32, y as f32]);
            straight(sample(&image, p[0], p[1], false))
        })
    }
}
