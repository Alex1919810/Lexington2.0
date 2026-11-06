use image::{AnimationDecoder, Frames};
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
fn gif_frames(path: &Path) -> Result<Frames<'static>, String> {
    Ok(image::codecs::gif::GifDecoder::new(BufReader::new(
        File::open(path).map_err(|e| e.to_string())?,
    ))
    .map_err(|e| e.to_string())?
    .into_frames())
}
pub(crate) fn delay(frame: &image::Frame) -> Duration {
    let (num, den) = frame.delay().numer_denom_ms();
    Duration::from_secs_f64(if num as f64 / (den as f64) < 20. {
        0.1
    } else {
        num as f64 / den as f64 / 1000.
    })
}
