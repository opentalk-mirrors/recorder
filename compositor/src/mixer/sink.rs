// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

//! Sink trait.

use anyhow::{Context, Result};
use gst::{GhostPad, Pipeline};
use gst_base::prelude::{ElementExt, GstBinExt};
use std::fmt::Debug;

use crate::debug;

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

    /// Called by `Mixer::drop()`.
    ///
    /// # Errors
    ///
    /// This cannot fail, it's doing nothing.
    fn on_exit(&mut self, _pipeline: &gst::Pipeline) -> Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct ActiveSink {
    pub(crate) pipeline: Pipeline,
    pub(crate) sink: Box<dyn Sink>,
}

impl Drop for ActiveSink {
    fn drop(&mut self) {
        debug!("Dropping Sink...");
        debug::debug_dot(&self.pipeline, "SINK-DROP");

        debug!("Stop Sink...");
        if let Err(error) = self.sink.on_exit(&self.pipeline) {
            error!("Unable to call on_exit on every output_sink, error: {error}");
        }

        debug!("Nulling Pipeline...");
        if let Err(error) = self.pipeline.set_state(gst::State::Null) {
            error!("Unable to set the pipeline to the `Null` state, error: {error}");
        }

        debug!("Exited Sink.");
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
        .by_name(name)
        .with_context(|| format!("unable to find element '{name}'"))?
        .static_pad(pad)
        .with_context(|| format!("unable to find pad '{pad}' for element '{name}'"))?;
    let ghost_pad =
        GhostPad::with_target(Some(name), &pad).context("failed to create ghost pad for pad")?;
    bin.add_pad(&ghost_pad)
        .context("unable to add GhostPad to bin")?;

    Ok(ghost_pad)
}
