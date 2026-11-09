//! Connected textured meshes; triangles share vertices and never alpha-over each other.
use super::render;
use image::RgbaImage;
use serde::Deserialize;

type Matrix = [f32; 6];
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum Mesh {
    Anchored {
        vertices: Vec<[f32; 3]>,
        indices: Vec<usize>,
    },
    Arm {
        vertices: Vec<[f32; 5]>,
        indices: Vec<usize>,
    },
}
pub(super) struct Pose {
    source: Vec<[f32; 2]>,
    positions: Vec<[f32; 2]>,
    indices: Vec<usize>,
}
impl Mesh {
    pub(super) fn validate(&self, canvas: [u32; 2], has_rig: bool) -> Result<(), String> {
        let (coordinates, indices) = match self {
            Self::Anchored { vertices, indices } => {
                if vertices
                    .iter()
                    .any(|v| v.iter().any(|n| !n.is_finite()) || !(0. ..=1.).contains(&v[2]))
                {
                    return Err("Live2D 发束网格权重无效。".into());
                }
                (
                    vertices.iter().map(|v| [v[0], v[1]]).collect::<Vec<_>>(),
                    indices,
                )
            }
            Self::Arm { vertices, indices } => {
                if !has_rig
                    || vertices.iter().any(|v| {
                        v.iter().any(|n| !n.is_finite())
                            || v[2..].iter().any(|n| !(0. ..=1.).contains(n))
                            || (v[2] + v[3] + v[4] - 1.).abs() > 0.001
                    })
                {
                    return Err("Live2D 手臂网格权重无效。".into());
                }
                (
                    vertices.iter().map(|v| [v[0], v[1]]).collect::<Vec<_>>(),
                    indices,
                )
            }
        };
        if !(3..=2048).contains(&coordinates.len())
            || indices.is_empty()
            || indices.len() > 12288
            || !indices.len().is_multiple_of(3)
            || coordinates.iter().any(|v| {
                v[0] < 0. || v[1] < 0. || v[0] > canvas[0] as f32 || v[1] > canvas[1] as f32
            })
            || indices.iter().any(|i| *i >= coordinates.len())
        {
            return Err("Live2D 网格坐标或索引无效。".into());
        }
        for triangle in indices.as_chunks::<3>().0.iter() {
            let [a, b, c] = [
                coordinates[triangle[0]],
                coordinates[triangle[1]],
                coordinates[triangle[2]],
            ];
            if area(a, b, c).abs() < 0.0001 {
                return Err("Live2D 网格三角形退化。".into());
            }
        }
        Ok(())
    }
    pub(super) fn pose(&self, rotation: Matrix, bones: [Matrix; 2]) -> Pose {
        let (source, positions, indices) = match self {
            Self::Anchored { vertices, indices } => {
                let source = vertices.iter().map(|v| [v[0], v[1]]).collect::<Vec<_>>();
                let positions = vertices
                    .iter()
                    .map(|v| {
                        let moved = render::apply(rotation, [v[0], v[1]]);
                        [
                            v[0] + (moved[0] - v[0]) * v[2],
                            v[1] + (moved[1] - v[1]) * v[2],
                        ]
                    })
                    .collect();
                (source, positions, indices.clone())
            }
            Self::Arm { vertices, indices } => {
                let source = vertices.iter().map(|v| [v[0], v[1]]).collect::<Vec<_>>();
                let positions = vertices
                    .iter()
                    .map(|v| {
                        let a = render::apply(bones[0], [v[0], v[1]]);
                        let b = render::apply(bones[1], [v[0], v[1]]);
                        [
                            v[0] * v[2] + a[0] * v[3] + b[0] * v[4],
                            v[1] * v[2] + a[1] * v[3] + b[1] * v[4],
                        ]
                    })
                    .collect();
                (source, positions, indices.clone())
            }
        };
        Pose {
            source,
            positions,
            indices,
        }
    }
}
fn area(a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> f32 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}
impl Pose {
    fn coordinates(&self, triangle: &[usize], point: [f32; 2]) -> Option<[f32; 2]> {
        let [a, b, c] = [
            self.positions[triangle[0]],
            self.positions[triangle[1]],
            self.positions[triangle[2]],
        ];
        let determinant = area(a, b, c);
        if determinant.abs() < 0.0001 {
            return None;
        }
        let u = area(point, b, c) / determinant;
        let v = area(a, point, c) / determinant;
        let w = 1. - u - v;
        if u < -0.0001 || v < -0.0001 || w < -0.0001 {
            return None;
        }
        let [a, b, c] = [
            self.source[triangle[0]],
            self.source[triangle[1]],
            self.source[triangle[2]],
        ];
        Some([
            a[0] * u + b[0] * v + c[0] * w,
            a[1] * u + b[1] * v + c[1] * w,
        ])
    }
    pub(super) fn hit(&self, image: &RgbaImage, point: [f32; 2]) -> bool {
        self.indices.as_chunks::<3>().0.iter().any(|t| {
            self.coordinates(t, point)
                .is_some_and(|p| render::sample(image, p[0], p[1], false)[3] >= 8. / 255.)
        })
    }
    pub(super) fn raster(&self, image: &RgbaImage) -> RgbaImage {
        let mut output = RgbaImage::new(image.width(), image.height());
        for triangle in self.indices.as_chunks::<3>().0.iter() {
            let points = triangle.map(|i| self.positions[i]);
            let left = points
                .iter()
                .map(|v| v[0])
                .fold(f32::INFINITY, f32::min)
                .floor()
                .max(0.) as u32;
            let right = points
                .iter()
                .map(|v| v[0])
                .fold(f32::NEG_INFINITY, f32::max)
                .ceil()
                .min(image.width() as f32 - 1.) as u32;
            let top = points
                .iter()
                .map(|v| v[1])
                .fold(f32::INFINITY, f32::min)
                .floor()
                .max(0.) as u32;
            let bottom = points
                .iter()
                .map(|v| v[1])
                .fold(f32::NEG_INFINITY, f32::max)
                .ceil()
                .min(image.height() as f32 - 1.) as u32;
            for y in top..=bottom {
                for x in left..=right {
                    if let Some(p) = self.coordinates(triangle, [x as f32, y as f32]) {
                        // Replacement, not over: shared edges cannot double alpha.
                        output.put_pixel(
                            x,
                            y,
                            render::straight(render::sample(image, p[0], p[1], false)),
                        );
                    }
                }
            }
        }
        output
    }
    #[cfg(test)]
    pub(super) fn minimum_area_ratio(&self) -> f32 {
        self.indices
            .as_chunks::<3>()
            .0
            .iter()
            .map(|t| {
                area(
                    self.positions[t[0]],
                    self.positions[t[1]],
                    self.positions[t[2]],
                ) / area(self.source[t[0]], self.source[t[1]], self.source[t[2]])
            })
            .fold(f32::INFINITY, f32::min)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_triangle_edges_preserve_alpha_and_anchored_mesh_does_not_fold() {
        let mesh = Mesh::Anchored {
            vertices: vec![[0., 0., 0.], [8., 0., 0.], [8., 8., 1.], [0., 8., 1.]],
            indices: vec![0, 1, 2, 0, 2, 3],
        };
        mesh.validate([8, 8], false).unwrap();
        let image = RgbaImage::from_pixel(8, 8, image::Rgba([40, 80, 120, 128]));
        let identity = [1., 0., 0., 0., 1., 0.];
        let pose = mesh.pose(identity, [identity; 2]);
        let raster = pose.raster(&image);
        assert!(raster.pixels().all(|p| p[3] == 128));
        for angle in [-6., -3., 0., 3., 6.] {
            let pose = mesh.pose(render::rotation([4., 0.], angle, 0.), [identity; 2]);
            assert!(pose.minimum_area_ratio() > 0.8);
        }
    }
}
