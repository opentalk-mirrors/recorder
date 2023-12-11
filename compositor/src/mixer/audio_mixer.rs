// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use crate::mixer::{AUDIO_CHANNELS, AUDIO_SAMPLE_RATE};
use anyhow::{Context, Result};
use glib::BoolError;
use gst::{
    element_error, prelude::*, Bin, Caps, Element, ElementFactory, FlowError, FlowSuccess,
    GhostPad, Pad, Sample, StreamError,
};
use gst_app::{AppSink, AppSinkCallbacks, AppSrc};
use tokio::sync::broadcast;

const QUEUE_SIZE: usize = 128; // expect a buffers of 10ms -> 1s queue size
#[derive(Debug)]
pub(crate) struct AudioMixer {
    bin: Bin,
    audiomixer: Element,
    buffer: broadcast::Sender<Sample>,
}

impl AudioMixer {
    fn build_caps() -> Result<Element, BoolError> {
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
            .build()
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn create() -> Result<Self> {
        let bin = Bin::new(Some("AudioMixer"));

        let audiotestsrc = ElementFactory::make("audiotestsrc")
            .name("Audio Background Source")
            .property("is-live", true)
            .property("volume", 0.0)
            .build()
            .context("unable to build audiotestsrc")?;
        let audiotestsrc_capssetter =
            Self::build_caps().context("unable to build audiotestsrc_capssetter")?;

        let audiomixer = ElementFactory::make("audiomixer")
            .name("audio-mixer")
            .property("ignore-inactive-pads", true)
            .build()
            .context("unable to build audiomixer")?;

        let audimixer_capssetter =
            Self::build_caps().context("unable to build audiomixer_capssetter")?;

        let queue = ElementFactory::make("queue")
            .build()
            .context("unable to build queue")?;
        let appsink = AppSink::builder().build();

        bin.add_many(&[&audiotestsrc, &audiotestsrc_capssetter, &audiomixer, &audimixer_capssetter, &queue,
             appsink.upcast_ref()])
            .context(
                "unable to add 'audiotestsrc', 'audiotestsrc_capssetter', 'audiomixer', 'audiomixer_capssetter', 'queue', 'clocksync' and 'appsink' to 'bin'",
            )?;

        audiotestsrc
            .link(&audiotestsrc_capssetter)
            .context("unable to link 'audiotestsrc' with 'capssetter'")?;

        let audiomixer_sink_pad = audiomixer
            .request_pad_simple("sink_%u")
            .context("unable to request sink pad for audiomixer")?;
        audiotestsrc_capssetter
            .static_pad("src")
            .context("unable to get static pad src from capssetter")?
            .link(&audiomixer_sink_pad)
            .context("unable to link audio_requested_pad with capssetter")?;

        Element::link_many(&[
            &audiomixer,
            &audimixer_capssetter,
            &queue,
            appsink.upcast_ref(),
        ])
        .context(
            "unable to link 'audiomixer', 'audimixer_capssetter', 'queue', 'clocksync' and 'appsink'",
        )?;

        let buffer = broadcast::Sender::new(QUEUE_SIZE);
        let sender = buffer.clone();
        appsink.set_callbacks(
            AppSinkCallbacks::builder()
                .new_sample({
                    move |app_sink| match app_sink.pull_sample() {
                        Ok(sample) => {
                            if let Err(error) = sender.send(sample) {
                                element_error!(
                                    app_sink,
                                    StreamError::Failed,
                                    ("unable to send sample to channel")
                                );
                                error!("unable to send sample to channel, received: {error}");
                                return Err(FlowError::Error);
                            }
                            Ok(FlowSuccess::Ok)
                        }
                        Err(error) => {
                            element_error!(
                                app_sink,
                                StreamError::Failed,
                                ("unable to pull sample from app_sink")
                            );
                            error!("unable to pull sample from app_sink, received: {error}");

                            Err(FlowError::Error)
                        }
                    }
                })
                .build(),
        );

        Ok(Self {
            bin,
            audiomixer,
            buffer,
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

    pub(crate) fn link_sink(&self, app_src: &AppSrc) {
        let mut receiver = self.buffer.subscribe();
        let app_src = app_src.clone();
        std::thread::spawn(move || {
            while let Ok(sample) = receiver.blocking_recv() {
                if let Err(error) = app_src.push_sample(&sample) {
                    let src_name = app_src.name();
                    match error {
                        FlowError::Flushing => {
                            debug!("Flush and exit app_src {src_name}");
                        }
                        _ => {
                            error!("Failed pushing sample to app_src {src_name} with error: {error:?}, sample: {sample:?}");
                        }
                    }
                    return;
                }
            }
        });
    }
}
