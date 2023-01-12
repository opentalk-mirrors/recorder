use crate::Overlay;

use super::{Size, Source};

/// Status of a stream if it is linked to a fake sink or the compositor.
#[derive(Debug)]
pub enum VideoLinkStatus {
    /// Video source is unlinked
    None,
    /// Video source is linked to this fakesink
    Fakesink(gst::Element),
    /// Video source is linked to the compositor
    Compositor(gst::Pad),
}

#[derive(Debug, Clone)]
pub struct StreamStatus {
    pub has_audio: bool,
    pub has_video: bool,
}

/// Represents a stream.
/// # Types
/// - `SRC`: Source type which implements trait [Source]
#[derive(Debug)]
pub struct Stream<SRC>
where
    SRC: Source,
{
    /// Name to be displayed within the sub title text.
    pub display_name: String,
    /// Wrapped AV source of this stream.
    pub source: SRC,
    /// Contains the pad this streams audio stream is linked to. None if its not linked.
    pub audio_mixer_pad: Option<gst::Pad>,
    /// Video link status of this stream.
    pub video_link_status: VideoLinkStatus,
    /// current streams status
    pub status: StreamStatus,
}

impl<SRC> Stream<SRC>
where
    SRC: Source,
{
    /// Create new stream and a source of type `SRC` into the given GStreamer pipeline.
    /// # Types
    /// - `SRC`: Source type which implements trait [Source]
    /// # Arguments
    /// - `pipeline`: Pipeline to add GStreamer elements into.
    /// - `id`: Unique ID of the stream.
    /// - `display_name`: Name to be displayed within the sub title text.
    /// - `params`: Parameters that will be forwarded to the source which gets created.
    pub fn new(
        pipeline: &gst::Pipeline,
        resolution: &Size,
        display_name: String,
        src_params: SRC::Parameters,
    ) -> Self {
        let source = SRC::new(pipeline, resolution, src_params);
        Self {
            display_name,
            source,
            audio_mixer_pad: None,
            video_link_status: VideoLinkStatus::None,
            status: StreamStatus {
                has_audio: true,
                has_video: true,
            },
        }
    }
    pub fn push_overlay(&mut self, overlay: Overlay) {
        self.source.overlays().push(overlay);
    }
}
