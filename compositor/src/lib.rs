/*!
    # Purpose
    The *compositor* crate manages a gstreamer pipeline which receives WebRTC input audio and video streams
    of so-called *participants* and mixes them together using the so-called *mixer*.

    - [Mixer](mixer::Mixer)
    - [Participants](mixer::Participant)

    It then composes an output image showing some of them (so-called *visibles*) in the output picture.

    All incoming audio of all the participants will be mixed together independent of if they are invisible or not.
    The output then will be written onto disk into a Dash instance which consists of several files (MPD and Transport Streams).

    # Source & Sink

    To read the input and write the output the following types are used

    - [WebRTC Source](mixer::WebRtcSource)
      Manages a connection to a WebRTC source and provides the content to the internal GStreamer pipeline.
    - [Dash sink](mixer::DashSink)
      Writes the output into a Dash instance consisting of an MPD file and several audio/video files.

    # Layouts

    Several so-called *layouts* can be used to control the output composite.

    - [Grid layout](layout::Grid)
      shows a grid of all visible participants
    - [Speaker layout](layout::Speaker)
      shows a bigger picture of the first visible participant (so-called *speaker*)
      and uses the rest of the available picture area to arrange all other visibles.

    # Generic source and sink traits

    - [Generic Source Trait](mixer::Source)
      Generic trait which the mixer is assuming for an input source.
    - [Generic Sink Trait](mixer::Sink)
      Generic trait which the mixer is assuming for an output sink.

    # Testing

    In addition there are some alternative sources and sinks included which are used for testing purposes.

    - [Test Source](mixer::TestSource)
      Test source which just generates some dummy participant audio and video data.
    - [Fake Sink](mixer::FakeSink)
      Sink without any output - just to make it run.
    - [Display Sink](mixer::DisplaySink)
      Sink which displays the output on the screen.
*/

#[macro_use]
extern crate log;

mod error;
mod layout;
mod mixer;

pub use layout::*;
pub use mixer::*;

#[test]
fn generate_example_pipeline_picture() {
    use gstreamer as gst;

    // initialize logging
    env_logger::init();

    // initialize GStreamer
    gst::init().unwrap();

    // get output resolution from arguments
    let resolution = Size {
        width: 640,
        height: 480,
    };

    // setup mixer
    let mut mixer = Mixer::<Grid, TestSource>::new::<FakeSink>(resolution, 3, 2, ()).unwrap();
    // generate pipeline DOT graph of the empty pipeline
    mixer.generate_dot_file("0_init", gst::DebugGraphDetails::STATES);

    // prepare test source parameters
    let params = TestSourceParameters::default();

    // add three participants
    mixer
        .add_participant("P1".into(), "".into(), params.clone())
        .unwrap();
    mixer
        .add_participant("P2".into(), "".into(), params.clone())
        .unwrap();
    mixer
        .add_participant("P3".into(), "".into(), params)
        .unwrap();
    // generate pipeline DOT graph
    mixer.generate_dot_file("1_add_participants", gst::DebugGraphDetails::STATES);

    // set two participants to be visible
    mixer.set_visibles(&["P1".into(), "P2".into()]).unwrap();
    // generate pipeline DOT graph
    mixer.generate_dot_file("2_set_visibles", gst::DebugGraphDetails::STATES);

    // start the pipeline
    mixer.play();
    // generate pipeline DOT graph
    mixer.generate_dot_file("3_playing", gst::DebugGraphDetails::STATES);
}
