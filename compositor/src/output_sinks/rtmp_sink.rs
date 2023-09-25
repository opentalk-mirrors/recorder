use serde::Deserialize;

use crate::*;

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
    pub fn new(parameters: RTMPParameters) -> RTMPSink {
        trace!("new()");

        let bin = gst::parse_bin_from_description(
            format!(
                r#"
            videoconvert
                name=rtmp-video
            ! x264enc speed-preset={video_speed_preset} tune=zerolatency bitrate={video_bitrate}
            ! video/x-h264,profile=high
            ! h264parse
            ! rtmp-mux.

            audioconvert
                name=rtmp-audio
            ! audioresample
            ! audio/x-raw,rate={audio_rate}
            ! fdkaacenc bitrate={audio_bitrate}
            ! audio/mpeg
            ! aacparse
            ! audio/mpeg, mpegversion=4
            ! rtmp-mux.

            flvmux
                name=rtmp-mux
                streamable=true
            ! rtmpsink
                location='{location}'
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
        .expect("failed to create rtmp sink pipeline");

        Self {
            video_sink_pad: add_ghost_pad(&bin, "rtmp-video", "sink"),
            audio_sink_pad: add_ghost_pad(&bin, "rtmp-audio", "sink"),
            bin,
        }
    }
}

impl Sink for RTMPSink {
    fn video(&self) -> gst::GhostPad {
        self.video_sink_pad.clone()
    }

    fn audio(&self) -> gst::GhostPad {
        self.audio_sink_pad.clone()
    }

    fn bin(&self) -> gst::Bin {
        self.bin.clone()
    }
}
