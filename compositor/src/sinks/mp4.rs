// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use anyhow::{bail, Context, Result};

use crate::{MatroskaParameters, MatroskaSink, Sink};

/// Writes out a single MP4 file using `FFmpeg`
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

impl Mp4Sink {
    /// Create and add new MP4 sink into existing pipeline.
    ///
    /// # Errors
    ///
    /// This can fail for the following reasons:
    /// - `MatroskaSink` couln't initialized
    /// - `ffmpeg` is missing
    /// - `params.file_path` cannot converted to UTF-8
    pub fn create(name: &str, params: &Mp4Parameters) -> Result<Self> {
        let matroska_sink =
            MatroskaSink::create(name, &MatroskaParameters::default()).context("")?;
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
            .with_context(|| {
                format!(
                    "file path '{:?}' cannot be converted to UTF-8",
                    params.file_path
                )
            })?
            .to_string();
        let process = Some(
            std::process::Command::new("ffmpeg")
                .args([
                    "-v", "warning", "-y", "-nostdin", "-i",
                    // read from localhost and given port
                    address, "-f", "mp4", &filename,
                ])
                .spawn()
                .context("failed to spawn FFmpeg process")?,
        );

        // return new instance
        Ok(Mp4Sink {
            matroska_sink,
            process,
            filename,
        })
    }
}

impl Sink for Mp4Sink {
    /// Get video sink pad from Matroska sink.
    #[must_use]
    fn video(&self) -> Option<gst::GhostPad> {
        self.matroska_sink.video()
    }

    /// Get audio sink pad from Matroska sink.
    #[must_use]
    fn audio(&self) -> gst::GhostPad {
        self.matroska_sink.audio()
    }

    #[must_use]
    fn bin(&self) -> gst::Bin {
        self.matroska_sink.bin()
    }

    /// Starts the `FFmpeg` receiver which catches the output of the matroska sink.
    fn on_play(&mut self) -> Result<()> {
        trace!("on_play()");

        // check if FFmpeg process is still running
        if let Some(process) = &mut self.process {
            let result = process
                .try_wait()
                .context("failed to get FFmpeg process status")?;

            if let Some(code) = result {
                bail!("ffmpeg process died with code {}", code);
            }
        }

        Ok(())
    }

    fn on_exit(&mut self, pipeline: &gst::Pipeline) -> Result<()> {
        trace!("on_exit()");

        crate::mixer::debug::debug_dot(pipeline, "on_exit");

        debug!("Closing file '{}'", self.filename);
        self.matroska_sink
            .on_exit(pipeline)
            .context("unable to call on_exit on matroska_sink")?;

        Ok(())
    }
}

impl Drop for Mp4Sink {
    fn drop(&mut self) {
        trace!("drop()");

        // Wait for ffmpeg to exit
        if let Some(mut handle) = self.process.take() {
            if let Err(error) = handle.wait() {
                error!("Wait on ffmpeg process failed, error: {error}");
            }
        } else {
            error!("Failed to get the ffmpeg process handle. Crashed?");
        }
    }
}
