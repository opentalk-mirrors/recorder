use crate::Sink;
use gst::{
    prelude::*,
    traits::{ElementExt, GstBinExt},
};

/// Displays compositor output on the screen.
pub struct DisplaySink {
    /// Video sink pad.
    video_sink_pad: gst::Pad,
    /// Audio sink pad.
    audio_sink_pad: gst::Pad,
}

impl Sink for DisplaySink {
    /// Needs no parameters.
    type Parameters = ();

    /// Create and add new display sink into existing pipeline.
    fn new(pipeline: &gst::Pipeline, _: ()) -> Self {
        trace!("new()");
        assert_eq!(pipeline.current_state(), gst::State::Null);

        // create video and audio sink
        let video_sink =
            gst::ElementFactory::make_with_name("xvimagesink", Some("display-video-sink"))
                .expect("failed to create xvimagesink");
        let audio_sink =
            gst::ElementFactory::make_with_name("pulsesink", Some("display-audio-sink"))
                .expect("failed to create pulsesink");

        // add sinks to pipeline
        pipeline
            .add(&video_sink)
            .expect("failed to add video display sink to pipeline");
        pipeline
            .add(&audio_sink)
            .expect("failed to add audio display sink to pipeline");

        // return new display sink
        DisplaySink {
            video_sink_pad: video_sink
                .static_pad("sink")
                .expect("failed to get sink pad of video display sink"),
            audio_sink_pad: audio_sink
                .static_pad("sink")
                .expect("failed to get sink pad of audio display sink"),
        }
    }

    /// Get video sink pad.
    fn video_sink_pad(&self) -> gst::Pad {
        self.video_sink_pad.clone()
    }

    /// Get audio sink pad.
    fn audio_sink_pad(&self) -> gst::Pad {
        self.audio_sink_pad.clone()
    }
}
