use crate::Sink;
use gst::prelude::*;
use std::{
    net::{SocketAddr, TcpListener},
    os::unix::prelude::AsRawFd,
    sync::mpsc,
};

/// Writes out *Matroska* mux-ed raw A/V on a TCP port
pub struct MatroskaSink {
    /// Video sink GStreamer pad.
    video_sink_pad: gst::Pad,
    /// Audio sink GStreamer pad.
    audio_sink_pad: gst::Pad,
    stop_listen: mpsc::Sender<()>,
    pub address: SocketAddr,
}

/// Specific parameters needed to create a Matroska sink
#[derive(Clone, Debug)]
pub struct MatroskaParameters {
    pub address: SocketAddr,
}

impl Default for MatroskaParameters {
    /// File parameters default
    fn default() -> Self {
        Self {
            address: SocketAddr::from(([127, 0, 0, 1], 0)),
        }
    }
}

impl Sink for MatroskaSink {
    type Parameters = MatroskaParameters;

    /// Create and add new Matroska sink into existing pipeline.
    fn new(pipeline: &gst::Pipeline, params: MatroskaParameters) -> Self {
        debug!("create new MatroskaSink: {params:?}");

        // create bin including codecs and the Matroska sink
        let bin = gst::parse_bin_from_description(
            &format!(
                r#"
                videoconvert
                    name=matroska-video
                ! videorate
                ! videoscale
                ! video/x-raw,format=I420,framerate=25/1,pixel-aspect-ratio=1/1,colorimetry=bt709
                ! matroska-mux.

                audioconvert
                    name=matroska-audio
                ! audio/x-raw,format=S16LE,layout=interleaved,rate=48000
                ! matroska-mux.

                matroskamux
                    name=matroska-mux
                    streamable=true
                    writing-app=OpenTalk
                ! queue
                    name=matroska-queue
                    max-size-time=300000000
                ! multifdsink
                    name=matroska-sink
                    blocksize=1048576
                    buffers-max={buffers_max}
                    sync-method=next-keyframe
                "#,
                buffers_max = 500
            ),
            false,
        )
        .unwrap();

        // add sink to pipeline
        pipeline.add(&bin).unwrap();

        // get elements from bin
        let audio = bin.by_name("matroska-audio").unwrap();
        let video = bin.by_name("matroska-video").unwrap();
        let sink = bin.by_name("matroska-sink").unwrap();

        // create ghost pads which link to codecs
        let audio_sink_pad =
            gst::GhostPad::with_target(None, &audio.static_pad("sink").unwrap()).unwrap();
        let video_sink_pad =
            gst::GhostPad::with_target(None, &video.static_pad("sink").unwrap()).unwrap();

        // add ghost pads to bin
        bin.add_pad(&audio_sink_pad).unwrap();
        bin.add_pad(&video_sink_pad).unwrap();

        // listen on given TCP port
        let (stop_listen, stop_receiver): (mpsc::Sender<()>, mpsc::Receiver<()>) = mpsc::channel();
        let listener = TcpListener::bind(params.address).unwrap();
        let address = listener.local_addr().unwrap();
        debug!("Start listening on {address}",);

        // spawn a thread which waits until the channel
        std::thread::spawn(move || loop {
            let (socket, _) = listener.accept().unwrap();
            trace!("Start sending matroska data");
            sink.emit_by_name_with_values("add", &[socket.as_raw_fd().to_value()]);
            stop_receiver.recv().unwrap();
            trace!("Stopped sending matroska data");
        });

        // return new Matroska sink
        Self {
            video_sink_pad: video_sink_pad.upcast(),
            audio_sink_pad: audio_sink_pad.upcast(),
            stop_listen,
            address,
        }
    }

    /// Get video sink pad.
    fn video_sink_pad(&self) -> gst::Pad {
        self.video_sink_pad.clone()
    }

    /// Get audio sink pad.
    fn audio_sink_pad(&self) -> gst::Pad {
        self.audio_sink_pad.clone()
    }

    fn on_exit(&mut self, _pipeline: &gst::Pipeline) {
        self.stop_listen.send(()).unwrap();
    }
}

impl Drop for MatroskaSink {
    fn drop(&mut self) {
        self.stop_listen.send(()).unwrap();
    }
}
