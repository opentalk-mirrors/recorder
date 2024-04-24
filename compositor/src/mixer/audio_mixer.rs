// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use crate::{
    mixer::{AUDIO_CHANNELS, AUDIO_SAMPLE_RATE},
    GstBinErrorExt, GstElementBuilderErrorExt, GstElementErrorExt, GstGhostPadErrorExt,
    GstPadErrorExt,
};
use anyhow::{Context, Result};
use gst::{
    element_error, prelude::*, Bin, Caps, Element, ElementFactory, FlowError, FlowSuccess,
    GhostPad, Pad, Sample, StreamError,
};
use gst_app::{AppSink, AppSinkCallbacks, AppSrc};
use gst_base::AggregatorStartTimeSelection;
use tokio::sync::broadcast;

const QUEUE_SIZE: usize = 128; // expect a buffers of 10ms -> 1s queue size

#[derive(Debug)]
pub(crate) struct AudioMixer {
    bin: Bin,
    audiomixer: Element,
    buffer: broadcast::Sender<Sample>,
    appsink: AppSink,
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
    pub(crate) fn create() -> Result<Self> {
        let bin = Bin::new(Some("AudioMixer"));

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
        let appsink = AppSink::builder().sync(true).build();

        bin.add_many_with_context(&[
            &audiotestsrc,
            &clocksync,
            &audiotestsrc_capssetter,
            &audiomixer,
            &audiomixer_capssetter,
            &queue,
            appsink.upcast_ref(),
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
            appsink.upcast_ref(),
        ])?;

        let buffer = broadcast::Sender::new(QUEUE_SIZE);
        let sender = buffer.clone();
        appsink.set_callbacks(
            AppSinkCallbacks::builder()
                .new_sample({
                    move |app_sink| match app_sink.pull_sample() {
                        Ok(sample) => {
                            sender.send(sample).ok();
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
            appsink,
        })
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
                        FlowError::Eos => {
                            debug!("Eos and exit app_src {src_name}");
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

impl Drop for AudioMixer {
    fn drop(&mut self) {
        self.appsink.set_callbacks(
            AppSinkCallbacks::builder()
                .new_sample(|_| Ok(FlowSuccess::Ok))
                .build(),
        );
    }
}
