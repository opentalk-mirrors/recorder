use super::Sink;
use gst::prelude::*;
use gstreamer as gst;
use std::io::Write;
use std::net::TcpListener;
use std::os::unix::prelude::AsRawFd;
use std::sync::mpsc;

/// Writes out *Matroska* mux-ed raw A/V on a TCP port
pub struct MatroskaSink {
    /// Video sink GStreamer pad.
    video_sink_pad: gst::Pad,
    /// Audio sink GStreamer pad.
    audio_sink_pad: gst::Pad,
    stop_sender: mpsc::Sender<()>,
}

/// Specific parameters needed to create a [FileSink]
#[derive(Clone)]
pub struct MatroskaParameters {
    pub local_address: String,
    pub port: u16,
}

impl Default for MatroskaParameters {
    /// File parameters default
    fn default() -> Self {
        Self {
            local_address: "127.0.0.1".to_string(),
            port: 9000,
        }
    }
}

impl Sink for MatroskaSink {
    type Parameters = MatroskaParameters;

    /// Create and add new Dash sink into existing pipeline.
    fn new(pipeline: &gst::Pipeline, params: Self::Parameters) -> Self {
        // create bin including codecs and the dash sink
        let bin = gst::parse_bin_from_description(
            &format!(
                r#"
                videoconvert
                    name=matroska-video
                ! videorate
                ! videoscale
                ! video/x-raw,format=I420,framerate=25/1,pixel-aspect-ratio=1/1
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
        //    gst::GhostPad::with_target(None, &mux.request_pad_simple("audio_%u").unwrap()).unwrap();
        let video_sink_pad =
            gst::GhostPad::with_target(None, &video.static_pad("sink").unwrap()).unwrap();
        //        gst::GhostPad::with_target(None, &mux.request_pad_simple("video_%u").unwrap()).unwrap();

        // add ghost pads to bin
        bin.add_pad(&audio_sink_pad).unwrap();
        bin.add_pad(&video_sink_pad).unwrap();

        // listen on given TCP port
        let address = &format!("{}:{}", params.local_address, params.port);
        trace!("Start listening on {}", address);

        let (_stop_sender, stop_receiver): (mpsc::Sender<()>, mpsc::Receiver<()>) = mpsc::channel();
        let listener = TcpListener::bind(address).unwrap();
        std::thread::spawn(move || loop {
            let (socket, _) = listener.accept().unwrap();
            trace!("Start sending matroska data");
            sink.emit_by_name_with_values("add", &[socket.as_raw_fd().to_value()]);
            stop_receiver.recv().unwrap();
            trace!("Stopped sending matroska data");
        });

        // return new Dash sink
        Self {
            video_sink_pad: video_sink_pad.upcast(),
            audio_sink_pad: audio_sink_pad.upcast(),
            stop_sender: _stop_sender,
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
}

impl Drop for MatroskaSink {
    fn drop(&mut self) {
        self.stop_sender.send(()).unwrap();
    }
}

struct Writer<W> {
    w: W,
}

impl<W: Write> Write for Writer<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        // println!("write {}", buf.len());
        self.w.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        // println!("Flush! ###############");

        self.w.flush()
    }

    fn write_vectored(&mut self, buffers: &[std::io::IoSlice<'_>]) -> std::io::Result<usize> {
        self.w.write_vectored(buffers)
    }

    fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
        self.w.write_all(buf)
    }

    fn write_fmt(&mut self, fmt: std::fmt::Arguments<'_>) -> std::io::Result<()> {
        self.w.write_fmt(fmt)
    }

    fn by_ref(&mut self) -> &mut Self
    where
        Self: Sized,
    {
        self
    }
}
