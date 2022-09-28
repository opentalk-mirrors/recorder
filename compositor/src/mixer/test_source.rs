use super::mixer::Source;
use crate::layout::*;
use gst::{
    prelude::GObjectExtManualGst,
    traits::{ElementExt, GstBinExt},
};
use gstreamer as gst;

#[derive(Clone)]
pub struct TestSource {
    pub video_fake_sink: Option<gst::Element>,
    pub video_sink_pad: gst::Pad,
    pub video_src_pad: gst::Pad,
    pub video_src_element: gst::Element,
    pub audio_fake_sink: Option<gst::Element>,
    pub audio_sink_pad: gst::Pad,
    pub audio_src_pad: gst::Pad,
    pub audio_src_element: gst::Element,
}

impl Source for TestSource {
    #[allow(dead_code)]
    fn new(pipeline: &gst::Pipeline, name: &str, pattern: &str, resolution: &Size) -> TestSource {
        trace!("create new TestSource '{name}'");

        let width = resolution.width;
        let height = resolution.height;

        // create video test src
        let video_test_src =
            gst::ElementFactory::make("videotestsrc", Some(&format!("video-testsrc-{name}")))
                .unwrap();
        video_test_src.set_property_from_str("pattern", pattern);
        video_test_src.set_property_from_str("is-live", "true");

        // create video caps setter
        let video_caps =
            gst::ElementFactory::make("capssetter", Some(&format!("video-caps-{name}"))).unwrap();
        video_caps.set_property_from_str(
            "caps",
            &format!("video/x-raw,format=RGB,width={width},height={height}",),
        );

        let video_queue =
            gst::ElementFactory::make("queue", Some(&format!("video-src-{name}"))).unwrap();

        // create video fake sink
        let video_fake_sink =
            gst::ElementFactory::make("fakesink", Some(&format!("fakesink-{name}"))).unwrap();
        video_fake_sink.set_property_from_str("sync", "true");

        // add video elements to pipeline
        pipeline.add(&video_test_src).unwrap();
        pipeline.add(&video_caps).unwrap();
        pipeline.add(&video_queue).unwrap();
        pipeline.add(&video_fake_sink).unwrap();

        // link video elements
        video_test_src.link(&video_caps).unwrap();
        video_caps.link(&video_queue).unwrap();
        video_queue.link(&video_fake_sink).unwrap();

        // create audio test src
        let audio_test_src =
            gst::ElementFactory::make("audiotestsrc", Some(&format!("audio-testsrc-{name}")))
                .unwrap();
        audio_test_src.set_property_from_str("volume", "0.01");
        audio_test_src.set_property_from_str("is-live", "true");

        // create audio caps setter
        let audio_caps =
            gst::ElementFactory::make("capssetter", Some(&format!("audio-caps-{name}"))).unwrap();
        audio_caps.set_property_from_str(
            "caps",
            &format!("audio/x-raw,format=S16LE,channels=2,layout=interleaved,rate=48000",),
        );

        // create audio queue
        let audio_queue =
            gst::ElementFactory::make("queue", Some(&format!("audio-src-{name}"))).unwrap();

        // create audio fake sink
        let audio_fake_sink =
            gst::ElementFactory::make("fakesink", Some(&format!("audio-fakesink-{name}"))).unwrap();
        audio_fake_sink.set_property_from_str("sync", "true");

        // add audio elements to pipeline
        pipeline.add(&audio_test_src).unwrap();
        pipeline.add(&audio_caps).unwrap();
        pipeline.add(&audio_queue).unwrap();
        pipeline.add(&audio_fake_sink).unwrap();

        // link audio elements
        audio_test_src.link(&audio_caps).unwrap();
        audio_caps.link(&audio_queue).unwrap();
        audio_queue.link(&audio_fake_sink).unwrap();

        TestSource {
            // remember elements and pads for connect/disconnect
            video_src_pad: video_queue.static_pad("src").unwrap(),
            video_src_element: video_test_src,
            video_sink_pad: video_fake_sink.static_pad("sink").unwrap(),
            video_fake_sink: Some(video_fake_sink),
            audio_src_pad: audio_queue.static_pad("src").unwrap(),
            audio_src_element: audio_test_src,
            audio_sink_pad: audio_fake_sink.static_pad("sink").unwrap(),
            audio_fake_sink: Some(audio_fake_sink),
        }
    }
    fn video_src_element(&self) -> &gst::Element {
        &self.video_src_element
    }
    fn video_src_pad(&self) -> &gst::Pad {
        &self.video_src_pad
    }
    fn video_sink_pad(&self) -> &gst::Pad {
        &self.video_sink_pad
    }
    fn video_fake_sink(&self) -> &Option<gst::Element> {
        &self.video_fake_sink
    }
    fn set_video_sink_pad(&mut self, sink_pad: gst::Pad) {
        self.video_sink_pad = sink_pad;
    }
    fn set_video_fake_sink(&mut self, fake_sink: Option<gst::Element>) {
        self.video_fake_sink = fake_sink;
    }
    fn audio_src_element(&self) -> &gst::Element {
        &self.audio_src_element
    }
    fn audio_src_pad(&self) -> &gst::Pad {
        &self.audio_src_pad
    }
    fn audio_sink_pad(&self) -> &gst::Pad {
        &self.audio_sink_pad
    }
    fn audio_fake_sink(&self) -> &Option<gst::Element> {
        &self.audio_fake_sink
    }
    fn set_audio_sink_pad(&mut self, sink_pad: gst::Pad) {
        self.audio_sink_pad = sink_pad;
    }
    fn set_audio_fake_sink(&mut self, fake_sink: Option<gst::Element>) {
        self.audio_fake_sink = fake_sink;
    }
}
