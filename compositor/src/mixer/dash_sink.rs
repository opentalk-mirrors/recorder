use super::Sink;
use gst::prelude::*;
use gstreamer as gst;
use std::io::Write;

pub struct DashSink {
    dashsink: gst::Element,

    video_sink_pad: gst::Pad,
    audio_sink_pad: gst::Pad,
}

impl Sink for DashSink {
    type Parameters = ();

    fn new(pipeline: &gst::Pipeline, _: ()) -> DashSink {
        let bin = gst::parse_bin_from_description(
            "
            dashsink
                name=dashsink
                muxer=mp4
                dynamic=true
                target-duration=2

            avenc_aac name=audio-encoder
            ! dashsink.audio_0

            x264enc name=video-encoder
            ! dashsink.video_0
        ",
            false,
        )
        .unwrap();

        pipeline.add(&bin).unwrap();

        let dashsink = bin.by_name("dashsink").unwrap();
        let audio_encode = bin.by_name("audio-encoder").unwrap();
        let video_encode = bin.by_name("video-encoder").unwrap();

        // dashsink.connect("get-fragment-stream", false, move |values| {
        //     let dashsink = values[0].get::<gst::Element>().unwrap();
        //     dashsink.stop_signal_emission_by_name("get-fragment-stream");

        //     let location = values[1].get::<String>().unwrap();
        //     println!("GET-FRAGMENT-STREAM @ {location:?}");

        //     let stream = gio::WriteOutputStream::new(Writer { w: Vec::new() });

        //     Some(stream.to_value())
        // });

        // dashsink.connect("get-playlist-stream", false, move |values| {
        //     let dashsink = values[0].get::<gst::Element>().unwrap();
        //     dashsink.stop_signal_emission_by_name("get-playlist-stream");

        //     let location = values[1].get::<String>().unwrap();
        //     println!("GET-PLAYLIST-STREAM {location}");

        //     let stream = gio::WriteOutputStream::new(Writer { w: Vec::new() });
        //     Some(stream.to_value())
        // });

        let audio_sink_pad =
            gst::GhostPad::with_target(None, &audio_encode.static_pad("sink").unwrap()).unwrap();
        let video_sink_pad =
            gst::GhostPad::with_target(None, &video_encode.static_pad("sink").unwrap()).unwrap();

        bin.add_pad(&audio_sink_pad).unwrap();
        bin.add_pad(&video_sink_pad).unwrap();

        Self {
            dashsink,
            video_sink_pad: video_sink_pad.upcast(),
            audio_sink_pad: audio_sink_pad.upcast(),
        }
    }

    fn video_sink_pad(&self) -> gst::Pad {
        self.video_sink_pad.clone()
    }

    fn audio_sink_pad(&self) -> gst::Pad {
        self.audio_sink_pad.clone()
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

    fn write_vectored(&mut self, bufs: &[std::io::IoSlice<'_>]) -> std::io::Result<usize> {
        self.w.write_vectored(bufs)
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
