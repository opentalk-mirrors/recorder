use crate::*;

/// Displays compositor output on the screen.
#[derive(Debug)]
pub struct DisplaySink {
    bin: gst::Bin,
    video_sink: gst::GhostPad,
    audio_sink: gst::GhostPad,
}

impl DisplaySink {
    /// Create and add new display sink into existing pipeline.
    pub fn new() -> DisplaySink {
        trace!("new()");

        // create new GStreamer pipeline
        let bin = gst::parse_bin_from_description(
            " 
            name=display-sink

            autovideosink
                name=video
                sync=true

            autoaudiosink
                name=audio
                sync=true
            ",
            false,
        )
        .expect("could not parse display link pipeline");

        // return new display sink
        Self {
            video_sink: add_ghost_pad(&bin, "video", "sink"),
            audio_sink: add_ghost_pad(&bin, "audio", "sink"),
            bin,
        }
    }
}

impl Default for DisplaySink {
    fn default() -> Self {
        Self::new()
    }
}

impl Sink for DisplaySink {
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
