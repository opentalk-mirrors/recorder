use crate::Sink;
use crate::{MatroskaParameters, MatroskaSink};

use gstreamer as gst;
use std::path::PathBuf;

/// Writes out *Dash* A/V files.
pub struct DashSink {
    /// Underlying Matroska sink.
    matroska_sink: MatroskaSink,
    params: DashParameters,
}

#[allow(dead_code)]
#[derive(Clone)]
pub enum SegmentType {
    AUTO,
    MP4,
    WEBM,
}

impl SegmentType {
    fn as_str(&self) -> &str {
        match self {
            Self::AUTO => "auto",
            Self::MP4 => "mp4",
            Self::WEBM => "webm",
        }
    }
}

/// Specific parameters needed to create.
#[derive(Clone)]
pub struct DashParameters {
    pub mpd: PathBuf,
    pub port: u16,
    pub bitrate: usize,
    pub seg_duration: f32,
    pub seg_type: SegmentType,
    pub matroska: MatroskaParameters,
}

impl Default for DashParameters {
    /// File parameters default.
    fn default() -> Self {
        Self {
            mpd: PathBuf::from("dash.mpd"),
            port: 9000,
            bitrate: 1024 * 1024,
            seg_duration: 5.0,
            seg_type: SegmentType::AUTO,
            matroska: Default::default(),
        }
    }
}

impl Sink for DashSink {
    type Parameters = DashParameters;

    /// Create and add new DASH sink into existing pipeline.
    fn new(pipeline: &gst::Pipeline, params: Self::Parameters) -> Self {
        // watch pipeline bus for getting into `Playing` state
        // return new instance
        Self {
            matroska_sink: MatroskaSink::new(pipeline, params.matroska.clone()),
            params,
        }
    }

    /// Get video sink pad from Matroska sink.
    fn video_sink_pad(&self) -> gst::Pad {
        self.matroska_sink.video_sink_pad()
    }

    /// Get audio sink pad from Matroska sink.
    fn audio_sink_pad(&self) -> gst::Pad {
        self.matroska_sink.audio_sink_pad()
    }

    /// Starts the FFmpeg receiver which catches the output of the matroska sink.
    /// # Arguments
    /// - `source`: URL of  
    fn on_play(&self) {
        // start ffmpeg to fetch output stream and create DASH files
        std::process::Command::new("ffmpeg")
            .args([
                "-v",
                "warning",
                "-y",
                "-nostdin",
                "-i",
                &format!("tcp://127.0.0.1:{}", self.params.port),
                "-map",
                "0",
                "-b:0",
                &self.params.bitrate.to_string(),
                "-use_timeline",
                "1",
                "-use_template",
                "1",
                "-adaptation_sets",
                "id=0,streams=v id=1,streams=a",
                "-seg_duration",
                &self.params.seg_duration.to_string(),
                "-dash_segment_type",
                self.params.seg_type.as_str(),
                "-f",
                "dash",
                self.params.mpd.to_str().unwrap(),
            ])
            .spawn()
            .unwrap();
    }
}
