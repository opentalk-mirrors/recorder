use super::TEST_OUTPUT_DIR;
use crate::*;
use core::time::Duration;
use gstreamer as gst;
use std::path::PathBuf;

#[test]
fn test_dash() {
    // init logger
    env_logger::init();

    // initialize gstreamer
    gst::init().unwrap();

    // get output resolution from arguments
    let resolution = Size {
        width: 640,
        height: 480,
    };

    // use default parameters for sink
    let sink_params = DashParameters {
        mpd: PathBuf::from(TEST_OUTPUT_DIR).join("test_dash"),
        ..Default::default()
    };

    // create grid mixer with test sources for participants and a MatroskaSink
    let mut mixer = Mixer::<Grid, TestSource, DashSink>::new(resolution, 4, sink_params).unwrap();

    // add a participant
    mixer
        .add_participant("test".into(), "".into(), Default::default())
        .unwrap();

    // start mixer
    mixer.play();

    mixer.generate_dot_file("test_dash", gst::DebugGraphDetails::ALL);

    // stir until done
    std::thread::sleep(Duration::from_secs(3));
}
