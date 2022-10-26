use super::Sink;
use gst::traits::{ElementExt, GstBinExt};
use gstreamer as gst;

pub struct DisplaySink {
    video_sink_pad: gst::Pad,
    audio_sink_pad: gst::Pad,
}

impl Sink for DisplaySink {
    type Parameters = ();
    #[allow(dead_code)]
    fn new(pipeline: &gst::Pipeline, _: ()) -> Self {
        let video_sink =
            gst::ElementFactory::make("xvimagesink", Some("display-video-sink")).unwrap();
        let audio_sink =
            gst::ElementFactory::make("pulsesink", Some("display-audio-sink")).unwrap();

        pipeline.add(&video_sink).unwrap();
        pipeline.add(&audio_sink).unwrap();

        DisplaySink {
            video_sink_pad: video_sink.static_pad("sink").unwrap(),
            audio_sink_pad: audio_sink.static_pad("sink").unwrap(),
        }
    }

    fn video_sink_pad(&self) -> gst::Pad {
        self.video_sink_pad.clone()
    }

    fn audio_sink_pad(&self) -> gst::Pad {
        self.audio_sink_pad.clone()
    }
}
