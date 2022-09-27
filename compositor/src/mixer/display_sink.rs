use crate::layout::*;
use gst::traits::{ElementExt, GstBinExt};
use gstreamer as gst;

pub struct DisplaySink {
    video_sink_pad: gst::Pad,
}

impl DisplaySink {
    #[allow(dead_code)]
    pub fn new(pipeline: &gst::Pipeline, _resolution: &Size) -> DisplaySink {
        let video_sink =
            gst::ElementFactory::make("xvimagesink", Some("display-video-sink")).unwrap();

        pipeline.add(&video_sink).unwrap();

        DisplaySink {
            video_sink_pad: video_sink.static_pad("sink").unwrap(),
        }
    }
    pub fn sink_pad(&self) -> &gst::Pad {
        &self.video_sink_pad
    }
}
