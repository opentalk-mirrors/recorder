// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::{add_ghost_pad, parse_bin_from_description_with_context, Sink};

/// Writes out *Matroska* mux-ed raw A/V on a TCP port
#[derive(Debug)]
pub struct MatroskaSink {
    bin: gst::Bin,
    video_sink: gst::GhostPad,
    audio_sink: gst::GhostPad,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MatroskaParameters {
    pub path: String,
}

impl MatroskaSink {
    /// Create and add new Matroska sink into existing pipeline.
    ///
    /// # Errors
    ///
    /// This can fail for the following reasons:
    /// - Cannot create `videoconvert` in `GStreamer`.
    /// - Cannot create `videorate` in `GStreamer`.
    /// - Cannot create `videoscale` in `GStreamer`.
    /// - Cannot create `mux` in `GStreamer`.
    /// - Cannot create `audioconvert` in `GStreamer`.
    /// - Cannot create `matroskamux` in `GStreamer`.
    /// - Cannot create `queue` in `GStreamer`.
    /// - Cannot create `multifdsink` in `GStreamer`.
    /// - The local address in `params.address` cannot be listened.
    /// - `GhostPad` cannot be created for `video_sink` or `audio_sink`.
    pub fn create(name: &str, params: &MatroskaParameters) -> Result<Self> {
        trace!("new({name}, {params:?})");

        // create bin including codecs and the Matroska sink
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
                ! vp9enc cpu-used=4 threads=4 deadline=1 min-quantizer=8
                ! mux.

                audioconvert
                    name=audio
                ! audio/x-raw,format=S16LE,layout=interleaved,rate=48000
                ! opusenc bitrate=96000
                ! mux.

                matroskamux
                    name=mux
                    writing-app=OpenTalk
                    offset-to-zero=true
                ! queue
                    name=matroska-queue
                    max-size-time=1000000000
                ! filesink
                    name=matroska-sink
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

        // return new Matroska sink
        Ok(Self {
            bin,
            video_sink,
            audio_sink,
        })
    }
}

impl Sink for MatroskaSink {
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
