use crate::Source;
use gstreamer as gst;

#[derive(Debug)]
pub enum LinkStatus {
    None,
    Fakesink(gst::Element),
    Compositor(gst::Pad),
}

#[derive(Debug)]
pub struct Participant<S>
where
    S: Source,
{
    pub name: String,
    pub source: S,
    pub audio_mixer_pad: Option<gst::Pad>,
    pub video_link_status: LinkStatus,
}

impl<S> Participant<S>
where
    S: Source,
{
    pub fn new(pipeline: &gst::Pipeline, name: String, params: S::Parameters) -> Self {
        Self {
            name,
            source: S::new(pipeline, params),
            audio_mixer_pad: None,
            video_link_status: LinkStatus::None,
        }
    }
}
