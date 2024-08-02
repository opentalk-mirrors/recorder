// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use anyhow::{Context, Result};
use gst::{prelude::*, Bin, Caps, Element, ElementFactory, GhostPad, Pad};
use gst_base::AggregatorStartTimeSelection;

use crate::{
    mixer::{AUDIO_CHANNELS, AUDIO_SAMPLE_RATE},
    GstBinErrorExt, GstElementBuilderErrorExt, GstElementErrorExt, GstGhostPadErrorExt,
    GstPadErrorExt, AUDIO_INTER_COMPOSITOR,
};

#[derive(Debug)]
pub(crate) struct AudioMixer {
    bin: Bin,
    audiomixer: Element,
}

impl AudioMixer {
    #[track_caller]
    fn build_caps() -> Result<Element> {
        ElementFactory::make("capssetter")
            .property(
                "caps",
                Caps::builder("audio/x-raw")
                    .field("format", "S16LE")
                    .field("channels", AUDIO_CHANNELS)
                    .field("layout", "interleaved")
                    .field("rate", AUDIO_SAMPLE_RATE)
                    .build(),
            )
            .build_with_context()
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn create(producer_id: u64) -> Result<Self> {
        let bin = Bin::builder().name("AudioMixer").build();
        let producer_name = format!("{AUDIO_INTER_COMPOSITOR}_{producer_id}",);

        let audiotestsrc = ElementFactory::make("audiotestsrc")
            .name("Audio Background Source")
            .property("is-live", true)
            .property("volume", 0.0)
            .build_with_context()?;
        let clocksync = ElementFactory::make("clocksync")
            .name("Audio Background Clocksync")
            .build_with_context()?;
        let audiotestsrc_capssetter =
            Self::build_caps().context("unable to build audiotestsrc_capssetter")?;

        let audiomixer = ElementFactory::make("audiomixer")
            .name("audio-mixer")
            .property("ignore-inactive-pads", true)
            .property("start-time-selection", AggregatorStartTimeSelection::First)
            .build_with_context()?;

        let audiomixer_capssetter = Self::build_caps()?;

        let queue = ElementFactory::make("queue").build_with_context()?;
        let intersink = ElementFactory::make("intersink")
            .property("producer-name", producer_name.as_str())
            .build_with_context()?;

        bin.add_many_with_context(&[
            &audiotestsrc,
            &clocksync,
            &audiotestsrc_capssetter,
            &audiomixer,
            &audiomixer_capssetter,
            &queue,
            &intersink,
        ])?;

        Element::link_many_with_context(&[&audiotestsrc, &clocksync, &audiotestsrc_capssetter])?;

        let audiomixer_sink_pad = audiomixer.request_pad_simple_with_context("sink_%u")?;
        audiotestsrc_capssetter
            .static_pad_with_context("src")?
            .link_with_context(&audiomixer_sink_pad)?;

        Element::link_many_with_context(&[
            &audiomixer,
            &audiomixer_capssetter,
            &queue,
            &intersink,
        ])?;

        Ok(Self { bin, audiomixer })
    }

    #[must_use]
    pub(crate) fn bin(&self) -> &Bin {
        &self.bin
    }

    pub(crate) fn link_src(&self, src: &impl IsA<Pad>) -> Result<GhostPad> {
        let requested_pad = self.audiomixer.request_pad_simple_with_context("sink_%u")?;

        let ghost_pad = GhostPad::with_target_with_context(None, &requested_pad)?;

        self.bin.add_pad_with_context(&ghost_pad)?;

        src.link_with_context(&ghost_pad)?;

        Ok(ghost_pad)
    }

    pub(crate) fn release_src(&self, src: &impl IsA<Pad>) -> Result<()> {
        if let Some(proxy_pad) = src.peer() {
            for ghost_pad in proxy_pad.iterate_internal_links() {
                let ghost_pad =
                    ghost_pad.context("unable to get ghost_pad from proxy_pad iterator")?;
                self.bin.remove_pad_with_context(&ghost_pad)?;
            }
        }

        self.audiomixer.release_request_pad(src);

        Ok(())
    }
}
