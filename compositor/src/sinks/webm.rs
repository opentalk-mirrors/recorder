// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::{add_ghost_pad, parse_bin_from_description_with_context, Sink};

#[derive(Debug)]
pub struct WebMSink {
    bin: gst::Bin,
    video_sink: gst::GhostPad,
    audio_sink: gst::GhostPad,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WebMParameters {
    pub path: String,
}

impl WebMSink {
    /// Create and add new WebM sink into existing pipeline.
    ///
    /// # Errors
    ///
    /// This can fail for the following reasons:
    /// - Cannot create `videoconvert` in `GStreamer`.
    /// - Cannot create `videorate` in `GStreamer`.
    /// - Cannot create `videoscale` in `GStreamer`.
    /// - Cannot create `mux` in `GStreamer`.
    /// - Cannot create `audioconvert` in `GStreamer`.
    /// - Cannot create `webmmux` in `GStreamer`.
    /// - Cannot create `queue` in `GStreamer`.
    /// - Cannot create `filedsink` in `GStreamer`.
    /// - The local address in `params.address` cannot be listened.
    /// - `GhostPad` cannot be created for `video_sink` or `audio_sink`.
    pub fn create(name: &str, params: &WebMParameters) -> Result<Self> {
        //
        trace!("new({name}, {params:?})");

        // The video encoder is setup for a buffer of 6s (vp9enc buffer-size=6000 [ms]) max.
        // Therefore the audio queue is set to 8s (queue max-size-time=8000000000 [ns]) and
        // the video queue to 2s (queue max-size-time=8000000000 [ns])
        let bin = parse_bin_from_description_with_context(
            &format!(
                r#"
                name="{name}"
                   
                videoconvert
                    name=video
                ! videorate
                    drop-only=true
                ! videoscale
                ! video/x-raw,format=I420,framerate=30/1,pixel-aspect-ratio=1/1,colorimetry=bt709
                ! vp8enc 
                  deadline=1 cpu-used=4 threads=4 token-partitions=1
                  end-usage=cbr target-bitrate=2600000 undershoot=90
                  buffer-size=6000 buffer-initial-size=4000 buffer-optimal-size=5000
                  dropframe-threshold=25 resize-allowed=true
                ! queue
                    max-size-time=2000000000 max-size-bytes=0 max-size-buffers=0
                ! mux.

                audioconvert
                    name=audio
                ! audio/x-raw,format=S16LE,layout=interleaved,rate=48000
                ! opusenc bitrate=96000 complexity=7 audio-type=voice
                ! queue
                    max-size-time=8000000000 max-size-bytes=0 max-size-buffers=0
                ! mux.

                webmmux
                    name=mux
                    writing-app=OpenTalk
                    offset-to-zero=true
                ! queue
                    name=webm-queue
                    max-size-time=3000000000
                    max-size-bytes=0
                    max-size-buffers=0
                ! filesink
                    name=webm-sink
                    location={filename}
                    buffer-mode=full
                "#,
                filename = params.path
            ),
            false,
        )?;

        let video_sink = add_ghost_pad(&bin, "video", "sink")
            .context("unable to add GhostPad for video sink")?;
        let audio_sink = add_ghost_pad(&bin, "audio", "sink")
            .context("unable to add GhostPad for audio sink")?;

        Ok(Self {
            bin,
            video_sink,
            audio_sink,
        })
    }
}

impl Sink for WebMSink {
    /// Get video sink pad.
    #[must_use]
    fn video(&self) -> Option<gst::GhostPad> {
        Some(self.video_sink.clone())
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
