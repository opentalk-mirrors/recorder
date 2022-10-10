use super::mixer::Source;
use crate::layout::*;
use gst::{
    prelude::*,
    traits::{ElementExt, GstBinExt},
};
use gstreamer as gst;

#[derive(Clone)]
pub struct TestSource {
    pub video_src_pad: gst::Pad,
    pub video_src_element: gst::Element,
    video_caps: gst::Element,
    pub audio_src_pad: gst::Pad,
    pub audio_src_element: gst::Element,
    audio_caps: gst::Element,
}

impl Source for TestSource {
    type Parameters = (String, &'static str, Size);

    fn new(pipeline: &gst::Pipeline, (name, pattern, resolution): Self::Parameters) -> TestSource {
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

        // add video elements to pipeline
        pipeline.add(&video_test_src).unwrap();
        pipeline.add(&video_caps).unwrap();

        // link video elements
        video_test_src.link(&video_caps).unwrap();

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
            "audio/x-raw,format=S16LE,channels=2,layout=interleaved,rate=48000",
        );

        // add audio elements to pipeline
        pipeline.add(&audio_test_src).unwrap();
        pipeline.add(&audio_caps).unwrap();

        // link audio elements
        audio_test_src.link(&audio_caps).unwrap();

        TestSource {
            // remember elements and pads for connect/disconnect
            video_src_pad: video_caps.static_pad("src").unwrap(),
            video_src_element: video_test_src,
            video_caps,
            audio_src_pad: audio_caps.static_pad("src").unwrap(),
            audio_src_element: audio_test_src,
            audio_caps,
        }
    }
    /// remove elements from pipeline
    fn remove(self, pipeline: &gst::Pipeline) {
        self.video_src_element.unlink(&self.video_caps);

        // remove video elements from pipeline
        pipeline.remove(&self.video_src_element).unwrap();
        pipeline.remove(&self.video_caps).unwrap();

        // unlink audio elements
        self.audio_src_element.unlink(&self.audio_caps);
        self.audio_src_element.set_state(gst::State::Null).unwrap();
        pipeline.remove(&self.audio_src_element).unwrap();
        pipeline.remove(&self.audio_caps).unwrap();
    }

    fn video_src_pad(&self) -> gst::Pad {
        self.video_src_pad.clone()
    }

    fn audio_src_pad(&self) -> gst::Pad {
        self.audio_src_pad.clone()
    }
}
