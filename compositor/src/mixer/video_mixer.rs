// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use anyhow::{Context, Result};
use gst::{
    element_error, prelude::*, Bin, Caps, Element, ElementFactory, FlowError, FlowSuccess,
    GhostPad, Pad, Sample, StreamError,
};
use gst_app::{AppSink, AppSinkCallbacks, AppSrc};
use tokio::sync::broadcast;

use crate::mixer::VIDEO_FRAMERATE;
use crate::{Overlay, Size};

const QUEUE_SIZE: usize = VIDEO_FRAMERATE as usize;
#[derive(Debug)]
pub(crate) struct VideoMixer {
    bin: Bin,
    compositor: Element,
    buffer: broadcast::Sender<Sample>,
}

impl VideoMixer {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn create(output_size: Size, overlay: &impl Overlay) -> Result<Self> {
        let bin = Bin::new(Some("VideoMixer"));

        let videotestsrc = ElementFactory::make("videotestsrc")
            .name("Video Background Source")
            .property_from_str("pattern", "black")
            .property("is-live", true)
            .build()
            .context("unable to build videotesetsrc_videotestsrc")?;
        let videotestsrc_capssetter = ElementFactory::make("capssetter")
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

        let queue = ElementFactory::make("queue")
            .build()
            .context("unable to build queue")?;
        let appsink: AppSink = AppSink::builder().build();

        bin.add_many(&[
            &videotestsrc,
            &videotestsrc_capssetter,
            &compositor,
            &overlay.element(),
            &queue,
            appsink.upcast_ref(),
        ])
        .context("unable to add 'videotestsrc', 'videotestsrc_capssetter', 'compositor', 'queue'  and 'appsink' to 'bin'")?;

        videotestsrc
            .link(&videotestsrc_capssetter)
            .context("unable to link 'videotestsrc' with 'capssetter'")?;

        let compositor_sink_pad = compositor
            .request_pad_simple("sink_%u")
            .context("unable to request sink pad for compositor")?;
        videotestsrc_capssetter
            .static_pad("src")
            .context("unable to get static pad src from capssetter")?
            .link(&compositor_sink_pad)
            .context("unable to link compositor_requested_pad with capssetter")?;

        Element::link_many(&[
            &compositor,
            &overlay.element(),
            &queue,
            appsink.upcast_ref(),
        ])
        .context("unable to link 'compositor', 'overlay', 'queue' and 'appsink'")?;

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
            compositor,
            buffer,
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
