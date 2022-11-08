use super::{
    matroska_sink::{MatroskaParameters, MatroskaSink},
    Sink,
};

use gst::prelude::*;
use gstreamer as gst;
use inotify::{Inotify, WatchMask};
use std::{ffi::OsStr, path::PathBuf};

/// Writes out *DASH* A/V files.
pub struct DashSink {
    /// Underlying Matroska sink.
    matroska_sink: MatroskaSink,
    /// remember parameters for delayed usage
    params: DashParameters,
    /// FFmpeg process
    process: Option<std::process::Child>,
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
    /// Bitrate to aim in output.
    pub bitrate: usize,
    /// Segment duration in seconds
    pub seg_duration: f32,
    /// DASH segment type
    pub seg_type: SegmentType,
    /// Called when new files are ready
    pub update: fn(files: Vec<&OsStr>),
}

fn update(files: Vec<&OsStr>) {
    trace!("updated files: {:?}", files);
}

impl Default for DashParameters {
    /// File parameters default.
    fn default() -> Self {
        Self {
            mpd: PathBuf::from("dash.mpd"),
            bitrate: 0x100000,
            seg_duration: 5.0,
            seg_type: SegmentType::AUTO,
            update,
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
            matroska_sink: MatroskaSink::new(
                pipeline,
                MatroskaParameters {
                    // use fixed localhost but with given port
                    address: format!("127.0.0.1:0").parse().unwrap(),
                },
            ),
            params,
            process: None,
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
    fn on_play(&mut self) {
        // check if FFmpeg process is still running
        if let Some(process) = &mut self.process {
            if process.try_wait().unwrap().is_none() {
                // then skip any further action
                return;
            }
        }
        // start ffmpeg to fetch output stream and create DASH files
        self.process = Some(
            std::process::Command::new("ffmpeg")
                .args([
                    "-v",
                    "warning",
                    "-y",
                    "-nostdin",
                    "-i",
                    // read from localhost and given port
                    &format!("tcp://{}", self.matroska_sink.address),
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
                .unwrap(),
        );

        // initialize inotify
        let mut inotify = Inotify::init().unwrap();
        // get target path from MPD file name
        let path = self.params.mpd.as_path().parent().unwrap().clone();
        trace!("Writing DASH files into {}", path.to_str().unwrap());
        // add watch to that folder
        inotify
            .add_watch(path, WatchMask::MOVED_TO | WatchMask::CLOSE)
            .expect("Failed to add file watch");
        let update = self.params.update;
        // spawn a thread which handles the watched events
        std::thread::spawn(move || {
            let mut buffer = [0; 1024];

            loop {
                let events = loop {
                    match inotify.read_events(&mut buffer) {
                        Ok(events) => break events,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => continue,
                        _ => panic!("Error while reading events"),
                    }
                };

                let files: Vec<&OsStr> = events
                    .filter(|event| event.name.is_some())
                    .map(|event| event.name.unwrap().clone())
                    .filter(|name| !str::ends_with(name.to_str().unwrap(), ".tmp"))
                    .collect();
                if !files.is_empty() {
                    update(files);
                }
            }
        });
    }

    /// Sends EOS into pipeline to flush output before
    fn on_exit(pipeline: &gst::Pipeline) {
        // send EOS into pipeline to flush output
        pipeline.send_event(gst::event::Eos::new());

        // wait until error or EOS
        let bus = pipeline.bus().unwrap();
        for msg in bus.iter_timed(gst::ClockTime::NONE) {
            use gst::MessageView;

            match msg.view() {
                MessageView::Error(err) => {
                    error!(
                        "Error received from element {:?}: {}",
                        err.src().map(|s| s.path_string()),
                        err.error()
                    );
                    debug!("Debugging information: {:?}", err.debug());
                    break;
                }
                MessageView::Eos(..) => break,
                _ => (),
            }
        }
    }
}
