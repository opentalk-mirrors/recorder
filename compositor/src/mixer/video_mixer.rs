// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use anyhow::{Context, Result};
use gst::{prelude::*, Bin, Caps, Element, ElementFactory, GhostPad, Pad};
use gst_base::AggregatorStartTimeSelection;

use crate::{
    GstBinErrorExt, GstElementBuilderErrorExt, GstElementErrorExt, GstGhostPadErrorExt,
    GstPadErrorExt, Overlay, Size, VIDEO_INTER_COMPOSITOR,
};

// const QUEUE_SIZE: usize = VIDEO_FRAMERATE as usize;

#[derive(Debug)]
pub(crate) struct VideoMixer {
    bin: Bin,
    compositor: Element,
}

impl VideoMixer {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn create(
        output_size: Size,
        overlay: &impl Overlay,
        producer_id: u64,
    ) -> Result<Self> {
        let bin = Bin::builder().name("VideoMixer").build();
        let producer_name = format!("{VIDEO_INTER_COMPOSITOR}_{producer_id}");

        let videotestsrc = ElementFactory::make("videotestsrc")
            .name("Video Background Source")
            .property_from_str("pattern", "black")
            .property("is-live", true)
            .build_with_context()?;
        let clocksync = ElementFactory::make("clocksync")
            .name("Video Background Clocksync")
            .build_with_context()?;
        let videotestsrc_capssetter = ElementFactory::make("capssetter")
            .property(
                "caps",
                Caps::builder("video/x-raw")
                    .field("width", output_size.width as i32)
                    .field("height", output_size.height as i32)
                    .build(),
            )
            .build_with_context()?;

        let compositor = ElementFactory::make("compositor")
            .name("compositor")
            .property("ignore-inactive-pads", true)
            .property("zero-size-is-unscaled", true)
            .property("start-time-selection", AggregatorStartTimeSelection::First)
            .build_with_context()?;

        let queue = ElementFactory::make("queue").build_with_context()?;
        let intersink = ElementFactory::make("intersink")
            .property("producer-name", producer_name.as_str())
            .build_with_context()?;

        bin.add_many_with_context(&[
            &videotestsrc,
            &clocksync,
            &videotestsrc_capssetter,
            &compositor,
            overlay.element(),
            &queue,
            &intersink,
        ])?;

        Element::link_many_with_context(&[&videotestsrc, &clocksync, &videotestsrc_capssetter])?;

        let compositor_sink_pad = compositor.request_pad_simple_with_context("sink_%u")?;
        videotestsrc_capssetter
            .static_pad_with_context("src")?
            .link_with_context(&compositor_sink_pad)?;

        Element::link_many_with_context(&[&compositor, overlay.element(), &queue, &intersink])?;

        Ok(Self { bin, compositor })
    }

    #[must_use]
    pub(crate) fn bin(&self) -> &Bin {
        &self.bin
    }

    pub(crate) fn link_src(&self, src: &impl IsA<Pad>) -> Result<GhostPad> {
        let requested_pad = self.compositor.request_pad_simple_with_context("sink_%u")?;
        requested_pad.set_property_from_str("sizing-policy", "keep-aspect-ratio");
        requested_pad.set_property("alpha", 0.0);

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

        self.compositor.release_request_pad(src);

        Ok(())
    }
}
