use crate::Source;
use gstreamer as gst;

#[derive(Debug)]
pub enum VideoLinkStatus {
    /// Video source is unlinked
    None,
    /// Video source is linked to this fakesink
    Fakesink(gst::Element),
    /// Video source is linked to this (nth) pad on the compositor
    Compositor(usize, gst::Pad),
}

#[derive(Debug)]
pub struct Participant<S>
where
    S: Source,
{
    pub id: String,
    /// Wrapped AV source of this participant
    pub source: S,
    /// Contains the pad this participants audio stream is linked to. None if its not linked.
    pub audio_mixer_pad: Option<gst::Pad>,
    /// Video link status of this participant
    pub video_link_status: VideoLinkStatus,
}

impl<S> Participant<S>
where
    S: Source,
{
    pub fn new(pipeline: &gst::Pipeline, id: String, params: S::Parameters) -> Self {
        Self {
            id,
            source: S::new(pipeline, params),
            audio_mixer_pad: None,
            video_link_status: VideoLinkStatus::None,
        }
    }
}
