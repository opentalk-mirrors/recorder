use crate::{layout::*, Source};
use gstreamer as gst;

#[derive(Debug, Clone)]
pub struct Participant<SRC>
where
    SRC: Source,
{
    pub name: String,
    pub source: SRC,
}

impl<SRC> Participant<SRC>
where
    SRC: Source,
{
    #[allow(dead_code)]
    pub fn new(pipeline: &gst::Pipeline, name: &str, resolution: &Size) -> Self {
        Self {
            name: name.to_string(),
            source: SRC::new(pipeline, name, "smpte", resolution),
        }
    }
    #[allow(dead_code)]
    pub fn is_video_linked(&self) -> bool {
        self.source.video_fake_sink().is_some()
    }
}
