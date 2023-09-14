use crate::*;

/// Fake sink to catch the compositor output without any further processing.
#[derive(Debug)]
pub struct FakeSink {
    bin: gst::Bin,
    video_sink: gst::GhostPad,
    audio_sink: gst::GhostPad,
}

impl FakeSink {
    /// Create and add new fake sink into existing pipeline.
    pub fn new(name: &str) -> Self {
        trace!("new({name})");

        // create new GStreamer pipeline
        let bin = gst::parse_bin_from_description(
            &format!(
                r#" 
                name="{name}"
    
                fakevideosink
                    name=video
    
                fakeaudiosink
                    name=audio
                "#
            ),
            false,
        )
        .expect("could not parse display link pipeline");

        // return new display sink
        FakeSink {
            video_sink: add_ghost_pad(&bin, "video", "sink"),
            audio_sink: add_ghost_pad(&bin, "audio", "sink"),
            bin,
        }
    }
}

impl Default for FakeSink {
    fn default() -> Self {
        Self::new("Fake Sink")
    }
}

impl Sink for FakeSink {
    /// Get video sink pad.
    fn video(&self) -> gst::GhostPad {
        self.video_sink.clone()
    }
    /// Get audio sink pad.
    fn audio(&self) -> gst::GhostPad {
        self.audio_sink.clone()
    }
    fn bin(&self) -> gst::Bin {
        self.bin.clone()
    }
}
