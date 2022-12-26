/*!
    # Purpose
    The *compositor* crate manages a [GStreamer](https://gstreamer.freedesktop.org/) pipeline which receives [WebRTC](https://webrtc.org/) input audio and video streams
    of so-called *streams* and mixes them together using the so-called *mixer*.

    - [Mixer](mixer::Mixer)
    - [Participant](mixer::Participant)

    It then composes an output image showing some of them (so-called *visibles*) in the output picture.

    All incoming audio of all the streams will be mixed together independent of if they are invisible or not.
    The output then will be written onto disk into a
    [MPEG-DASH](https://de.wikipedia.org/wiki/Dynamic_Adaptive_Streaming_over_HTTP (Dynamic Adaptive Streaming over HTTP))
    instance which consists of several files including the
    [MPD](https://ott.dolby.com/OnDelKits/DDP/Dolby_Digital_Plus_Online_Delivery_Kit_v1.5/Documentation/Playback/SDM/help_files/topics/c_dash_mpd_ov.html (media presentation description))
    and Transport Streams.

    # Source & Sink

    To read the input and write the output the following types are used

    - [WebRtcSource](sources::WebRtcSource)
      manages a connection to a WebRTC source and provides the content to the internal GStreamer pipeline.
    - [DashSink](sinks::DashSink)
      writes the output into a Dash instance consisting of an MPD file and several audio/video files.
    - [Mp4Sink](sinks::Mp4Sink)
      writes the output into a MPEG4 file.
    - [MatroskaSink](sinks::MatroskaSink)
      listens on a TCP port to write the raw output to, after someone connects.

    # Layouts

    Several so-called *layouts* can be used to control the output composite.

    - [Grid](layout::Grid)
      shows a grid of all visible streams
    - [Speaker](layout::Speaker)
      shows a bigger picture of the first visible stream (so-called *speaker*)
      and uses the rest of the available picture area to arrange all other visibles.

    # Generic traits for extending capabilities

    - [Source](mixer::Source)
      is a trait which the mixer is assuming for an input source.
    - [Sink](mixer::Sink)
      is a trait which the mixer is assuming for an output sink.
    - [Layout](layout::Layout)
      is a trait which the mixer is assuming for display layout of the recording

    # Testing

    In addition there are some alternative sources and sinks included which are used for testing purposes.

    - [TestSource](sources::TestSource)
      which just generates some dummy stream audio and video data.
    - [FakeSink](sinks::FakeSink)
      is a sink without any output - just to make it run.
    - [DisplaySink](sinks::DisplaySink)
      is a sink which displays the output on the screen.
*/

#[macro_use]
extern crate log;

mod error;
mod layout;
mod mixer;
mod overlays;
mod sinks;
mod sources;
#[cfg(test)]
mod tests;

pub use error::*;
pub use layout::*;
pub use mixer::*;
pub use overlays::*;
pub use sinks::*;
pub use sources::*;

#[test]
fn generate_example_pipeline_picture() {
    // initialize logging
    let _ = env_logger::try_init();

    // initialize GStreamer
    gst::init().unwrap();

    // get output resolution from arguments
    let resolution = Size {
        width: 640,
        height: 480,
    };

    // setup mixer
    let mut mixer =
        Mixer::<Grid, TestSource, FakeSink, u32>::new(resolution, Some(2), (), SpeakerMode::None)
            .unwrap();
    // generate pipeline DOT graph of the empty pipeline
    mixer.generate_dot_file("0_init", gst::DebugGraphDetails::STATES);

    // prepare test source parameters
    let params = TestSourceParameters::default();

    // add three streams
    mixer.add_stream(1, "P1".into(), params.clone()).unwrap();
    mixer.add_stream(2, "P2".into(), params.clone()).unwrap();
    mixer.add_stream(3, "P3".into(), params).unwrap();
    // generate pipeline DOT graph
    mixer.generate_dot_file("1_add_streams", gst::DebugGraphDetails::STATES);

    // set two streams to be visible
    mixer.set_visibles(&[1, 2]).unwrap();
    // generate pipeline DOT graph
    mixer.generate_dot_file("2_set_visibles", gst::DebugGraphDetails::STATES);

    // start the pipeline
    mixer.play();
    // generate pipeline DOT graph
    mixer.generate_dot_file("3_playing", gst::DebugGraphDetails::STATES);
}
