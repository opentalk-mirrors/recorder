use crate::*;
use gst::traits::{ElementExt, GstBinExt};
use std::fmt::Display;

/// Video test patterns.
#[derive(Clone, Debug)]
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
    bin: gst::Bin,
    video_inp_pad: gst::Pad,
    audio_inp_pad: gst::Pad,
}

/// Specific parameters needed to create a [TestSource]
#[derive(Clone, Debug)]
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
    fn new<ID>(
        id: &ID,
        pipeline: &gst::Pipeline,
        resolution: &Size,
        params: TestSourceParameters,
    ) -> TestSource
    where
        ID: Display,
    {
        trace!("new( {id} {resolution:?}, {params:?} )",);

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
        debug!("Padding TestSource to {width}:{height}");

        use std::cmp::min;
        let (out_width, out_height) =
            (min(resolution.width, width), min(resolution.height, height));
        debug!("Resizing TestSource to {out_width}:{out_height}");

        // create bin including codecs and the dash sink
        let bin = gst::parse_bin_from_description(
            &(format!(
                r#"
                name="participant_{id}"
                "#,
            ) + &match params.pattern {
                Pattern::Location(location) => format!(
                    r#"
                    filesrc
                        location={location}
                    ! pngdec
                    ! textoverlay
                        font-desc="Helvetica Bold 25"
                        valignment=center
                        halignment=center
                        text="{name}"
                        color=0xffffff80
                    ! videoconvert
                    ! videoscale
                    ! capssetter
                        caps=video/x-raw,format=RGB,width={out_width},height={out_height}
                    ! imagefreeze
                        name=video-inp
                        is-live=true
                    ! queue
                        name=video-out
                        max-size-time=2000000000
                    "#,
                    name = params.name.clone().unwrap_or_default()
                ),

                _ => {
                    let pattern: &str = params.pattern.into();
                    format!(
                        r#"
                        videotestsrc
                            name=video-inp
                            pattern={pattern}
                            is-live=true
                        ! capssetter
                            caps=video/x-raw,format=RGB,width={width},height={height}
                        ! videoscale
                        ! capssetter
                            caps=video/x-raw,format=RGB,width={out_width},height={out_height}
                        ! queue
                            name=video-out
                            max-size-time=2000000000
                        "#,
                    )
                }
            } + r#"
                audiotestsrc
                    name=audio-inp
                    volume=0.01
                    is-live=true
                ! capssetter
                    caps=audio/x-raw,format=S16LE,channels=2,layout=interleaved,rate=48000
                ! queue
                    name=audio-out
                    max-size-time=2000000000
            "#),
            false,
        )
        .expect("failed to create test source bin");

        // add video elements to pipeline
        pipeline.add(&bin).unwrap();

        // get elements from bin
        let video_inp = bin.by_name("video-inp").unwrap();
        let video_inp_pad = video_inp.static_pad("src").unwrap();
        // get elements from bin
        let video_out = bin.by_name("video-out").unwrap();
        let video_out_pad = video_out.static_pad("src").unwrap();

        let audio_inp = bin.by_name("audio-inp").unwrap();
        let audio_inp_pad = audio_inp.static_pad("src").unwrap();
        let audio_out = bin.by_name("audio-out").unwrap();
        let audio_out_pad = audio_out.static_pad("src").unwrap();

        let video_out_pad = gst::GhostPad::with_target(Some("video"), &video_out_pad)
            .expect("failed to create ghost pad for webrtc video output");
        let audio_out_pad = gst::GhostPad::with_target(Some("audio"), &audio_out_pad)
            .expect("failed to create ghost pad for webrtc audio output");

        bin.add_pad(&video_out_pad)
            .expect("failed to add video output ghost pad to webrtc bin");
        bin.add_pad(&audio_out_pad)
            .expect("failed to add audio output ghost pad to webrtc bin");

        TestSource {
            // remember elements and pads for connect/disconnect
            bin,
            video_inp_pad,
            audio_inp_pad,
        }
    }

    fn video_inp_pad(&self) -> Option<gst::Pad> {
        Some(self.video_inp_pad.clone())
    }

    fn audio_inp_pad(&self) -> Option<gst::Pad> {
        Some(self.audio_inp_pad.clone())
    }

    fn bin(&self) -> gst::Bin {
        self.bin.clone()
    }
}
