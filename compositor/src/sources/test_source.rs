use super::{Size, Source};
use gst::prelude::*;
use gst::traits::{ElementExt, GstBinExt};

/// Source that generates dummy picture and sound to simulate a participant's input.
#[derive(Clone)]
pub struct TestSource {
    /// Video source GStreamer pad.
    pub video_src_pad: gst::Pad,
    /// Video source GStreamer element pad.
    pub video_src_element: gst::Element,
    /// GStreamer element to manage video caps.
    video_caps: gst::Element,
    /// Audio source GStreamer pad.
    pub audio_src_pad: gst::Pad,
    /// Audio source GStreamer element pad.
    pub audio_src_element: gst::Element,
    /// GStreamer element to manage audio caps.
    audio_caps: gst::Element,
}

/// Specific parameters needed to create a [TestSource]
#[derive(Clone)]
pub struct TestSourceParameters {
    /// Pattern to produce
    /// (see: [this list](https://gstreamer.freedesktop.org/documentation/videotestsrc/index.html?gi-language=c#GstVideoTestSrcPattern)).
    pub pattern: String,
    /// Resolution of the generated picture.
    pub resolution: Size,
}

impl Default for TestSourceParameters {
    /// [TestSource]'s default parameters
    fn default() -> Self {
        Self {
            pattern: "smpte".into(),
            resolution: Size::SD,
        }
    }
}

impl Source for TestSource {
    /// Forward parameters to [Source]'s generic type
    type Parameters = TestSourceParameters;

    /// Create a new [TestSource] and add it to the given pipeline.
    fn new(pipeline: &gst::Pipeline, id: String, params: TestSourceParameters) -> TestSource {
        trace!("create new TestSource '{}'", id);

        let width = params.resolution.width;
        let height = params.resolution.height;
        let pattern = params.pattern;

        // create video test src
        let video_test_src = gst::ElementFactory::make_with_name(
            "videotestsrc",
            Some(&format!("video-testsrc-{id}")),
        )
        .unwrap();
        video_test_src.set_property_from_str("pattern", &pattern);
        video_test_src.set_property_from_str("is-live", "true");
        video_test_src.set_property_from_str("num-buffers", "100");

        // create video caps setter
        let video_caps =
            gst::ElementFactory::make_with_name("capssetter", Some(&format!("video-caps-{id}")))
                .unwrap();
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
        let audio_test_src = gst::ElementFactory::make_with_name(
            "audiotestsrc",
            Some(&format!("audio-testsrc-{id}")),
        )
        .unwrap();
        audio_test_src.set_property_from_str("volume", "0.01");
        audio_test_src.set_property_from_str("is-live", "true");

        // create audio caps setter
        let audio_caps =
            gst::ElementFactory::make_with_name("capssetter", Some(&format!("audio-caps-{id}")))
                .unwrap();
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
        // remove video elements from pipeline
        pipeline.remove(&self.video_src_element).unwrap();
        pipeline.remove(&self.video_caps).unwrap();

        // remove audio elements
        pipeline.remove(&self.audio_src_element).unwrap();
        pipeline.remove(&self.audio_caps).unwrap();
    }

    /// Get video source pad.
    fn video_src_pad(&self) -> gst::Pad {
        self.video_src_pad.clone()
    }

    /// Get audio source pad.
    fn audio_src_pad(&self) -> gst::Pad {
        self.audio_src_pad.clone()
    }
}
