// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

//! Stream status.

use core::fmt::Debug;

use gst_base::prelude::*;
use types::signaling::media::MediaSessionState;

use crate::{Source, TextOverlay};

/// Represents a stream.
///
/// # Types
///
/// - `SRC`: Source type which implements trait [Source]
///
#[derive(Debug)]
pub struct Stream<SRC>
where
    SRC: Source + Debug,
{
    /// Name to be displayed within the subtitle text.
    pub display_name: String,
    /// Wrapped AV source of this stream.
    pub source: SRC,
    // the bin of the source
    pub bin: gst::Bin,
    // the video src ghost pad
    pub video: Option<gst::GhostPad>,
    // the audio src ghost pad
    pub audio: gst::GhostPad,
    // source's overlay
    pub overlay: TextOverlay,
    /// current stream status
    pub status: MediaSessionState,
}

impl<SRC> Stream<SRC>
where
    SRC: Source + Debug,
    SRC::Parameters: Debug,
{
    /// Find compositor sink by looking where our ghost pad is connected to.
    pub fn compositor_sink(&self) -> Option<gst::Pad> {
        self.video.clone().and_then(|video| video.target())
    }

    /// Get the videoconvertscale `Pad` from the stream.
    pub fn videoconvertscale(&self) -> Option<gst::Element> {
        self.bin.by_name("videoconvertscale")
    }

    /// Get the capsfilter `Pad` from the stream.
    pub fn capsfilter(&self) -> Option<gst::Element> {
        self.bin.by_name("capsfilter")
    }

    /// Find audiomixer sink by looking where our ghost pad is connected to.
    pub fn audiomixer_sink(&self) -> Option<gst::Pad> {
        self.audio.target()
    }
}
