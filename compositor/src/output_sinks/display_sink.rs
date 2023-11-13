// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use crate::{add_ghost_pad, Sink};

/// Displays compositor output on the screen.
#[derive(Debug)]
pub struct DisplaySink {
    bin: gst::Bin,
    audio_sink: gst::GhostPad,
    video_sink: Option<gst::GhostPad>,
}

impl DisplaySink {
    /// Create and add new display sink into existing pipeline.
    ///
    /// # Panics
    ///
    /// This can panic if the `DisplaySink` can't be created in `GStreamer`.
    #[must_use]
    pub fn new(name: &str, has_video: bool) -> Self {
        trace!("new({name})");

        let mut description = format!(
            r#" 
                name="{name}"
                
                autoaudiosink
                    name=audio
                    sync=true
                "#
        )
        .to_string();

        if has_video {
            description += r#"
                autovideosink
                    name=video
                    sync=false
                "#;
        }

        // create new GStreamer pipeline
        // HINT: Enabling the sync for video and audio for the same time is blocking in multisink
        let bin = gst::parse_bin_from_description(&description, false)
            .expect("could not parse display link pipeline");

        let video_sink = if has_video {
            Some(add_ghost_pad(&bin, "video", "sink"))
        } else {
            None
        };

        let audio_sink = add_ghost_pad(&bin, "audio", "sink");

        Self {
            bin,
            audio_sink,
            video_sink,
        }
    }
}

impl Default for DisplaySink {
    fn default() -> Self {
        Self::new("Display Sink", true)
    }
}

impl Sink for DisplaySink {
    /// Get video sink pad.
    #[must_use]
    fn video(&self) -> Option<gst::GhostPad> {
        self.video_sink.clone()
    }

    /// Get audio sink pad.
    #[must_use]
    fn audio(&self) -> gst::GhostPad {
        self.audio_sink.clone()
    }
    #[must_use]
    fn bin(&self) -> gst::Bin {
        self.bin.clone()
    }
}
