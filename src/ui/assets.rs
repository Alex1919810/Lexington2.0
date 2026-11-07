use crate::tools::gif::Decoder;
use eframe::egui::{self, ColorImage, TextureHandle};
use std::path::Path;

// Linear mipmaps match downscaled browser artwork and bubble backgrounds.
const TEXTURE_OPTIONS: egui::TextureOptions =
    egui::TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear));

pub struct Picture {
    decoder: Decoder,
    texture: TextureHandle,
    size: egui::Vec2,
    pub(super) opaque_regions: Vec<egui::Rect>,
}
impl Picture {
    pub fn load(ctx: &egui::Context, path: &Path) -> Result<Self, String> {
        let (decoder, image) = Decoder::load(path)?;
        let size = egui::vec2(image.width() as f32, image.height() as f32);
        let opaque_regions = opaque_regions(&image);
        let texture =
            ctx.load_texture(path.to_string_lossy(), color_image(&image), TEXTURE_OPTIONS);
        Ok(Self {
            decoder,
            texture,
            size,
            opaque_regions,
        })
    }
    pub fn size(&self) -> egui::Vec2 {
        self.size
    }
    pub fn texture(&mut self) -> &TextureHandle {
        if let Some(image) = self.decoder.advance() {
            self.opaque_regions = opaque_regions(&image);
            self.texture.set(color_image(&image), TEXTURE_OPTIONS);
        }
        &self.texture
    }
}
// Retain only compact alpha coverage, not another RGBA frame. A cell is at most
// two artwork pixels on screen; max alpha preserves thin visible details.
fn opaque_regions(image: &image::RgbaImage) -> Vec<egui::Rect> {
    let width = image.width().min(256);
    let height = image.height().min(256);
    let mut coverage = vec![false; (width * height) as usize];
    // Visit each grid cell, stopping as soon as its maximum alpha reaches the
    // threshold. Ceil boundaries exactly invert floor(pixel * grid / image).
    for y in 0..height {
        let top = (y * image.height()).div_ceil(height);
        let bottom = ((y + 1) * image.height()).div_ceil(height);
        for x in 0..width {
            let left = (x * image.width()).div_ceil(width) as usize;
            let right = ((x + 1) * image.width()).div_ceil(width) as usize;
            coverage[(y * width + x) as usize] = (top..bottom).any(|row| {
                let start = (row as usize * image.width() as usize + left) * 4;
                image.as_raw()[start..start + (right - left) * 4]
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .any(|pixel| pixel[3] >= 8)
            });
        }
    }
    let mut regions = Vec::new();
    for y in 0..height {
        let mut x = 0;
        while x < width {
            if !coverage[(y * width + x) as usize] {
                x += 1;
                continue;
            }
            let start = x;
            while x < width && coverage[(y * width + x) as usize] {
                x += 1;
            }
            regions.push(egui::Rect::from_min_max(
                egui::pos2(start as f32 / width as f32, y as f32 / height as f32),
                egui::pos2(x as f32 / width as f32, (y + 1) as f32 / height as f32),
            ));
        }
    }
    regions
}
fn color_image(image: &image::RgbaImage) -> ColorImage {
    ColorImage::from_rgba_unmultiplied(
        [image.width() as usize, image.height() as usize],
        image.as_raw(),
    )
}

pub(super) fn icons(ctx: &egui::Context) -> Vec<TextureHandle> {
    [
        include_bytes!("../../resource/icon/pin.png").as_slice(),
        include_bytes!("../../resource/icon/unpin.png").as_slice(),
        include_bytes!("../../resource/icon/character.png").as_slice(),
        include_bytes!("../../resource/icon/clock.png").as_slice(),
    ]
    .iter()
    .enumerate()
    .map(|(n, bytes)| {
        let image = image::load_from_memory(bytes)
            .expect("内置图标")
            .into_rgba8();
        ctx.load_texture(
            format!("icon-{n}"),
            egui::ColorImage::from_rgba_unmultiplied(
                [image.width() as usize, image.height() as usize],
                image.as_raw(),
            ),
            egui::TextureOptions::LINEAR,
        )
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::gif::delay;
    use std::{
        fs::File,
        io::BufReader,
        time::{Duration, Instant},
    };
    #[test]
    fn compact_alpha_grid_matches_pixel_max_for_uneven_dimensions() {
        for (width, height) in [(1, 1), (17, 9), (257, 263), (511, 389), (1024, 1024)] {
            let image = image::RgbaImage::from_fn(width, height, |x, y| {
                image::Rgba([3, 5, 7, if (x * 31 + y * 17) % 127 == 0 { 8 } else { 7 }])
            });
            let regions = opaque_regions(&image);
            let w = width.min(256);
            let h = height.min(256);
            let mut expected = vec![false; (w * h) as usize];
            for (x, y, pixel) in image.enumerate_pixels() {
                if pixel[3] >= 8 {
                    expected[((y * h / height) * w + x * w / width) as usize] = true;
                }
            }
            let mut actual = vec![false; expected.len()];
            for region in regions {
                let y = (region.min.y * h as f32).round() as u32;
                for x in (region.min.x * w as f32).round() as u32
                    ..(region.max.x * w as f32).round() as u32
                {
                    actual[(y * w + x) as usize] = true;
                }
            }
            assert_eq!(actual, expected, "{width}x{height}");
        }
    }
    #[test]
    fn bundled_pet_gif_restarts_at_first_frame_after_a_full_cycle() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("resource/illustration/lexington/gif/BiKiNi_Damage.gif");
        let mut options = gif::DecodeOptions::new();
        options.skip_frame_decoding(true);
        let decoder = options
            .read_info(BufReader::new(File::open(&path).unwrap()))
            .unwrap();
        assert_eq!(
            decoder.repeat(),
            gif::Repeat::Finite(0),
            "fixture has no loop extension"
        );
        let frame_count = decoder.into_iter().map(Result::unwrap).count().div_ceil(2);
        assert!(frame_count > 1);
        let ctx = egui::Context::default();
        let mut pic = Picture::load(&ctx, &path).unwrap();
        let texture_id = pic.texture.id();
        let (_, first) = ctx
            .tex_manager()
            .write()
            .take_delta()
            .set
            .into_iter()
            .find(|(id, _)| *id == texture_id)
            .unwrap();
        let egui::ImageData::Color(first) = first.image else {
            panic!("RGBA texture expected")
        };
        for frame_index in 1..=frame_count {
            pic.decoder.next_frame = Instant::now() - Duration::from_millis(1);
            pic.texture();
            assert!(
                pic.decoder.animation.is_some(),
                "stopped after frame {frame_index}"
            );
            assert_eq!(pic.texture.id(), texture_id);
            // Drain each upload so the test also keeps memory bounded.
            let (_, frame) = ctx
                .tex_manager()
                .write()
                .take_delta()
                .set
                .into_iter()
                .find(|(id, _)| *id == texture_id)
                .expect("next frame upload");
            if frame_index == frame_count {
                let egui::ImageData::Color(frame) = frame.image else {
                    panic!("RGBA texture expected")
                };
                assert_eq!(frame.size, first.size);
                assert_eq!(
                    frame.pixels, first.pixels,
                    "second cycle must start at frame zero"
                );
            }
        }
    }
    #[test]
    fn streamed_gif_composites_partial_frames_and_restores_disposal() {
        let path =
            std::env::temp_dir().join(format!("lexington-disposal-{}.gif", rand::random::<u64>()));
        {
            let mut encoder = gif::Encoder::new(
                File::create(&path).unwrap(),
                2,
                2,
                &[255, 0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0],
            )
            .unwrap();
            for (left, top, width, height, pixels, dispose) in [
                (0, 0, 2, 2, vec![0; 4], gif::DisposalMethod::Keep),
                (1, 0, 1, 1, vec![1], gif::DisposalMethod::Previous),
                (0, 1, 1, 1, vec![2], gif::DisposalMethod::Background),
                (0, 0, 1, 1, vec![3], gif::DisposalMethod::Keep),
                (0, 0, 1, 1, vec![3], gif::DisposalMethod::Keep),
            ] {
                encoder
                    .write_frame(&gif::Frame {
                        left,
                        top,
                        width,
                        height,
                        dispose,
                        transparent: Some(3),
                        delay: 2,
                        buffer: std::borrow::Cow::Owned(pixels),
                        ..Default::default()
                    })
                    .unwrap();
            }
        }
        let ctx = egui::Context::default();
        let mut pic = Picture::load(&ctx, &path).unwrap();
        let red = egui::Color32::RED;
        let blue = egui::Color32::BLUE;
        for (index, expected) in [
            [red; 4],
            [red, red, blue, red],
            [red, red, egui::Color32::TRANSPARENT, red],
        ]
        .into_iter()
        .enumerate()
        {
            if index > 0 {
                pic.decoder.next_frame = Instant::now() - Duration::from_millis(1);
                pic.texture();
            }
            let delta = ctx.tex_manager().write().take_delta();
            let image = &delta
                .set
                .iter()
                .find(|(id, _)| *id == pic.texture.id())
                .unwrap()
                .1
                .image;
            let egui::ImageData::Color(image) = image else {
                panic!("RGBA texture expected")
            };
            assert_eq!(image.pixels, expected, "composited frame {index}");
            for (pixel, expected) in expected.iter().enumerate() {
                let point = egui::pos2(
                    (pixel % 2) as f32 / 2. + 0.25,
                    (pixel / 2) as f32 / 2. + 0.25,
                );
                assert_eq!(
                    pic.opaque_regions.iter().any(|r| r.contains(point)),
                    expected.a() >= 8,
                    "input coverage must follow composited transparency at frame {index}"
                );
            }
        }
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn gifs_default_to_looping_and_honor_explicit_limits_and_frame_delays() {
        let path =
            std::env::temp_dir().join(format!("lexington-gif-{}.gif", rand::random::<u64>()));
        for (repeat, expected) in [
            (None, None),
            (Some(gif::Repeat::Infinite), None),
            (Some(gif::Repeat::Finite(2)), Some(2)),
        ] {
            {
                let mut encoder =
                    gif::Encoder::new(File::create(&path).unwrap(), 2, 2, &[255, 0, 0, 0, 255, 0])
                        .unwrap();
                if let Some(repeat) = repeat {
                    encoder.set_repeat(repeat).unwrap();
                }
                for index in [0, 1, 1] {
                    let frame = gif::Frame {
                        width: 2,
                        height: 2,
                        delay: 2,
                        buffer: std::borrow::Cow::Owned(vec![index; 4]),
                        ..Default::default()
                    };
                    encoder.write_frame(&frame).unwrap();
                }
            }
            let ctx = egui::Context::default();
            let mut pic = Picture::load(&ctx, &path).unwrap();
            assert_eq!(pic.decoder.repeats_left, expected);
            let texture_id = pic.texture.id();
            // Check frame contents across four cycles, including the transition
            // back to frame zero and exhaustion of an explicit repeat count.
            for frame_index in 0..8 {
                if frame_index > 0 {
                    pic.decoder.next_frame = Instant::now() - Duration::from_millis(1);
                    pic.texture();
                }
                let playing = expected.is_none_or(|n| frame_index < (usize::from(n) + 1) * 2);
                assert_eq!(
                    pic.decoder.animation.is_some(),
                    playing,
                    "frame {frame_index}"
                );
                assert_eq!(pic.texture.id(), texture_id, "reuse the streaming texture");
                let delta = ctx.tex_manager().write().take_delta();
                let update = delta.set.iter().find(|(id, _)| *id == texture_id);
                if playing {
                    let egui::ImageData::Color(image) = &update.expect("frame upload").1.image
                    else {
                        panic!("RGBA texture expected")
                    };
                    let color = if frame_index % 2 == 0 {
                        egui::Color32::RED
                    } else {
                        egui::Color32::GREEN
                    };
                    assert_eq!(image.pixels, [color; 4], "frame {frame_index}");
                } else {
                    assert!(update.is_none(), "stopped GIF must retain its last frame");
                }
            }
        }
        let frame = image::Frame::from_parts(
            image::RgbaImage::new(2, 2),
            0,
            0,
            image::Delay::from_numer_denom_ms(0, 1),
        );
        assert_eq!(delay(&frame), Duration::from_millis(100));
        std::fs::remove_file(path).unwrap();
    }
}
