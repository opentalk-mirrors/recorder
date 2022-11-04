use crate::Sink;
use crate::{MatroskaParameters, MatroskaSink};

use gstreamer as gst;
use std::path::PathBuf;

/// Writes out *DASH* A/V files.
pub struct DashSink {
    /// Underlying Matroska sink.
    matroska_sink: MatroskaSink,
    params: DashParameters,
}

/// DASH segment type
#[allow(dead_code)]
#[derive(Clone)]
pub enum SegmentType {
    /// Select DASH segment files format based on the stream codec.
    AUTO,
    /// Use ISOBMFF format.
    MP4,
    /// Use WebM format.
    WEBM,
}

impl SegmentType {
    /// Get segment type as string.
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
    /// Path, name and extension of the MPD file to create (e.g. `./output/my_media.mpd`).
    /// All further media files will be created beside the MPD file.
    /// All files will be overwritten!
    pub mpd: PathBuf,
    /// TCP port to use to connect the ffmpeg process to the matroska sink.
    pub port: u16,
    /// Bitrate to aim in output.
    pub bitrate: usize,
    /// Segment duration in seconds
    pub seg_duration: f32,
    /// DASH segment type
    pub seg_type: SegmentType,
    /// Parameters for underlying matroska sink.
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
    fn new(pipeline: &gst::Pipeline, params: DashParameters) -> Self {
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
