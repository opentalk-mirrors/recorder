use super::Source;

/// Status of a participant if it is linked to a fake sink or the compositor.
#[derive(Debug)]
pub enum VideoLinkStatus {
    /// Video source is unlinked
    None,
    /// Video source is linked to this fakesink
    Fakesink(gst::Element),
    /// Video source is linked to this (nth) pad on the compositor
    Compositor(usize, gst::Pad),
}

/// Represents a participant.
/// # Types
/// - `SRC`: Source type which implements trait [Source]
#[derive(Debug)]
pub struct Participant<SRC>
where
    SRC: Source,
{
    /// Name to be displayed within the "who's speaking" text.
    pub display_name: String,
    /// Wrapped AV source of this participant.
    pub source: SRC,
    /// Contains the pad this participants audio stream is linked to. None if its not linked.
    pub audio_mixer_pad: Option<gst::Pad>,
    /// Video link status of this participant.
    pub video_link_status: VideoLinkStatus,
}

impl<SRC> Participant<SRC>
where
    SRC: Source,
{
    /// Create new participant and a source of type `SRC` into the given GStreamer pipeline.
    /// # Types
    /// - `SRC`: Source type which implements trait [Source]
    /// # Arguments
    /// - `pipeline`: Pipeline to add GStreamer elements into.
    /// - `id`: Unique ID of the participant.
    /// - `display_name`: Name to be displayed within the "who's speaking" text.
    /// - `params`: Parameters that will be forwarded to the source which gets created.
    pub fn new(
        pipeline: &gst::Pipeline,
        display_name: String,
        src_params: SRC::Parameters,
    ) -> Self {
        Self {
            display_name,
            source: SRC::new(pipeline, src_params),
            audio_mixer_pad: None,
            video_link_status: VideoLinkStatus::None,
        }
    }
}
