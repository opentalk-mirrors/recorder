// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use anyhow::{Context, Result};
use gst::{prelude::*, Bin, Caps, Element, ElementFactory, GhostPad, Pad};

#[derive(Debug)]
pub(crate) struct AudioMixer {
    bin: Bin,
    audiomixer: Element,
    tee: Element,
}

impl AudioMixer {
    pub(crate) fn create() -> Result<Self> {
        let bin = Bin::new(Some("AudioMixer"));

        let audiotestsrc = ElementFactory::make("audiotestsrc")
            .name("Audio Background Source")
            .property("is-live", true)
            .property("volume", 0.0)
            .build()
            .context("unable to build audiotestsrc")?;
        let capssetter = ElementFactory::make("capssetter")
            .name("Audio Background Capssetter")
            .property(
                "caps",
                Caps::builder("audio/x-raw")
                    .field("format", "S16LE")
                    .field("channels", 2)
                    .field("layout", "interleaved")
                    .field("rate", 48_000)
                    .build(),
            )
            .build()
            .context("unable to build capssetter")?;

        let audiomixer = ElementFactory::make("audiomixer")
            .name("audio-mixer")
            .property("ignore-inactive-pads", true)
            .build()
            .context("unable to build audiomixer")?;
        let tee = ElementFactory::make("tee")
            .name("audio-tee")
            .property("allow-not-linked", true)
            .build()
            .context("unable to build tee")?;

        bin.add_many(&[&audiotestsrc, &capssetter, &audiomixer, &tee])
            .context(
                "unable to add 'audiotestsrc', 'capssetter', 'audiomixer' and 'tee' to 'bin'",
            )?;

        audiotestsrc
            .link(&capssetter)
            .context("unable to link 'audiotestsrc' with 'capssetter'")?;

        let audiomixer_sink_pad = audiomixer
            .request_pad_simple("sink_%u")
            .context("unable to request sink pad for audiomixer")?;
        capssetter
            .static_pad("src")
            .context("unable to get static pad src from capssetter")?
            .link(&audiomixer_sink_pad)
            .context("unable to link audio_requested_pad with capssetter")?;

        audiomixer
            .link(&tee)
            .context("unable to link 'audiomixer' and 'tee'")?;

        Ok(Self {
            bin,
            audiomixer,
            tee,
        })
    }

    #[must_use]
    pub(crate) fn bin(&self) -> &Bin {
        &self.bin
    }

    pub(crate) fn link_src(&self, src: &impl IsA<Pad>) -> Result<GhostPad> {
        let requested_pad = self
            .audiomixer
            .request_pad_simple("sink_%u")
            .context("unable to request 'sink' pad for 'audiomixer'")?;

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

        self.audiomixer.release_request_pad(src);

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
