use super::matroska_sink::{MatroskaParameters, MatroskaSink};
use crate::Sink;
use derivative::Derivative;
use gst::prelude::*;
use inotify::{Inotify, WatchMask};
use std::{ffi::OsStr, net::SocketAddr, path::PathBuf};
use tempfile::TempDir;

/// Writes out *DASH* A/V files.
pub struct DashSink {
    /// Underlying Matroska sink.
    matroska_sink: MatroskaSink,
    /// remember parameters for delayed usage
    params: DashParameters,
    /// FFmpeg process
    process: Option<std::process::Child>,
    /// Temporary directory to write dash files into.
    /// Is set if no output directory is specified
    temp_dir: Option<TempDir>,
}

/// DASH segment type
#[derive(Clone, Debug)]
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
#[derive(Derivative)]
#[derivative(Debug)]
pub struct DashParameters {
    /// Path to write the dash files to.
    /// Existing files will be overridden.
    /// If None a temporary directory will be used.
    pub output_dir: Option<PathBuf>,
    /// Bitrate to aim in output.
    pub bitrate: usize,
    /// Segment duration in seconds
    pub seg_duration: f32,
    /// DASH segment type
    pub seg_type: SegmentType,
    /// Called when new files are ready
    #[derivative(Debug = "ignore")]
    pub update_callback: fn(files: Vec<&OsStr>),
}

fn update(files: Vec<&OsStr>) {
    debug!("Updated files: {:?}", files);
}

impl Default for DashParameters {
    /// File parameters default.
    fn default() -> Self {
        Self {
            output_dir: None,
            bitrate: 0x100000,
            seg_duration: 5.0,
            seg_type: SegmentType::AUTO,
            update_callback: update,
        }
    }
}

impl Sink for DashSink {
    type Parameters = DashParameters;

    /// Create and add new DASH sink into existing pipeline.
    fn new(pipeline: &gst::Pipeline, params: DashParameters) -> Self {
        trace!("new( {params:?} )");
        assert_eq!(pipeline.current_state(), gst::State::Null);

        // watch pipeline bus for getting into `Playing` state
        // return new instance
        Self {
            matroska_sink: MatroskaSink::new(
                pipeline,
                MatroskaParameters {
                    // use fixed localhost but with given port
                    address: SocketAddr::from(([127, 0, 0, 1], 0)),
                },
            ),
            params,
            process: None,
            temp_dir: None,
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
    fn on_play(&mut self) {
        trace!("on_play()");

        // check if FFmpeg process is still running
        if let Some(process) = &mut self.process {
            if process
                .try_wait()
                .expect("failed to get FFmpeg process status")
                .is_none()
            {
                // then skip any further action
                return;
            }
        }

        let (output_dir, mpd_path) = {
            if let Some(path) = &self.params.output_dir {
                (path.as_ref(), path.join("dash.mpd"))
            } else {
                let temp_dir = self
                    .temp_dir
                    .insert(tempfile::tempdir().expect("failed to find tmpdir"));
                (temp_dir.path(), temp_dir.path().join("dash.mpd"))
            }
        };

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
                    mpd_path
                        .to_str()
                        .expect("failed to convert MPD path into printable string"),
                ])
                .spawn()
                .expect("failed to spawn FFmpeg process"),
        );

        debug!("Setting up DASH target path: {output_dir:?}");

        // check if the output directory exists
        let output_dir = output_dir.canonicalize().expect("invalid DASH target path");

        // spawn a thread which checks for file updates
        std::thread::spawn({
            // initialize inotify
            let mut inotify = Inotify::init().expect("failed to initialize Inotify");
            debug!("Writing DASH files into {}", output_dir.to_string_lossy());

            // add watch to that folder
            inotify
                .add_watch(output_dir, WatchMask::MOVED_TO | WatchMask::CLOSE)
                .expect("Failed to add file watch");
            // get a copy of the callback
            let update = self.params.update_callback;
            move || {
                let mut buffer = [0; 1024];

                loop {
                    let events = loop {
                        match inotify.read_events(&mut buffer) {
                            Ok(events) => break events,
                            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                                continue
                            }
                            _ => panic!("Error while reading events"),
                        }
                    };

                    let files: Vec<&OsStr> = events
                        .filter_map(|event| event.name)
                        .filter(|name| {
                            !name
                                .to_str()
                                .expect("failed to convert Inotify event name into string")
                                .ends_with(".tmp")
                        })
                        .collect();
                    if !files.is_empty() {
                        update(files);
                    }
                }
            }
        });
    }

    /// Sends EOS into pipeline to flush output before
    fn on_exit(&mut self, pipeline: &gst::Pipeline) {
        trace!("on_exit()");

        // send EOS into pipeline to flush output
        pipeline.send_event(gst::event::Eos::new());

        while pipeline.current_state() == gst::State::Null {}

        // Drop temp_dir to delete directory
        self.temp_dir.take();
    }
}
