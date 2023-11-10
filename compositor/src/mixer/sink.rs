// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

//! Sink trait.

use std::fmt::Debug;

use gst_base::prelude::{ElementExt, GstBinExt};

use crate::debug;

/// Trait of an output sink.
pub trait Sink: Send + Debug + 'static {
    /// Get sink pad of the video sink.
    fn video(&self) -> gst::GhostPad;

    /// Get sink pad of the audio sink.
    fn audio(&self) -> gst::GhostPad;

    fn bin(&self) -> gst::Bin;

    /// Called by `Mixer::play()`.
    fn on_play(&mut self) {}

    /// Called by `Mixer::pause()`.
    fn on_pause(&mut self) {}

    /// Called by `Mixer::drop()`.
    fn on_exit(&mut self, _pipeline: &gst::Pipeline) {}
}

/// Adds a `GhostPad` to the given `Bin`.
///
/// # Panics
///
/// This can panic if creating and adding the `Ghostpad` is failing.
#[allow(clippy::must_use_candidate)]
pub fn add_ghost_pad(bin: &gst::Bin, name: &str, pad: &str) -> gst::GhostPad {
    trace!(
        "add_ghost_pad({bin}, {name}, {pad}) ",
        bin = debug::name(bin)
    );
    // add ghost pad connected to video sink pad
    let ghost_pad = gst::GhostPad::with_target(
        Some(name),
        &bin.by_name(name)
            .expect("can not find element to ghost")
            .static_pad(pad)
            .expect("failed to get pad of element to ghost"),
    )
    .expect("failed to create ghost pad for pad");
    bin.add_pad(&ghost_pad)
        .expect("cannot add ghost pad to bin");
    ghost_pad
}
