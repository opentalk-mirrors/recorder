use crate::*;
use core::time::Duration;
use gstreamer as gst;

#[test]
fn test_matroska() {
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
    let sink_params = MatroskaParameters::default();

    // create grid mixer with test sources for participants and a MatroskaSink
    let mut mixer =
        Mixer::<Grid, TestSource, MatroskaSink>::new(resolution, 4, sink_params).unwrap();

    // add a participant
    mixer
        .add_participant("test".into(), "".into(), Default::default())
        .unwrap();

    // start mixer
    mixer.play();

    // stir until done
    std::thread::sleep(Duration::from_secs(10));
}
