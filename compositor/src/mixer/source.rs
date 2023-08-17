//! Source trait.

/// Trait of a participant's audio/video source.
pub trait Source {
    /// Generic parameter type to overwrite by trait implementers.
    type Parameters;

    /// Create an add a new source to a pipeline.
    ///
    /// Creates a bunch of elements based on given parameters and adds them to the pipeline.
    ///
    /// # Arguments
    ///
    /// - `id`: Stream identifier under which this stream can be addressed later.
    /// - `params`: Source's proprietary parameters.
    ///
    fn new<ID>(id: &ID, params: Self::Parameters) -> Self
    where
        ID: std::fmt::Display;

    /// Return the source's bin.
    fn bin(&self) -> gst::Bin;

    /// return true if source currently is delivering video content
    fn is_video_connected(&self) -> bool {
        true
    }

    /// return true if source currently is delivering audio content
    fn is_audio_connected(&self) -> bool {
        true
    }

    fn video(&self) -> gst::GhostPad;
    fn audio(&self) -> gst::GhostPad;
}
