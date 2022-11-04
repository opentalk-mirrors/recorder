use super::Sink;
use gst::prelude::*;
use gstreamer as gst;
use std::io::Write;

/// Writes out dash MPD/ts into files.
pub struct DashSink {
    /// Video sink GStreamer pad.
    video_sink_pad: gst::Pad,
    /// Audio sink GStreamer pad.
    audio_sink_pad: gst::Pad,
}

/// Specific parameters needed to create a [DashSink]
#[derive(Clone)]
pub struct DashParameters {
    /// Path where the MPD and its fragments will be written.
    pub mpd_root_path: String,
    /// Filename of the mpd to write.
    pub mpd_file_name: String,
    /// BaseURL to set in the MPD.
    pub mpd_baseurl: String,
    /// The target duration in seconds of a segment/file
    /// (0 - disabled, useful for management of segment duration by the streaming server).
    pub target_duration: u64,
}

impl Default for DashParameters {
    /// Dash parameters default
    fn default() -> Self {
        Self {
            mpd_root_path: "./".into(),
            mpd_file_name: "dash.mpd".into(),
            mpd_baseurl: "".into(),
            target_duration: 15,
        }
    }
}

impl Sink for DashSink {
    type Parameters = DashParameters;

    /// Create and add new Dash sink into existing pipeline.
    fn new(pipeline: &gst::Pipeline, params: Self::Parameters) -> Self {
        // create bin including codecs and the dash sink
        let bin = gst::parse_bin_from_description(
            &format!(
                r#"
            dashsink
                name=dashsink
                muxer=ts
                dynamic=false
                mpd-filename="{mpd_file_name}"
                mpd-root-path="{mpd_root_path}"
                mpd-baseurl="{mpd_baseurl}"
                target-duration={target_duration}

            avenc_aac name=audio-encoder
            ! dashsink.audio_0

            x264enc name=video-encoder
            ! dashsink.video_0
        "#,
                mpd_file_name = params.mpd_file_name,
                mpd_root_path = params.mpd_root_path,
                mpd_baseurl = params.mpd_baseurl,
                target_duration = params.target_duration
            ),
            false,
        )
        .unwrap();

        // add sink to pipeline
        pipeline.add(&bin).unwrap();

        // get codes from bin
        bin.by_name("dashsink").unwrap();
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

        // create ghost pads which link to codecs
        let audio_sink_pad =
            gst::GhostPad::with_target(None, &audio_encode.static_pad("sink").unwrap()).unwrap();
        let video_sink_pad =
            gst::GhostPad::with_target(None, &video_encode.static_pad("sink").unwrap()).unwrap();

        // add ghost pads to bin
        bin.add_pad(&audio_sink_pad).unwrap();
        bin.add_pad(&video_sink_pad).unwrap();

        // return new Dash sink
        Self {
            video_sink_pad: video_sink_pad.upcast(),
            audio_sink_pad: audio_sink_pad.upcast(),
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
