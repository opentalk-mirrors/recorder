/// Trait of a participant's audio/video source.
pub trait Source {
    /// Generic parameter type to overwrite by trait implementers.
    type Parameters;
    /// Create an add a new source to a pipeline.
    /// Creates a bunch of elements based on given parameters and adds them to the pipeline.
    fn new(pipeline: &gst::Pipeline, id: String, params: Self::Parameters) -> Self;
    /// Remove existing source from pipeline.
    /// Decouples and removes all elements from the pipeline which are created within this source.
    fn remove(self, pipeline: &gst::Pipeline);
    /// Get source pad of the video source.
    fn video_src_pad(&self) -> gst::Pad;
    /// Get source pad of the audio source.
    fn audio_src_pad(&self) -> gst::Pad;
}
