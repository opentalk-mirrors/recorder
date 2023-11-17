// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use anyhow::{Context, Result};
use gst::{prelude::*, Bin, Caps, Element, ElementFactory, GhostPad, Pad};

use crate::{Overlay, Size};

#[derive(Debug)]
pub(crate) struct VideoMixer {
    bin: Bin,
    compositor: Element,
    tee: Element,
}

impl VideoMixer {
    pub(crate) fn create(output_size: Size, overlay: &impl Overlay) -> Result<Self> {
        let bin = Bin::new(Some("VideoMixer"));

        let videotestsrc = ElementFactory::make("videotestsrc")
            .name("Video Background Source")
            .property_from_str("pattern", "black")
            .property("is-live", true)
            .build()
            .context("unable to build videotestsrc")?;
        let capssetter = ElementFactory::make("capssetter")
            .name("Video Background Capssetter")
            .property(
                "caps",
                Caps::builder("video/x-raw")
                    .field("format", "RGB")
                    .field("width", output_size.width as i32)
                    .field("height", output_size.height as i32)
                    .build(),
            )
            .build()
            .context("unable to build capssetter")?;

        let compositor = ElementFactory::make("compositor")
            .name("compositor")
            .property("ignore-inactive-pads", true)
            .property("zero-size-is-unscaled", true)
            .build()
            .context("unable to build compositor")?;
        let tee = ElementFactory::make("tee")
            .name("tee")
            .property("allow-not-linked", true)
            .build()
            .context("unable to build queue")?;

        bin.add_many(&[
            &videotestsrc,
            &capssetter,
            &compositor,
            &overlay.element(),
            &tee,
        ])
        .context("unable to add 'videotestsrc', 'capssetter', 'compositor' and 'tee' to 'bin'")?;

        videotestsrc
            .link(&capssetter)
            .context("unable to link 'videotestsrc' with 'capssetter'")?;

        let compositor_sink_pad = compositor
            .request_pad_simple("sink_%u")
            .context("unable to request sink pad for compositor")?;
        capssetter
            .static_pad("src")
            .context("unable to get static pad src from capssetter")?
            .link(&compositor_sink_pad)
            .context("unable to link compositor_requested_pad with capssetter")?;

        crate::debug::dot(&bin, "beforeoverlay");

        Element::link_many(&[&compositor, &overlay.element(), &tee])
            .context("unable to link 'compositor', 'overlay' and 'tee'")?;

        Ok(Self {
            bin,
            compositor,
            tee,
        })
    }

    #[must_use]
    pub(crate) fn bin(&self) -> &Bin {
        &self.bin
    }

    pub(crate) fn link_src(&self, src: &impl IsA<Pad>) -> Result<GhostPad> {
        let requested_pad = self
            .compositor
            .request_pad_simple("sink_%u")
            .context("unable to request 'sink' pad for 'compositor'")?;
        requested_pad.set_property_from_str("sizing-policy", "keep-aspect-ratio");
        requested_pad.set_property("alpha", 0.0);

        let ghost_pad = GhostPad::with_target(None, &requested_pad)
            .context("unable to create 'GhostPad' for 'src'")?;

        self.bin
            .add_pad(&ghost_pad)
            .context("unable to add 'ghost_pad' to 'bin'")?;

        src.link(&ghost_pad)
            .context("unable to link 'ghost_pad' with 'requested_pad'")?;

        Ok(ghost_pad)
    }

    pub(crate) fn release_src(&self, src: &impl IsA<Pad>) -> Result<()> {
        if let Some(proxy_pad) = src.peer() {
            for ghost_pad in proxy_pad.iterate_internal_links() {
                let ghost_pad =
                    ghost_pad.context("unable to get ghost_pad from proxy_pad iterator")?;
                self.bin
                    .remove_pad(&ghost_pad)
                    .context("unable to remove ghost_pad form bin")?;
            }
        }

        self.compositor.release_request_pad(src);

        Ok(())
    }

    pub(crate) fn link_sink(&self, sink: &impl IsA<Pad>) -> Result<GhostPad> {
        let requested_pad = self
            .tee
            .request_pad_simple("src_%u")
            .context("unable to request 'src' pad for 'tee'")?;

        let queue = ElementFactory::make("queue")
            .build()
            .context("unable to build queue")?;

        self.bin.add(&queue).context("unable to add queue to bin")?;

        let queue_sink = queue
            .static_pad("sink")
            .context("unable to get sink for queue")?;

        requested_pad
            .link(&queue_sink)
            .context("unable to link requested_pad with queue")?;

        let queue_src = queue
            .static_pad("src")
            .context("unable to get src for queue")?;

        let ghost_pad = GhostPad::with_target(None, &queue_src)
            .context("unable to create 'GhostPad' for 'queue src'")?;

        self.bin
            .add_pad(&ghost_pad)
            .context("unable to add 'ghost_pad' to 'bin'")?;

        ghost_pad
            .link(sink)
            .context("unable to link 'requested_pad' with 'ghost_pad'")?;

        Ok(ghost_pad)
    }

    // TODO: This function will be used in the future, when the sink is dynamically changed.
    #[allow(dead_code)]
    pub(crate) fn release_sink(&self, src: &impl IsA<Pad>) {
        self.tee.release_request_pad(src);
    }
}
