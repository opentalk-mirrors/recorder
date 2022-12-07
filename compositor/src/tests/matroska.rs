use crate::*;
use core::time::Duration;

#[test]
fn test_matroska() {
    // init logger
    let _ = env_logger::try_init();

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
    let mut mixer = Mixer::<Grid, TestSource, MatroskaSink, u32>::new(
        resolution,
        4,
        sink_params,
        SpeakerMode::None,
    )
    .unwrap();

    // add a participant
    mixer
        .add_participant(0, "Participant 0".into(), Default::default())
        .unwrap();

    // start mixer
    mixer.play();

    mixer.generate_dot_file("test_matroska", gst::DebugGraphDetails::ALL);

    // stir until done
    std::thread::sleep(Duration::from_secs(3));
}
