// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

//! Stream status.

use core::fmt::{Debug, Display};
use gst_base::prelude::*;

use crate::{AnyOverlay, Source};

/// Turns on or off video or audio.
#[derive(Debug, Clone)]
pub struct StreamStatus {
    /// stream currently provides audio
    pub has_audio: bool,
    /// stream currently provides video
    pub has_video: bool,
}

impl StreamStatus {
    #[must_use]
    pub fn none() -> Self {
        Self {
            has_audio: false,
            has_video: false,
        }
    }
    #[must_use]
    pub fn audio() -> Self {
        Self {
            has_audio: true,
            has_video: false,
        }
    }
    #[must_use]
    pub fn video() -> Self {
        Self {
            has_audio: false,
            has_video: true,
        }
    }
}

impl Default for StreamStatus {
    fn default() -> Self {
        Self {
            has_audio: true,
            has_video: true,
        }
    }
}

impl Display for StreamStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (self.has_video, self.has_audio) {
            (true, false) => write!(f, "video only"),
            (true, true) => write!(f, "audio/video"),
            (false, true) => write!(f, "audio only"),
            (false, false) => write!(f, "no media"),
        }
    }
}

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
    /// Name to be displayed within the sub title text.
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
    pub overlay: AnyOverlay,
    /// current stream status
    pub status: StreamStatus,
}

impl<SRC> Stream<SRC>
where
    SRC: Source + Debug,
    SRC::Parameters: Debug,
{
    /// Find compositor sink by looking where our ghost pad is connected to.
    ///
    /// # Panics
    ///
    /// This can panic if the stream has no video `Pad`.
    pub fn compositor_sink(&self) -> Option<gst::Pad> {
        self.video.clone().map(|video| {
            video
                .peer()
                .expect("expecting video source bin to be connected to compositor")
        })
    }

    /// Get the videoconvertscale `Pad` from the stream.
    ///
    /// # Panics
    ///
    /// This can panic if the stream has no videoconvertscale `Pad`.
    pub fn videoconvertscale(&self) -> gst::Element {
        self.bin
            .by_name("videoconvertscale")
            .expect("unable to get the videoconvertscale from the bin")
    }

    /// Get the capsfilter `Pad` from the stream.
    ///
    /// # Panics
    ///
    /// This can panic if the stream has no capsfilter `Pad`.
    pub fn capsfilter(&self) -> gst::Element {
        self.bin
            .by_name("capsfilter")
            .expect("unable to get the capsfilter from the bin")
    }

    /// Find audiomixer sink by looking where our ghost pad is connected to.
    ///
    /// # Panics
    ///
    /// This can panic if the stream has no audio `Pad`.
    pub fn audiomixer_sink(&self) -> gst::Pad {
        self.audio
            .peer()
            .expect("expecting audio source bin to be connected to audiomixer")
    }
}
