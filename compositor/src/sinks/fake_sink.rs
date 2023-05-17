use crate::{Sink, SinkBuilder};
use gst::{
    prelude::*,
    traits::{ElementExt, GstBinExt},
};

/// Fake sink to catch the compositor output without any further processing.
#[derive(Debug)]
pub struct FakeSink {
    /// Video sink pad.
    video_sink_pad: gst::Pad,
    /// Audio sink pad.
    audio_sink_pad: gst::Pad,
}

#[derive(Default)]
pub struct FakeSinkBuilder();

impl SinkBuilder for FakeSinkBuilder {
    fn build(&self, pipeline: &gst::Pipeline) -> Box<dyn Sink> {
        Box::new(FakeSink::new(pipeline))
    }
}
impl FakeSink {
    /// Create and add new fake sink into existing pipeline.
    pub fn new(pipeline: &gst::Pipeline) -> Self {
        trace!("new()");
        assert_eq!(pipeline.current_state(), gst::State::Null);

        // create video and audio sink
        let video_sink =
            gst::ElementFactory::make_with_name("fakevideosink", Some("fake-video-sink"))
                .expect("failed to create video fakesink");
        let audio_sink =
            gst::ElementFactory::make_with_name("fakeaudiosink", Some("fake-audio-sink"))
                .expect("failed to create audio fakesink");

        // add sinks to pipeline
        pipeline
            .add(&video_sink)
            .expect("failed to add video fake sink to pipeline");
        pipeline
            .add(&audio_sink)
            .expect("failed to add video fake sink to pipeline");

        // return new display sink
        FakeSink {
            video_sink_pad: video_sink
                .static_pad("sink")
                .expect("failed to get sink pad of video fake sink"),
            audio_sink_pad: audio_sink
                .static_pad("sink")
                .expect("failed to get sink pad of audio fake sink"),
        }
    }
}

impl Sink for FakeSink {
    /// Get video sink pad.
    fn video_sink_pad(&self) -> gst::Pad {
        self.video_sink_pad.clone()
    }

    /// Get audio sink pad.
    fn audio_sink_pad(&self) -> gst::Pad {
        self.audio_sink_pad.clone()
    }
}
