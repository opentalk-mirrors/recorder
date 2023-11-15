// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use crate::{add_ghost_pad, Sink};

/// Fake sink to catch the compositor output without any further processing.
#[derive(Debug)]
pub struct FakeSink {
    bin: gst::Bin,
    audio_sink: gst::GhostPad,
    video_sink: Option<gst::GhostPad>,
}

impl FakeSink {
    /// Create and add new fake sink into existing pipeline.
    ///
    /// # Panics
    ///
    /// This can panic if the `FakeSink` can't be created in `GStreamer`.
    #[must_use]
    pub fn new(name: &str, has_video: bool) -> Self {
        trace!("new({name})");

        let mut description = format!(
            r#" 
                name="{name}"
                
                fakeaudiosink
                    name=audio
                "#
        )
        .to_string();

        if has_video {
            description += r#"
                fakevideosink
                    name=video
                "#;
        }

        // create new GStreamer pipeline
        let bin = gst::parse_bin_from_description(&description, false)
            .expect("could not parse display link pipeline");

        let video_sink = if has_video {
            Some(add_ghost_pad(&bin, "video", "sink"))
        } else {
            None
        };

        let audio_sink = add_ghost_pad(&bin, "audio", "sink");

        FakeSink {
            bin,
            audio_sink,
            video_sink,
        }
    }
}

impl Default for FakeSink {
    fn default() -> Self {
        Self::new("Fake Sink", true)
    }
}

impl Sink for FakeSink {
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
