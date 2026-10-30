use std::io::Cursor;
use std::sync::mpsc::{Receiver, Sender, channel};

use egui::ColorImage;
use figcraft_engine::playground::{Frame, FrameFormat};

const MAX_PICTURE_SIDE: u32 = 8192;
const MAX_PICTURE_BYTES: u64 = 256 << 20;

/// Decodes the images the page sends on a thread of its own, so the interface never waits for a
/// JPEG. Only the newest image is decoded; the ones queued behind it are dropped. Where no thread
/// can start (the web), an image is decoded in place.
pub struct Decoder {
    jobs: Option<Sender<Frame>>,
    done: Receiver<Option<ColorImage>>,
    inline: Option<Option<ColorImage>>,
}

impl Decoder {
    pub fn new(ctx: &egui::Context) -> Self {
        let (jobs, queue) = channel::<Frame>();
        let (done_tx, done) = channel::<Option<ColorImage>>();
        let (woken, viewport) = (ctx.clone(), ctx.viewport_id());
        let spawned = std::thread::Builder::new().name("figcraft-playground-decode".into()).spawn(move || {
            while let Ok(mut frame) = queue.recv() {
                while let Ok(newer) = queue.try_recv() {
                    frame = newer;
                }
                let picture = std::panic::catch_unwind(|| decode(&frame)).unwrap_or(None);
                if done_tx.send(picture).is_err() {
                    break;
                }
                woken.request_repaint_of(viewport);
            }
        });
        let jobs = match spawned {
            Ok(_) => Some(jobs),
            Err(error) => {
                log::warn!("playground images: no decoder thread ({error}); decoding in place");
                None
            }
        };
        Self { jobs, done, inline: None }
    }

    pub fn submit(&mut self, frame: Frame) {
        let Some(jobs) = &self.jobs else {
            self.inline = Some(decode(&frame));
            return;
        };
        if let Err(std::sync::mpsc::SendError(frame)) = jobs.send(frame) {
            self.jobs = None;
            self.inline = Some(decode(&frame));
        }
    }

    pub fn take(&mut self) -> Option<Option<ColorImage>> {
        self.inline.take().or_else(|| self.done.try_iter().last())
    }
}

fn decode(frame: &Frame) -> Option<ColorImage> {
    let format = match frame.format {
        FrameFormat::Jpeg => image::ImageFormat::Jpeg,
        FrameFormat::Png => image::ImageFormat::Png,
    };
    let mut reader = image::ImageReader::with_format(Cursor::new(&frame.data), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_PICTURE_SIDE);
    limits.max_image_height = Some(MAX_PICTURE_SIDE);
    limits.max_alloc = Some(MAX_PICTURE_BYTES);
    reader.limits(limits);
    let rgba = reader.decode().ok()?.into_rgba8();
    let size = [usize::try_from(rgba.width()).ok()?, usize::try_from(rgba.height()).ok()?];
    Some(ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()))
}
