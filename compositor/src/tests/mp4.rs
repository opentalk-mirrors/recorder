use crate::*;
use core::time::Duration;

#[test]
fn test_mp4() {
    // init logger
    let _ = env_logger::try_init();

    // initialize gstreamer
    gst::init().unwrap();

    // get output resolution from arguments
    let resolution = Size {
        width: 640,
        height: 480,
    };

    // create grid mixer with test sources for participants and a MatroskaSink
    let mut mixer = Mixer::<Grid, TestSource, Mp4Sink, u32>::new(
        resolution,
        4,
        4,
        Mp4SinkParams {
            file_path: format!("{}/mp4sink.mp4", super::TEST_OUTPUT_DIR),
        },
    )
    .unwrap();

    // add a participant
    mixer
        .add_participant(0, "Participant 0".into(), Default::default())
        .unwrap();

    // start mixer
    mixer.play();

    mixer.generate_dot_file("test_mp4", gst::DebugGraphDetails::ALL);

    // stir until done
    std::thread::sleep(Duration::from_secs(4));
}
