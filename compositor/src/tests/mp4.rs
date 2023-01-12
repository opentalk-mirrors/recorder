use crate::*;

#[test]
fn test_mp4() {
    // initialize for testing
    testing::init();

    // create grid mixer with test sources for streams and a MatroskaSink
    let mut mixer = Mixer::<TestSource, Mp4Sink, u32>::new(
        testing::RESOLUTION,
        Mp4SinkParams {
            file_path: testing::output_file("mp4sink.mp4").into(),
        },
    )
    .unwrap();

    // add a stream
    mixer
        .add_stream(0, "Participant 0".into(), Default::default())
        .unwrap();

    // start mixer
    mixer.play();

    mixer.generate_dot_file("test_mp4", testing::DOT_DETAILS);

    // stir until done
    testing::wait_secs(4);
}
