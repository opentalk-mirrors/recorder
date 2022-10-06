use super::mixer::Source;
use crate::layout::*;
use gst::{
    prelude::{ElementExtManual, GObjectExtManualGst},
    traits::{ElementExt, GstBinExt},
};
use gstreamer as gst;

#[derive(Clone)]
pub struct TestSource {
    pub video_fake_sink: Option<gst::Element>,
    video_sink: gst::Element,
    pub video_sink_pad: gst::Pad,
    pub video_src_pad: gst::Pad,
    pub video_src_element: gst::Element,
    video_caps: gst::Element,
    pub audio_fake_sink: Option<gst::Element>,
    audio_sink: gst::Element,
    pub audio_sink_pad: gst::Pad,
    pub audio_src_pad: gst::Pad,
    pub audio_src_element: gst::Element,
    audio_caps: gst::Element,
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

        // create video fake sink
        let video_fake_sink =
            gst::ElementFactory::make("fakesink", Some(&format!("fakesink-{name}"))).unwrap();
        video_fake_sink.set_property_from_str("sync", "true");

        // add video elements to pipeline
        pipeline.add(&video_test_src).unwrap();
        pipeline.add(&video_caps).unwrap();
        pipeline.add(&video_fake_sink).unwrap();

        // link video elements
        video_test_src.link(&video_caps).unwrap();
        video_caps.link(&video_fake_sink).unwrap();

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

        // create audio fake sink
        let audio_fake_sink =
            gst::ElementFactory::make("fakesink", Some(&format!("audio-fakesink-{name}"))).unwrap();
        audio_fake_sink.set_property_from_str("sync", "true");

        // add audio elements to pipeline
        pipeline.add(&audio_test_src).unwrap();
        pipeline.add(&audio_caps).unwrap();
        pipeline.add(&audio_fake_sink).unwrap();

        // link audio elements
        audio_test_src.link(&audio_caps).unwrap();
        audio_caps.link(&audio_fake_sink).unwrap();

        TestSource {
            // remember elements and pads for connect/disconnect
            video_src_pad: video_caps.static_pad("src").unwrap(),
            video_src_element: video_test_src,
            video_sink_pad: video_fake_sink.static_pad("sink").unwrap(),
            video_sink: video_fake_sink.clone(),
            video_fake_sink: Some(video_fake_sink),
            video_caps,
            audio_src_pad: audio_caps.static_pad("src").unwrap(),
            audio_src_element: audio_test_src,
            audio_sink_pad: audio_fake_sink.static_pad("sink").unwrap(),
            audio_sink: audio_fake_sink.clone(),
            audio_fake_sink: Some(audio_fake_sink),
            audio_caps,
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
    fn set_video_sink(&mut self, sink_pad: gst::Pad, sink: gst::Element) {
        self.video_sink_pad = sink_pad;
        self.video_sink = sink;
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
    fn set_audio_sink(&mut self, sink_pad: gst::Pad, sink: gst::Element) {
        self.audio_sink_pad = sink_pad;
        self.audio_sink = sink;
    }
    fn set_audio_fake_sink(&mut self, fake_sink: Option<gst::Element>) {
        self.audio_fake_sink = fake_sink;
    }
    /// remove elements from pipeline
    fn remove(&self, pipeline: &gst::Pipeline) {
        self.video_src_element.unlink(&self.video_caps);
        self.video_caps.unlink(&self.video_sink);

        // remove video elements from pipeline
        if let Some(video_fake_sink) = &self.video_fake_sink {
            pipeline.remove(video_fake_sink).unwrap();
        }
        pipeline.remove(&self.video_src_element).unwrap();
        if let Some(audio_fake_sink) = &self.audio_fake_sink {
            pipeline.remove(audio_fake_sink).unwrap();
        }
        pipeline.remove(&self.video_caps).unwrap();

        // unlink audio elements
        self.audio_src_element.unlink(&self.audio_caps);
        self.audio_caps.unlink(&self.audio_sink);
        self.audio_src_element.set_state(gst::State::Null).unwrap();
        pipeline.remove(&self.audio_src_element).unwrap();
        pipeline.remove(&self.audio_caps).unwrap();
    }
}
