use crate::*;

/// Fake sink to catch the compositor output without any further processing.
#[derive(Debug)]
pub enum TestSink {
    Fake(FakeSink),
    Display(DisplaySink),
}

impl TestSink {
    /// Create and add new fake sink into existing pipeline.
    pub fn new(name: &str) -> Self {
        trace!("new({name})");

        let use_display = std::env::var("USE_DISPLAY").is_ok();
        if use_display {
            info!("using display sink because display is available");
            Self::Display(DisplaySink::new(name))
        } else {
            info!("using fake sink");
            Self::Fake(FakeSink::new(name))
        }
    }
}

impl Default for TestSink {
    fn default() -> Self {
        Self::new("Test Sink")
    }
}

impl Sink for TestSink {
    fn bin(&self) -> gst::Bin {
        match self {
            Self::Fake(sink) => sink.bin(),
            Self::Display(sink) => sink.bin(),
        }
    }
    /// Get video sink pad.
    fn video(&self) -> gst::GhostPad {
        match self {
            Self::Fake(sink) => sink.video(),
            Self::Display(sink) => sink.video(),
        }
    }

    /// Get audio sink pad.
    fn audio(&self) -> gst::GhostPad {
        match self {
            Self::Fake(sink) => sink.audio(),
            Self::Display(sink) => sink.audio(),
        }
    }
}
