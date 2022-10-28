use super::Sink;
use gst::traits::{ElementExt, GstBinExt};
use gstreamer as gst;

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
    #[allow(dead_code)]
    fn new(pipeline: &gst::Pipeline, _: ()) -> Self {
        // create video and audio sink
        let video_sink =
            gst::ElementFactory::make("xvimagesink", Some("display-video-sink")).unwrap();
        let audio_sink =
            gst::ElementFactory::make("pulsesink", Some("display-audio-sink")).unwrap();

        // add sinks to pipeline
        pipeline.add(&video_sink).unwrap();
        pipeline.add(&audio_sink).unwrap();

        // return new display sink
        DisplaySink {
            video_sink_pad: video_sink.static_pad("sink").unwrap(),
            audio_sink_pad: audio_sink.static_pad("sink").unwrap(),
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
