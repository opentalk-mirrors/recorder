use super::*;
use crate::*;

#[test]
fn test_dash() {
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
    let sink_params = DashParameters {
        output_dir: Some(super::TEST_OUTPUT_DIR.into()),
        ..Default::default()
    };

    // create grid mixer with test sources for streams and a MatroskaSink
    let mut mixer = Mixer::<Grid, TestSource, DashSink, u32>::new(
        resolution,
        None,
        sink_params,
        SpeakerMode::None,
    )
    .unwrap();

    // add a stream
    mixer
        .add_stream(0, "Participant 0".into(), Default::default())
        .unwrap();

    // start mixer
    mixer.play();

    mixer.generate_dot_file("test_dash", gst::DebugGraphDetails::ALL);

    // stir until done
    wait_secs(20);
}
