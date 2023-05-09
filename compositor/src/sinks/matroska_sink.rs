use crate::{Sink, SinkBuilder};
use gst::prelude::*;
use serde::Deserialize;
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
#[derive(Clone, Debug, Deserialize)]
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

pub struct MatroskaSinkBuilder {
    params: MatroskaParameters,
}

impl MatroskaSinkBuilder {
    pub fn new(params: MatroskaParameters) -> Self {
        Self { params }
    }
}

impl SinkBuilder for MatroskaSinkBuilder {
    fn build(&self, pipeline: &gst::Pipeline) -> Box<dyn Sink> {
        Box::new(MatroskaSink::new(pipeline, self.params.clone()))
    }
}

impl MatroskaSink {
    /// Create and add new Matroska sink into existing pipeline.
    pub fn new(pipeline: &gst::Pipeline, params: MatroskaParameters) -> Self {
        trace!("new( {params:?} )");
        assert_eq!(pipeline.current_state(), gst::State::Null);

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
        .expect("failed to create matroska sink pipeline");

        // add sink to pipeline
        pipeline
            .add(&bin)
            .expect("failed to add matroska sink's bin into pipeline");

        // get elements from bin
        let audio = bin
            .by_name("matroska-audio")
            .expect("failed to get matroska-audio from pipeline");
        let video = bin
            .by_name("matroska-video")
            .expect("failed to get matroska-video from pipeline");
        let sink = bin
            .by_name("matroska-sink")
            .expect("failed to get matroska-sink from pipeline");

        // create ghost pads which link to codecs
        let audio_ghost_pad = gst::GhostPad::with_target(
            None,
            &audio
                .static_pad("sink")
                .expect("failed to get sink pad of audio matroska sink"),
        )
        .expect("failed to create ghost pad for audio matroska sink");
        let video_ghost_pad = gst::GhostPad::with_target(
            None,
            &video
                .static_pad("sink")
                .expect("failed to get sink pad of video matroska sink"),
        )
        .expect("failed to create ghost pad for video matroska sink");

        // add ghost pads to bin
        bin.add_pad(&audio_ghost_pad)
            .expect("failed to add matroska audio ghost pad to pipeline");
        bin.add_pad(&video_ghost_pad)
            .expect("failed to add matroska audio ghost pad to pipeline");

        // listen on given TCP port
        let (stop_listen, stop_receiver): (mpsc::Sender<()>, mpsc::Receiver<()>) = mpsc::channel();
        let listener =
            TcpListener::bind(params.address).expect("failed to bind matroska's TCP listener");
        let address = listener
            .local_addr()
            .expect("failed to get  matroska's local listening address");
        debug!("Start listening on {address}");

        // spawn a thread which waits until the channel
        std::thread::spawn(move || loop {
            let (socket, _) = listener
                .accept()
                .expect("failed to accept incoming TCP connection in matroska");
            trace!("Start sending matroska data");
            sink.emit_by_name_with_values("add", &[socket.as_raw_fd().to_value()]);
            stop_receiver
                .recv()
                .expect("failed to wait for TCP receiver stop");
            trace!("Stopped sending matroska data");
        });

        // return new Matroska sink
        Self {
            video_sink_pad: video_ghost_pad.upcast(),
            audio_sink_pad: audio_ghost_pad.upcast(),
            stop_listen,
            address,
        }
    }
}

impl Sink for MatroskaSink {
    /// Get video sink pad.
    fn video_sink_pad(&self) -> gst::Pad {
        self.video_sink_pad.clone()
    }

    /// Get audio sink pad.
    fn audio_sink_pad(&self) -> gst::Pad {
        self.audio_sink_pad.clone()
    }

    fn on_exit(&mut self, _pipeline: &gst::Pipeline) {
        trace!("on_exit()");

        self.stop_listen
            .send(())
            .expect("failed to send stop to TCP listener");
    }
}

impl Drop for MatroskaSink {
    fn drop(&mut self) {
        trace!("drop()");

        self.stop_listen
            .send(())
            .expect("failed to send stop to TCP listener");
    }
}
