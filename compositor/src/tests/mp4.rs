use crate::*;

#[test]
fn test_mp4() {
    // initialize for testing
    testing::init();
    // create grid mixer with test sources for streams and a MatroskaSink
    let mut mixer = Talk::<TestSource, u32>::new(
        testing::RESOLUTION,
        Mp4Sink::new(
            "test",
            Mp4Parameters {
                file_path: testing::output_file("mp4sink.mp4").into(),
                ..Default::default()
            },
        ),
        testing::MAX_STREAMS,
    )
    .unwrap();

    // add a stream
    mixer
        .add_stream(
            StreamId::camera(0),
            "Participant 0",
            Default::default(),
            Default::default(),
        )
        .unwrap();
    mixer.layout::<Grid>().unwrap();

    mixer.dot("test_mp4", testing::DOT_PARAMS);

    // stir until done
    testing::wait_secs(4);
}
