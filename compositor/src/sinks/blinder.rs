// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use anyhow::Result;
use gst::GhostPad;
use gst_base::prelude::*;

use crate::{
    parse_bin_from_description_with_context, GstBinErrorExt, GstElementBuilderErrorExt,
    GstElementErrorExt, GstGhostPadErrorExt, GstPadErrorExt, Sink, Size, TestSourceParameters,
};

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
        let bin = parse_bin_from_description_with_context(
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
        )?;

        bin.add_with_context(&params.sink.bin())?;

        let video_selector = bin.by_name_with_context("video-selector")?;
        let video_blind_sink = video_selector.static_pad_with_context("sink_0")?;
        let video_signal_sink = video_selector.request_pad_simple_with_context("sink_%u")?;
        let video = GhostPad::with_target_with_context(Some("video"), &video_signal_sink)?;
        bin.add_pad_with_context(&video)?;

        if let Some(video_sink) = &params.sink.video() {
            video_selector
                .static_pad_with_context("src")?
                .link_with_context(video_sink)?;
        } else {
            let fakesink = gst::ElementFactory::make("fakesink").build_with_context()?;
            bin.add_with_context(&fakesink)?;
            let fakesink_sink_pad = fakesink.static_pad_with_context("sink")?;
            video_selector
                .static_pad_with_context("src")?
                .link_with_context(&fakesink_sink_pad)?;
            fakesink.sync_state_with_parent_with_context()?;
        }

        let audio_selector = bin.by_name_with_context("audio-selector")?;
        let audio_blind_sink = audio_selector.static_pad_with_context("sink_0")?;
        let audio_signal_sink = audio_selector.request_pad_simple_with_context("sink_%u")?;
        let audio = GhostPad::with_target_with_context(Some("audio-sink"), &audio_signal_sink)?;
        bin.add_pad_with_context(&audio)?;
        audio_selector
            .static_pad_with_context("src")?
            .link_with_context(&params.sink.audio())?;

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
