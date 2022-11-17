use super::{Size, Source};
use gst::prelude::*;
use gst::traits::{ElementExt, GstBinExt};

/// Source that generates dummy picture and sound to simulate a participant's input.
#[derive(Clone)]
pub struct TestSource {
    bin: gst::Bin,
    pub video_src_pad: gst::Pad,
    pub audio_src_pad: gst::Pad,
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
    fn new(pipeline: &gst::Pipeline, params: TestSourceParameters) -> TestSource {
        let bin = format!(
            r#"
        videotestsrc pattern={pattern} is-live=true num-buffers=100 !
            video/x-raw,format=RGB,width={width},height={height} !
            queue name=video-output

        audiotestsrc volume=0.01 is-live=true !
            audio/x-raw,format=S16LE,channels=2,layout=interleaved,rate=48000 !
            queue name=audio-output
        "#,
            pattern = params.pattern,
            width = params.resolution.width,
            height = params.resolution.height,
        );

        let bin = gst::parse_bin_from_description(&bin, false).unwrap();

        let video_output = bin.by_name("video-output").unwrap();
        let video_output_pad = video_output.static_pad("src").unwrap();
        let video_output_pad = gst::GhostPad::with_target(None, &video_output_pad)
            .unwrap()
            .upcast();
        bin.add_pad(&video_output_pad).unwrap();

        let audio_output = bin.by_name("audio-output").unwrap();
        let audio_output_pad = audio_output.static_pad("src").unwrap();
        let audio_output_pad = gst::GhostPad::with_target(None, &audio_output_pad)
            .unwrap()
            .upcast();
        bin.add_pad(&audio_output_pad).unwrap();
        pipeline.add(&bin).unwrap();

        Self {
            bin,
            video_src_pad: video_output_pad,
            audio_src_pad: audio_output_pad,
        }
    }

    /// remove elements from pipeline
    fn remove(self, pipeline: &gst::Pipeline) {
        pipeline.remove(&self.bin).unwrap();
        self.bin.set_state(gst::State::Null).unwrap();
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
