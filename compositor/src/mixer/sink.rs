// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

//! Sink trait.

use anyhow::Result;
use gst::{prelude::ElementExtManual, ClockTime, GhostPad, MessageType, Pipeline};
use gst_base::prelude::ElementExt;
use std::fmt::Debug;

use crate::{debug, GstBinErrorExt, GstElementErrorExt, GstGhostPadErrorExt};

/// Trait of an output sink.
pub trait Sink: Send + Debug + 'static {
    /// Get sink pad of the video sink.
    fn video(&self) -> Option<gst::GhostPad>;

    /// Get sink pad of the audio sink.
    fn audio(&self) -> gst::GhostPad;

    fn bin(&self) -> gst::Bin;

    /// Called by `Mixer::play()`.
    ///
    /// # Errors
    ///
    /// This cannot fail, it's doing nothing.
    fn on_play(&mut self) -> Result<()> {
        Ok(())
    }

    /// Called by `Mixer::pause()`.
    fn on_pause(&mut self) {}
}

#[derive(Debug)]
pub(crate) struct ActiveSink {
    pub(crate) pipeline: Pipeline,
    // The sink needs to be hold until it's dropped at the end
    pub(crate) _inner: Box<dyn Sink>,
}

impl Drop for ActiveSink {
    fn drop(&mut self) {
        debug!("Dropping ActiveSink...");
        debug::debug_dot(&self.pipeline, "SINK-DROP");

        debug!("Send EOS to pipeline");
        self.pipeline.send_event(gst::event::Eos::new());

        debug!("Wait for EOS to be done");
        if let Some(bus) = self.pipeline.bus() {
            if bus
                .timed_pop_filtered(ClockTime::NONE, &[MessageType::Eos])
                .is_none()
            {
                error!("unable to send the EOS");
            }
        } else {
            error!("Unable to send EOS, there is no bus in the pipeline");
        }

        debug!("Nulling Pipeline...");
        if let Err(error) = self.pipeline.set_state_with_context(gst::State::Null) {
            error!("Unable to set the pipeline to the `Null` state, error: {error}");
        }

        debug!("Nulling Pipeline completed, remove sink");
    }
}

/// Adds a `GhostPad` to the given `Bin`.
///
/// # Errors
///
/// There are three reasons why this could fail:
/// - The element name cannot be found in the bin.
/// - The pad cannot be found in the element.
/// - The `GhostPad` cannot be added to the bin.
#[allow(clippy::must_use_candidate)]
pub fn add_ghost_pad(bin: &gst::Bin, name: &str, pad: &str) -> Result<gst::GhostPad> {
    trace!(
        "add_ghost_pad({bin}, {name}, {pad}) ",
        bin = debug::name(bin)
    );
    let pad = bin
        .by_name_with_context(name)?
        .static_pad_with_context(pad)?;
    let ghost_pad = GhostPad::with_target_with_context(Some(name), &pad)?;
    bin.add_pad_with_context(&ghost_pad)?;

    Ok(ghost_pad)
}
