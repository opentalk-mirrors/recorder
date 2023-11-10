// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use std::fmt::Display;

use crate::{add_ghost_pad, Size, Source};

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
#[derive(Clone, Debug)]
pub struct TestSource {
    bin: gst::Bin,
    video_src: gst::GhostPad,
    audio_src: gst::GhostPad,
}

/// Specific parameters needed to create a [`TestSource`]
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
    /// [`TestSource`]'s default parameters
    fn default() -> Self {
        Self {
            pattern: Pattern::Smpte,
            resolution: Size::SD,
            name: None,
        }
    }
}

impl Source for TestSource {
    /// Forward parameters to [`Source`]'s generic type
    type Parameters = TestSourceParameters;

    /// Create a new [`TestSource`] and add it to the given pipeline.
    fn new<ID>(id: &ID, params: Self::Parameters) -> TestSource
    where
        ID: Display,
    {
        trace!("new( {id}, {params:?} )",);

        // create bin including codecs and the dash sink
        let bin = gst::parse_bin_from_description(
            &(format!(
                r#"
                name="Test Input Source: {id}"
                "#,
            ) + &if let Pattern::Location(location) = params.pattern {
                format!(
                    r#"
                    filesrc
                        name="Picture File Loader"
                        location={location}
                    ! pngdec
                        name="PNG Picture Decoder"
                    ! textoverlay
                        name="Naming Overlay"
                        font-desc="Helvetica Bold 25"
                        valignment=center
                        halignment=center
                        text="{name}"
                        color=0xffffff80
                    ! imagefreeze
                        name="Video Generator"
                        is-live=true
                    ! queue
                        name="video"
                        max-size-time=2000000000
                    "#,
                    name = params.name.clone().unwrap_or_default()
                )
            } else {
                let pattern: &str = params.pattern.into();
                format!(
                    r#"
                        videotestsrc
                            name="Video Test Source"
                            pattern={pattern}
                            is-live=true
                        ! queue
                            name=video
                            max-size-time=2000000000
                        "#,
                )
            } + r#"
                audiotestsrc
                    name="Audio Test Source"
                    volume=0.01
                    is-live=true
                ! capssetter
                    name="Audio Capssetter"
                    caps=audio/x-raw,format=S16LE,channels=2,layout=interleaved,rate=48000
                ! queue
                    name=audio
                    max-size-time=2000000000
            "#),
            false,
        )
        .expect("failed to create test source bin");

        TestSource {
            // remember elements and pads for connect/disconnect
            video_src: add_ghost_pad(&bin, "video", "src"),
            audio_src: add_ghost_pad(&bin, "audio", "src"),
            bin,
        }
    }

    fn bin(&self) -> gst::Bin {
        self.bin.clone()
    }
    fn video(&self) -> gst::GhostPad {
        self.video_src.clone()
    }
    fn audio(&self) -> gst::GhostPad {
        self.audio_src.clone()
    }
}
