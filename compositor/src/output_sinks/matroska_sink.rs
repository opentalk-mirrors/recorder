// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use gst::prelude::*;
use serde::Deserialize;
use std::{
    net::{SocketAddr, TcpListener},
    os::unix::prelude::AsRawFd,
    sync::mpsc,
};

use crate::{add_ghost_pad, Sink};

/// Writes out *Matroska* mux-ed raw A/V on a TCP port
#[derive(Debug)]
pub struct MatroskaSink {
    bin: gst::Bin,
    stop_listen: mpsc::Sender<()>,
    pub address: SocketAddr,
    video_sink: gst::GhostPad,
    audio_sink: gst::GhostPad,
}

/// Specific parameters needed to create a Matroska sink
#[derive(Clone, Debug, Deserialize)]
pub struct MatroskaParameters {
    /// address to send output to
    pub address: SocketAddr,
}

impl MatroskaSink {
    /// Create and add new Matroska sink into existing pipeline.
    pub fn new(name: &str, params: MatroskaParameters) -> Self {
        trace!("new({name})");

        // create bin including codecs and the Matroska sink
        let bin = gst::parse_bin_from_description(
            &format!(
                r#"
                name="{name}"

                videoconvert
                    name=video
                ! videorate
                ! videoscale
                ! video/x-raw,format=I420,framerate=25/1,pixel-aspect-ratio=1/1,colorimetry=bt709
                ! mux.

                audioconvert
                    name=audio
                ! audio/x-raw,format=S16LE,layout=interleaved,rate=48000
                ! mux.

                matroskamux
                    name=mux
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

        // listen on given TCP port
        let (stop_listen, stop_receiver): (mpsc::Sender<()>, mpsc::Receiver<()>) = mpsc::channel();
        let listener =
            TcpListener::bind(params.address).expect("failed to bind matroska's TCP listener");
        let address = listener
            .local_addr()
            .expect("failed to get  matroska's local listening address");
        debug!("Start listening on {address}");

        let sink_weak = bin
            .by_name("matroska-sink")
            .expect("failed to get matroska-sink from pipeline")
            .downgrade();

        // spawn a thread which waits until the channel
        std::thread::spawn(move || loop {
            let Some(sink) = sink_weak.upgrade() else {
                return;
            };
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
            stop_listen,
            address,
            video_sink: add_ghost_pad(&bin, "video", "sink"),
            audio_sink: add_ghost_pad(&bin, "audio", "sink"),
            bin,
        }
    }
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
    /// Get video sink pad.
    fn video(&self) -> gst::GhostPad {
        self.video_sink.clone()
    }

    /// Get audio sink pad.
    fn audio(&self) -> gst::GhostPad {
        self.audio_sink.clone()
    }

    fn bin(&self) -> gst::Bin {
        self.bin.clone()
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
