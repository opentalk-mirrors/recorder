// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use super::matroska_sink::MatroskaSink;
use crate::{MatroskaParameters, Sink};

/// Writes out a single MP4 file using FFmpeg
#[derive(Debug)]
pub struct Mp4Sink {
    /// Underlying Matroska sink.
    matroska_sink: MatroskaSink,
    /// FFmpeg process.
    process: Option<std::process::Child>,
    /// Output filename.
    filename: String,
}

/// MP4 Sink parameters
#[derive(Debug)]
pub struct Mp4Parameters {
    /// name of the sink
    pub name: &'static str,
    /// Output file path
    pub file_path: std::path::PathBuf,
}

impl Default for Mp4Parameters {
    fn default() -> Self {
        Self {
            name: "MP4 Sink",
            file_path: std::env::current_dir().expect("could not get current directory"),
        }
    }
}

impl Mp4Sink {
    /// Create and add new MP4 sink into existing pipeline.
    pub fn new(name: &str, params: Mp4Parameters) -> Self {
        let matroska_sink = MatroskaSink::new(name, MatroskaParameters::default());
        let address = &format!("tcp://{}", matroska_sink.address);

        // TODO: use free codecs instead of ffmpeg's mp4 default.
        // using the commented out codec settings often leads to errors when ending the recording and 10-20 seconds
        // missing in the end. following errors are printed:
        //
        // [matroska,webm @ 0x557819b598c0] File ended prematurely
        // [matroska,webm @ 0x557819b598c0] Seek to desired resync point failed. Seeking to earliest point available instead.
        debug!(
            "Starting ffmpeg to process into output DASH into \"{:?}\", connection is: {address}",
            params.file_path
        );
        let filename = params
            .file_path
            .to_str()
            .expect("invalid path which could not be converted into UTF8")
            .to_string();
        let process = Some(
            std::process::Command::new("ffmpeg")
                .args([
                    "-v", "warning", "-y", "-nostdin", "-i",
                    // read from localhost and given port
                    address, "-f", "mp4", &filename,
                ])
                .spawn()
                .expect("failed to spawn FFmpeg process"),
        );

        // return new instance
        Mp4Sink {
            matroska_sink,
            process,
            filename,
        }
    }
}

impl Sink for Mp4Sink {
    /// Get video sink pad from Matroska sink.
    fn video(&self) -> gst::GhostPad {
        self.matroska_sink.video()
    }

    /// Get audio sink pad from Matroska sink.
    fn audio(&self) -> gst::GhostPad {
        self.matroska_sink.audio()
    }

    fn bin(&self) -> gst::Bin {
        self.matroska_sink.bin()
    }

    /// Starts the FFmpeg receiver which catches the output of the matroska sink.
    fn on_play(&mut self) {
        trace!("on_play()");

        // check if FFmpeg process is still running
        if let Some(process) = &mut self.process {
            let result = process
                .try_wait()
                .expect("failed to get FFmpeg process status");

            if let Some(code) = result {
                error!("ffmpeg process died with code {}", code);
            }
        }
    }

    fn on_exit(&mut self, pipeline: &gst::Pipeline) {
        trace!("on_exit()");

        crate::mixer::debug::debug_dot(pipeline, "on_exit");

        debug!("Closing file '{}'", self.filename);
        self.matroska_sink.on_exit(pipeline);
    }
}

impl Drop for Mp4Sink {
    fn drop(&mut self) {
        trace!("drop()");

        // Wait for ffmpeg to exit
        let mut handle = self
            .process
            .take()
            .expect("Failed to get the ffmpeg process handle. Crashed?");
        handle.wait().expect("Wait on ffmpeg process failed.");
    }
}
