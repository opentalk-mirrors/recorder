// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use anyhow::{Context, Result};

use crate::{FakeSink, Sink, SystemSink};

/// Fake sink to catch the compositor output without any further processing.
#[derive(Debug)]
pub enum TestSink {
    Fake(FakeSink),
    Display(SystemSink),
}

impl TestSink {
    /// Create and add new fake sink into existing pipeline.
    ///
    /// # Errors
    ///
    /// This can fail if the `DisplaySink` or `FakeSink` cannot be created.
    pub fn new(name: &str) -> Result<Self> {
        trace!("new({name})");

        let use_display = std::env::var("USE_DISPLAY").is_ok();
        let use_video = std::env::var("USE_VIDEO").is_ok();
        let sink = if use_display {
            info!("using display sink because display is available");
            Self::Display(SystemSink::new(name, use_video).context("unable to create DisplaySink")?)
        } else {
            info!("using fake sink");
            Self::Fake(FakeSink::new(name, use_video).context("unable to create FakeSink")?)
        };

        Ok(sink)
    }
}

impl Sink for TestSink {
    #[must_use]
    fn bin(&self) -> gst::Bin {
        match self {
            Self::Fake(sink) => sink.bin(),
            Self::Display(sink) => sink.bin(),
        }
    }
    /// Get video sink pad.
    #[must_use]
    fn video(&self) -> Option<gst::GhostPad> {
        match self {
            Self::Fake(sink) => sink.video(),
            Self::Display(sink) => sink.video(),
        }
    }

    /// Get audio sink pad.
    #[must_use]
    fn audio(&self) -> gst::GhostPad {
        match self {
            Self::Fake(sink) => sink.audio(),
            Self::Display(sink) => sink.audio(),
        }
    }
}
