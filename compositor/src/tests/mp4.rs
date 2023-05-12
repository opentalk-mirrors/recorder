use crate::*;

#[test]
fn test_mp4() {
    // initialize for testing
    testing::init();
    // create grid mixer with test sources for streams and a MatroskaSink
    let mut mixer = Talk::<TestSource, u32>::new(
        testing::RESOLUTION,
        Box::new(Mp4SinkBuilder::new(Mp4SinkParams {
            file_path: testing::output_file("mp4sink.mp4").into(),
        })),
        None,
    )
    .unwrap();

    // add a stream
    mixer
        .add_stream(
            StreamId::camera(0),
            "Participant 0".into(),
            Default::default(),
            Default::default(),
        )
        .unwrap();
    mixer.layout::<Grid>().unwrap();

    mixer.dot("test_mp4", testing::DOT_PARAMS);

    // stir until done
    testing::wait_secs(4);
}
