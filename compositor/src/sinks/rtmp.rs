// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::{add_ghost_pad, Sink};

const DEFAULT_AUDIO_RATE: usize = 48000;
const DEFAULT_AUDIO_BITRATE: usize = 96000;
const DEFAULT_VIDEO_BITRATE: usize = 6000;

/// RTMP compositor output to stream over RTMP.
#[derive(Debug)]
pub struct RTMPSink {
    bin: gst::Bin,
    video_sink_pad: gst::GhostPad,
    audio_sink_pad: gst::GhostPad,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RTMPParameters {
    pub location: String,
    pub audio_bitrate: Option<usize>,
    pub audio_rate: Option<usize>,
    pub video_bitrate: Option<usize>,
    pub video_speed_preset: Option<SpeedPreset>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub enum SpeedPreset {
    Ultrafast = 1,
    Superfast = 2,
    Veryfast = 3,
    Faster = 4,
    Fast = 5,
    #[default]
    Medium = 6,
    Slow = 7,
    Slower = 8,
    Veryslow = 9,
    Placebo = 10,
    None = 0,
}

impl RTMPSink {
    /// Create and add new rtmp sink into existing pipeline.
    ///
    /// # Errors
    ///
    /// This can fail for the following reasons:
    /// - Unable to create `videoconvert` for `GStreamer`.
    /// - Unable to create `x264enc` for `GStreamer`.
    /// - Unable to create `h264parse` for `GStreamer`.
    /// - Unable to create `mux` for `GStreamer`.
    /// - Unable to create `audioconvert` for `GStreamer`.
    /// - Unable to create `audioresample` for `GStreamer`.
    /// - Unable to create `fdkaacenc` for `GStreamer`.
    /// - Unable to create `aacparse` for `GStreamer`.
    /// - Unable to create `flvmux` for `GStreamer`.
    /// - Unable to create `rtmpsink` for `GStreamer`.
    /// - `GhostPad` cannot be created for the `video_sink_pad` or `audio_sink_pad`.
    pub fn create(name: &str, parameters: RTMPParameters) -> Result<RTMPSink> {
        trace!("new({name})");

        let bin = gst::parse_bin_from_description(
            format!(
                r#"
            name="{name}"
                
            videoconvert
                name=video
            ! x264enc speed-preset={video_speed_preset} tune=zerolatency bitrate={video_bitrate}
            ! video/x-h264,profile=high
            ! h264parse
            ! mux.

            audioconvert
                name=audio
            ! audioresample
            ! audio/x-raw,rate={audio_rate}
            ! fdkaacenc bitrate={audio_bitrate}
            ! audio/mpeg
            ! aacparse
            ! audio/mpeg, mpegversion=4
            ! mux.

            flvmux
                name=mux
                streamable=true
            ! rtmpsink
                location="{location}"
            "#,
                location = parameters.location,
                audio_bitrate = parameters.audio_bitrate.unwrap_or(DEFAULT_AUDIO_BITRATE),
                audio_rate = parameters.audio_rate.unwrap_or(DEFAULT_AUDIO_RATE),
                video_bitrate = parameters.video_bitrate.unwrap_or(DEFAULT_VIDEO_BITRATE),
                video_speed_preset = parameters.video_speed_preset.unwrap_or_default() as usize,
            )
            .as_str(),
            false,
        )
        .context("failed to create rtmp sink pipeline")?;

        let video_sink_pad = add_ghost_pad(&bin, "video", "sink")
            .context("unable to add GhostPad for video sink")?;
        let audio_sink_pad = add_ghost_pad(&bin, "audio", "sink")
            .context("unable to add GhostPad for audio sink")?;

        Ok(Self {
            bin,
            video_sink_pad,
            audio_sink_pad,
        })
    }
}

impl Sink for RTMPSink {
    #[must_use]
    fn video(&self) -> Option<gst::GhostPad> {
        Some(self.video_sink_pad.clone())
    }

    #[must_use]
    fn audio(&self) -> gst::GhostPad {
        self.audio_sink_pad.clone()
    }

    #[must_use]
    fn bin(&self) -> gst::Bin {
        self.bin.clone()
    }
}
