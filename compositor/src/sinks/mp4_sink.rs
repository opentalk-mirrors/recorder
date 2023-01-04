use super::matroska_sink::{MatroskaParameters, MatroskaSink};
use crate::Sink;
use gst::prelude::*;

/// Writes out a single MP4 file using FFmpeg
pub struct Mp4Sink {
    /// Underlying Matroska sink.
    matroska_sink: MatroskaSink,

    /// FFmpeg process
    process: Option<std::process::Child>,

    // Path to save mp4 output to
    file_path: String,
}

#[derive(Debug)]
pub struct Mp4SinkParams {
    pub file_path: String,
}

impl Sink for Mp4Sink {
    type Parameters = Mp4SinkParams;

    /// Create and add new MP4 sink into existing pipeline.
    fn new(pipeline: &gst::Pipeline, params: Self::Parameters) -> Self {
        debug!("create new MP4Sink: {params:?}");

        // watch pipeline bus for getting into `Playing` state
        // return new instance
        Self {
            matroska_sink: MatroskaSink::new(pipeline, MatroskaParameters::default()),
            process: None,
            file_path: params.file_path,
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

        let address = &format!("tcp://{}", self.matroska_sink.address);

        // TODO: use free codecs instead of ffmpeg's mp4 default.
        // using the commented out codec settings often leads to errors when ending the recording and 10-20 seconds
        // missing in the end. following errors are printed:
        //
        // [matroska,webm @ 0x557819b598c0] File ended prematurely
        // [matroska,webm @ 0x557819b598c0] Seek to desired resync point failed. Seeking to earliest point available instead.
        debug!(
            "Starting ffmpeg to process into output DASH into \"{}\", connection is: {address}",
            self.file_path
        );
        self.process = Some(
            std::process::Command::new("ffmpeg")
                .args([
                    "-v",
                    "warning",
                    "-y",
                    "-nostdin",
                    "-i",
                    // read from localhost and given port
                    address,
                    // set video codec
                    //"-codec:v:0",
                    //"libvpx-vp9",
                    // set bitrate for video
                    //"-b:v:0",
                    //"500K",
                    // Set audio codec
                    //"-codec:a:0",
                    //"libopus",
                    // set bitrate for audio
                    //"-b:a:0",
                    //"64K",
                    "-f",
                    "mp4",
                    &self.file_path,
                ])
                .spawn()
                .expect("failed to spawn FFmpeg process"),
        );
    }

    fn on_exit(&mut self, pipeline: &gst::Pipeline) {
        // send EOS into pipeline to flush output
        pipeline.send_event(gst::event::Eos::new());

        // wait until error or EOS
        let bus = pipeline.bus().expect("failed to get bus of pipeline");
        for msg in bus.iter_timed(gst::ClockTime::from_seconds(1)) {
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

        self.matroska_sink.on_exit(pipeline);
    }
}

impl Drop for Mp4Sink {
    fn drop(&mut self) {
        // Wait for ffmpeg to exit
        let mut handle = self
            .process
            .take()
            .expect("Failed to get the ffmpeg process handle. Crashed?");
        handle.wait().expect("Wait on ffmpeg process failed.");
    }
}
