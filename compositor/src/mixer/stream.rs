use super::{Size, Source};
use crate::Overlay;
use core::fmt::{Debug, Display};

/// Status of a stream if it is linked to a fake sink or the compositor.
#[derive(Debug)]
pub enum LinkStatus {
    /// Video source is unlinked
    None,
    /// Video source is not linked to the mixer
    Unlinked(gst::Element),
    /// Video source is linked to the mixer
    Linked(gst::Element),
}

impl LinkStatus {
    pub fn valve(self) -> Option<gst::Element> {
        match self {
            LinkStatus::Linked(valve) => Some(valve),
            LinkStatus::Unlinked(valve) => Some(valve),
            _ => None,
        }
    }
}

/// Turns on or off video or audio.
#[derive(Debug, Clone)]
pub struct StreamStatus {
    /// stream currently provides audio
    pub has_audio: bool,
    /// stream currently provides video
    pub has_video: bool,
}

impl StreamStatus {
    pub fn none() -> Self {
        Self {
            has_audio: false,
            has_video: false,
        }
    }
    pub fn audio() -> Self {
        Self {
            has_audio: true,
            has_video: false,
        }
    }
    pub fn video() -> Self {
        Self {
            has_audio: false,
            has_video: true,
        }
    }
}

impl Default for StreamStatus {
    fn default() -> Self {
        Self {
            has_audio: true,
            has_video: true,
        }
    }
}

impl Display for StreamStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (self.has_video, self.has_audio) {
            (true, false) => write!(f, "video only"),
            (true, true) => write!(f, "audio/video"),
            (false, true) => write!(f, "audio only"),
            (false, false) => write!(f, "no media"),
        }
    }
}

/// Represents a stream.
///
/// # Types
///
/// - `SRC`: Source type which implements trait [Source]
///
#[derive(Debug)]
pub struct Stream<SRC>
where
    SRC: Source,
    SRC::Parameters: Debug,
{
    /// Name to be displayed within the sub title text.
    pub display_name: String,
    /// Wrapped AV source of this stream.
    pub source: SRC,
    /// Video link status of this stream.
    pub video_link_status: LinkStatus,
    /// Video link status of this stream.
    pub audio_link_status: LinkStatus,
    /// current stream status
    pub status: StreamStatus,
    /// Overlays
    pub overlays: Vec<Overlay>,
}

impl<SRC> Stream<SRC>
where
    SRC: Source,
    SRC::Parameters: Debug,
{
    /// Create new stream and a source of type `SRC` into the given GStreamer pipeline.
    ///
    /// # Arguments
    ///
    /// - `pipeline`: Pipeline to add GStreamer elements into.
    /// - `id`: Unique ID of the stream.
    /// - `display_name`: Name to be displayed within the sub title text.
    /// - `params`: Parameters that will be forwarded to the source which gets created.
    ///
    pub fn new<ID>(
        id: &ID,
        pipeline: &gst::Pipeline,
        resolution: &Size,
        display_name: String,
        params: SRC::Parameters,
        overlays: Vec<Overlay>,
    ) -> Self
    where
        ID: Display,
    {
        trace!("new( {resolution:?}, {display_name:?}, {params:?} )");
        Self {
            display_name,
            source: SRC::new(id, pipeline, resolution, params),
            video_link_status: LinkStatus::None,
            audio_link_status: LinkStatus::None,
            status: StreamStatus {
                has_audio: true,
                has_video: true,
            },
            overlays,
        }
    }
}
