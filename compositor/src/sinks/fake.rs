// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use anyhow::{Context, Result};

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
    /// # Errors
    ///
    /// This can fail if the `FakeSink` can't be created in `GStreamer`.
    pub fn create(name: &str, has_video: bool) -> Result<Self> {
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
            .context("could not parse display link pipeline")?;

        let video_sink = if has_video {
            let pad = add_ghost_pad(&bin, "video", "sink")
                .context("unable to add GhostPad for video sink")?;
            Some(pad)
        } else {
            None
        };

        let audio_sink = add_ghost_pad(&bin, "audio", "sink")
            .context("unable to add GhostPad for audio sink")?;

        Ok(FakeSink {
            bin,
            audio_sink,
            video_sink,
        })
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
