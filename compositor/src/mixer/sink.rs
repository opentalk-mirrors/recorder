pub trait SinkBuilder {
    /// Create an add a sink to the pipeline.
    ///
    /// Creates a bunch of elements based on given parameters and adds them to the pipeline.
    fn build(&self, pipeline: &gst::Pipeline) -> Box<dyn Sink>;
}

/// Trait of an output sink.
pub trait Sink: Send + 'static {
    /// Get sink pad of the video sink.
    fn video_sink_pad(&self) -> gst::Pad;

    /// Get sink pad of the audio sink.
    fn audio_sink_pad(&self) -> gst::Pad;

    /// Called by `Mixer::play()`.
    fn on_play(&mut self) {}

    /// Called by `Mixer::pause()`.
    fn on_pause(&mut self) {}

    /// Called by `Mixer::drop()`.
    fn on_exit(&mut self, _pipeline: &gst::Pipeline) {}
}
