use super::*;
use crate::*;

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

    // create grid mixer with test sources for streams and a MatroskaSink
    let mut mixer = Mixer::<Grid, TestSource, Mp4Sink, u32>::new(
        resolution,
        None,
        Mp4SinkParams {
            file_path: format!("{}/mp4sink.mp4", super::TEST_OUTPUT_DIR),
        },
        SpeakerMode::None,
    )
    .unwrap();

    // add a stream
    mixer
        .add_stream(0, "Participant 0".into(), Default::default())
        .unwrap();

    // start mixer
    mixer.play();

    mixer.generate_dot_file("test_mp4", gst::DebugGraphDetails::ALL);

    // stir until done
    wait_secs(4);
}
