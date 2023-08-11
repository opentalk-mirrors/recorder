use super::Size;

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
    /// - `pipeline`: Pipeline to insert the source into.
    /// - `params`: Source's proprietary parameters.
    ///
    fn new<ID>(
        id: &ID,
        pipeline: &gst::Pipeline,
        resolution: &Size,
        params: Self::Parameters,
    ) -> Self
    where
        ID: std::fmt::Display;

    /// Return the source's bin.
    fn bin(&self) -> gst::Bin;

    /// Return video source pad of element `inp` if available.
    fn video_inp_pad(&self) -> Option<gst::Pad>;

    /// Return audio source pad of element `inp` if available.
    fn audio_inp_pad(&self) -> Option<gst::Pad>;

    /// return true if source currently is delivering video content
    fn is_video_connected(&self) -> bool {
        true
    }

    /// return true if source currently is delivering audio content
    fn is_audio_connected(&self) -> bool {
        true
    }
}
