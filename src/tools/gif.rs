use std::{
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

pub struct Decoder {
    pub(crate) animation: Option<GifFrames>,
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
                .next_sampled()
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
            let mut frame = self.animation.as_mut().and_then(GifFrames::next_sampled);
            if frame.is_none() {
                if self.repeats_left == Some(0) {
                    self.animation = None;
                    return None;
                }
                if let Some(left) = &mut self.repeats_left {
                    *left -= 1;
                }
                if let Some(animation) = &mut self.animation
                    && animation.restart(&self.path).is_err()
                {
                    self.animation = None;
                }
                frame = self.animation.as_mut().and_then(GifFrames::next_sampled);
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
// Only indexed pixels and palette/metadata are cached; RGBA stays one frame
// plus a disposal canvas. Build during playback to avoid a startup decode burst.
const INDEX_CACHE_LIMIT: usize = 96 * 1024 * 1024;

pub(crate) struct GifFrames {
    reader: Option<<gif::Decoder<BufReader<File>> as IntoIterator>::IntoIter>,
    canvas: image::RgbaImage,
    palette: Vec<u8>,
    cache: Vec<gif::Frame<'static>>,
    cache_bytes: usize,
    cache_limit: usize,
    cache_enabled: bool,
    cursor: usize,
}
impl GifFrames {
    // Retain source frames 0, 2, 4, ... and add each omitted frame's delay
    // to the preceding displayed frame. Never pair across a loop boundary.
    fn next_sampled(&mut self) -> Option<image::ImageResult<image::Frame>> {
        let first = match self.next()? {
            Ok(frame) => frame,
            Err(error) => return Some(Err(error)),
        };
        let mut duration = delay(&first);
        match self.next_frame(false) {
            Some(Ok(skipped)) => duration += delay(&skipped),
            Some(Err(error)) => return Some(Err(error)),
            None => {}
        }
        Some(Ok(image::Frame::from_parts(
            first.into_buffer(),
            0,
            0,
            image::Delay::from_numer_denom_ms(duration.as_millis() as u32, 1),
        )))
    }
    fn restart(&mut self, path: &Path) -> Result<(), String> {
        if self.reader.is_none() && self.cache_enabled && !self.cache.is_empty() {
            // Each loop starts on a transparent canvas, as when reopening a GIF.
            self.canvas.as_mut().fill(0);
            self.cursor = 0;
        } else {
            // Once over budget, stay streaming rather than rebuilding a prefix
            // on every loop. Dropping/replacing Picture releases this whole cache.
            *self = gif_frames_with_limit(
                path,
                if self.cache_enabled {
                    self.cache_limit
                } else {
                    0
                },
            )?;
        }
        Ok(())
    }
    fn next_frame(&mut self, render: bool) -> Option<image::ImageResult<image::Frame>> {
        let decoded;
        let frame = if let Some(reader) = &mut self.reader {
            decoded = match reader.next() {
                Some(Ok(frame)) => frame,
                None => {
                    self.reader = None;
                    self.cursor = self.cache.len();
                    return None;
                }
                Some(Err(error)) => {
                    return Some(Err(image::ImageError::IoError(std::io::Error::other(
                        error,
                    ))));
                }
            };
            if self.cache_enabled {
                let bytes = decoded.buffer.len()
                    + decoded.palette.as_ref().map_or(0, Vec::len)
                    + std::mem::size_of::<gif::Frame<'static>>();
                if bytes <= self.cache_limit.saturating_sub(self.cache_bytes) {
                    self.cache_bytes += bytes;
                    self.cache.push(decoded);
                    self.cache.last().expect("just cached frame")
                } else {
                    // A partial sequential cache would still need a decoder at
                    // the loop boundary; release it and use bounded streaming.
                    self.cache = Vec::new();
                    self.cache_bytes = 0;
                    self.cache_enabled = false;
                    &decoded
                }
            } else {
                &decoded
            }
        } else {
            let frame = self.cache.get(self.cursor)?;
            self.cursor += 1;
            frame
        };
        let palette = frame.palette.as_deref().unwrap_or(&self.palette);
        let mut colors = [[0_u8; 4]; 256];
        for (index, rgb) in palette.as_chunks::<3>().0.iter().take(256).enumerate() {
            colors[index] = [rgb[0], rgb[1], rgb[2], 255];
        }
        // Skipped Keep frames still affect the next visible frame. Compose
        // directly into the disposal canvas without allocating/copying RGBA.
        // Skipped Previous/Background need only apply their disposal below.
        let keep = matches!(
            frame.dispose,
            gif::DisposalMethod::Any | gif::DisposalMethod::Keep
        );
        let mut output = if render {
            self.canvas.clone()
        } else if keep {
            std::mem::take(&mut self.canvas)
        } else {
            image::RgbaImage::new(0, 0)
        };
        let dimensions = if render || keep {
            output.dimensions()
        } else {
            self.canvas.dimensions()
        };
        let left = u32::from(frame.left).min(dimensions.0) as usize;
        let top = u32::from(frame.top).min(dimensions.1) as usize;
        let width = usize::from(frame.width).min(dimensions.0 as usize - left);
        let height = usize::from(frame.height).min(dimensions.1 as usize - top);
        let stride = dimensions.0 as usize * 4;
        // Work on contiguous rows: no full-canvas per-pixel coordinate division,
        // RGBA intermediate frame, or palette conversion for every pixel.
        if (render || keep) && width > 0 && height > 0 {
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
                if render {
                    self.canvas.clone_from(&output);
                } else {
                    self.canvas = std::mem::take(&mut output);
                }
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
impl Iterator for GifFrames {
    type Item = image::ImageResult<image::Frame>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_frame(true)
    }
}
fn gif_frames(path: &Path) -> Result<GifFrames, String> {
    gif_frames_with_limit(path, INDEX_CACHE_LIMIT)
}
fn gif_frames_with_limit(path: &Path, cache_limit: usize) -> Result<GifFrames, String> {
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
    let cache_enabled = cache_limit > palette.len();
    let cache_bytes = if cache_enabled { palette.len() } else { 0 };
    Ok(GifFrames {
        reader: Some(reader.into_iter()),
        canvas,
        palette,
        cache: Vec::new(),
        cache_bytes,
        cache_limit,
        cache_enabled,
        cursor: 0,
    })
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
    fn indexed_cache_replays_without_file_and_over_budget_stays_streaming() {
        let path =
            std::env::temp_dir().join(format!("lexington-cache-{}.gif", rand::random::<u64>()));
        {
            let mut encoder =
                gif::Encoder::new(File::create(&path).unwrap(), 4, 3, &[255, 0, 0, 0, 255, 0])
                    .unwrap();
            for index in [0, 1, 0] {
                encoder
                    .write_frame(&gif::Frame {
                        width: 4,
                        height: 3,
                        delay: 2,
                        dispose: gif::DisposalMethod::Keep,
                        buffer: std::borrow::Cow::Owned(vec![index; 12]),
                        ..Default::default()
                    })
                    .unwrap();
            }
        }
        let mut cached = gif_frames(&path).unwrap();
        let expected: Vec<_> = cached.by_ref().map(Result::unwrap).collect();
        assert_eq!(cached.cache.len(), 3);
        assert!(cached.reader.is_none());
        assert!(cached.cache_bytes <= INDEX_CACHE_LIMIT);
        let size = std::mem::size_of::<gif::Frame<'static>>();
        let mut streaming = gif_frames_with_limit(&path, size + 12 + 6).unwrap();
        for _ in 0..3 {
            for expected in &expected {
                assert_eq!(
                    streaming.next().unwrap().unwrap().buffer(),
                    expected.buffer()
                );
            }
            assert!(streaming.next().is_none());
            assert!(!streaming.cache_enabled);
            assert_eq!(streaming.cache_bytes, 0);
            assert!(streaming.cache.is_empty());
            streaming.restart(&path).unwrap();
        }
        // A completed cache must not reopen the source, even at the loop boundary.
        std::fs::remove_file(&path).unwrap();
        for _ in 0..4 {
            cached.restart(&path).unwrap();
            for expected in &expected {
                let actual = cached.next().unwrap().unwrap();
                assert_eq!(actual.buffer(), expected.buffer());
                assert_eq!(actual.delay(), expected.delay());
            }
            assert!(cached.next().is_none());
            assert!(cached.next().is_none());
        }
        assert!(streaming.by_ref().all(|f| f.is_ok()));
        assert!(streaming.restart(&path).is_err());
    }

    #[test]
    #[ignore = "decoder benchmark; run with --release --ignored --nocapture"]
    fn frame_sampling_performance() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("resource/illustration/lexington/gif/BiKiNi_Normal.gif");
        for sampled in [false, true] {
            let mut decoder = gif_frames(&path).unwrap();
            let warmup = Instant::now();
            while let Some(frame) = if sampled {
                decoder.next_sampled()
            } else {
                decoder.next()
            } {
                std::hint::black_box(frame.unwrap());
            }
            let warmup = warmup.elapsed();
            let start = Instant::now();
            let mut count = 0;
            for _ in 0..10 {
                decoder.restart(&path).unwrap();
                while let Some(frame) = if sampled {
                    decoder.next_sampled()
                } else {
                    decoder.next()
                } {
                    std::hint::black_box(frame.unwrap());
                    count += 1;
                }
            }
            eprintln!(
                "sampled={sampled}, indexed={} bytes, first_loop={warmup:?}, {count} steady_frames={:?}",
                decoder.cache_bytes,
                start.elapsed()
            );
        }
    }

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
                gif::DisposalMethod::Keep,
            ]
            .into_iter()
            .cycle()
            .take(13)
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
        let expected: Vec<_> =
            image::codecs::gif::GifDecoder::new(BufReader::new(File::open(&path).unwrap()))
                .unwrap()
                .into_frames()
                .map(Result::unwrap)
                .collect();
        // Odd frame count, variable/short delays and all disposal methods on
        // retained and skipped frames; validate streaming and cached loops.
        for limit in [0, INDEX_CACHE_LIMIT] {
            let mut actual = gif_frames_with_limit(&path, limit).unwrap();
            for cycle in 0..3 {
                if cycle > 0 {
                    actual.restart(&path).unwrap();
                }
                for pair in expected.chunks(2) {
                    let frame = actual.next_sampled().unwrap().unwrap();
                    assert_eq!(frame.buffer(), pair[0].buffer());
                    assert_eq!(delay(&frame), pair.iter().map(delay).sum::<Duration>());
                }
                assert!(actual.next_sampled().is_none());
            }
        }
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    #[ignore = "full pixel comparison; run with --release --ignored"]
    fn bundled_frames_match_reference_decoder_pixel_for_pixel() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut count = 0;
        for illustration in crate::backend::resources::illustrations(root)
            .iter()
            .filter(|p| p.format == "gif")
        {
            let mut actual = gif_frames(&illustration.path).unwrap();
            let (mut playback, mut current) = Decoder::load(&illustration.path).unwrap();
            for cycle in 0..2 {
                if cycle > 0 {
                    actual.restart(&illustration.path).unwrap();
                }
                let mut reference = image::codecs::gif::GifDecoder::new(BufReader::new(
                    File::open(&illustration.path).unwrap(),
                ))
                .unwrap()
                .into_frames();
                let mut index = 0;
                while let Some(expected) = reference.next() {
                    let expected = expected.unwrap();
                    let mut duration = delay(&expected);
                    if let Some(skipped) = reference.next() {
                        duration += delay(&skipped.unwrap());
                    }
                    let frame = actual.next_sampled().unwrap().unwrap();
                    assert_eq!(
                        frame.buffer(),
                        expected.buffer(),
                        "{} cycle {cycle} frame {index}",
                        illustration.path.display()
                    );
                    assert_eq!(&current, expected.buffer());
                    assert_eq!(delay(&frame), duration);
                    playback.next_frame = Instant::now() - Duration::from_millis(1);
                    current = playback.advance().expect("looped sampled frame");
                    index += 2;
                    count += 1;
                }
                assert!(actual.next_sampled().is_none());
                assert!(actual.cache_enabled);
                assert!(actual.cache_bytes <= INDEX_CACHE_LIMIT);
            }
        }
        assert!(count > 500);
    }
}
