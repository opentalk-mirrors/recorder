use crate::{Size, Source};
use gst::prelude::*;
use gst::traits::{ElementExt, GstBinExt};

#[derive(Clone)]
#[allow(dead_code)]
pub enum Pattern {
    /// image file
    Location(String),
    /// SMPTE 100%% color bars
    Smpte,
    /// Random (television snow)
    Snow,
    /// 100%% Black
    Black,
    /// 100%% White
    White,
    /// Red
    Red,
    /// Green
    Green,
    /// Blue
    Blue,
    /// Checkers 1px
    Checkers1,
    /// Checkers 2px
    Checkers2,
    /// Checkers 4px
    Checkers4,
    /// Checkers 8px
    Checkers8,
    /// Circular
    Circular,
    /// Blink
    Blink,
    /// SMPTE 75%% color bars
    Smpte75,
    /// Zone plate
    ZonePlate,
    /// Gamut checkers
    Gamut,
    /// Chroma zone plate
    ChromaZonePlate,
    /// Solid color
    SolidColor,
    /// Moving ball
    Ball,
    /// SMPTE 100%% color bars
    Smpte100,
    /// Bar
    Bar,
    /// Pinwheel
    PinWheel,
    /// Spokes
    Spokes,
    /// Gradient
    Gradient,
    /// Colors
    Colors,
    /// SMPTE test pattern, RP 219 conformant
    SmpteRp219,
}

impl From<Pattern> for &'static str {
    fn from(s: Pattern) -> &'static str {
        match s {
            Pattern::Location(_) => panic!("location can not be used as pattern!"),
            Pattern::Smpte => "smpte",
            Pattern::Snow => "snow",
            Pattern::Black => "black",
            Pattern::White => "white",
            Pattern::Red => "red",
            Pattern::Green => "green",
            Pattern::Blue => "blue",
            Pattern::Checkers1 => "checkers-1",
            Pattern::Checkers2 => "checkers-2",
            Pattern::Checkers4 => "checkers-4",
            Pattern::Checkers8 => "checkers-8",
            Pattern::Circular => "circular",
            Pattern::Blink => "blink",
            Pattern::Smpte75 => "smpte75",
            Pattern::ZonePlate => "zone-plate",
            Pattern::Gamut => "gamut",
            Pattern::ChromaZonePlate => "chroma-zone-plate",
            Pattern::SolidColor => "solid-color",
            Pattern::Ball => "ball",
            Pattern::Smpte100 => "smpte100",
            Pattern::Bar => "bar",
            Pattern::PinWheel => "pinwheel",
            Pattern::Spokes => "spokes",
            Pattern::Gradient => "gradient",
            Pattern::Colors => "colors",
            Pattern::SmpteRp219 => "smpte-rp-219",
        }
    }
}

/// Source that generates dummy picture and sound to simulate a participant's input.
#[derive(Clone)]
pub struct TestSource {
    /// Video source GStreamer pad.
    pub video_src_pad: gst::Pad,
    /// Video source GStreamer element.
    pub video_bin: gst::Bin,
    /// Audio source GStreamer pad.
    pub audio_src_pad: gst::Pad,
    /// Audio source GStreamer element.
    pub audio_bin: gst::Bin,
}

/// Specific parameters needed to create a [TestSource]
#[derive(Clone)]
pub struct TestSourceParameters {
    /// Pattern to produce
    pub pattern: Pattern,
    /// Resolution of the generated picture.
    pub resolution: Size,
    // name that will be display as overlay
    pub name: Option<String>,
}

impl Default for TestSourceParameters {
    /// [TestSource]'s default parameters
    fn default() -> Self {
        Self {
            pattern: Pattern::Smpte,
            resolution: Size::SD,
            name: None,
        }
    }
}

impl Source for TestSource {
    /// Forward parameters to [Source]'s generic type
    type Parameters = TestSourceParameters;

    /// Create a new [TestSource] and add it to the given pipeline.
    fn new(
        pipeline: &gst::Pipeline,
        resolution: &Size,
        params: TestSourceParameters,
    ) -> TestSource {
        trace!(
            "create new TestSource, resolution: (WxH){}:{}",
            params.resolution.width,
            params.resolution.height
        );

        // substitute parameters for easy us with format!()
        let (width, height) = if resolution.ratio() == params.resolution.ratio() {
            (params.resolution.width, params.resolution.height)
        } else if resolution.ratio() > params.resolution.ratio() {
            (
                (params.resolution.height as f64 * resolution.ratio()) as usize,
                params.resolution.height,
            )
        } else {
            (
                params.resolution.width,
                (params.resolution.width as f64 / resolution.ratio()) as usize,
            )
        };
        trace!("padding TestSource to {width}:{height}");

        use std::cmp::min;
        let (out_width, out_height) =
            (min(resolution.width, width), min(resolution.height, height));
        trace!("resizing TestSource to {out_width}:{out_height}");

        // create bin including codecs and the dash sink
        let video_bin = match params.pattern {
            Pattern::Location(location) => gst::parse_bin_from_description(
                &format!(
                    r#"
                    filesrc
                        location={location}
                    ! pngdec
                    ! videoconvert
                    ! imagefreeze
                        is-live=true
                    ! videobox
                        fill=black
                        autocrop=true
                    ! capssetter
                        caps=video/x-raw,format=RGB,width={width},height={height}
                    ! videoscale
                    ! capssetter
                        caps=video/x-raw,format=RGB,width={out_width},height={out_height}
                    ! textoverlay
                        font-desc="Helvetica Bold 25"
                        valignment=center
                        halignment=center
                        text="{name}"
                        color=0xffffff80
                    ! queue
                        name=video-testsrc
                    "#,
                    name = params.name.unwrap_or_default()
                ),
                false,
            ),
            _ => {
                let pattern: &str = params.pattern.into();
                gst::parse_bin_from_description(
                    &format!(
                        r#"
                        videotestsrc
                            pattern={pattern}
                            is-live=true
                        ! capssetter
                            caps=video/x-raw,format=RGB,width={width},height={height}
                        ! videoscale
                        ! capssetter
                            caps=video/x-raw,format=RGB,width={out_width},height={out_height}
                        ! queue
                            name=video-testsrc
                        "#
                    ),
                    false,
                )
            }
        }
        .unwrap();
        // add video elements to pipeline
        pipeline.add(&video_bin).unwrap();

        // get elements from bin
        let video = video_bin.by_name("video-testsrc").unwrap();

        // create ghost pads which link to codecs
        let video_src_ghostpad =
            gst::GhostPad::with_target(None, &video.static_pad("src").unwrap()).unwrap();
        video_bin.add_pad(&video_src_ghostpad).unwrap();

        let audio_bin = gst::parse_bin_from_description(
            r#"
                audiotestsrc
                    volume=0.01
                    is-live=true
                ! capssetter
                    caps=audio/x-raw,format=S16LE,channels=2,layout=interleaved,rate=48000
                ! queue
                    name=audio-testsrc
            "#,
            false,
        )
        .unwrap();
        // add audio elements to pipeline
        pipeline.add(&audio_bin).unwrap();

        // get elements from bin
        let audio = audio_bin.by_name("audio-testsrc").unwrap();

        // create ghost pads which link to codecs
        let audio_src_ghostpad =
            gst::GhostPad::with_target(None, &audio.static_pad("src").unwrap()).unwrap();
        audio_bin.add_pad(&audio_src_ghostpad).unwrap();

        TestSource {
            // remember elements and pads for connect/disconnect
            video_src_pad: video_src_ghostpad.upcast::<gst::Pad>(),
            video_bin,
            audio_src_pad: audio_src_ghostpad.upcast::<gst::Pad>(),
            audio_bin,
        }
    }

    /// remove elements from pipeline
    fn remove(self, pipeline: &gst::Pipeline) {
        // remove video elements from pipeline
        pipeline.remove(&self.video_bin).unwrap();
        self.video_bin.set_state(gst::State::Null).unwrap();
        // remove audio elements
        pipeline.remove(&self.audio_bin).unwrap();
        self.audio_bin.set_state(gst::State::Null).unwrap();
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
