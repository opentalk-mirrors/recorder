use crate::{layout::*, Source};
use gstreamer as gst;

#[derive(Debug, Clone)]
pub struct Participant<S>
where
    S: Source,
{
    pub name: String,
    pub source: S,
}

impl<S> Participant<S>
where
    S: Source,
{
    #[allow(dead_code)]
    pub fn new(pipeline: &gst::Pipeline, name: &str, resolution: &Size) -> Self {
        Self {
            name: name.to_string(),
            source: S::new(pipeline, name, "smpte", resolution),
        }
    }
    #[allow(dead_code)]
    pub fn is_video_linked(&self) -> bool {
        self.source.video_fake_sink().is_some()
    }
}
