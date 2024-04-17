// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use crate::mixer::VIDEO_FRAMERATE;
use crate::{
    GstBinErrorExt, GstElementBuilderErrorExt, GstElementErrorExt, GstGhostPadErrorExt,
    GstPadErrorExt, Overlay, Size,
};
use anyhow::{Context, Result};
use gst::{
    element_error, prelude::*, Bin, Caps, Element, ElementFactory, FlowError, FlowSuccess,
    GhostPad, Pad, Sample, StreamError,
};
use gst_app::{AppSink, AppSinkCallbacks, AppSrc};
use gst_base::AggregatorStartTimeSelection;
use tokio::sync::broadcast;

const QUEUE_SIZE: usize = VIDEO_FRAMERATE as usize;

#[derive(Debug)]
pub(crate) struct VideoMixer {
    bin: Bin,
    compositor: Element,
    buffer: broadcast::Sender<Sample>,
    appsink: AppSink,
}

impl VideoMixer {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn create(output_size: Size, overlay: &impl Overlay) -> Result<Self> {
        let bin = Bin::new(Some("VideoMixer"));

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
                    .field("format", "I420")
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
        let appsink: AppSink = AppSink::builder().sync(true).build();

        bin.add_many_with_context(&[
            &videotestsrc,
            &clocksync,
            &videotestsrc_capssetter,
            &compositor,
            &overlay.element(),
            &queue,
            appsink.upcast_ref(),
        ])?;

        Element::link_many_with_context(&[&videotestsrc, &clocksync, &videotestsrc_capssetter])?;

        let compositor_sink_pad = compositor.request_pad_simple_with_context("sink_%u")?;
        videotestsrc_capssetter
            .static_pad_with_context("src")?
            .link_with_context(&compositor_sink_pad)?;

        Element::link_many_with_context(&[
            &compositor,
            &overlay.element(),
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
            compositor,
            buffer,
            appsink,
        })
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

impl Drop for VideoMixer {
    fn drop(&mut self) {
        self.appsink.set_callbacks(
            AppSinkCallbacks::builder()
                .new_sample(|_| Ok(FlowSuccess::Ok))
                .build(),
        );
    }
}
