use crate::{testing::RESOLUTION, *};

/// generate an example of a usual pipeline
#[test]
fn generate_example_pipeline_picture() {
    // initialize logging
    let _ = env_logger::try_init();

    // initialize GStreamer
    gst::init().unwrap();

    // setup mixer
    let mut mixer = Mixer::<TestSource, FakeSink, u32>::new(RESOLUTION, ()).unwrap();
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
    mixer.layout::<Grid>().unwrap();
    // generate pipeline DOT graph
    mixer.generate_dot_file("2_set_visibles", gst::DebugGraphDetails::STATES);

    // start the pipeline
    mixer.play();
    // generate pipeline DOT graph
    mixer.generate_dot_file("3_playing", gst::DebugGraphDetails::STATES);
}
