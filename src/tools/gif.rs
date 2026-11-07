use image::Frames;
use std::{
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

pub struct Decoder {
    pub(crate) animation: Option<Frames<'static>>,
    path: PathBuf,
    pub(crate) next_frame: Instant,
    pub(crate) repeats_left: Option<u16>,
}
impl Decoder {
    pub fn load(path: &Path) -> Result<(Self, image::RgbaImage), String> {
        let repeats_left = if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("gif"))
        {
            match gif::DecodeOptions::new()
                .read_info(BufReader::new(File::open(path).map_err(|e| e.to_string())?))
                .map_err(|e| e.to_string())?
                .repeat()
            {
                // Existing pet GIFs omit the loop extension. The decoder reports
                // Finite(0) for those files; keep desktop animations running.
                gif::Repeat::Infinite | gif::Repeat::Finite(0) => None,
                gif::Repeat::Finite(n) => Some(n),
            }
        } else {
            Some(0)
        };
        let (animation, image, delay) = if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("gif"))
        {
            let mut frames = gif_frames(path)?;
            let frame = frames
                .next()
                .ok_or("图片没有帧")?
                .map_err(|e| e.to_string())?;
            let delay = delay(&frame);
            (Some(frames), frame.into_buffer(), delay)
        } else {
            (
                None,
                image::open(path).map_err(|e| e.to_string())?.into_rgba8(),
                Duration::from_secs(1),
            )
        };
        Ok((
            Self {
                animation,
                repeats_left,
                path: path.into(),
                next_frame: Instant::now() + delay,
            },
            image,
        ))
    }
    pub fn advance(&mut self) -> Option<image::RgbaImage> {
        if self.animation.is_some() && Instant::now() >= self.next_frame {
            let mut frame = self.animation.as_mut().and_then(Iterator::next);
            if frame.is_none() {
                if self.repeats_left == Some(0) {
                    self.animation = None;
                    return None;
                }
                if let Some(left) = &mut self.repeats_left {
                    *left -= 1;
                }
                self.animation = gif_frames(&self.path).ok();
                frame = self.animation.as_mut().and_then(Iterator::next);
            }
            match frame {
                Some(Ok(frame)) => {
                    self.next_frame += delay(&frame);
                    return Some(frame.into_buffer());
                }
                _ => self.animation = None,
            }
        }
        None
    }
}
// Keep one disposal canvas and one returned frame, never a cache of the animation.
struct GifFrames {
    reader: gif::Decoder<BufReader<File>>,
    canvas: image::RgbaImage,
    palette: Vec<u8>,
}
impl Iterator for GifFrames {
    type Item = image::ImageResult<image::Frame>;

    fn next(&mut self) -> Option<Self::Item> {
        let frame = match self.reader.read_next_frame() {
            Ok(Some(frame)) => frame,
            Ok(None) => return None,
            Err(error) => {
                return Some(Err(image::ImageError::IoError(std::io::Error::other(
                    error,
                ))));
            }
        };
        let palette = frame.palette.as_deref().unwrap_or(&self.palette);
        let mut colors = [[0_u8; 4]; 256];
        for (index, rgb) in palette.as_chunks::<3>().0.iter().take(256).enumerate() {
            colors[index] = [rgb[0], rgb[1], rgb[2], 255];
        }
        let mut output = self.canvas.clone();
        let left = u32::from(frame.left).min(output.width()) as usize;
        let top = u32::from(frame.top).min(output.height()) as usize;
        let width = usize::from(frame.width).min(output.width() as usize - left);
        let height = usize::from(frame.height).min(output.height() as usize - top);
        let stride = output.width() as usize * 4;
        // Work on contiguous rows: no full-canvas per-pixel coordinate division,
        // RGBA intermediate frame, or palette conversion for every pixel.
        if width > 0 && height > 0 {
            for (row, indices) in output.as_mut()[top * stride..]
                .chunks_exact_mut(stride)
                .take(height)
                .zip(frame.buffer.chunks_exact(usize::from(frame.width)))
            {
                for (pixel, &index) in row[left * 4..(left + width) * 4]
                    .as_chunks_mut::<4>()
                    .0
                    .iter_mut()
                    .zip(indices)
                {
                    if Some(index) != frame.transparent {
                        if usize::from(index) * 3 + 2 >= palette.len() {
                            return Some(Err(image::ImageError::IoError(std::io::Error::other(
                                "GIF palette index out of range",
                            ))));
                        }
                        pixel.copy_from_slice(&colors[usize::from(index)]);
                    }
                }
            }
        }
        match frame.dispose {
            gif::DisposalMethod::Any | gif::DisposalMethod::Keep => {
                self.canvas.clone_from(&output);
            }
            gif::DisposalMethod::Background if width > 0 && height > 0 => {
                for row in self.canvas.as_mut()[top * stride..]
                    .chunks_exact_mut(stride)
                    .take(height)
                {
                    row[left * 4..(left + width) * 4].fill(0);
                }
            }
            // Previous leaves the canvas as it was before this frame.
            _ => {}
        }
        Some(Ok(image::Frame::from_parts(
            output,
            0,
            0,
            image::Delay::from_numer_denom_ms(u32::from(frame.delay) * 10, 1),
        )))
    }
}
fn gif_frames(path: &Path) -> Result<Frames<'static>, String> {
    let reader = gif::DecodeOptions::new()
        .read_info(BufReader::new(File::open(path).map_err(|e| e.to_string())?))
        .map_err(|e| e.to_string())?;
    // Keep the previous image decoder's allocation limit for custom resources.
    let mut limits = image::Limits::default();
    limits
        .reserve(u64::from(reader.width()) * u64::from(reader.height()) * 8)
        .map_err(|e| e.to_string())?;
    let canvas = image::RgbaImage::new(u32::from(reader.width()), u32::from(reader.height()));
    let palette = reader.global_palette().unwrap_or_default().to_vec();
    Ok(Frames::new(Box::new(GifFrames {
        reader,
        canvas,
        palette,
    })))
}
pub(crate) fn delay(frame: &image::Frame) -> Duration {
    let (num, den) = frame.delay().numer_denom_ms();
    Duration::from_secs_f64(if num as f64 / (den as f64) < 20. {
        0.1
    } else {
        num as f64 / den as f64 / 1000.
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::AnimationDecoder;

    #[test]
    fn local_palettes_interlacing_and_partial_disposals_match_reference() {
        let path =
            std::env::temp_dir().join(format!("lexington-palettes-{}.gif", rand::random::<u64>()));
        {
            let mut encoder =
                gif::Encoder::new(File::create(&path).unwrap(), 9, 7, &[255, 0, 0, 0, 255, 0])
                    .unwrap();
            for (index, dispose) in [
                gif::DisposalMethod::Keep,
                gif::DisposalMethod::Previous,
                gif::DisposalMethod::Background,
                gif::DisposalMethod::Any,
            ]
            .into_iter()
            .cycle()
            .take(12)
            .enumerate()
            {
                let full = index == 0;
                let (width, height) = if full { (9, 7) } else { (5, 3) };
                encoder
                    .write_frame(&gif::Frame {
                        left: if full { 0 } else { (index % 4) as u16 },
                        top: if full { 0 } else { (index % 3) as u16 },
                        width,
                        height,
                        dispose,
                        interlaced: index % 2 == 0,
                        palette: (index % 3 == 0).then(|| vec![0, 0, 255, 255, 255, 0]),
                        transparent: (index % 2 == 1).then_some(0),
                        delay: index as u16 + 1,
                        buffer: std::borrow::Cow::Owned(
                            (0..width * height).map(|p| (p % 2) as u8).collect(),
                        ),
                        ..Default::default()
                    })
                    .unwrap();
            }
        }
        let reference =
            image::codecs::gif::GifDecoder::new(BufReader::new(File::open(&path).unwrap()))
                .unwrap()
                .into_frames();
        let mut actual = gif_frames(&path).unwrap();
        for expected in reference {
            let expected = expected.unwrap();
            let actual = actual.next().unwrap().unwrap();
            assert_eq!(actual.buffer(), expected.buffer());
            assert_eq!(actual.delay(), expected.delay());
        }
        assert!(actual.next().is_none());
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    #[ignore = "full pixel comparison; run with --release --ignored"]
    fn bundled_frames_match_reference_decoder_pixel_for_pixel() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let paths = crate::backend::resources::illustrations(root);
        let mut count = 0;
        for illustration in paths.iter().filter(|p| p.format == "gif") {
            let reference = image::codecs::gif::GifDecoder::new(BufReader::new(
                File::open(&illustration.path).unwrap(),
            ))
            .unwrap()
            .into_frames();
            let mut actual = gif_frames(&illustration.path).unwrap();
            for (index, frame) in reference.enumerate() {
                let expected = frame.unwrap();
                let actual = actual.next().expect("missing frame").unwrap();
                assert_eq!(actual.delay(), expected.delay());
                assert_eq!(
                    actual.buffer(),
                    expected.buffer(),
                    "{} frame {index}",
                    illustration.path.display()
                );
                count += 1;
            }
            assert!(actual.next().is_none());
        }
        assert!(count > 500);
    }
}
