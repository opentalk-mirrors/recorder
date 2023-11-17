// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use anyhow::{Context, Result};
use gst_base::prelude::*;

use crate::{Sink, Size, TestSourceParameters};

/// Trait to use blinders
/// @TODO: move out of this file if more blinders exist
pub trait Blinder {
    /// set blinder on or off
    fn blind(&self, blind: bool);
}

/// Parameters of `BlinderSink`
#[allow(dead_code)]
pub struct TestBlinderParams {
    pub name: &'static str,
    pub resolution: Size,
    pub sink: Box<dyn Sink>,
    pub alt_source_params: TestSourceParameters,
}

/// Blinder which selects between two sources - one original and an alternative.
#[derive(Debug, Clone)]
pub struct TestBlinder {
    video: gst::GhostPad,
    video_selector: gst::Element,
    video_signal_sink: gst::Pad,
    video_blind_sink: gst::Pad,
    audio: gst::GhostPad,
    audio_selector: gst::Element,
    audio_signal_sink: gst::Pad,
    audio_blind_sink: gst::Pad,
    bin: gst::Bin,
}

impl Blinder for TestBlinder {
    fn blind(&self, blind: bool) {
        if blind {
            self.video_selector
                .set_property("active-pad", self.video_blind_sink.clone());
            self.audio_selector
                .set_property("active-pad", self.audio_blind_sink.clone());
        } else {
            self.video_selector
                .set_property("active-pad", self.video_signal_sink.clone());
            self.audio_selector
                .set_property("active-pad", self.audio_signal_sink.clone());
        }
    }
}

impl Sink for TestBlinder {
    fn bin(&self) -> gst::Bin {
        self.bin.clone()
    }

    fn video(&self) -> Option<gst::GhostPad> {
        Some(self.video.clone())
    }

    fn audio(&self) -> gst::GhostPad {
        self.audio.clone()
    }
}

impl TestBlinder {
    /// Create new blinder sink.
    ///
    /// # Errors
    ///
    /// This can throw an error if the underlaying `GStreamer` is having
    /// trouble.
    pub fn create(params: &TestBlinderParams) -> Result<Self> {
        let bin = gst::parse_bin_from_description(
            &format!(
                r#"
            name="Test Blinder"

            videotestsrc
                pattern=black
            ! video/x-raw,width={width},height={height}
            ! input-selector
                name=video-selector

            audiotestsrc
                volume=0.0
            ! input-selector
                name=audio-selector
            "#,
                width = params.resolution.width,
                height = params.resolution.height
            ),
            false,
        )
        .context("could not create blinder bin")?;

        bin.add(&params.sink.bin())
            .context("could not add target sink to bin")?;

        let video_selector = bin
            .by_name("video-selector")
            .context("can not file video input selector")?;
        let video_blind_sink = video_selector
            .static_pad("sink_0")
            .context("could not get video selector blind sink")?;
        let video_signal_sink = video_selector
            .request_pad_simple("sink_%u")
            .context("could not get sink at video input selector")?;
        let video = gst::GhostPad::with_target(Some("video"), &video_signal_sink)
            .context("failed to create video ghost pad for participant overlay sink")?;
        bin.add_pad(&video)
            .context("failed to add video ghost pad to participant overlay bin")?;

        if let Some(video_sink) = &params.sink.video() {
            video_selector
                .static_pad("src")
                .context("could not get src pad from video input selector")?
                .link(video_sink)
                .context("failed to link video selector with target sink")?;
        } else {
            let fakesink = gst::ElementFactory::make("fakesink").build()?;
            bin.add(&fakesink)
                .context("unable to add `fakesink` to `bin`")?;
            let fakesink_sink_pad = fakesink
                .static_pad("sink")
                .context("unable to get static pad `sink` from `fakesink`")?;
            video_selector
                .static_pad("src")
                .context("could not get src pad from video input selector")?
                .link(&fakesink_sink_pad)
                .context("failed to link video selector with target sink")?;
            fakesink
                .sync_state_with_parent()
                .context("unable to sync `fakesink` with parent")?;
        }

        let audio_selector = bin
            .by_name("audio-selector")
            .context("can not file audio input selector")?;
        let audio_blind_sink = audio_selector
            .static_pad("sink_0")
            .context("could not get audio selector blind sink")?;
        let audio_signal_sink = audio_selector
            .request_pad_simple("sink_%u")
            .context("could not get sink at audio input selector")?;
        let audio = gst::GhostPad::with_target(Some("audio-sink"), &audio_signal_sink)
            .context("failed to create audio ghost pad for blinder overlay sink")?;
        bin.add_pad(&audio)
            .context("failed to add audio ghost pad to blinder overlay bin")?;
        audio_selector
            .static_pad("src")
            .context("could not get src pad from audio input selector")?
            .link(&params.sink.audio())
            .context("failed to link audio selector with target sink")?;

        Ok(Self {
            video,
            video_selector,
            video_signal_sink,
            video_blind_sink,
            audio,
            audio_selector,
            audio_signal_sink,
            audio_blind_sink,
            bin,
        })
    }
}
